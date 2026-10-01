//! Allowlist extraction of Codex rollout lines (§11, §13.2).
//!
//! Every retained value passes a validator from `tracr_core::evidence`.
//! Message, reasoning, compaction and tool payloads are never read beyond
//! their record type; tool calls keep only their call ID and tool name.
//! Content-bearing record kinds produce no observation at all.

use serde_json::{Map, Value};
use tracr_core::adapter::ExtractContext;
use tracr_core::events::FactKind;
use tracr_core::evidence::{
    ContextFacts, ExtractOutcome, ExtractedRecord, Facts, ModelFacts, ModelSource, NativeIds,
    ParseStatus, QuarantineFacts, QuarantinedRecord, TemporalExtent, TimeQuality, ToolKind,
    ToolResultRef, ToolStart, UsageFacts, UsageIdentity, UsageScope, UsageSemantics,
    bounded_quarantine_keys, context_text, ident, model_id, native_id, tool_name,
};
use tracr_core::manifest::CapabilityManifest;
use tracr_core::time::UtcMillis;

type Obj = Map<String, Value>;

/// `response_item` kinds that carry content only: no observation (§11).
const CONTENT_ITEMS: &[&str] = &["message", "reasoning", "compaction", "agent_message"];
/// `item_completed` item types tracr reads. Others are activity detail that
/// the rollout also records as tool calls; they produce no observation.
const READ_ITEMS: &[&str] = &["UserMessage", "AgentMessage"];

#[derive(Debug)]
struct ShapeError {
    field: &'static str,
}

fn shape(field: &'static str) -> ShapeError {
    ShapeError { field }
}

fn str_field<'a>(o: &'a Obj, key: &str) -> Option<&'a str> {
    o.get(key).and_then(Value::as_str)
}

fn obj_field<'a>(o: &'a Obj, key: &str) -> Option<&'a Obj> {
    o.get(key).and_then(Value::as_object)
}

fn u64_field(o: &Obj, key: &str) -> Option<u64> {
    o.get(key).and_then(Value::as_u64)
}

fn opt_id<'a>(o: &'a Obj, key: &str) -> Option<&'a str> {
    str_field(o, key).and_then(native_id)
}

fn req_id<'a>(o: &'a Obj, key: &'static str) -> Result<&'a str, ShapeError> {
    opt_id(o, key).ok_or_else(|| shape(key))
}

fn bounded(o: &Obj, key: &str, max: usize) -> Option<String> {
    str_field(o, key)
        .and_then(|v| context_text(v, max))
        .map(str::to_owned)
}

/// The turn an item belongs to, from Codex's per-item metadata.
fn item_turn(p: &Obj) -> Option<String> {
    obj_field(p, "internal_chat_message_metadata_passthrough")
        .and_then(|m| opt_id(m, "turn_id"))
        .map(str::to_owned)
}

struct Line<'a> {
    payload: &'a Obj,
    at: Option<UtcMillis>,
    facts: Facts,
}

fn record(
    mut l: Line<'_>,
    kind: FactKind,
    subtype: Option<&str>,
    native_record_id: Option<String>,
    native: NativeIds,
) -> ExtractedRecord {
    // The normalizer reads facts only: the turn and parent thread go there too.
    if let Some(turn) = &native.turn {
        l.facts.set_link("turn", turn);
    }
    if let Some(parent) = &native.parent_session {
        l.facts.set_link("parent_thread", parent);
    }
    ExtractedRecord {
        native_record_id,
        native,
        occurred_at: l.at,
        time_quality: if l.at.is_some() {
            TimeQuality::Source
        } else {
            TimeQuality::Receipt
        },
        producer_version: None,
        kind,
        subtype: subtype.map(str::to_owned),
        facts: l.facts,
    }
}

