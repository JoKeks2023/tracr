//! Allowlist extraction of Claude Code transcript lines (§11, §13.2).
//!
//! Every retained value passes a validator from `tracr_core::evidence`
//! (identifier, native ID, model ID, bounded context text). Content (prompt
//! and message text, tool inputs and outputs, attachments) is only inspected
//! for its shape: block types and a leading envelope tag from an allowlist.
//! None of it is copied.

use std::collections::BTreeMap;

use serde_json::{Map, Value};
use tracr_core::adapter::{ExtractContext, StreamRole};
use tracr_core::events::{FactKind, PROMPT_SUBMITTED};
use tracr_core::evidence::{
    ContextFacts, ExtractOutcome, ExtractedRecord, Facts, Marker, ModelFacts, ModelSource,
    NativeIds, ParseStatus, PullRequestFacts, QuarantineFacts, QuarantinedRecord,
    ReportedCostFacts, TemporalExtent, TimeQuality, ToolKind, ToolResultRef, ToolStart, UsageFacts,
    UsageIdentity, UsageScope, UsageSemantics, WorktreeFacts, bounded_quarantine_keys,
    context_text, ident, model_id, native_id, tool_name,
};
use tracr_core::manifest::CapabilityManifest;
use tracr_core::time::UtcMillis;

use crate::manifest::{CONTENT_TAGS, HUMAN_CONTENT_TAGS, HUMAN_PROMPT_SOURCES};

type Obj = Map<String, Value>;

/// A required field is missing or has the wrong shape: the record is
/// quarantined (AD-19 shape checks).
#[derive(Debug)]
struct ShapeError {
    field: &'static str,
}

fn shape(field: &'static str) -> ShapeError {
    ShapeError { field }
}

pub fn extract(
    line: &[u8],
    ctx: &ExtractContext<'_>,
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
    let result = match record_type {
        "user" => user(obj, ctx, manifest),
        "assistant" => assistant(obj, ctx),
        "system" => system(obj, ctx),
        "cost-state" => cost_state(obj, line),
        "pr-link" => pr_link(obj),
        "continued-in" => continued_in(obj),
        "relocated" => relocated(obj),
        "worktree-state" => worktree_state(obj),
        "queue-operation" => queue_operation(obj),
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
        Ok(record) => ExtractOutcome::Record(Box::new(record)),
        Err(e) => quarantine(
            Some(obj),
            line,
            ParseStatus::Failed,
            "shape_check_failed",
            Some(e.field),
        ),
    }
}

// ---------------------------------------------------------------------------
// Field helpers
// ---------------------------------------------------------------------------

fn str_field<'a>(o: &'a Obj, key: &str) -> Option<&'a str> {
    o.get(key).and_then(Value::as_str)
}

fn bool_field(o: &Obj, key: &str) -> Option<bool> {
    o.get(key).and_then(Value::as_bool)
}

fn u64_field(o: &Obj, key: &str) -> Option<u64> {
    o.get(key).and_then(Value::as_u64)
}

fn obj_field<'a>(o: &'a Obj, key: &str) -> Option<&'a Obj> {
    o.get(key).and_then(Value::as_object)
}

fn opt_id<'a>(o: &'a Obj, key: &str) -> Option<&'a str> {
    str_field(o, key).and_then(native_id)
}

fn req_id<'a>(o: &'a Obj, key: &'static str) -> Result<&'a str, ShapeError> {
    opt_id(o, key).ok_or_else(|| shape(key))
}

fn opt_time(o: &Obj, key: &str) -> Option<UtcMillis> {
    str_field(o, key).and_then(UtcMillis::parse_rfc3339)
}

fn req_time(o: &Obj, key: &'static str) -> Result<UtcMillis, ShapeError> {
    opt_time(o, key).ok_or_else(|| shape(key))
}

fn version(o: &Obj) -> Option<String> {
    str_field(o, "version").and_then(ident).map(str::to_owned)
}

fn bounded(o: &Obj, key: &str, max: usize) -> Option<String> {
    str_field(o, key)
        .and_then(|v| context_text(v, max))
        .map(str::to_owned)
}

