//! Extraction on schema-faithful lines (shapes profiled from local rollouts):
//! every record kind maps as intended and no content is retained (§11).

use tracr_core::adapter::{ExtractContext, ObservationAdapter, StreamRef, StreamRole};
use tracr_core::events::FactKind;
use tracr_core::evidence::{ExtractOutcome, Marker, StreamKey, UsageSemantics};
use tracr_provider_codex::CodexAdapter;
use tracr_testkit::codex::{CodexThread, Origin, SECRET};

const T: &str = "019f0000-aaaa-7000-8000-00000000000a";
const P: &str = "019fa06c-0000-7000-8000-000000000001";

fn extract(line: &str) -> ExtractOutcome {
    let s = StreamRef {
        stream_key: StreamKey::new("codex:m:t"),
        role: StreamRole::Main,
        native_session: Some(T.into()),
        native_agent: None,
    };
    CodexAdapter::new().extract(line.as_bytes(), &ExtractContext { stream: &s })
}

fn key(o: &ExtractOutcome) -> String {
    match o {
        ExtractOutcome::Record(r) => format!(
            "{}/{}",
            r.kind.as_str(),
            r.subtype.as_deref().unwrap_or("-")
        ),
        ExtractOutcome::Ignore { record_type } => format!("ignored/{record_type}"),
        ExtractOutcome::Quarantine(q) => format!("quarantined/{}", q.facts.reason),
    }
}

fn all_lines(t: &mut CodexThread) -> Vec<String> {
    let mut v = vec![t.meta("09:00")];
    v.extend(t.human_turn("09:01", "09:02", "turn_1", "resp_1", 1000, 100));
    v.push(t.command_item("09:02", "turn_1"));
    v.push(t.token_count("09:02", 1000, 500, 100));
    v.extend(t.turn("09:03", "turn_2"));
    v.push(t.turn_aborted("09:04", "turn_2"));
    v.push(t.task_complete("09:05", "turn_2", true));
    v.push(t.world_state("09:05"));
    v
}

#[test]
fn every_kind_maps_and_nothing_is_quarantined() {
    let mut t = CodexThread::new(T, Origin::Interactive);
    let keys: Vec<String> = all_lines(&mut t).iter().map(|l| key(&extract(l))).collect();
    assert_eq!(
        keys,
        [
            "system_event/session_meta",
            "system_event/task_started",
            "system_event/turn_context",
            "user_interaction/UserMessage",
            "ignored/response_item.message",
            "ignored/response_item.reasoning",
            "agent_message/tool_call",
            "tool_finished/-",
            "agent_message/AgentMessage",
            "agent_message/token_usage_record",
            "system_event/task_complete",
            "ignored/event_msg.item_completed",
            "agent_message/token_count",
            "system_event/task_started",
            "system_event/turn_context",
            "turn_interrupted/-",
            "system_event/task_complete",
            "ignored/world_state",
        ]
    );
}

#[test]
fn no_content_is_retained() {
    let mut t = CodexThread::new(T, Origin::Interactive);
    for line in all_lines(&mut t) {
        if let ExtractOutcome::Record(r) = extract(&line) {
            let text = format!("{r:?}");
            assert!(!text.contains(SECRET), "content retained from {line}");
        }
    }
}

#[test]
fn usage_record_states_fresh_input_and_identity() {
    let mut t = CodexThread::new(T, Origin::Interactive);
    let ExtractOutcome::Record(r) =
        extract(&t.usage_record("09:02", "turn_1", "resp_1", 1000, 400, 100))
    else {
        panic!("record");
    };
    let u = r.facts.usage.unwrap();
    assert_eq!(u.semantics, UsageSemantics::Delta);
    assert_eq!(u.input_total, Some(1000));
    assert_eq!(u.input_fresh, Some(600), "cached input is part of input");
    assert_eq!(u.cache_read, Some(400));
    assert_eq!(
        u.cache_creation, None,
        "cache-write semantics are not established"
    );
    assert_eq!(u.identity.request_id.as_deref(), Some("resp_1"));
    assert_eq!(r.native_record_id.as_deref(), Some("usage:resp_1"));
    assert_eq!(
        r.facts.links.get("turn").map(String::as_str),
        Some("turn_1")
    );
}

#[test]
fn session_meta_keeps_origin_and_parent_reference() {
    let mut i = CodexThread::new(T, Origin::Interactive);
    let ExtractOutcome::Record(r) = extract(&i.meta("09:00")) else {
        panic!()
    };
    assert_eq!(r.producer_version.as_deref(), Some("0.153.4"));
    assert_eq!(
        r.facts.markers.get("source"),
        Some(&Marker::Ident("vscode".into()))
    );
    assert!(r.native.parent_session.is_none());
    assert_eq!(r.facts.context.unwrap().git_branch.as_deref(), Some("main"));

    let mut g = CodexThread::new(T, Origin::Guardian { parent: P.into() });
    let ExtractOutcome::Record(r) = extract(&g.meta("09:00")) else {
        panic!()
    };
    assert_eq!(r.kind, FactKind::SystemEvent);
    assert_eq!(
        r.facts.markers.get("subagent_kind"),
        Some(&Marker::Ident("guardian".into()))
    );
    assert_eq!(r.native.parent_session.as_deref(), Some(P));
    assert_eq!(
        r.facts.links.get("parent_thread").map(String::as_str),
        Some(P)
    );

    let mut s = CodexThread::new(T, Origin::Spawned { parent: P.into() });
    let ExtractOutcome::Record(r) = extract(&s.meta("09:00")) else {
        panic!()
    };
    assert_eq!(
        r.facts.markers.get("subagent_kind"),
        Some(&Marker::Ident("thread_spawn".into()))
    );
    assert_eq!(
        r.facts.links.get("forked_from").map(String::as_str),
        Some(P)
    );
}

#[test]
fn unknown_kinds_are_quarantined_visibly() {
    let line = r#"{"timestamp":"2026-09-28T09:00:00.000Z","type":"event_msg","payload":{"type":"brand_new_event"},"ordinal":3}"#;
    assert_eq!(key(&extract(line)), "quarantined/shape_check_failed");
    let line =
        r#"{"timestamp":"2026-09-28T09:00:00.000Z","type":"new_record","payload":{},"ordinal":3}"#;
    assert_eq!(key(&extract(line)), "quarantined/unknown_record_type");
}
