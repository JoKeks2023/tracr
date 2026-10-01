//! Walkthrough C before review (S1.6): three parallel sessions, two
//! providers. Claude A "PDF Export" and C "Docs cleanup", Codex B "Server
//! Fix", collected on one day with the device active.
//!
//! Must hold: three sessions listed individually; human time counted once
//! (52 counted, 13 unknown); C has no human minutes; counted time is
//! attributed structurally to the engaged session's repository.

use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use tempfile::TempDir;
use tracr_app::runtime::Collector;
use tracr_app::{App, Reader, RootKind};
use tracr_core::device::DeviceSample;
use tracr_core::evidence::IngestTrigger;
use tracr_core::time::UtcMillis;
use tracr_platform::ManualClock;
use tracr_testkit::claude::ClaudeSession;
use tracr_testkit::codex::{CodexThread, Origin};
use tracr_testkit::write_lines;

const A: &str = "aaaaaaaa-1111-4111-8111-111111111111";
const C: &str = "cccccccc-3333-4333-8333-333333333333";
const B: &str = "019f0000-bbbb-7000-8000-00000000000b";
const MIN: i64 = 60_000;

fn at(hhmm: &str) -> UtcMillis {
    UtcMillis::parse_rfc3339(&tracr_testkit::collector::at(hhmm)).unwrap()
}

fn hm(t: i64) -> String {
    let m = (t / MIN) % (24 * 60);
    format!("{:02}:{:02}", m / 60, m % 60)
}