fn context(o: &Obj) -> Option<ContextFacts> {
    let c = ContextFacts {
        cwd: bounded(o, "cwd", 4096),
        git_branch: bounded(o, "gitBranch", 512),
        worktree: None,
    };
    (c.cwd.is_some() || c.git_branch.is_some()).then_some(c)
}

fn time_quality(at: Option<UtcMillis>) -> TimeQuality {
    if at.is_some() {
        TimeQuality::Source
    } else {
        TimeQuality::Receipt
    }
}

// ---------------------------------------------------------------------------
// Content shape (never content)
// ---------------------------------------------------------------------------

struct ContentShape {
    kind: &'static str,
    block_types: Vec<String>,
    tool_results: Vec<ToolResultRef>,
    tool_uses: Vec<ToolStart>,
    tag: Option<&'static str>,
}

/// The leading `<tag>` of a string, if it is one of the known envelope tags.
fn leading_tag(text: &str) -> Option<&'static str> {
    let rest = text.trim_start().strip_prefix('<')?;
    let name = &rest[..rest.find('>')?];
    CONTENT_TAGS.iter().copied().find(|t| *t == name)
}

fn inspect_content(content: Option<&Value>) -> ContentShape {
    let mut s = ContentShape {
        kind: "absent",
        block_types: Vec::new(),
        tool_results: Vec::new(),
        tool_uses: Vec::new(),
        tag: None,
    };
    match content {
        None | Some(Value::Null) => {}
        Some(Value::String(text)) => {
            s.kind = "text";
            s.tag = leading_tag(text);
        }
        Some(Value::Array(blocks)) => {
            s.kind = "blocks";
            let mut first_text_seen = false;
            for block in blocks.iter().filter_map(Value::as_object) {
                let Some(t) = str_field(block, "type") else {
                    continue;
                };
                if let Some(t) = ident(t) {
                    s.block_types.push(t.to_owned());
                }
                match t {
                    "tool_result" => s.tool_results.push(ToolResultRef {
                        tool_use_id: opt_id(block, "tool_use_id").map(str::to_owned),
                    }),
                    "tool_use" | "server_tool_use" => {
                        let name = str_field(block, "name").and_then(tool_name);
                        let kind = match name {
                            Some(n) if n.starts_with("mcp__") => ToolKind::Extension,
                            Some(_) => ToolKind::Builtin,
                            None => ToolKind::Unknown,
                        };
                        s.tool_uses.push(ToolStart {
                            id: opt_id(block, "id").map(str::to_owned),
                            name: name.map(str::to_owned),
                            kind,
                        });
                    }
                    "text" if !first_text_seen => {
                        // Only the leading text block can carry an envelope tag.
                        first_text_seen = true;
                        s.tag = str_field(block, "text").and_then(leading_tag);
                    }
                    _ => {}
                }
            }
            s.block_types.sort();
            s.block_types.dedup();
        }
        Some(_) => s.kind = "other",
    }
    s
}

fn record_content_shape(facts: &mut Facts, c: &ContentShape) {
    facts.set_ident("content_kind", c.kind);
    if !c.block_types.is_empty() {
        facts.markers.insert(
            "content_block_types".into(),
            Marker::Idents(c.block_types.clone()),
        );
    }
    if let Some(tag) = c.tag {
        facts.set_ident("content_tag", tag);
    }
}

// ---------------------------------------------------------------------------
// Conversation records: user, assistant, system
// ---------------------------------------------------------------------------

struct Common<'a> {
    uuid: &'a str,
    session: &'a str,
    at: UtcMillis,
    agent: Option<&'a str>,
    version: Option<String>,
    facts: Facts,
}

