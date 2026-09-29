//! Rebuild scenarios (architecture §8.1, §12.2): copied context, parallel
//! sessions listed individually, and a deterministic rebuild (INV-6).

use std::path::{Path, PathBuf};

use tempfile::TempDir;
use tracr_app::{App, Reader, RootKind, SystemClock};
use tracr_store::derived;
use tracr_testkit::claude::ClaudeSession;
use tracr_testkit::write_lines;

const A: &str = "aaaaaaaa-1111-4111-8111-111111111111";
const B: &str = "bbbbbbbb-2222-4222-8222-222222222222";
const DAY: &str = "2026-09-28";

/// A copied line: the same record (same `uuid`) rewritten into another
/// session, as Claude Code does when it resumes or forks a conversation.
fn copied(line: &str, into: &str) -> String {
    let mut v: serde_json::Value = serde_json::from_str(line).unwrap();
    v["sessionId"] = serde_json::json!(into);
    v.to_string()
}

fn transcript(root: &Path, session: &str) -> PathBuf {
    root.join("-Users-dev-project")
        .join(format!("{session}.jsonl"))
}

fn import_and_rebuild(data: &Path, root: &Path) -> tracr_app::derive::RebuildReport {
    let mut app = App::open_writer(data, Box::new(SystemClock)).unwrap();
    app.add_root(RootKind::Provider, root).unwrap();
    app.import(&mut |_, _| {}).unwrap();
    app.rebuild().unwrap()
}

fn session<'a>(day: &'a tracr_app::day::DayAgents, native: &str) -> &'a tracr_app::day::SessionDay {
    day.sessions
        .iter()
        .find(|s| s.native_session_id == native)
        .unwrap_or_else(|| panic!("session {native} listed"))
}

fn output_tokens(s: &tracr_app::day::SessionDay) -> i64 {
    s.usage.iter().map(|u| u.output.sum).sum()
}

#[test]
fn resume_original_never_continued_keeps_its_history_and_the_copy_derives_nothing_from_it() {
    let dir = TempDir::new().unwrap();
    let (data, root) = (dir.path().join("data"), dir.path().join("projects"));
    let mut a = ClaudeSession::new(A);
    let a_lines = vec![
        a.prompt("09:00"),
        a.assistant("09:01", "msg_1", 100),
        a.prompt("09:10"),
        a.assistant("09:11", "msg_2", 200),
    ];
    write_lines(&transcript(&root, A), &a_lines);
    let mut b = ClaudeSession::new(B);
    let mut b_lines: Vec<String> = a_lines.iter().map(|l| copied(l, B)).collect();
    b_lines.extend([b.prompt("09:30"), b.assistant("09:31", "msg_3", 300)]);
    write_lines(&transcript(&root, B), &b_lines);

    let report = import_and_rebuild(&data, &root);
    assert_eq!(report.copied_records, 4);
    assert_eq!(report.usage_duplicates, 2);

    let day = Reader::open(&data).unwrap().day_agents(DAY).unwrap();
    assert_eq!(day.sessions.len(), 2, "both sessions listed individually");
    let (sa, sb) = (session(&day, A), session(&day, B));
    assert_eq!(
        sa.engagement_instants.len(),
        2,
        "the original keeps its prompts"
    );
    assert_eq!(
        sb.engagement_instants.len(),
        1,
        "copied prompts are no engagement"
    );
    assert_eq!(output_tokens(sa), 300);
    assert_eq!(
        output_tokens(sb),
        300,
        "copied usage is not counted in the copy"
    );
    assert_eq!(sb.actors[0].turns, 1);
}

