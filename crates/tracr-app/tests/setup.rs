//! Project setup facts (TRACR-23): repositories tracr observed under an
//! approved repository root are inspected; others are not.

use std::process::Command;
use std::sync::Arc;

use tempfile::TempDir;
use tracr_app::runtime::Collector;
use tracr_app::{App, Reader, RootKind};
use tracr_core::evidence::IngestTrigger;
use tracr_core::time::UtcMillis;
use tracr_platform::ManualClock;
use tracr_testkit::claude::ClaudeSession;
use tracr_testkit::write_lines;

const A: &str = "aaaaaaaa-1111-4111-8111-111111111111";

#[test]
fn observed_repositories_under_approved_roots_are_inspected() {
    let dir = TempDir::new().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let (data, root, code) = (base.join("data"), base.join("projects"), base.join("code"));
    let repo = code.join("shop");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::create_dir_all(&root).unwrap();
    assert!(
        Command::new("git")
            .args(["init", "-q"])
            .current_dir(&repo)
            .status()
            .unwrap()
            .success()
    );
    std::fs::write(repo.join("CLAUDE.md"), "Follow @docs/rules.md.").unwrap();

    let at = UtcMillis::parse_rfc3339(&tracr_testkit::collector::at("09:00")).unwrap();
    let clock = Arc::new(ManualClock::new(at));
    let mut app = App::open_writer(&data, Box::new(clock.clone())).unwrap();
    app.add_root(RootKind::Provider, &root).unwrap();
    app.add_root(RootKind::Repository, &code).unwrap();
    let mut c = Collector::begin(app, "test", true).unwrap();
    let mut a = ClaudeSession::new(A);
    a.cwd = repo.to_string_lossy().into_owned();
    write_lines(
        &root.join("-code-shop").join(format!("{A}.jsonl")),
        &[a.prompt("09:00")],
    );
    c.reconcile_all(IngestTrigger::Startup).unwrap();
    c.probe_git().unwrap();

    let view = Reader::open(&data).unwrap().project_setup().unwrap();
    assert_eq!(view.projects.len(), 1);
    let p = &view.projects[0];
    assert!(p.path.ends_with("shop"));
    assert_eq!(p.instructions[0].references, ["docs/rules.md"]);
    let rules: Vec<&str> = p.findings.iter().map(|f| f.rule).collect();
    assert_eq!(
        rules,
        [
            "setup.reference_missing@1",
            "setup.single_instruction_file@1"
        ]
    );
    assert!(view.plans.is_empty(), "no Claude folder approved");
}