fn common<'a>(
    o: &'a Obj,
    ctx: &ExtractContext<'_>,
    record_type: &str,
) -> Result<Common<'a>, ShapeError> {
    let uuid = req_id(o, "uuid")?;
    let session = req_id(o, "sessionId")?;
    let at = req_time(o, "timestamp")?;
    let mut facts = Facts {
        record_type: Some(record_type.to_owned()),
        context: context(o),
        ..Facts::default()
    };
    if let Some(v) = bool_field(o, "isSidechain") {
        facts.set_bool("is_sidechain", v);
    }
    facts.set_bool("agent_id_present", o.contains_key("agentId"));
    for (field, marker) in [
        ("entrypoint", "entrypoint"),
        ("userType", "user_type"),
        ("sessionKind", "session_kind"),
    ] {
        if let Some(v) = str_field(o, field) {
            facts.set_ident(marker, v);
        }
    }
    facts.set_ident(
        "stream_role",
        match ctx.stream.role {
            StreamRole::Main => "main",
            StreamRole::Subagent => "subagent",
        },
    );
    facts.set_bool(
        "session_matches_stream",
        ctx.stream.native_session.as_deref() == Some(session),
    );
    for (field, link) in [
        ("parentUuid", "parent_uuid"),
        ("logicalParentUuid", "logical_parent_uuid"),
    ] {
        if let Some(id) = opt_id(o, field) {
            facts.set_link(link, id);
        }
    }
    Ok(Common {
        uuid,
        session,
        at,
        agent: opt_id(o, "agentId"),
        version: version(o),
        facts,
    })
}

fn native(c: &Common<'_>) -> NativeIds {
    NativeIds {
        session: Some(c.session.to_owned()),
        agent: c.agent.map(str::to_owned),
        item: Some(c.uuid.to_owned()),
        ..NativeIds::default()
    }
}

/// Why a user-role record is not a human prompt, or `None` if it is one.
///
/// The rule of architecture §13.2, validated against real transcripts: main
/// chain, interactive entrypoint, not meta or compact summary, no tool
/// result, no scheduled or automated origin (`origin.kind`, `turnOrigin`,
/// `promptSource`), and no tool/system output envelope.
struct PromptSignals<'a> {
    role: StreamRole,
    sidechain: bool,
    agent_present: bool,
    entrypoint: Option<&'a str>,
    meta: bool,
    compact_summary: bool,
    transcript_only: bool,
    scheduled: bool,
    origin_kind: Option<&'a str>,
    turn_origin: Option<&'a str>,
    prompt_source: Option<&'a str>,
    tag: Option<&'static str>,
    content_kind: &'static str,
}

