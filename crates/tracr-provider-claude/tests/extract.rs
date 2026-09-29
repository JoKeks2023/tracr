//! Extraction tests on the structure-only fixture corpus and on hand-written
//! records (architecture §11, §12.1, §13.2).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use tracr_core::adapter::{ExtractContext, ObservationAdapter, StreamRef, StreamRole};
use tracr_core::events::FactKind;
use tracr_core::evidence::{
    ExtractOutcome, Marker, ModelSource, ParseStatus, StreamKey, TemporalExtent, UsageSemantics,
};
use tracr_core::time::UtcMillis;
use tracr_provider_claude::ClaudeAdapter;

fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn stream(role: StreamRole) -> StreamRef {
    StreamRef {
        stream_key: StreamKey::new("claude:test:stream"),
        role,
        native_session: Some("11111111-1111-4111-8111-111111111111".into()),
        native_agent: None,
    }
}

fn outcome_key(o: &ExtractOutcome) -> String {
    match o {
        ExtractOutcome::Record(r) => match &r.subtype {
            Some(s) => format!("{}/{s}", r.kind.as_str()),
            None => r.kind.as_str().to_owned(),
        },
        ExtractOutcome::Ignore { record_type } => format!("ignored/{record_type}"),
        ExtractOutcome::Quarantine(q) => format!("quarantined/{}", q.facts.reason),
    }
}

/// Everything an outcome would write into the ledger, as text.
fn retained_text(o: &ExtractOutcome) -> String {
    match o {
        ExtractOutcome::Record(r) => format!(
            "{} {:?} {:?} {:?} {:?}",
            serde_json::to_string(&r.facts).unwrap(),
            r.native,
            r.native_record_id,
            r.subtype,
            r.producer_version
        ),
        ExtractOutcome::Quarantine(q) => format!(
            "{} {:?} {:?} {:?}",
            serde_json::to_string(&q.facts).unwrap(),
            q.native,
            q.native_record_id,
            q.producer_version
        ),
        ExtractOutcome::Ignore { record_type } => record_type.clone(),
    }
}

/// Finds a sanitizer sentinel (`cx<digits>zz` or `kx<digits>zz`) in `text`.
/// Sentinels are identifier-shaped, so they would survive every identifier
/// validator: finding one means content was routed into retained facts.
fn sentinel_in(text: &str) -> Option<String> {
    let b = text.as_bytes();
    for i in 0..b.len().saturating_sub(4) {
        if (b[i] == b'c' || b[i] == b'k') && b[i + 1] == b'x' {
            let digits = b[i + 2..].iter().take_while(|c| c.is_ascii_digit()).count();
            let end = i + 2 + digits;
            if digits > 0 && b.get(end) == Some(&b'z') && b.get(end + 1) == Some(&b'z') {
                return Some(text[i..end + 2].to_owned());
            }
        }
    }
    None
}

fn version_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(fixtures_dir())
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.join("expected.json").exists())
        .collect();
    dirs.sort();
    dirs
}