pub fn extract(
    line: &[u8],
    _ctx: &ExtractContext<'_>,
    manifest: &CapabilityManifest,
) -> ExtractOutcome {
    let value: Value = match serde_json::from_slice(line) {
        Ok(v) => v,
        Err(_) => return quarantine(None, line, ParseStatus::Failed, "unparseable_json", None),
    };
    let Some(obj) = value.as_object() else {
        return quarantine(
            None,
            line,
            ParseStatus::UnknownFormat,
            "not_an_object",
            None,
        );
    };
    let Some(record_type) = str_field(obj, "type") else {
        return quarantine(
            Some(obj),
            line,
            ParseStatus::UnknownFormat,
            "missing_type",
            None,
        );
    };
    if let Some(ignored) = manifest
        .ignored_record_types
        .iter()
        .find(|t| **t == record_type)
    {
        return ExtractOutcome::Ignore {
            record_type: (*ignored).to_owned(),
        };
    }
    let Some(payload) = obj_field(obj, "payload") else {
        return quarantine(
            Some(obj),
            line,
            ParseStatus::Failed,
            "shape_check_failed",
            Some("payload"),
        );
    };
    let mut facts = Facts {
        record_type: Some(record_type.to_owned()),
        ..Facts::default()
    };
    if let Some(n) = obj.get("ordinal").and_then(Value::as_i64) {
        facts.set_int("ordinal", n);
    }
    let l = Line {
        payload,
        at: str_field(obj, "timestamp").and_then(UtcMillis::parse_rfc3339),
        facts,
    };
    let result = match record_type {
        "session_meta" => session_meta(l),
        "turn_context" => turn_context(l),
        "token_usage_record" => token_usage_record(l),
        "event_msg" => match str_field(payload, "type") {
            Some("task_started") => turn_event(l, "task_started"),
            Some("task_complete") => task_complete(l),
            Some("turn_aborted") => turn_aborted(l),
            Some("token_count") => token_count(l),
            Some("thread_settings_applied") => thread_settings(l),
            Some("item_completed") => item_completed(l),
            _ => Err(shape("payload.type")),
        },
        "response_item" => match str_field(payload, "type") {
            Some(t) if CONTENT_ITEMS.contains(&t) => {
                return ExtractOutcome::Ignore {
                    record_type: format!("response_item.{t}"),
                };
            }
            Some("custom_tool_call" | "function_call") => tool_call(l),
            Some("custom_tool_call_output" | "function_call_output") => tool_output(l),
            _ => Err(shape("payload.type")),
        },
        _ => {
            return quarantine(
                Some(obj),
                line,
                ParseStatus::UnknownFormat,
                "unknown_record_type",
                None,
            );
        }
    };
    match result {
        Ok(Some(r)) => ExtractOutcome::Record(Box::new(r)),
        Ok(None) => ExtractOutcome::Ignore {
            record_type: format!(
                "{record_type}.{}",
                str_field(payload, "type")
                    .and_then(ident)
                    .unwrap_or("unknown")
            ),
        },
        Err(e) => quarantine(
            Some(obj),
            line,
            ParseStatus::Failed,
            "shape_check_failed",
            Some(e.field),
        ),
    }
}

type Out = Result<Option<ExtractedRecord>, ShapeError>;