fn non_human_reason(s: &PromptSignals<'_>, manifest: &CapabilityManifest) -> Option<&'static str> {
    if s.role == StreamRole::Subagent {
        return Some("subagent_stream");
    }
    if s.sidechain || s.agent_present {
        return Some("sidechain");
    }
    if !manifest.is_interactive(s.entrypoint) {
        return Some("non_interactive_entrypoint");
    }
    if s.meta {
        return Some("meta");
    }
    if s.compact_summary {
        return Some("compact_summary");
    }
    if s.transcript_only {
        return Some("transcript_only");
    }
    if s.scheduled {
        return Some("scheduled");
    }
    if s.origin_kind.is_some_and(|k| k != "human")
        || s.turn_origin.is_some_and(|t| t != "human")
        || s.prompt_source
            .is_some_and(|p| !HUMAN_PROMPT_SOURCES.contains(&p))
    {
        return Some("automated_origin");
    }
    if s.tag.is_some_and(|t| !HUMAN_CONTENT_TAGS.contains(&t)) {
        return Some("tool_or_system_output");
    }
    if s.content_kind == "absent" {
        return Some("no_content");
    }
    None
}

fn user(
    o: &Obj,
    ctx: &ExtractContext<'_>,
    manifest: &CapabilityManifest,
) -> Result<ExtractedRecord, ShapeError> {
    let mut c = common(o, ctx, "user")?;
    let message = obj_field(o, "message").ok_or_else(|| shape("message"))?;
    if let Some(role) = str_field(message, "role") {
        c.facts.set_ident("role", role);
    }
    let content = inspect_content(message.get("content"));
    record_content_shape(&mut c.facts, &content);

    let tool_use_result = o.contains_key("toolUseResult");
    let has_tool_result = !content.tool_results.is_empty();
    c.facts.set_bool("tool_use_result_present", tool_use_result);
    c.facts
        .set_bool("tool_result_block_present", has_tool_result);
    c.facts.tool_results = content.tool_results.clone();

    let meta = bool_field(o, "isMeta").unwrap_or(false);
    let compact_summary = bool_field(o, "isCompactSummary").unwrap_or(false);
    let transcript_only = bool_field(o, "isVisibleInTranscriptOnly").unwrap_or(false);
    let scheduled = o.contains_key("scheduledTaskId") || o.contains_key("scheduledFireId");
    let interrupted = o.contains_key("interruptedMessageId");
    for (field, marker) in [
        ("isMeta", "is_meta"),
        ("isCompactSummary", "is_compact_summary"),
        ("isVisibleInTranscriptOnly", "is_visible_in_transcript_only"),
    ] {
        if let Some(v) = bool_field(o, field) {
            c.facts.set_bool(marker, v);
        }
    }
    c.facts.set_bool("scheduled_present", scheduled);
    c.facts
        .set_bool("interrupted_message_id_present", interrupted);

    let prompt_source = str_field(o, "promptSource");
    let turn_origin = str_field(o, "turnOrigin");
    let origin_kind = obj_field(o, "origin").and_then(|m| str_field(m, "kind"));
    for (marker, value) in [
        ("prompt_source", prompt_source),
        ("turn_origin", turn_origin),
        ("origin_kind", origin_kind),
        ("queue_priority", str_field(o, "queuePriority")),
    ] {
        if let Some(v) = value {
            c.facts.set_ident(marker, v);
        }
    }
    if o.get("origin").is_some_and(|v| !v.is_object()) {
        c.facts.set_bool("origin_non_object", true);
    }
    for (field, link) in [
        ("promptId", "prompt_id"),
        ("sourceToolUseID", "source_tool_use_id"),
        ("sourceToolAssistantUUID", "source_tool_assistant_uuid"),
        ("interruptedMessageId", "interrupted_message_id"),
    ] {
        if let Some(id) = opt_id(o, field) {
            c.facts.set_link(link, id);
        }
    }

    let (kind, subtype) = if tool_use_result || has_tool_result {
        (FactKind::ToolFinished, None)
    } else if interrupted {
        (FactKind::TurnInterrupted, None)
    } else {
        let signals = PromptSignals {
            role: ctx.stream.role,
            sidechain: bool_field(o, "isSidechain").unwrap_or(false),
            agent_present: o.contains_key("agentId"),
            entrypoint: str_field(o, "entrypoint"),
            meta,
            compact_summary,
            transcript_only,
            scheduled,
            origin_kind,
            turn_origin,
            prompt_source,
            tag: content.tag,
            content_kind: content.kind,
        };
        match non_human_reason(&signals, manifest) {
            None => (FactKind::UserInteraction, Some(PROMPT_SUBMITTED)),
            Some(reason) => (FactKind::AgentInput, Some(reason)),
        }
    };

    Ok(ExtractedRecord {
        native_record_id: Some(c.uuid.to_owned()),
        native: native(&c),
        occurred_at: Some(c.at),
        time_quality: TimeQuality::Source,
        producer_version: c.version.clone(),
        kind,
        subtype: subtype.map(str::to_owned),
        facts: c.facts,
    })
}

/// Usage as stated on one assistant record. Claude re-emits a message while
/// streaming, so each record states the request's usage *so far*: semantics
/// `snapshot`, scope `request`. Electing one record per request identity is
/// the normalizer's job (§8.1, S1.3). Missing categories stay `None`.
fn usage(u: &Obj, request_id: Option<&str>, message_id: Option<&str>, at: UtcMillis) -> UsageFacts {
    let numeric_map = |key: &str| -> BTreeMap<String, u64> {
        obj_field(u, key)
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| Some((ident(k)?.to_owned(), v.as_u64()?)))
                    .collect()
            })
            .unwrap_or_default()
    };
    UsageFacts {
        semantics: UsageSemantics::Snapshot,
        scope: UsageScope::Request,
        input_fresh: u64_field(u, "input_tokens"),
        input_total: None,
        cache_read: u64_field(u, "cache_read_input_tokens"),
        cache_creation: u64_field(u, "cache_creation_input_tokens"),
        cache_creation_tiers: numeric_map("cache_creation"),
        output: u64_field(u, "output_tokens"),
        reasoning_output: obj_field(u, "output_tokens_details")
            .and_then(|d| u64_field(d, "thinking_tokens")),
        tool_charges: numeric_map("server_tool_use"),
        service_tier: str_field(u, "service_tier")
            .and_then(ident)
            .map(str::to_owned),
        identity: UsageIdentity {
            request_id: request_id.map(str::to_owned),
            message_id: message_id.map(str::to_owned),
        },
        includes_copied_context: None,
        extent: TemporalExtent::Instant { at },
    }
}

