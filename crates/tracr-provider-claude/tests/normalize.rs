//! Normalizer tests (architecture §8.1, §13.2; walkthroughs D and E-1).

use std::fs;
use std::path::Path;

use tracr_core::adapter::{ExtractContext, ObservationAdapter, StreamRef, StreamRole};
use tracr_core::events::FactKind;
use tracr_core::evidence::{ExtractOutcome, StreamKey};
use tracr_core::normalize::{ActorRef, EventKind, Normalizer, ObservationInput, StreamOutput};
use tracr_core::time::UtcMillis;
use tracr_provider_claude::ClaudeAdapter;
use tracr_provider_claude::normalize::ClaudeNormalizer;
use tracr_testkit::claude::ClaudeSession;

const S: &str = "11111111-1111-4111-8111-111111111111";

fn stream(role: StreamRole, agent: Option<&str>) -> StreamRef {
    StreamRef {
        stream_key: StreamKey::new("claude:test:stream"),
        role,
        native_session: Some(S.into()),
        native_agent: agent.map(str::to_owned),
    }
}

/// Extracts lines the way ingestion would and builds normalizer input.
fn inputs(stream: &StreamRef, lines: &[String]) -> Vec<ObservationInput> {
    let adapter = ClaudeAdapter::new();
    let ctx = ExtractContext { stream };
    lines
        .iter()
        .enumerate()
        .filter_map(|(i, l)| match adapter.extract(l.as_bytes(), &ctx) {
            ExtractOutcome::Record(r) => Some(ObservationInput {
                observation_id: format!("obs{i}"),
                seq: i as i64,
                record_key: r.native_record_id.clone().unwrap_or(format!("h{i}")),
                occurred_at: r.occurred_at,
                observed_at: UtcMillis(0),
                producer_version: r.producer_version.clone(),
                manifest_validated: true,
                kind: r.kind,
                subtype: r.subtype.clone(),
                time_quality: r.time_quality,
                facts: r.facts.clone(),
            }),
            _ => None,
        })
        .collect()
}

fn normalize(stream: &StreamRef, lines: &[String]) -> StreamOutput {
    ClaudeNormalizer::new().normalize_stream(stream, &inputs(stream, lines))
}

fn kinds(out: &StreamOutput) -> Vec<String> {
    out.events
        .iter()
        .map(|e| match &e.subtype {
            Some(s) => format!("{}/{s}", e.kind.as_str()),
            None => e.kind.as_str().to_owned(),
        })
        .collect()
}

#[test]
fn d_streaming_reemissions_elect_the_last_record_910_not_1510() {
    let mut c = ClaudeSession::new(S);
    let lines = vec![
        c.prompt("09:00"),
        c.assistant("09:01", "msg_m1", 120),
        c.assistant("09:01", "msg_m1", 480),
        c.assistant("09:02", "msg_m1", 910),
    ];
    let out = normalize(&stream(StreamRole::Main, None), &lines);
    assert_eq!(out.usage.len(), 1, "one request/message identity");
    let u = &out.usage[0];
    assert_eq!(u.usage.output, Some(910));
    assert_eq!(u.sources, vec![1, 2, 3]);
    assert_eq!(u.observation, 3);
    assert_eq!(u.identity.as_deref(), Some("req_msg_m1/msg_m1"));
    assert_eq!(u.actor, ActorRef::Unknown, "AD-21: main chain is unknown");
    assert_eq!(
        u.model.as_ref().map(|m| m.0.as_str()),
        Some("claude-opus-5-5")
    );
}

#[test]
fn a_human_prompt_yields_an_instant_and_a_human_turn() {
    let mut c = ClaudeSession::new(S);
    let lines = vec![
        c.prompt("09:00"),
        c.assistant("09:01", "msg_a", 5),
        c.tool_result("09:02", "toolu_msg_a"),
    ];
    let out = normalize(&stream(StreamRole::Main, None), &lines);
    assert_eq!(
        kinds(&out),
        vec![
            "user_interaction/prompt_submitted",
            "turn_started/human",
            "model_observed",
            "tool_started",
            "tool_finished",
        ]
    );
    assert_eq!(out.events[0].actor, ActorRef::Human);
    assert_eq!(out.events[1].actor, ActorRef::Unknown);
    let started = &out.events[3];
    assert_eq!(started.native_id.as_deref(), Some("toolu_msg_a"));
    assert_eq!(
        started.tool.as_ref().and_then(|t| t.0.as_deref()),
        Some("Bash")
    );
}