#[test]
fn fixture_corpus_covers_the_validated_range() {
    let versions: Vec<String> = version_dirs()
        .iter()
        .map(|d| d.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(versions, ["2.1.231", "2.1.236", "2.1.267", "2.1.281"]);
    let adapter = ClaudeAdapter::new();
    for v in &versions {
        assert!(adapter.manifest().select(Some(v)).is_validated(), "{v}");
    }
    assert!(!adapter.manifest().select(Some("2.1.282")).is_validated());
    assert!(!adapter.manifest().select(None).is_validated());
}

#[test]
fn fixtures_match_the_independent_reference_classification() {
    let adapter = ClaudeAdapter::new();
    let mut prompts = 0;
    for dir in version_dirs() {
        let expected: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(dir.join("expected.json")).unwrap()).unwrap();
        for (file, exp) in expected.as_object().unwrap() {
            let role = match exp["role"].as_str().unwrap() {
                "subagent" => StreamRole::Subagent,
                _ => StreamRole::Main,
            };
            let s = stream(role);
            let ctx = ExtractContext { stream: &s };
            let mut got: BTreeMap<String, u64> = BTreeMap::new();
            let text = fs::read_to_string(dir.join(file)).unwrap();
            for line in text.lines() {
                *got.entry(outcome_key(&adapter.extract(line.as_bytes(), &ctx)))
                    .or_default() += 1;
            }
            let want: BTreeMap<String, u64> = exp["outcomes"]
                .as_object()
                .unwrap()
                .iter()
                .map(|(k, v)| (k.clone(), v.as_u64().unwrap()))
                .collect();
            assert_eq!(got, want, "{}/{file}", dir.display());
            prompts += got
                .get("user_interaction/prompt_submitted")
                .copied()
                .unwrap_or(0);
        }
    }
    assert!(prompts > 0, "the corpus must exercise human prompts");
}