fn assistant(o: &Obj, ctx: &ExtractContext<'_>) -> Result<ExtractedRecord, ShapeError> {
    let mut c = common(o, ctx, "assistant")?;
    let message = obj_field(o, "message").ok_or_else(|| shape("message"))?;
    if let Some(role) = str_field(message, "role") {
        c.facts.set_ident("role", role);
    }
    let message_id = opt_id(message, "id");
    let request_id = opt_id(o, "requestId");

    c.facts.model = str_field(message, "model").map(|m| match model_id(m) {
        Some(id) => ModelFacts {
            id: Some(id.to_owned()),
            source: ModelSource::Reported,
        },
        None => ModelFacts {
            id: None,
            source: ModelSource::Unknown,
        },
    });
    let content = inspect_content(message.get("content"));
    record_content_shape(&mut c.facts, &content);
    c.facts.tools_started = content.tool_uses;
    if let Some(reason) = str_field(message, "stop_reason") {
        c.facts.set_ident("stop_reason", reason);
    }
    for (field, marker) in [
        ("isApiErrorMessage", "is_api_error_message"),
        ("isAbortedMidStream", "is_aborted_mid_stream"),
    ] {
        if let Some(v) = bool_field(o, field) {
            c.facts.set_bool(marker, v);
        }
    }
    if let Some(status) = o.get("apiErrorStatus").and_then(Value::as_i64) {
        c.facts.set_int("api_error_status", status);
    }
    c.facts.usage = obj_field(message, "usage").map(|u| usage(u, request_id, message_id, c.at));

    let mut ids = native(&c);
    ids.request = request_id.map(str::to_owned);
    ids.message = message_id.map(str::to_owned);
    Ok(ExtractedRecord {
        native_record_id: Some(c.uuid.to_owned()),
        native: ids,
        occurred_at: Some(c.at),
        time_quality: TimeQuality::Source,
        producer_version: c.version.clone(),
        kind: FactKind::AgentMessage,
        subtype: None,
        facts: c.facts,
    })
}

fn system(o: &Obj, ctx: &ExtractContext<'_>) -> Result<ExtractedRecord, ShapeError> {
    let mut c = common(o, ctx, "system")?;
    let subtype = str_field(o, "subtype").and_then(ident).map(str::to_owned);
    if let Some(level) = str_field(o, "level") {
        c.facts.set_ident("level", level);
    }
    if let Some(v) = bool_field(o, "isMeta") {
        c.facts.set_bool("is_meta", v);
    }
    c.facts.set_bool(
        "compact_metadata_present",
        o.contains_key("compactMetadata"),
    );
    if subtype.as_deref() == Some("turn_duration") {
        c.facts.stated_duration_ms = o.get("durationMs").and_then(Value::as_i64);
    }
    Ok(ExtractedRecord {
        native_record_id: Some(c.uuid.to_owned()),
        native: native(&c),
        occurred_at: Some(c.at),
        time_quality: TimeQuality::Source,
        producer_version: c.version.clone(),
        kind: FactKind::SystemEvent,
        subtype,
        facts: c.facts,
    })
}

// ---------------------------------------------------------------------------
// Session-level records (no native record ID: content-derived keys)
// ---------------------------------------------------------------------------

fn session_record(
    o: &Obj,
    record_type: &str,
    kind: FactKind,
    subtype: Option<String>,
    at: Option<UtcMillis>,
    facts: Facts,
) -> Result<ExtractedRecord, ShapeError> {
    let session = req_id(o, "sessionId")?;
    Ok(ExtractedRecord {
        native_record_id: None,
        native: NativeIds {
            session: Some(session.to_owned()),
            ..NativeIds::default()
        },
        occurred_at: at,
        time_quality: time_quality(at),
        producer_version: version(o),
        kind,
        subtype,
        facts: Facts {
            record_type: Some(record_type.to_owned()),
            ..facts
        },
    })
}