/// Thread identity, origin and parent linkage. The parent reference is kept
/// as a retained fact; whether it links the thread is the normalizer's gate.
fn session_meta(mut l: Line<'_>) -> Out {
    let p = l.payload;
    let thread = req_id(p, "id")?;
    let mut native = NativeIds {
        session: Some(thread.to_owned()),
        ..NativeIds::default()
    };
    match p.get("source") {
        Some(Value::String(s)) => match ident(s) {
            Some(s) => l.facts.set_ident("source", s),
            None => l.facts.set_bool("source_non_ident", true),
        },
        Some(Value::Object(src)) => {
            if let Some(sub) = obj_field(src, "subagent") {
                l.facts.set_ident("source", "subagent");
                if let Some(other) = str_field(sub, "other") {
                    l.facts.set_ident("subagent_kind", other);
                } else if let Some(spawn) = obj_field(sub, "thread_spawn") {
                    l.facts.set_ident("subagent_kind", "thread_spawn");
                    if let Some(parent) = opt_id(spawn, "parent_thread_id") {
                        native.parent_session = Some(parent.to_owned());
                    }
                    if let Some(depth) = spawn.get("depth").and_then(Value::as_i64) {
                        l.facts.set_int("subagent_depth", depth);
                    }
                }
            } else {
                l.facts.set_bool("source_unknown_shape", true);
            }
        }
        _ => {}
    }
    if let Some(parent) = opt_id(p, "parent_thread_id") {
        native
            .parent_session
            .get_or_insert_with(|| parent.to_owned());
    }
    if let Some(fork) = opt_id(p, "forked_from_id") {
        l.facts.set_link("forked_from", fork);
    }
    if let Some(s) = str_field(p, "thread_source") {
        l.facts.set_ident("thread_source", s);
    }
    if let Some(s) = str_field(p, "model_provider") {
        l.facts.set_ident("model_provider", s);
    }
    if let Some(n) = p
        .get("subagent_history_start_ordinal")
        .and_then(Value::as_i64)
    {
        // Retained, not used: locally it always equals the thread's own line
        // count, so it does not mark copied history.
        l.facts.set_int("subagent_history_start_ordinal", n);
    }
    let context = ContextFacts {
        cwd: bounded(p, "cwd", 4096),
        git_branch: obj_field(p, "git").and_then(|g| bounded(g, "branch", 512)),
        worktree: None,
    };
    if context.cwd.is_some() || context.git_branch.is_some() {
        l.facts.context = Some(context);
    }
    let raw_version = str_field(p, "cli_version").and_then(ident);
    if let Some(raw) = raw_version {
        // Alpha builds (`0.149.0-alpha.4.3`) are gated by their numeric core.
        l.facts.set_ident("cli_version", raw);
    }
    let version = raw_version.map(|v| crate::manifest::version_core(v).to_owned());
    let mut r = record(
        l,
        FactKind::SystemEvent,
        Some("session_meta"),
        Some(format!("session_meta:{thread}")),
        native,
    );
    r.producer_version = version;
    Ok(Some(r))
}

fn turn_context(mut l: Line<'_>) -> Out {
    let p = l.payload;
    let turn = req_id(p, "turn_id")?;
    let l_ordinal = match l.facts.markers.get("ordinal") {
        Some(tracr_core::evidence::Marker::Int(n)) => Some(*n),
        _ => None,
    };
    if let Some(m) = str_field(p, "model").and_then(model_id) {
        l.facts.model = Some(ModelFacts {
            id: Some(m.to_owned()),
            source: ModelSource::Reported,
        });
    }
    if let Some(cwd) = bounded(p, "cwd", 4096) {
        l.facts.context = Some(ContextFacts {
            cwd: Some(cwd),
            ..ContextFacts::default()
        });
    }
    let native = NativeIds {
        turn: Some(turn.to_owned()),
        ..NativeIds::default()
    };
    Ok(Some(record(
        l,
        FactKind::SystemEvent,
        Some("turn_context"),
        // A turn can carry several contexts (a setting changed mid-turn).
        Some(match l_ordinal {
            Some(n) => format!("turn_context:{turn}@{n}"),
            None => format!("turn_context:{turn}"),
        }),
        native,
    )))
}

fn turn_event(l: Line<'_>, subtype: &'static str) -> Out {
    let turn = req_id(l.payload, "turn_id")?;
    let native = NativeIds {
        turn: Some(turn.to_owned()),
        ..NativeIds::default()
    };
    Ok(Some(record(
        l,
        FactKind::SystemEvent,
        Some(subtype),
        Some(format!("{subtype}:{turn}")),
        native,
    )))
}

fn task_complete(mut l: Line<'_>) -> Out {
    let p = l.payload;
    if let Some(ms) = p.get("duration_ms").and_then(Value::as_i64) {
        l.facts.stated_duration_ms = Some(ms);
    }
    if let Some(err) = obj_field(p, "error") {
        l.facts.set_bool("has_error", true);
        if let Some(info) = str_field(err, "codex_error_info") {
            l.facts.set_ident("error_kind", info);
        }
    }
    turn_event(l, "task_complete")
}

