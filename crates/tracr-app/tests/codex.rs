//! Codex scenarios (S1.6): walkthrough D (Codex part), G-r with the linkage
//! gate on and off, F5 (archived thread), and INV-8 symmetry with Claude.

use std::path::{Path, PathBuf};

use tempfile::TempDir;
use tracr_app::{App, Reader, RootKind, SystemClock};
use tracr_testkit::claude::ClaudeSession;
use tracr_testkit::codex::{CodexThread, Origin};
use tracr_testkit::write_lines;

const B: &str = "019fa06c-1111-7000-8000-000000000001";
const CHILD: &str = "019fa06c-2222-7000-8000-000000000002";
const DAY: &str = "2026-09-28";

struct Env {
    _dir: TempDir,
    data: PathBuf,
    sessions: PathBuf,
    archived: PathBuf,
}

fn env() -> Env {
    let dir = TempDir::new().unwrap();
    let data = dir.path().join("data");
    let sessions = dir.path().join("codex/sessions");
    let archived = dir.path().join("codex/archived_sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    std::fs::create_dir_all(&archived).unwrap();
    Env {
        data,
        sessions,
        archived,
        _dir: dir,
    }
}

fn live_path(e: &Env, t: &CodexThread) -> PathBuf {
    e.sessions.join("2026/09/28").join(t.file_name())
}

fn rebuild(e: &Env, extra_roots: &[&Path]) -> tracr_app::derive::RebuildReport {
    let mut app = App::open_writer(&e.data, Box::new(SystemClock)).unwrap();
    app.add_root(RootKind::Provider, &e.sessions).unwrap();
    app.add_root(RootKind::Provider, &e.archived).unwrap();
    for r in extra_roots {
        app.add_root(RootKind::Provider, r).unwrap();
    }
    app.import(&mut |_, _| {}).unwrap();
    app.rebuild().unwrap()
}

fn input_total(day: &tracr_app::day::DayAgents) -> i64 {
    day.sessions
        .iter()
        .flat_map(|s| &s.usage)
        .map(|u| u.input_fresh.sum + u.cache_read.sum)
        .sum()
}

#[test]
fn d_cumulative_totals_become_deltas() {
    let e = env();
    let mut t = CodexThread::new(B, Origin::Interactive);
    let mut lines = vec![t.meta("09:00")];
    lines.extend(t.turn("09:00", "turn_1"));
    lines.push(t.user_message("09:00", "turn_1", "um_1"));
    // Walkthrough D: cumulative 0.25 / 0.25 / 0.45 / 0.60 M.
    lines.push(t.token_count("09:05", 250_000, 0, 0));
    lines.push(t.token_count("09:06", 250_000, 0, 0));
    lines.push(t.token_count("09:10", 450_000, 0, 0));
    lines.push(t.token_count("09:20", 600_000, 0, 0));
    write_lines(&live_path(&e, &t), &lines);
    rebuild(&e, &[]);

    let day = Reader::open(&e.data).unwrap().day_agents(DAY).unwrap();
    assert_eq!(day.sessions.len(), 1);
    assert_eq!(input_total(&day), 600_000, "0.60 M, not 1.55 M");
    let s = &day.sessions[0];
    assert_eq!(s.provider, "codex");
    assert_eq!(s.diagnostics.get("unchanged_total_suppressed"), Some(&1));
    assert_eq!(
        s.engagement_instants.len(),
        1,
        "an interactive thread's prompt"
    );
    // No price rule for Codex models: unpriced, never zero.
    assert_eq!(s.cost.amount_nano, None);
    assert_eq!(s.cost.status(), Some("unpriced"));
}

#[test]
fn d_usage_records_win_over_cumulative_totals() {
    let e = env();
    let mut t = CodexThread::new(B, Origin::Interactive);
    let mut lines = vec![t.meta("09:00")];
    lines.extend(t.human_turn("09:00", "09:02", "turn_1", "resp_1", 300_000, 1_000));
    lines.push(t.token_count("09:02", 300_000, 150_000, 1_000));
    lines.extend(t.human_turn("09:10", "09:12", "turn_2", "resp_2", 300_000, 1_000));
    lines.push(t.token_count("09:12", 600_000, 300_000, 2_000));
    write_lines(&live_path(&e, &t), &lines);
    rebuild(&e, &[]);

    let day = Reader::open(&e.data).unwrap().day_agents(DAY).unwrap();
    assert_eq!(
        input_total(&day),
        600_000,
        "token_count is not counted again"
    );
    let s = &day.sessions[0];
    assert_eq!(s.engagement_instants.len(), 2);
    let line = &s.usage[0];
    assert_eq!(line.model.as_deref(), Some("gpt-5.6-luna"));
    assert_eq!(line.records, 2);
    assert_eq!(line.cache_read.sum, 300_000);
    assert_eq!(line.output.sum, 2_000);
}

fn g_r(child_version: &str) -> tracr_app::day::DayAgents {
    let e = env();
    let mut b = CodexThread::new(B, Origin::Interactive);
    let mut lines = vec![b.meta("09:00")];
    lines.extend(b.human_turn("09:00", "09:30", "turn_b", "resp_b", 600_000, 1_000));
    write_lines(&live_path(&e, &b), &lines);

    let mut c = CodexThread::new(
        CHILD,
        Origin::Guardian {
            parent: B.to_owned(),
        },
    );
    c.cli_version = child_version.to_owned();
    c.model = "codex-auto-review".into();
    let mut lines = vec![c.meta("09:10")];
    lines.extend(c.turn("09:10", "turn_c"));
    // The reviewer receives a prompt from the parent: never human engagement.
    lines.push(c.user_message("09:10", "turn_c", "um_c"));
    lines.push(c.usage_record("09:12", "turn_c", "resp_c", 200_000, 0, 500));
    lines.push(c.task_complete("09:12", "turn_c", false));
    write_lines(&e.archived.join(c.file_name()), &lines);
    rebuild(&e, &[]);
    Reader::open(&e.data).unwrap().day_agents(DAY).unwrap()
}

#[test]
fn g_r_gate_on_links_the_child_as_a_known_actor() {
    let day = g_r("0.153.4");
    assert_eq!(
        day.sessions.len(),
        1,
        "the child joins its parent's session"
    );
    let s = &day.sessions[0];
    assert_eq!(s.native_session_id, B);
    let kinds: Vec<(&str, Option<&str>)> = s
        .actors
        .iter()
        .map(|a| (a.actor_kind.as_str(), a.native_agent_id.as_deref()))
        .collect();
    assert!(kinds.contains(&("known", Some(CHILD))));
    assert!(kinds.contains(&("unknown", None)), "never promoted to main");
    let by_actor = |kind: &str| -> i64 {
        s.usage
            .iter()
            .filter(|u| {
                s.actors
                    .iter()
                    .any(|a| Some(&a.actor_id) == u.actor_id.as_ref() && a.actor_kind == kind)
            })
            .map(|u| u.input_fresh.sum + u.cache_read.sum)
            .sum()
    };
    assert_eq!(by_actor("unknown"), 600_000);
    assert_eq!(by_actor("known"), 200_000);
    assert_eq!(input_total(&day), 800_000);
    assert_eq!(
        s.engagement_instants.len(),
        1,
        "only the parent's human prompt"
    );
}

#[test]
fn g_r_gate_off_keeps_the_child_apart() {
    let day = g_r("0.130.0");
    assert_eq!(day.sessions.len(), 2);
    let child = day
        .sessions
        .iter()
        .find(|s| s.native_session_id == CHILD)
        .expect("child listed on its own");
    assert_eq!(child.diagnostics.get("unlinked_subagent"), Some(&1));
    assert!(child.actors.iter().all(|a| a.actor_kind == "unknown"));
    assert!(child.engagement_instants.is_empty());
    assert_eq!(input_total(&day), 800_000, "the Codex total is unchanged");
}

#[test]
fn f5_archiving_a_thread_keeps_its_source() {
    let e = env();
    let mut t = CodexThread::new(B, Origin::Interactive);
    let mut lines = vec![t.meta("09:00")];
    lines.extend(t.human_turn("09:00", "09:02", "turn_1", "resp_1", 100_000, 500));
    let live = live_path(&e, &t);
    write_lines(&live, &lines);
    rebuild(&e, &[]);

    // Codex moves the rollout to archived_sessions.
    std::fs::rename(&live, e.archived.join(t.file_name())).unwrap();
    let report = rebuild(&e, &[]);
    assert_eq!(report.streams, 1, "one source, not two");
    let day = Reader::open(&e.data).unwrap().day_agents(DAY).unwrap();
    assert_eq!(day.sessions.len(), 1);
    assert_eq!(input_total(&day), 100_000);
}

/// INV-8: the same human prompts from Claude or from Codex give the same
/// human timeline.
#[test]
fn inv8_human_results_do_not_depend_on_the_provider() {
    let prompts = ["09:00", "09:12", "09:20", "10:05"];

    let claude = env();
    let projects = claude._dir.path().join("projects");
    let mut a = ClaudeSession::new("aaaaaaaa-1111-4111-8111-111111111111");
    let mut lines = Vec::new();
    for (n, p) in prompts.iter().enumerate() {
        lines.push(a.prompt(p));
        lines.push(a.assistant(p, &format!("msg_{n}"), 10));
    }
    write_lines(
        &projects
            .join("-Users-dev-project")
            .join("aaaaaaaa-1111-4111-8111-111111111111.jsonl"),
        &lines,
    );
    rebuild(&claude, &[&projects]);

    let codex = env();
    let mut t = CodexThread::new(B, Origin::Interactive);
    let mut lines = vec![t.meta("09:00")];
    for (n, p) in prompts.iter().enumerate() {
        let turn = format!("turn_{n}");
        lines.extend(t.turn(p, &turn));
        lines.push(t.user_message(p, &turn, &format!("um_{n}")));
        lines.push(t.usage_record(p, &turn, &format!("resp_{n}"), 10, 0, 10));
    }
    write_lines(&live_path(&codex, &t), &lines);
    rebuild(&codex, &[]);

    let atoms = |e: &Env| -> Vec<(i64, i64, String)> {
        Reader::open(&e.data)
            .unwrap()
            .day_agents(DAY)
            .unwrap()
            .human
            .atoms
            .iter()
            .map(|a| (a.start, a.end, a.status.clone()))
            .collect()
    };
    let (c, x) = (atoms(&claude), atoms(&codex));
    assert!(!c.is_empty());
    assert_eq!(c, x);
}

/// A child that arrives after its parent was derived is re-derived with the
/// whole parent session; the parent's own records stay.
#[test]
fn a_late_child_updates_its_parent_session_incrementally() {
    let e = env();
    let mut b = CodexThread::new(B, Origin::Interactive);
    let mut lines = vec![b.meta("09:00")];
    lines.extend(b.human_turn("09:00", "09:30", "turn_b", "resp_b", 600_000, 1_000));
    write_lines(&live_path(&e, &b), &lines);
    rebuild(&e, &[]);

    let mut c = CodexThread::new(
        CHILD,
        Origin::Guardian {
            parent: B.to_owned(),
        },
    );
    let mut lines = vec![c.meta("09:10")];
    lines.extend(c.turn("09:10", "turn_c"));
    lines.push(c.usage_record("09:12", "turn_c", "resp_c", 200_000, 0, 500));
    write_lines(&live_path(&e, &c), &lines);
    let mut app = App::open_writer(&e.data, Box::new(SystemClock)).unwrap();
    app.import(&mut |_, _| {}).unwrap();
    app.update_derived().unwrap();

    let day = Reader::open(&e.data).unwrap().day_agents(DAY).unwrap();
    assert_eq!(day.sessions.len(), 1);
    assert_eq!(input_total(&day), 800_000);
    assert_eq!(day.sessions[0].engagement_instants.len(), 1);
}

/// Codex item IDs repeat across unrelated threads (seen locally): two
/// threads with the same prompt item ID both keep their prompts. Only a
/// global native ID may mark a copy.
#[test]
fn repeated_item_ids_across_threads_are_not_copies() {
    let e = env();
    for (thread, at) in [(B, "09:00"), (CHILD, "11:00")] {
        let mut t = CodexThread::new(thread, Origin::Interactive);
        let mut lines = vec![t.meta(at)];
        lines.extend(t.turn(at, &format!("turn_{thread}")));
        lines.push(t.user_message(at, &format!("turn_{thread}"), "same-item-id"));
        write_lines(&live_path(&e, &t), &lines);
    }
    let report = rebuild(&e, &[]);
    assert_eq!(report.copied_records, 0);
    let day = Reader::open(&e.data).unwrap().day_agents(DAY).unwrap();
    let prompts: usize = day
        .sessions
        .iter()
        .map(|s| s.engagement_instants.len())
        .sum();
    assert_eq!(prompts, 2);
}