#[test]
fn no_content_reaches_retained_facts() {
    let adapter = ClaudeAdapter::new();
    let mut checked = 0;
    for dir in version_dirs() {
        for file in ["main.jsonl", "records.jsonl", "subagent.jsonl"] {
            let Ok(text) = fs::read_to_string(dir.join(file)) else {
                continue;
            };
            let role = if file == "subagent.jsonl" {
                StreamRole::Subagent
            } else {
                StreamRole::Main
            };
            let s = stream(role);
            for line in text.lines() {
                let out = adapter.extract(line.as_bytes(), &ExtractContext { stream: &s });
                let retained = retained_text(&out);
                assert_eq!(
                    sentinel_in(&retained),
                    None,
                    "content sentinel retained from {}/{file}: {retained}",
                    dir.display()
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 500);
}

// ---------------------------------------------------------------------------
// Hand-written records
// ---------------------------------------------------------------------------

const TS: &str = "2026-09-28T14:05:00.000Z";

fn user_line(extra: &str, content: &str) -> String {
    format!(
        r#"{{"type":"user","uuid":"aaaaaaaa-0000-4000-8000-000000000001","sessionId":"11111111-1111-4111-8111-111111111111","timestamp":"{TS}","version":"2.1.281","entrypoint":"cli","userType":"external","isSidechain":false,"cwd":"/Users/dev/p","gitBranch":"main","message":{{"role":"user","content":{content}}}{extra}}}"#
    )
}

fn classify(line: &str, role: StreamRole) -> ExtractOutcome {
    let s = stream(role);
    ClaudeAdapter::new().extract(line.as_bytes(), &ExtractContext { stream: &s })
}

fn key(line: &str) -> String {
    outcome_key(&classify(line, StreamRole::Main))
}

#[test]
fn human_prompt_rule() {
    let prompt = r#""SECRET PROMPT TEXT""#;
    assert_eq!(
        key(&user_line(
            r#","promptSource":"typed","turnOrigin":"human","origin":{"kind":"human"}"#,
            prompt
        )),
        "user_interaction/prompt_submitted"
    );
    // Queued prompts are typed by the human while the agent is busy.
    assert_eq!(
        key(&user_line(
            r#","promptSource":"queued","origin":{"kind":"human"}"#,
            prompt
        )),
        "user_interaction/prompt_submitted"
    );
    // Older producers without origin markers.
    assert_eq!(
        key(&user_line("", prompt)),
        "user_interaction/prompt_submitted"
    );
    // A slash command the human typed.
    assert_eq!(
        key(&user_line("", r#""<command-name>/clear</command-name>""#)),
        "user_interaction/prompt_submitted"
    );
}

#[test]
fn automated_and_tool_records_are_not_human_prompts() {
    let text = r#""x""#;
    for (extra, content, want) in [
        (
            r#","promptSource":"system","turnOrigin":"task_notification","origin":{"kind":"task-notification"}"#,
            text,
            "agent_input/automated_origin",
        ),
        (
            r#","origin":{"kind":"peer","body":"PEER MESSAGE"}"#,
            text,
            "agent_input/automated_origin",
        ),
        (
            r#","turnOrigin":"auto_continuation""#,
            text,
            "agent_input/automated_origin",
        ),
        // Each origin marker counts on its own.
        (
            r#","promptSource":"system""#,
            text,
            "agent_input/automated_origin",
        ),
        (r#","isMeta":true"#, text, "agent_input/meta"),
        (
            r#","isCompactSummary":true"#,
            text,
            "agent_input/compact_summary",
        ),
        (r#","scheduledTaskId":"t1""#, text, "agent_input/scheduled"),
        (r#","isSidechain":true"#, text, "agent_input/sidechain"),
        (r#","agentId":"abc123""#, text, "agent_input/sidechain"),
        (
            "",
            r#""<local-command-stdout>Set effort</local-command-stdout>""#,
            "agent_input/tool_or_system_output",
        ),
        (
            "",
            r#""<task-notification>done</task-notification>""#,
            "agent_input/tool_or_system_output",
        ),
        (
            "",
            r#""<system-reminder>x</system-reminder>""#,
            "agent_input/tool_or_system_output",
        ),
        (
            r#","interruptedMessageId":"msg_1""#,
            r#"[{"type":"text","text":"[Request interrupted by user]"}]"#,
            "turn_interrupted",
        ),
        (
            r#","toolUseResult":{"stdout":"OUT"}"#,
            r#"[{"type":"tool_result","tool_use_id":"toolu_1","content":"OUT"}]"#,
            "tool_finished",
        ),
    ] {
        // The duplicated "isSidechain" key is fine: serde_json keeps the last value.
        assert_eq!(key(&user_line(extra, content)), want, "{extra} {content}");
    }
    let sdk = user_line("", text).replace(r#""entrypoint":"cli""#, r#""entrypoint":"sdk-ts""#);
    assert_eq!(key(&sdk), "agent_input/non_interactive_entrypoint");
    assert_eq!(
        outcome_key(&classify(&user_line("", text), StreamRole::Subagent)),
        "agent_input/subagent_stream"
    );
}

#[test]
fn prompt_facts_keep_markers_and_context_but_no_text() {
    let line = user_line(
        r#","promptSource":"typed","turnOrigin":"human","origin":{"kind":"human"},"promptId":"p-1","parentUuid":"aaaaaaaa-0000-4000-8000-000000000000""#,
        r#""SECRET PROMPT TEXT""#,
    );
    let ExtractOutcome::Record(r) = classify(&line, StreamRole::Main) else {
        panic!()
    };
    assert_eq!(r.kind, FactKind::UserInteraction);
    assert_eq!(r.occurred_at, UtcMillis::parse_rfc3339(TS));
    assert_eq!(
        r.native_record_id.as_deref(),
        Some("aaaaaaaa-0000-4000-8000-000000000001")
    );
    assert_eq!(r.producer_version.as_deref(), Some("2.1.281"));
    let m = &r.facts.markers;
    assert_eq!(m.get("prompt_source"), Some(&Marker::Ident("typed".into())));
    assert_eq!(m.get("origin_kind"), Some(&Marker::Ident("human".into())));
    assert_eq!(m.get("entrypoint"), Some(&Marker::Ident("cli".into())));
    assert_eq!(m.get("content_kind"), Some(&Marker::Ident("text".into())));
    assert_eq!(
        r.facts.links.get("prompt_id").map(String::as_str),
        Some("p-1")
    );
    let ctx = r.facts.context.as_ref().unwrap();
    assert_eq!(
        (ctx.cwd.as_deref(), ctx.git_branch.as_deref()),
        (Some("/Users/dev/p"), Some("main"))
    );
    assert!(!serde_json::to_string(&r.facts).unwrap().contains("SECRET"));
}

#[test]
fn peer_message_body_is_never_retained() {
    let line = user_line(
        r#","origin":{"kind":"peer","body":"PEER SECRET","from":"a1"}"#,
        r#""x""#,
    );
    let out = classify(&line, StreamRole::Main);
    assert!(!retained_text(&out).contains("PEER SECRET"));
}

#[test]
fn assistant_usage_is_kept_as_stated_with_unknown_categories_absent() {
    let line = r#"{"type":"assistant","uuid":"bbbbbbbb-0000-4000-8000-000000000001","sessionId":"11111111-1111-4111-8111-111111111111","timestamp":"2026-09-28T14:08:00.000Z","version":"2.1.281","requestId":"req_01","entrypoint":"cli","isSidechain":false,"message":{"id":"msg_01","role":"assistant","model":"claude-opus-5-5","stop_reason":"tool_use","content":[{"type":"thinking","thinking":"HIDDEN"},{"type":"text","text":"ANSWER"},{"type":"tool_use","id":"toolu_1","name":"Bash","input":{"command":"rm -rf SECRET"}},{"type":"tool_use","id":"toolu_2","name":"mcp__huly__list_issues","input":{}}],"usage":{"input_tokens":12,"cache_creation_input_tokens":300,"cache_read_input_tokens":4000,"output_tokens":910,"output_tokens_details":{"thinking_tokens":100},"cache_creation":{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":300},"server_tool_use":{"web_search_requests":1,"web_fetch_requests":0},"service_tier":"standard"}}}"#;
    let out = classify(line, StreamRole::Main);
    assert!(
        !retained_text(&out).contains("SECRET")
            && !retained_text(&out).contains("HIDDEN")
            && !retained_text(&out).contains("ANSWER")
    );
    let ExtractOutcome::Record(r) = out else {
        panic!()
    };
    assert_eq!(r.kind, FactKind::AgentMessage);
    assert_eq!(
        (r.native.request.as_deref(), r.native.message.as_deref()),
        (Some("req_01"), Some("msg_01"))
    );
    let model = r.facts.model.as_ref().unwrap();
    assert_eq!(
        (model.id.as_deref(), model.source),
        (Some("claude-opus-5-5"), ModelSource::Reported)
    );
    let u = r.facts.usage.as_ref().unwrap();
    assert_eq!(u.semantics, UsageSemantics::Snapshot);
    assert_eq!(u.input_fresh, Some(12));
    assert_eq!(
        u.input_total, None,
        "not stated by the source: unknown, not derived"
    );
    assert_eq!(u.cache_creation, Some(300));
    assert_eq!(u.cache_read, Some(4000));
    assert_eq!(u.output, Some(910));
    assert_eq!(u.reasoning_output, Some(100));
    assert_eq!(
        u.cache_creation_tiers.get("ephemeral_1h_input_tokens"),
        Some(&300)
    );
    assert_eq!(u.tool_charges.get("web_search_requests"), Some(&1));
    assert_eq!(u.identity.request_id.as_deref(), Some("req_01"));
    assert_eq!(
        u.extent,
        TemporalExtent::Instant {
            at: UtcMillis::parse_rfc3339("2026-09-28T14:08:00.000Z").unwrap()
        }
    );
    let tools: Vec<(Option<&str>, &str)> = r
        .facts
        .tools_started
        .iter()
        .map(|t| (t.name.as_deref(), t.kind.as_str()))
        .collect();
    assert_eq!(
        tools,
        [
            (Some("Bash"), "builtin"),
            (Some("mcp__huly__list_issues"), "extension")
        ]
    );

    let sparse = r#"{"type":"assistant","uuid":"bbbbbbbb-0000-4000-8000-000000000002","sessionId":"s","timestamp":"2026-09-28T14:09:00Z","message":{"id":"msg_02","model":"<synthetic>","content":[],"usage":{"output_tokens":0}}}"#;
    let ExtractOutcome::Record(r) = classify(sparse, StreamRole::Main) else {
        panic!()
    };
    let u = r.facts.usage.unwrap();
    assert_eq!(
        (u.input_fresh, u.cache_read, u.output),
        (None, None, Some(0)),
        "missing is unknown; 0 is an observed zero"
    );
    assert_eq!(u.identity.request_id, None);
    assert_eq!(r.facts.model.unwrap().id.as_deref(), Some("<synthetic>"));
}

#[test]
fn reported_cost_is_its_own_kind_with_unknown_time() {
    let line = r#"{"type":"cost-state","sessionId":"11111111-1111-4111-8111-111111111111","totalCostUSD":12.5,"hasUnknownModelCost":false,"totalDuration":1234}"#;
    let ExtractOutcome::Record(r) = classify(line, StreamRole::Main) else {
        panic!()
    };
    assert_eq!(r.kind, FactKind::ReportedCostSnapshot);
    assert_eq!(r.occurred_at, None, "the record states no time");
    assert_eq!(r.time_quality.as_str(), "receipt");
    assert_eq!(r.native_record_id, None, "content-derived key");
    let c = r.facts.reported_cost.unwrap();
    assert_eq!(
        (c.value_as_stated.as_str(), c.value_nano),
        ("12.5", Some(12_500_000_000))
    );
    assert_eq!(
        (c.semantics.as_str(), c.scope.as_str()),
        ("cumulative", "session")
    );
}

#[test]
fn ignored_quarantined_and_shape_failures() {
    assert_eq!(
        key(r#"{"type":"last-prompt","lastPrompt":"SECRET","sessionId":"s"}"#),
        "ignored/last-prompt"
    );
    assert_eq!(
        key(r#"{"type":"ai-title","aiTitle":"SECRET","sessionId":"s"}"#),
        "ignored/ai-title"
    );

    let unknown = r#"{"type":"brand-new-type","uuid":"cccccccc-0000-4000-8000-000000000001","sessionId":"s","timestamp":"2026-09-28T14:10:00Z","secretText":"SECRET","nested":{"deepKey":"SECRET"},"bad key":1,"/path/key":2}"#;
    let out = classify(unknown, StreamRole::Main);
    assert!(!retained_text(&out).contains("SECRET"));
    let ExtractOutcome::Quarantine(q) = out else {
        panic!()
    };
    assert_eq!(q.parse_status, ParseStatus::UnknownFormat);
    assert_eq!(q.facts.native_type.as_deref(), Some("brand-new-type"));
    assert_eq!(
        q.facts.top_level_keys,
        [
            "nested",
            "secretText",
            "sessionId",
            "timestamp",
            "type",
            "uuid"
        ]
    );
    assert!(q.occurred_at.is_some());
    assert_eq!(q.facts.byte_len, unknown.len() as u64);

    let ExtractOutcome::Quarantine(q) = classify(r#"{"type":"user","message":"#, StreamRole::Main)
    else {
        panic!()
    };
    assert_eq!(
        (q.parse_status, q.facts.reason.as_str()),
        (ParseStatus::Failed, "unparseable_json")
    );
    assert!(q.facts.top_level_keys.is_empty());

    let no_message = r#"{"type":"assistant","uuid":"dddddddd-0000-4000-8000-000000000001","sessionId":"s","timestamp":"2026-09-28T14:11:00Z"}"#;
    let ExtractOutcome::Quarantine(q) = classify(no_message, StreamRole::Main) else {
        panic!()
    };
    assert_eq!(
        (q.facts.reason.as_str(), q.facts.detail.as_deref()),
        ("shape_check_failed", Some("message"))
    );

    let bad_time = r#"{"type":"user","uuid":"eeeeeeee-0000-4000-8000-000000000001","sessionId":"s","timestamp":"not a time","message":{"content":"x"}}"#;
    let ExtractOutcome::Quarantine(q) = classify(bad_time, StreamRole::Main) else {
        panic!()
    };
    assert_eq!(q.facts.detail.as_deref(), Some("timestamp"));
}

#[test]
fn turn_durations_are_agent_time_facts_only() {
    let line = r#"{"type":"system","subtype":"turn_duration","uuid":"ffffffff-0000-4000-8000-000000000001","sessionId":"s","timestamp":"2026-09-28T14:12:00Z","durationMs":65000,"isMeta":false,"content":"SECRET"}"#;
    let out = classify(line, StreamRole::Main);
    assert!(!retained_text(&out).contains("SECRET"));
    let ExtractOutcome::Record(r) = out else {
        panic!()
    };
    assert_eq!(
        (r.kind, r.subtype.as_deref()),
        (FactKind::SystemEvent, Some("turn_duration"))
    );
    assert_eq!(r.facts.stated_duration_ms, Some(65_000));
}

#[test]
fn interactive_values_are_fixture_validated() {
    // AD-19: an interactive-source value may only yield engagement if the
    // fixture corpus covers it.
    let adapter = ClaudeAdapter::new();
    let mut seen = std::collections::BTreeSet::new();
    for dir in version_dirs() {
        for file in ["main.jsonl", "records.jsonl"] {
            for line in fs::read_to_string(dir.join(file)).unwrap().lines() {
                let v: serde_json::Value = serde_json::from_str(line).unwrap();
                if let Some(e) = v.get("entrypoint").and_then(|e| e.as_str()) {
                    seen.insert(e.to_owned());
                }
            }
        }
    }
    for value in &adapter.manifest().interactive_values {
        assert!(
            seen.contains(*value),
            "{value} declared interactive but absent from fixtures"
        );
    }
    let vscode = user_line("", r#""hi""#)
        .replace(r#""entrypoint":"cli""#, r#""entrypoint":"claude-vscode""#);
    assert_eq!(key(&vscode), "agent_input/non_interactive_entrypoint");
}

#[test]
fn slash_command_and_skill_invocations_are_human() {
    // Current producers lead a typed /command or skill invocation with <command-message>.
    for content in [
        r#""<command-message>review</command-message>\n<command-name>/review</command-name>\n<command-args>the PR</command-args>""#,
        r#""<command-message>init</command-message>\n<command-name>/init</command-name>""#,
        r#""<command-name>/clear</command-name>""#,
        r#""<bash-input>ls</bash-input>""#,
    ] {
        let line = user_line(
            r#","promptSource":"typed","origin":{"kind":"human"}"#,
            content,
        );
        assert_eq!(key(&line), "user_interaction/prompt_submitted", "{content}");
    }
    // Its expansion is an isMeta record: agent input, not a second prompt.
    let expansion = user_line(
        r#","isMeta":true"#,
        r#""<command-message>review</command-message> expanded""#,
    );
    assert_eq!(key(&expansion), "agent_input/meta");
    // Output envelopes are not human.
    assert_eq!(
        key(&user_line("", r#""<bash-stdout>out</bash-stdout>""#)),
        "agent_input/tool_or_system_output"
    );
}

#[test]
fn only_the_leading_text_block_carries_a_tag() {
    let typed_then_reminder = r#"[{"type":"text","text":"please fix it"},{"type":"text","text":"<system-reminder>ctx</system-reminder>"}]"#;
    assert_eq!(
        key(&user_line(
            r#","promptSource":"typed","origin":{"kind":"human"}"#,
            typed_then_reminder
        )),
        "user_interaction/prompt_submitted"
    );
    let reminder_first = r#"[{"type":"text","text":"<system-reminder>ctx</system-reminder>"},{"type":"text","text":"x"}]"#;
    assert_eq!(
        key(&user_line("", reminder_first)),
        "agent_input/tool_or_system_output"
    );
}

#[test]
fn one_word_content_never_reaches_facts() {
    // Identifier-shaped content would pass every identifier validator, so it
    // is the hardest case for the allowlist: it must still never be copied.
    const W: &str = "zqxjkp";
    let lines = [
        user_line(r#","promptSource":"typed""#, &format!("\"{W}\"")),
        user_line(
            "",
            &format!(
                r#"[{{"type":"text","text":"{W}"}},{{"type":"image","source":{{"data":"{W}"}}}}]"#
            ),
        ),
        user_line(
            &format!(r#","toolUseResult":{{"stdout":"{W}","stderr":"{W}"}}"#),
            &format!(r#"[{{"type":"tool_result","tool_use_id":"toolu_1","content":"{W}"}}]"#),
        ),
        user_line(
            &format!(r#","origin":{{"kind":"peer","body":"{W}","from":"a1"}}"#),
            &format!("\"{W}\""),
        ),
        format!(
            r#"{{"type":"queue-operation","operation":"enqueue","sessionId":"s","timestamp":"{TS}","content":"{W}"}}"#
        ),
        format!(
            r#"{{"type":"assistant","uuid":"bbbbbbbb-0000-4000-8000-000000000009","sessionId":"s","timestamp":"{TS}","message":{{"id":"msg_9","model":"m","content":[{{"type":"thinking","thinking":"{W}"}},{{"type":"text","text":"{W}"}},{{"type":"tool_use","id":"toolu_9","name":"Bash","input":{{"command":"{W}"}}}}]}}}}"#
        ),
        format!(
            r#"{{"type":"system","subtype":"local_command","uuid":"ffffffff-0000-4000-8000-000000000009","sessionId":"s","timestamp":"{TS}","content":"{W}"}}"#
        ),
        // Quarantine keeps bounded top-level key *names* (§4.4), never values.
        format!(
            r#"{{"type":"never-seen","uuid":"u9","someKey":"{W}","nested":{{"k":"{W}"}},"sessionId":"s"}}"#
        ),
    ];
    for line in &lines {
        let retained = retained_text(&classify(line, StreamRole::Main));
        assert!(
            !retained.contains(W),
            "content retained from {line}: {retained}"
        );
    }
}

#[test]
fn worktree_exit_is_a_fact_not_a_quarantine() {
    let exit = r#"{"type":"worktree-state","worktreeSession":null,"sessionId":"s"}"#;
    let ExtractOutcome::Record(r) = classify(exit, StreamRole::Main) else {
        panic!("quarantined")
    };
    assert_eq!(r.kind, FactKind::WorktreeObserved);
    assert_eq!(
        r.facts.markers.get("worktree_active"),
        Some(&Marker::Bool(false))
    );
    let missing = r#"{"type":"worktree-state","sessionId":"s"}"#;
    assert!(matches!(
        classify(missing, StreamRole::Main),
        ExtractOutcome::Quarantine(_)
    ));
}

#[test]
fn reported_cost_keeps_the_literal_number() {
    for (literal, nano) in [
        ("10.233278500000003", None),
        ("11.811627600000001", None),
        ("0.5", Some(500_000_000)),
        ("1.23e-5", None),
    ] {
        let line = format!(r#"{{"type":"cost-state","sessionId":"s","totalCostUSD":{literal}}}"#);
        let ExtractOutcome::Record(r) = classify(&line, StreamRole::Main) else {
            panic!()
        };
        let c = r.facts.reported_cost.unwrap();
        assert_eq!((c.value_as_stated.as_str(), c.value_nano), (literal, nano));
    }
    let not_a_number = r#"{"type":"cost-state","sessionId":"s","totalCostUSD":"12"}"#;
    assert!(matches!(
        classify(not_a_number, StreamRole::Main),
        ExtractOutcome::Quarantine(_)
    ));
}