fn repo(base: &Path, name: &str) -> String {
    let dir = base.join(name);
    std::fs::create_dir_all(&dir).unwrap();
    for args in [
        vec!["init", "-q"],
        vec![
            "remote",
            "add",
            "origin",
            &format!("https://github.com/example/{name}.git"),
        ],
    ] {
        assert!(
            Command::new("git")
                .args(&args)
                .current_dir(&dir)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    dir.to_string_lossy().into_owned()
}

/// Agent output every 4 minutes across a run, so it is one execution segment.
fn every_4_min(from: &str, to: &str) -> Vec<String> {
    let (mut t, end) = (at(from).0, at(to).0);
    let mut out = Vec::new();
    while t < end {
        out.push(hm(t));
        t += 4 * MIN;
    }
    out.push(to.to_owned());
    out
}

fn claude_run(s: &mut ClaudeSession, lines: &mut Vec<String>, from: &str, to: &str, tag: &str) {
    for (n, t) in every_4_min(from, to).iter().enumerate() {
        lines.push(s.assistant(t, &format!("{tag}_{n}"), 10));
    }
}

#[test]
fn c_before_review_parallel_sessions_count_human_time_once() {
    let dir = TempDir::new().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let data = base.join("data");
    let claude_root = base.join("claude/projects");
    let codex_root = base.join("codex/sessions");
    let code = base.join("code");
    std::fs::create_dir_all(&claude_root).unwrap();
    std::fs::create_dir_all(&codex_root).unwrap();

    // Session C: one prompt starting a long run.
    let mut c = ClaudeSession::new(C);
    c.cwd = repo(&code, "notes");
    let mut lines = vec![c.prompt("08:50")];
    claude_run(&mut c, &mut lines, "08:51", "09:50", "c");
    write_lines(
        &claude_root.join("-code-notes").join(format!("{C}.jsonl")),
        &lines,
    );

    // Session A: prompts 09:00, 09:10, 09:25 (approval), 09:27.
    let mut a = ClaudeSession::new(A);
    a.cwd = repo(&code, "tracr-app");
    let mut lines = vec![a.prompt("09:00")];
    claude_run(&mut a, &mut lines, "09:01", "09:09", "a1");
    lines.push(a.prompt("09:10"));
    claude_run(&mut a, &mut lines, "09:11", "09:24", "a2");
    lines.push(a.prompt("09:25"));
    lines.push(a.prompt("09:27"));
    claude_run(&mut a, &mut lines, "09:28", "09:45", "a3");
    write_lines(
        &claude_root
            .join("-code-tracr-app")
            .join(format!("{A}.jsonl")),
        &lines,
    );

    // Session B (Codex): prompts 09:26, 09:28 (approval), 09:40, 09:55;
    // cumulative counters reaching 0.8 M.
    let mut b = CodexThread::new(B, Origin::Interactive);
    b.cwd = repo(&code, "server");
    let mut lines = vec![b.meta("09:26")];
    let mut total = 0;
    for (n, (prompt, from, to)) in [
        ("09:26", "09:26", "09:26"),
        ("09:28", "09:29", "09:39"),
        ("09:40", "09:41", "09:54"),
        ("09:55", "09:56", "10:10"),
    ]
    .iter()
    .enumerate()
    {
        let turn = format!("turn_{n}");
        lines.extend(b.turn(prompt, &turn));
        lines.push(b.user_message(prompt, &turn, &format!("um_{n}")));
        if from != to {
            for (k, t) in every_4_min(from, to).iter().enumerate() {
                let call = format!("call_{n}_{k}");
                lines.push(b.tool_call(t, &turn, &call));
                lines.push(b.tool_output(t, &turn, &call));
            }
            total += 266_667;
            lines.push(b.token_count(to, total.min(800_000), 0, 0));
            lines.push(b.task_complete(to, &turn, false));
        }
    }
    write_lines(&codex_root.join("2026/09/28").join(b.file_name()), &lines);

    // Collect 08:45–10:15, device active throughout.
    let clock = Arc::new(ManualClock::new(at("08:45")));
    let mut app = App::open_writer(&data, Box::new(clock.clone())).unwrap();
    app.add_root(RootKind::Provider, &claude_root).unwrap();
    app.add_root(RootKind::Provider, &codex_root).unwrap();
    app.add_root(RootKind::Repository, &code).unwrap();
    let mut col = Collector::begin(app, "test", true).unwrap();
    let mut t = at("08:45");
    while t <= at("10:15") {
        clock.set(t);
        col.sample(&DeviceSample {
            at: t,
            idle_ms: Some(1_000),
            locked: Some(false),
            last_sleep_wake: None,
        })
        .unwrap();
        if (t.0 / 1000) % 60 == 0 {
            col.heartbeat().unwrap();
        }
        t = UtcMillis(t.0 + 15_000);
    }
    col.reconcile_all(IngestTrigger::Startup).unwrap();
    col.probe_git().unwrap();
    col.rebuild().unwrap();
    col.end("test done").unwrap();

    let day = Reader::open(&data)
        .unwrap()
        .day_agents("2026-09-28")
        .unwrap();

    // Three sessions, each listed individually, two providers.
    assert_eq!(day.sessions.len(), 3);
    let providers: Vec<&str> = day.sessions.iter().map(|s| s.provider.as_str()).collect();
    assert_eq!(providers.iter().filter(|p| **p == "codex").count(), 1);

    // Human time: one timeline, counted once.
    let h = &day.human;
    let summary: Vec<(String, String, String, Option<String>)> = h
        .atoms
        .iter()
        .map(|x| {
            (
                hm(x.start),
                hm(x.end),
                x.status.clone(),
                x.unknown_reason.clone(),
            )
        })
        .collect();
    assert_eq!(h.counted_ms, 52 * MIN, "25 + 27 inferred: {summary:?}");
    assert_eq!(h.unknown_ms, 13 * MIN, "10 + 3: {summary:?}");
    assert_eq!(
        h.unknown_by_reason.get("unresolved_window"),
        Some(&(10 * MIN)),
        "08:50–09:00, C → A"
    );
    assert_eq!(
        h.unknown_by_reason.get("conflicting_evidence"),
        Some(&(3 * MIN)),
        "09:25–09:28, A and B interleave"
    );
    let span = h.evidence_span.map(|(s, e)| e - s);
    assert_eq!(span, Some(65 * MIN), "08:50–09:55, an upper bound only");

    // C's long run is agent time only: no human minutes are attributed to notes.
    assert_eq!(
        h.counted_by_target.get("example/tracr-app"),
        Some(&(25 * MIN))
    );
    assert_eq!(h.counted_by_target.get("example/server"), Some(&(27 * MIN)));
    assert_eq!(h.counted_by_target.get("example/notes"), None);
    assert_eq!(h.counted_by_target.get("unattributed"), None);

    // Agent time: overlapping execution is kept per session, never merged.
    let exec = |native: &str| -> i64 {
        day.sessions
            .iter()
            .find(|s| s.native_session_id == native)
            .unwrap()
            .actors
            .iter()
            .map(|a| a.execution_ms)
            .sum()
    };
    // A segment opens at its turn's start, the prompt instant. The
    // walkthrough's illustrative runs start one minute after each prompt
    // (59 · 38 · 37); with the turn start as evidence the same runs are
    // 60 · 41 · 40. Either way they overlap and exceed the 80-minute wall clock.
    assert_eq!((exec(C), exec(A), exec(B)), (60 * MIN, 41 * MIN, 40 * MIN));

    // Codex B usage from cumulative counters: 0.8 M.
    let b_input: i64 = day
        .sessions
        .iter()
        .find(|s| s.native_session_id == B)
        .unwrap()
        .usage
        .iter()
        .map(|u| u.input_fresh.sum + u.cache_read.sum)
        .sum();
    assert_eq!(b_input, 800_000);
}