fn turn_aborted(mut l: Line<'_>) -> Out {
    let p = l.payload;
    let turn = req_id(p, "turn_id")?;
    if let Some(reason) = str_field(p, "reason") {
        l.facts.set_ident("reason", reason);
    }
    let native = NativeIds {
        turn: Some(turn.to_owned()),
        ..NativeIds::default()
    };
    Ok(Some(record(
        l,
        FactKind::TurnInterrupted,
        None,
        Some(format!("turn_aborted:{turn}")),
        native,
    )))
}

fn thread_settings(mut l: Line<'_>) -> Out {
    let Some(s) = obj_field(l.payload, "thread_settings") else {
        return Err(shape("thread_settings"));
    };
    if let Some(tier) = str_field(s, "service_tier") {
        l.facts.set_ident("service_tier", tier);
    }
    if let Some(m) = str_field(s, "model").and_then(model_id) {
        l.facts.model = Some(ModelFacts {
            id: Some(m.to_owned()),
            source: ModelSource::Reported,
        });
    }
    Ok(Some(record(
        l,
        FactKind::SystemEvent,
        Some("thread_settings"),
        None,
        NativeIds::default(),
    )))
}

fn item_completed(mut l: Line<'_>) -> Out {
    let p = l.payload;
    let item = obj_field(p, "item").ok_or_else(|| shape("item"))?;
    let item_type = str_field(item, "type").ok_or_else(|| shape("item.type"))?;
    if !READ_ITEMS.contains(&item_type) {
        return Ok(None);
    }
    let id = req_id(item, "id")?;
    l.facts.set_ident("item_type", item_type);
    if let Some(phase) = str_field(item, "phase") {
        l.facts.set_ident("phase", phase);
    }
    let native = NativeIds {
        turn: opt_id(p, "turn_id").map(str::to_owned),
        item: Some(id.to_owned()),
        ..NativeIds::default()
    };
    let kind = if item_type == "UserMessage" {
        // Human or not is the normalizer's decision (interactive source, AD-19).
        FactKind::UserInteraction
    } else {
        FactKind::AgentMessage
    };
    Ok(Some(record(
        l,
        kind,
        Some(item_type),
        // Item IDs repeat across unrelated threads (33 locally): scoped by thread.
        Some(match opt_id(p, "thread_id") {
            Some(thread) => format!("item:{thread}:{id}"),
            None => format!("item:{id}"),
        }),
        native,
    )))
}

fn tool_call(mut l: Line<'_>) -> Out {
    let p = l.payload;
    let call = req_id(p, "call_id")?;
    let kind = if p.get("namespace").is_some() {
        ToolKind::Extension
    } else {
        ToolKind::Builtin
    };
    l.facts.tools_started.push(ToolStart {
        id: Some(call.to_owned()),
        name: str_field(p, "name").and_then(tool_name).map(str::to_owned),
        kind,
    });
    let native = NativeIds {
        turn: item_turn(p),
        tool_call: Some(call.to_owned()),
        ..NativeIds::default()
    };
    // Call IDs are unique across threads locally; item IDs are not.
    let id = format!("call:{call}");
    Ok(Some(record(
        l,
        FactKind::AgentMessage,
        Some("tool_call"),
        Some(id),
        native,
    )))
}

fn tool_output(mut l: Line<'_>) -> Out {
    let p = l.payload;
    let call = req_id(p, "call_id")?;
    l.facts.tool_results.push(ToolResultRef {
        tool_use_id: Some(call.to_owned()),
    });
    let native = NativeIds {
        turn: item_turn(p),
        tool_call: Some(call.to_owned()),
        ..NativeIds::default()
    };
    let id = format!("output:{call}");
    Ok(Some(record(
        l,
        FactKind::ToolFinished,
        None,
        Some(id),
        native,
    )))
}

