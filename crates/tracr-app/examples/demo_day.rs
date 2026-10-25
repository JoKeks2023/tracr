//! An invented working day for README and website screenshots:
//! `cargo run -p tracr-app --example demo_day -- <out dir>`.
//!
//! Writes provider files, three throwaway git repos and a complete data
//! directory `<out>/data` (device active, one locked lunch break), derived and
//! ready to open: `TRACR_DATA_DIR=<out>/data` with the CLI or the desktop app.
//! It never touches a real ledger. Times are UTC; the reporting day is
//! 2026-09-28 (09:00 local = 07:00Z).
//!
//! The day is built like walkthrough C: one session is engaged at a time
//! (prompts every few minutes, agent output every 4 minutes), with a few
//! handoffs, one overlap and one long unattended run.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use serde_json::{Value, json};
use tracr_app::runtime::Collector;
use tracr_app::{App, RootKind};
use tracr_core::device::DeviceSample;
use tracr_core::evidence::IngestTrigger;
use tracr_core::time::UtcMillis;
use tracr_platform::ManualClock;
use tracr_testkit::claude::ClaudeSession;
use tracr_testkit::codex::{CodexThread, Origin};
use tracr_testkit::write_lines;

const MIN: i64 = 60_000;

fn at(hhmm: &str) -> UtcMillis {
    UtcMillis::parse_rfc3339(&tracr_testkit::collector::at(hhmm)).unwrap()
}