#[test]
fn automated_entrypoint_prompts_run_the_agent_but_are_never_human() {
    let mut c = ClaudeSession::new(S);
    c.entrypoint = "sdk-cli".into();
    let lines = vec![c.prompt("09:00"), c.assistant("09:01", "msg_a", 5)];
    let out = normalize(&stream(StreamRole::Main, None), &lines);
    assert!(
        !out.events
            .iter()
            .any(|e| e.kind == EventKind::UserInteraction),
        "no engagement from a non-interactive entrypoint"
    );
    assert_eq!(kinds(&out)[0], "turn_started/automated");
    assert_eq!(
        out.diagnostics
            .get("prompt_from_non_interactive_entrypoint"),
        Some(&1)
    );
}

#[test]
fn task_notifications_start_automated_turns() {
    let mut c = ClaudeSession::new(S);
    let lines = vec![
        c.task_notification("10:00"),
        c.assistant("10:01", "msg_a", 5),
    ];
    let out = normalize(&stream(StreamRole::Main, None), &lines);
    assert_eq!(kinds(&out)[0], "turn_started/automated");
    assert!(
        !out.events
            .iter()
            .any(|e| e.kind == EventKind::UserInteraction)
    );
}

#[test]
fn subagent_streams_are_their_known_agent_and_never_human() {
    let mut c = ClaudeSession::new(S);
    let lines = vec![c.prompt("09:00"), c.assistant("09:01", "msg_a", 5)];
    let out = normalize(&stream(StreamRole::Subagent, Some("a1")), &lines);
    assert_eq!(kinds(&out)[0], "turn_started/agent");
    assert!(
        out.events
            .iter()
            .all(|e| e.actor == ActorRef::Known("a1".into()))
    );
    assert_eq!(out.usage[0].actor, ActorRef::Known("a1".into()));
}

#[test]
fn records_without_request_and_message_ids_stay_unidentified_and_separate() {
    let mut c = ClaudeSession::new(S);
    let strip = |l: String| {
        let mut v: serde_json::Value = serde_json::from_str(&l).unwrap();
        v.as_object_mut().unwrap().remove("requestId");
        v["message"].as_object_mut().unwrap().remove("id");
        v.to_string()
    };
    let lines = vec![
        strip(c.assistant("09:01", "x", 5)),
        strip(c.assistant("09:02", "x", 7)),
    ];
    let out = normalize(&stream(StreamRole::Main, None), &lines);
    assert_eq!(out.usage.len(), 2, "unidentified usage is never collapsed");
    assert!(out.usage.iter().all(|u| u.identity.is_none()));
}

#[test]
fn cwd_changes_are_events() {
    let mut c = ClaudeSession::new(S);
    let a = c.prompt("09:00");
    c.cwd = "/Users/dev/other".into();
    let b = c.prompt("09:05");
    let out = normalize(&stream(StreamRole::Main, None), &[a, b]);
    assert_eq!(
        kinds(&out).iter().filter(|k| *k == "cwd_changed").count(),
        1
    );
}

/// E-1: the normalizer re-decides human prompts from retained markers and
/// agrees with the extractor's classification on every fixture record.
#[test]
fn e1_renormalization_from_retained_facts_agrees_with_extraction_on_fixtures() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let mut checked = 0;
    for version in fs::read_dir(&dir).unwrap() {
        let version = version.unwrap().path();
        if !version.is_dir() {
            continue;
        }
        for (file, role, agent) in [
            ("main.jsonl", StreamRole::Main, None),
            ("subagent.jsonl", StreamRole::Subagent, Some("fixture")),
        ] {
            let path = version.join(file);
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            let s = stream(role, agent);
            let lines: Vec<String> = text.lines().map(str::to_owned).collect();
            let input = inputs(&s, &lines);
            let out = ClaudeNormalizer::new().normalize_stream(&s, &input);
            for (i, r) in input.iter().enumerate() {
                let extractor_human = r.kind == FactKind::UserInteraction;
                let normalizer_human = out
                    .events
                    .iter()
                    .any(|e| e.observation == i && e.kind == EventKind::UserInteraction);
                assert_eq!(
                    extractor_human,
                    normalizer_human,
                    "{}: record {} ({:?}/{:?})",
                    path.display(),
                    r.record_key,
                    r.kind,
                    r.subtype
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 100, "fixtures exercised: {checked}");
}