/// Codex token counts: `input_tokens` includes `cached_input_tokens`, and
/// `output_tokens` includes `reasoning_output_tokens` (locally,
/// `total_tokens = input + output` on every record). Cache writes are kept
/// as a marker only: whether they are part of input is not established.
fn usage_of(
    u: &Obj,
    semantics: UsageSemantics,
    scope: UsageScope,
    at: Option<UtcMillis>,
) -> Option<UsageFacts> {
    let input = u64_field(u, "input_tokens");
    let cached = u64_field(u, "cached_input_tokens");
    Some(UsageFacts {
        semantics,
        scope,
        input_fresh: match (input, cached) {
            (Some(i), Some(c)) if c <= i => Some(i - c),
            _ => None,
        },
        input_total: input,
        cache_read: cached,
        cache_creation: None,
        cache_creation_tiers: Default::default(),
        output: u64_field(u, "output_tokens"),
        reasoning_output: u64_field(u, "reasoning_output_tokens"),
        tool_charges: Default::default(),
        service_tier: None,
        identity: UsageIdentity::default(),
        includes_copied_context: None,
        extent: at.map_or(TemporalExtent::Unknown, |at| TemporalExtent::Instant { at }),
    })
}

fn token_usage_record(mut l: Line<'_>) -> Out {
    let p = l.payload;
    let response = req_id(p, "response_id")?;
    let u = obj_field(p, "usage").ok_or_else(|| shape("usage"))?;
    let mut usage = usage_of(u, UsageSemantics::Delta, UsageScope::Request, l.at)
        .ok_or_else(|| shape("usage"))?;
    usage.identity.request_id = Some(response.to_owned());
    if let Some(cw) = u64_field(u, "cache_write_input_tokens") {
        l.facts.set_int(
            "cache_write_input_tokens",
            i64::try_from(cw).unwrap_or(i64::MAX),
        );
    }
    l.facts.usage = Some(usage);
    let native = NativeIds {
        session: opt_id(p, "thread_id").map(str::to_owned),
        turn: opt_id(p, "turn_id").map(str::to_owned),
        request: Some(response.to_owned()),
        ..NativeIds::default()
    };
    Ok(Some(record(
        l,
        FactKind::AgentMessage,
        Some("token_usage_record"),
        Some(format!("usage:{response}")),
        native,
    )))
}

/// Cumulative thread totals. Only used where a thread has no
/// `token_usage_record` (older producers); the normalizer turns them into deltas.
fn token_count(mut l: Line<'_>) -> Out {
    let p = l.payload;
    let Some(total) = obj_field(p, "info").and_then(|i| obj_field(i, "total_token_usage")) else {
        // A rate-limit-only update: nothing about usage.
        return Ok(None);
    };
    l.facts.usage = usage_of(total, UsageSemantics::Cumulative, UsageScope::Session, l.at);
    Ok(Some(record(
        l,
        FactKind::AgentMessage,
        Some("token_count"),
        None,
        NativeIds::default(),
    )))
}

fn quarantine(
    obj: Option<&Obj>,
    line: &[u8],
    status: ParseStatus,
    reason: &str,
    detail: Option<&str>,
) -> ExtractOutcome {
    let payload_type = obj
        .and_then(|o| obj_field(o, "payload"))
        .and_then(|p| str_field(p, "type"))
        .and_then(ident);
    ExtractOutcome::Quarantine(Box::new(QuarantinedRecord {
        parse_status: status,
        native_record_id: None,
        native: NativeIds::default(),
        occurred_at: obj
            .and_then(|o| str_field(o, "timestamp"))
            .and_then(UtcMillis::parse_rfc3339),
        producer_version: None,
        facts: QuarantineFacts {
            reason: reason.to_owned(),
            detail: detail.and_then(ident).map(str::to_owned),
            native_type: obj
                .and_then(|o| str_field(o, "type"))
                .and_then(ident)
                .map(|t| match payload_type {
                    Some(p) => format!("{t}.{p}"),
                    None => t.to_owned(),
                }),
            byte_len: line.len() as u64,
            top_level_keys: obj
                .map(|o| bounded_quarantine_keys(o.keys().map(String::as_str)))
                .unwrap_or_default(),
        },
    }))
}