fn hm(t: i64) -> String {
    let m = (t / MIN) % (24 * 60);
    format!("{:02}:{:02}", m / 60, m % 60)
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

/// Replaces the placeholder usage of an assistant line with realistic sizes.
fn sized(line: String, model: &str, n: u64) -> String {
    let mut v: Value = serde_json::from_str(&line).unwrap();
    v["message"]["model"] = json!(model);
    let write = 2_000 + (n * 613) % 7_000;
    let u = &mut v["message"]["usage"];
    u["input_tokens"] = json!(12);
    u["output_tokens"] = json!(2_200 + (n * 937) % 3_200);
    u["cache_creation_input_tokens"] = json!(write);
    u["cache_creation"] =
        json!({"ephemeral_5m_input_tokens": write, "ephemeral_1h_input_tokens": 0});
    u["cache_read_input_tokens"] = json!(120_000 + (n % 25) * 14_000);
    v.to_string()
}

struct Work {
    s: ClaudeSession,
    lines: Vec<String>,
    model: &'static str,
    n: u64,
}

impl Work {
    fn new(id: &str, cwd: &str, branch: &str, model: &'static str) -> Self {
        let mut s = ClaudeSession::new(id);
        s.cwd = cwd.into();
        s.branch = branch.into();
        Self {
            s,
            lines: vec![],
            model,
            n: 0,
        }
    }

    fn prompt(&mut self, at: &str) {
        self.lines.push(self.s.prompt(at));
    }

    /// Agent output every 4 minutes from `from` to `to`.
    fn run(&mut self, from: &str, to: &str) {
        for t in every_4_min(from, to) {
            self.n += 1;
            let id = format!("msg_{}_{}", &self.s.session_id[..4], self.n);
            let a = self.s.assistant(&t, &id, 10);
            self.lines.push(sized(a, self.model, self.n));
            self.lines
                .push(self.s.tool_result(&t, &format!("toolu_{id}")));
        }
    }

    /// A prompt, then output until the next prompt.
    fn turn(&mut self, prompt: &str, until: &str) {
        self.prompt(prompt);
        let from = hm(at(prompt).0 + MIN);
        self.run(&from, until);
    }

    fn finish(mut self, total: &str, out: &Path, cwd: &str) {
        self.lines.push(self.s.cost_state(total));
        let dir = cwd.replace(['/', '.'], "-");
        let path = out
            .join("claude/projects")
            .join(dir)
            .join(format!("{}.jsonl", self.s.session_id));
        write_lines(&path, &self.lines);
    }
}

fn repo(code: &Path, owner: &str, name: &str) -> String {
    let dir = code.join(owner).join(name);
    std::fs::create_dir_all(&dir).unwrap();
    for args in [
        vec!["init", "-q"],
        vec![
            "remote",
            "add",
            "origin",
            &format!("https://github.com/{owner}/{name}.git"),
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

fn main() {
    let out = PathBuf::from(std::env::args().nth(1).expect("usage: demo_day <out dir>"));
    let code = out.join("code");
    let web_dir = repo(&code, "acme", "web");
    let api_dir = repo(&code, "acme", "api");
    let notes_dir = repo(&code, "side", "notes");

    // acme/web: the long session of the day, two engaged blocks, and a long
    // unattended run while the human works on the api.
    let mut web = Work::new(
        "a1b2c3d4-0000-4000-8000-00000000a001",
        &web_dir,
        "feat/ACME-142-invoice-export",
        "claude-opus-5-5",
    );
    let first = [
        ("07:02", "07:11"),
        ("07:12", "07:23"),
        ("07:24", "07:35"),
        ("07:36", "07:47"),
        ("07:48", "07:59"),
        ("08:00", "08:13"),
        ("08:14", "08:27"),
        ("08:28", "08:39"),
        ("08:40", "08:51"),
        ("08:52", "09:04"),
        ("09:05", "09:17"),
        ("09:18", "09:31"),
        ("09:32", "09:46"),
    ];
    for (p, until) in first {
        web.turn(p, until);
    }
    let second = [
        ("11:24", "11:35"),
        ("11:36", "11:49"),
        ("11:50", "12:03"),
        ("12:04", "12:17"),
        ("12:18", "12:31"),
        ("12:32", "12:45"),
        ("12:46", "13:01"),
    ];
    for (p, until) in second {
        web.turn(p, until);
    }
    web.prompt("13:42");
    web.run("13:43", "14:22"); // a long run, no human minutes
    web.finish("15.90", &out, &web_dir);

    // acme/api: the human moves here three times.
    let mut api = Work::new(
        "b2c3d4e5-0000-4000-8000-00000000b002",
        &api_dir,
        "fix/ACME-151-rate-limit",
        "claude-opus-5-5",
    );
    for (p, until) in [
        ("09:50", "09:59"),
        ("10:00", "10:09"),
        ("10:10", "10:16"),
        ("13:20", "13:31"),
        ("13:32", "13:41"),
        ("13:50", "14:01"),
        ("14:02", "14:11"),
        ("14:12", "14:20"),
    ] {
        api.turn(p, until);
    }
    api.finish("4.46", &out, &api_dir);

    // side/notes: a late block in a lighter model.
    let mut notes = Work::new(
        "c3d4e5f6-0000-4000-8000-00000000c003",
        &notes_dir,
        "main",
        "claude-sonnet-5",
    );
    for (p, until) in [
        ("14:34", "14:45"),
        ("14:46", "14:57"),
        ("14:58", "15:09"),
        ("15:10", "15:21"),
        ("15:22", "15:33"),
        ("15:34", "15:45"),
    ] {
        notes.turn(p, until);
    }
    notes.finish("2.86", &out, &notes_dir);

    // One Codex thread on the api project, in two blocks.
    let mut c = CodexThread::new("019fd000-aaaa-7000-8000-0000000000d1", Origin::Interactive);
    c.cwd = api_dir.clone();
    c.model = "gpt-5.6-terra".into();
    c.service_tier = Some("default".into());
    let mut lines = vec![c.meta("08:15")];
    for (n, (prompt, to, input)) in [
        ("08:15", "08:26", 380_000u64),
        ("08:27", "08:38", 520_000),
        ("13:05", "13:16", 610_000),
        ("13:17", "13:19", 450_000),
    ]
    .iter()
    .enumerate()
    {
        let turn = format!("turn_{n}");
        lines.extend(c.turn(prompt, &turn));
        lines.push(c.user_message(prompt, &turn, &format!("um_{n}")));
        for (k, t) in every_4_min(&hm(at(prompt).0 + MIN), to).iter().enumerate() {
            let call = format!("call_{n}_{k}");
            lines.push(c.tool_call(t, &turn, &call));
            lines.push(c.tool_output(t, &turn, &call));
        }
        lines.push(c.usage_record(to, &turn, &format!("resp_{n}"), *input, input / 2, 9_000));
        lines.push(c.task_complete(to, &turn, false));
    }
    write_lines(
        &out.join("codex/sessions/2026/09/28").join(c.file_name()),
        &lines,
    );

    // Collect the day with the device active, except a locked lunch break.
    let data = out.join("data");
    let clock = Arc::new(ManualClock::new(at("06:55")));
    let mut app = App::open_writer(&data, Box::new(clock.clone())).unwrap();
    app.add_root(RootKind::Provider, &out.join("claude/projects"))
        .unwrap();
    app.add_root(RootKind::Provider, &out.join("codex/sessions"))
        .unwrap();
    app.add_root(RootKind::Repository, &code).unwrap();
    let mut col = Collector::begin(app, "demo", true).unwrap();
    let (end, lunch) = (at("15:50"), (at("10:20"), at("11:20")));
    let mut t = at("06:55");
    while t <= end {
        clock.set(t);
        let locked = t >= lunch.0 && t < lunch.1;
        col.sample(&DeviceSample {
            at: t,
            idle_ms: Some(if locked { 3_000_000 } else { 1_000 }),
            locked: Some(locked),
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
    col.end("demo done").unwrap();

    println!("wrote demo day under {}", out.display());
}