/// Exact conversion of a plain decimal (at most 9 fractional digits) to
/// nano-units. Anything else stays `None`; the stated text is kept anyway.
pub fn decimal_to_nano(text: &str) -> Option<i64> {
    let (negative, body) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let (int, frac) = body.split_once('.').unwrap_or((body, ""));
    if int.is_empty()
        || !int.bytes().all(|b| b.is_ascii_digit())
        || !frac.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    if frac.len() > 9 {
        return None;
    }
    let int: i64 = int.parse().ok()?;
    let frac_nano: i64 = if frac.is_empty() {
        0
    } else {
        format!("{frac:0<9}").parse().ok()?
    };
    let value = int.checked_mul(1_000_000_000)?.checked_add(frac_nano)?;
    Some(if negative { -value } else { value })
}

/// Claude Code's own cumulative session cost figure: a provider-reported
/// cost figure, never spend and never summed (spec cost model §1).
/// The literal text of `totalCostUSD`, exactly as written in the source.
#[derive(serde::Deserialize)]
struct CostLiteral<'a> {
    #[serde(rename = "totalCostUSD", borrow)]
    total: Option<&'a serde_json::value::RawValue>,
}

fn cost_state(o: &Obj, line: &[u8]) -> Result<ExtractedRecord, ShapeError> {
    if !o.get("totalCostUSD").is_some_and(Value::is_number) {
        return Err(shape("totalCostUSD"));
    }
    let stated = serde_json::from_slice::<CostLiteral<'_>>(line)
        .ok()
        .and_then(|c| c.total)
        .map(|raw| raw.get().trim().to_owned())
        .ok_or_else(|| shape("totalCostUSD"))?;
    let mut facts = Facts {
        reported_cost: Some(ReportedCostFacts {
            value_nano: decimal_to_nano(&stated),
            value_as_stated: stated,
            unit_as_stated: "USD".into(),
            source_field: "totalCostUSD".into(),
            semantics: UsageSemantics::Cumulative,
            scope: UsageScope::Session,
        }),
        ..Facts::default()
    };
    if let Some(v) = bool_field(o, "hasUnknownModelCost") {
        facts.set_bool("has_unknown_model_cost", v);
    }
    session_record(
        o,
        "cost-state",
        FactKind::ReportedCostSnapshot,
        None,
        None,
        facts,
    )
}

fn is_repository_slug(s: &str) -> bool {
    let mut parts = s.split('/');
    let ok = |p: Option<&str>| {
        p.is_some_and(|p| {
            !p.is_empty()
                && p.len() <= 100
                && p.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
        })
    };
    ok(parts.next()) && ok(parts.next()) && parts.next().is_none()
}

fn pr_link(o: &Obj) -> Result<ExtractedRecord, ShapeError> {
    let number = u64_field(o, "prNumber").ok_or_else(|| shape("prNumber"))?;
    let repository = str_field(o, "prRepository")
        .filter(|r| is_repository_slug(r))
        .ok_or_else(|| shape("prRepository"))?;
    let facts = Facts {
        pull_request: Some(PullRequestFacts {
            repository: repository.to_owned(),
            number,
        }),
        ..Facts::default()
    };
    session_record(
        o,
        "pr-link",
        FactKind::PrObserved,
        None,
        opt_time(o, "timestamp"),
        facts,
    )
}

fn continued_in(o: &Obj) -> Result<ExtractedRecord, ShapeError> {
    let linked = req_id(o, "continuedInSessionId")?;
    let facts = Facts {
        linked_session: Some(linked.to_owned()),
        ..Facts::default()
    };
    session_record(
        o,
        "continued-in",
        FactKind::SessionContinued,
        None,
        opt_time(o, "timestamp"),
        facts,
    )
}

fn relocated(o: &Obj) -> Result<ExtractedRecord, ShapeError> {
    let cwd = bounded(o, "relocatedCwd", 4096).ok_or_else(|| shape("relocatedCwd"))?;
    let facts = Facts {
        context: Some(ContextFacts {
            cwd: Some(cwd),
            ..ContextFacts::default()
        }),
        ..Facts::default()
    };
    session_record(
        o,
        "relocated",
        FactKind::SessionRelocated,
        None,
        opt_time(o, "timestamp"),
        facts,
    )
}