#[test]
fn fork_copy_continued_first_the_original_still_keeps_its_pre_fork_history() {
    let dir = TempDir::new().unwrap();
    let (data, root) = (dir.path().join("data"), dir.path().join("projects"));
    let mut a = ClaudeSession::new(A);
    let prefix = vec![a.prompt("09:00"), a.assistant("09:01", "msg_1", 100)];
    let mut a_lines = prefix.clone();
    a_lines.extend([a.prompt("10:00"), a.assistant("10:01", "msg_9", 900)]);
    write_lines(&transcript(&root, A), &a_lines);
    std::thread::sleep(std::time::Duration::from_millis(20)); // distinct file creation times
    let mut b = ClaudeSession::new(B);
    let mut b_lines: Vec<String> = prefix.iter().map(|l| copied(l, B)).collect();
    b_lines.extend([b.prompt("09:30"), b.assistant("09:31", "msg_5", 500)]);
    write_lines(&transcript(&root, B), &b_lines);

    import_and_rebuild(&data, &root);
    let day = Reader::open(&data).unwrap().day_agents(DAY).unwrap();
    let (sa, sb) = (session(&day, A), session(&day, B));
    assert_eq!(sa.engagement_instants.len(), 2);
    assert_eq!(sb.engagement_instants.len(), 1);
    assert_eq!(output_tokens(sa), 1000);
    assert_eq!(output_tokens(sb), 500);
}

#[test]
fn parallel_sessions_overlap_and_are_never_merged() {
    let dir = TempDir::new().unwrap();
    let (data, root) = (dir.path().join("data"), dir.path().join("projects"));
    let mut a = ClaudeSession::new(A);
    write_lines(
        &transcript(&root, A),
        &[
            a.prompt("09:00"),
            a.assistant("09:05", "msg_a", 1),
            a.assistant("09:20", "msg_b", 1),
        ],
    );
    let mut b = ClaudeSession::new(B);
    write_lines(
        &transcript(&root, B),
        &[b.prompt("09:02"), b.assistant("09:10", "msg_c", 1)],
    );
    import_and_rebuild(&data, &root);
    let day = Reader::open(&data).unwrap().day_agents(DAY).unwrap();
    assert_eq!(day.sessions.len(), 2);
    let exec: Vec<i64> = [A, B]
        .iter()
        .map(|s| session(&day, s).actors.iter().map(|a| a.execution_ms).sum())
        .collect();
    // 09:00–09:20 has a 15-minute quiet gap (> 10): 09:00–09:05 only; B 09:02–09:10.
    assert_eq!(exec, vec![5 * 60_000, 8 * 60_000]);
    assert!(
        day.pending.iter().any(|p| p.lane == "human"),
        "human time is explicitly not derived yet, never zero"
    );
}

#[test]
fn rebuild_is_deterministic_and_reimport_changes_nothing() {
    let dir = TempDir::new().unwrap();
    let (data, root) = (dir.path().join("data"), dir.path().join("projects"));
    let mut a = ClaudeSession::new(A);
    let a_lines = vec![a.prompt("09:00"), a.assistant("09:01", "msg_1", 100)];
    write_lines(&transcript(&root, A), &a_lines);
    let mut b = ClaudeSession::new(B);
    let mut b_lines: Vec<String> = a_lines.iter().map(|l| copied(l, B)).collect();
    b_lines.extend([b.prompt("09:30"), b.assistant("09:31", "msg_3", 300)]);
    write_lines(&transcript(&root, B), &b_lines);

    import_and_rebuild(&data, &root);
    let dump =
        || derived::canonical_dump(&derived::open_read_only(&data).unwrap().unwrap()).unwrap();
    let first = dump();
    assert!(first.contains("## agent_segment"));
    let mut app = App::open_writer(&data, Box::new(SystemClock)).unwrap();
    app.rebuild().unwrap();
    assert_eq!(dump(), first, "same ledger, same derived rows (INV-6)");
    let r = app.import(&mut |_, _| {}).unwrap();
    assert_eq!(
        r.provider_observations_after,
        r.provider_observations_before
    );
    app.rebuild().unwrap();
    assert_eq!(dump(), first, "re-import changes no derived row");
}
