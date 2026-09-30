//! Rebuild scenarios (architecture §8.1, §12.2): copied context, parallel
//! sessions listed individually, and a deterministic rebuild (INV-6).

use std::path::{Path, PathBuf};

use tempfile::TempDir;
use tracr_app::{App, Reader, RootKind, SystemClock, UtcMillis};
use tracr_platform::ManualClock;
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
    // No collector ran: nothing is counted, and the time between the
    // prompts is unknown for lack of coverage, never zero and never work.
    assert_eq!(day.human.counted_ms, 0);
    assert_eq!(
        day.human.unknown_by_reason.get("coverage_gap"),
        Some(&(2 * 60_000))
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

    // INV-6 compares rebuilds at the same `as_of`: coverage reaches up to it.
    let fixed = || Box::new(ManualClock::new(UtcMillis(1_790_600_000_000)));
    let mut app = App::open_writer(&data, fixed()).unwrap();
    app.add_root(RootKind::Provider, &root).unwrap();
    app.import(&mut |_, _| {}).unwrap();
    app.rebuild().unwrap();
    let dump =
        || derived::canonical_dump(&derived::open_read_only(&data).unwrap().unwrap()).unwrap();
    let first = dump();
    assert!(first.contains("## agent_segment") && first.contains("## human_atom"));
    app.rebuild().unwrap();
    assert_same(
        &dump(),
        &first,
        "same ledger, same as_of, same derived rows (INV-6)",
    );
    let r = app.import(&mut |_, _| {}).unwrap();
    assert_eq!(
        r.provider_observations_after,
        r.provider_observations_before
    );
    app.rebuild().unwrap();
    assert_same(&dump(), &first, "re-import changes no derived row");
}

/// Compares two canonical dumps and reports only the lines that differ.
fn assert_same(got: &str, want: &str, what: &str) {
    let g: std::collections::BTreeSet<&str> = got.lines().collect();
    let w: std::collections::BTreeSet<&str> = want.lines().collect();
    let only_got: Vec<&&str> = g.difference(&w).collect();
    let only_want: Vec<&&str> = w.difference(&g).collect();
    assert!(
        only_got.is_empty() && only_want.is_empty(),
        "{what}\n+ {only_got:#?}\n- {only_want:#?}"
    );
}

#[test]
fn agent_sessions_csv_has_one_row_per_day_session_actor_and_prompts_once() {
    let dir = TempDir::new().unwrap();
    let (data, root) = (dir.path().join("data"), dir.path().join("projects"));
    let mut a = ClaudeSession::new(A);
    a.cwd = "/Users/dev/my, project".into(); // a comma must be quoted
    write_lines(
        &transcript(&root, A),
        &[
            a.prompt("09:00"),
            a.assistant("09:01", "m1", 100),
            a.prompt("09:05"),
            a.assistant("09:06", "m2", 50),
        ],
    );
    let mut sub = ClaudeSession::new(A);
    sub.uuid_prefix = "5ab5ab5a".into();
    write_lines(
        &root
            .join("-Users-dev-project")
            .join(A)
            .join("subagents")
            .join("agent-abc123.jsonl"),
        &[sub.prompt("09:02"), sub.assistant("09:03", "m3", 7)],
    );
    import_and_rebuild(&data, &root);
    let csv =
        tracr_app::export::agent_sessions_csv(&Reader::open(&data).unwrap(), DAY, DAY).unwrap();
    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 3, "header + main chain + subagent:\n{csv}");
    assert!(lines[0].starts_with("day,timezone,provider,session_id"));
    assert!(csv.contains("\"/Users/dev/my, project\""), "{csv}");
    let header = tracr_app::export::AGENT_SESSIONS_COLUMNS;
    let prompts = header.iter().position(|c| *c == "human_prompts").unwrap();
    let output = header.iter().position(|c| *c == "output_tokens").unwrap();
    let plain: Vec<Vec<String>> = lines[1..].iter().map(|l| parse_csv_line(l)).collect();
    assert!(plain.iter().all(|r| r.len() == header.len()));
    assert_eq!(plain[0][6], "/Users/dev/my, project");
    let main = plain.iter().find(|r| r[8] == "unknown").unwrap();
    let agent = plain.iter().find(|r| r[8] == "known").unwrap();
    assert_eq!(main[prompts], "2");
    assert_eq!(agent[prompts], "", "prompts only on the main-chain row");
    assert_eq!(main[output], "150");
    assert_eq!(agent[output], "7");
}

/// Minimal RFC 4180 line parser for the assertions above.
fn parse_csv_line(line: &str) -> Vec<String> {
    let (mut out, mut cur, mut quoted, mut chars) =
        (Vec::new(), String::new(), false, line.chars().peekable());
    while let Some(c) = chars.next() {
        match (c, quoted) {
            ('"', true) if chars.peek() == Some(&'"') => {
                cur.push('"');
                chars.next();
            }
            ('"', _) => quoted = !quoted,
            (',', false) => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}