fn worktree_state(o: &Obj) -> Result<ExtractedRecord, ShapeError> {
    // `worktreeSession: null` is written when the session leaves its worktree.
    if o.get("worktreeSession").is_some_and(Value::is_null) {
        let mut facts = Facts::default();
        facts.set_bool("worktree_active", false);
        return session_record(
            o,
            "worktree-state",
            FactKind::WorktreeObserved,
            None,
            opt_time(o, "timestamp"),
            facts,
        );
    }
    let ws = obj_field(o, "worktreeSession").ok_or_else(|| shape("worktreeSession"))?;
    let worktree = WorktreeFacts {
        original_cwd: bounded(ws, "originalCwd", 4096),
        original_branch: bounded(ws, "originalBranch", 512),
        original_head_commit: str_field(ws, "originalHeadCommit")
            .and_then(ident)
            .map(str::to_owned),
        worktree_path: bounded(ws, "worktreePath", 4096),
        worktree_branch: bounded(ws, "worktreeBranch", 512),
    };
    let mut facts = Facts {
        context: Some(ContextFacts {
            worktree: Some(worktree),
            ..ContextFacts::default()
        }),
        ..Facts::default()
    };
    facts.set_bool("worktree_active", true);
    session_record(
        o,
        "worktree-state",
        FactKind::WorktreeObserved,
        None,
        opt_time(o, "timestamp"),
        facts,
    )
}

/// Queue operations (a prompt typed while the agent was busy is enqueued and
/// later absorbed). Only the operation and reason are kept; the queued text
/// (`content`) is never read.
fn queue_operation(o: &Obj) -> Result<ExtractedRecord, ShapeError> {
    let operation = str_field(o, "operation")
        .and_then(ident)
        .ok_or_else(|| shape("operation"))?;
    let mut facts = Facts::default();
    facts.set_ident("operation", operation);
    if let Some(reason) = str_field(o, "reason") {
        facts.set_ident("reason", reason);
    }
    session_record(
        o,
        "queue-operation",
        FactKind::QueueOperation,
        Some(operation.to_owned()),
        opt_time(o, "timestamp"),
        facts,
    )
}

// ---------------------------------------------------------------------------
// Quarantine (bounded facts, §4.4)
// ---------------------------------------------------------------------------

fn quarantine(
    obj: Option<&Obj>,
    line: &[u8],
    status: ParseStatus,
    reason: &str,
    detail: Option<&str>,
) -> ExtractOutcome {
    let native = NativeIds {
        session: obj.and_then(|o| opt_id(o, "sessionId")).map(str::to_owned),
        item: obj.and_then(|o| opt_id(o, "uuid")).map(str::to_owned),
        ..NativeIds::default()
    };
    ExtractOutcome::Quarantine(Box::new(QuarantinedRecord {
        parse_status: status,
        native_record_id: native.item.clone(),
        native,
        occurred_at: obj.and_then(|o| opt_time(o, "timestamp")),
        producer_version: obj.and_then(version),
        facts: QuarantineFacts {
            reason: reason.to_owned(),
            detail: detail.and_then(ident).map(str::to_owned),
            native_type: obj
                .and_then(|o| str_field(o, "type"))
                .and_then(ident)
                .map(str::to_owned),
            byte_len: line.len() as u64,
            top_level_keys: obj
                .map(|o| bounded_quarantine_keys(o.keys().map(String::as_str)))
                .unwrap_or_default(),
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_conversion_is_exact_or_absent() {
        assert_eq!(decimal_to_nano("12.5"), Some(12_500_000_000));
        assert_eq!(decimal_to_nano("0.000000001"), Some(1));
        assert_eq!(decimal_to_nano("3"), Some(3_000_000_000));
        assert_eq!(
            decimal_to_nano("0.1234567891"),
            None,
            "more than 9 decimals is not exact"
        );
        assert_eq!(decimal_to_nano("1e-5"), None);
        assert_eq!(decimal_to_nano("-2.25"), Some(-2_250_000_000));
    }

    #[test]
    fn leading_tags_come_from_the_allowlist_only() {
        assert_eq!(
            leading_tag("<command-name>/clear</command-name>"),
            Some("command-name")
        );
        assert_eq!(
            leading_tag("  <local-command-stdout>x"),
            Some("local-command-stdout")
        );
        assert_eq!(leading_tag("<my-own-words> hi"), None);
        assert_eq!(leading_tag("plain prompt"), None);
    }
}
