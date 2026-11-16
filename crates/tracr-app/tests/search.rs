//! Session search over metadata (TRACR-21): filters by period, provider,
//! branch, text, model, tool, project, binding and cost; facets list the
//! values of the period. Never content.

use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use tempfile::TempDir;
use tracr_app::bindings::{BindingOp, Subject};
use tracr_app::projects::ProjectOp;
use tracr_app::runtime::Collector;
use tracr_app::search::SessionQuery;
use tracr_app::{App, Reader, RootKind};
use tracr_core::evidence::IngestTrigger;
use tracr_core::time::UtcMillis;
use tracr_platform::ManualClock;
use tracr_testkit::claude::ClaudeSession;
use tracr_testkit::write_lines;

const A: &str = "aaaaaaaa-1111-4111-8111-111111111111";
const B: &str = "bbbbbbbb-2222-4222-8222-222222222222";
const DAY: &str = "2026-09-28";

fn at(hhmm: &str) -> UtcMillis {
    UtcMillis::parse_rfc3339(&tracr_testkit::collector::at(hhmm)).unwrap()
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

fn q() -> SessionQuery {
    SessionQuery {
        from: DAY.into(),
        to: DAY.into(),
        ..SessionQuery::default()
    }
}

#[test]
fn sessions_are_found_by_metadata_and_facets_offer_the_values() {
    let dir = TempDir::new().unwrap();
    let base = std::fs::canonicalize(dir.path()).unwrap();
    let (data, root, code) = (base.join("data"), base.join("projects"), base.join("code"));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::create_dir_all(&code).unwrap();
    let clock = Arc::new(ManualClock::new(at("08:55")));
    let mut app = App::open_writer(&data, Box::new(clock.clone())).unwrap();
    app.add_root(RootKind::Provider, &root).unwrap();
    app.add_root(RootKind::Repository, &code).unwrap();
    let shop = app
        .project_op(&ProjectOp::Create {
            name: "Shop".into(),
        })
        .unwrap();
    let mut c = Collector::begin(app, "test", true).unwrap();

    // A: repository "shop", branch tracr-12-checkout, calls Bash, opus.
    let mut a = ClaudeSession::new(A);
    a.cwd = repo(&code, "shop");
    a.branch = "tracr-12-checkout".into();
    write_lines(
        &root.join("-code-shop").join(format!("{A}.jsonl")),
        &[a.prompt("09:00"), a.assistant("09:01", "m1", 100)],
    );
    // B: repository "notes", branch main, a prompt only (no model, no tools).
    let mut b = ClaudeSession::new(B);
    b.cwd = repo(&code, "notes");
    write_lines(
        &root.join("-code-notes").join(format!("{B}.jsonl")),
        &[b.prompt("10:00")],
    );
    clock.set(at("11:00"));
    c.reconcile_all(IngestTrigger::Startup).unwrap();
    c.probe_git().unwrap();
    c.rebuild().unwrap();

    let reader = || Reader::open(&data).unwrap();
    let found = |query: SessionQuery| -> Vec<String> {
        let mut ids: Vec<String> = reader()
            .search_sessions(&query)
            .unwrap()
            .list
            .sessions
            .into_iter()
            .map(|s| s.native_session_id)
            .collect();
        ids.sort();
        ids
    };

    let all = reader().search_sessions(&q()).unwrap();
    assert_eq!(all.total, 2);
    assert_eq!(all.list.sessions.len(), 2);
    assert_eq!(all.facets.providers, ["claude_code"]);
    assert_eq!(all.facets.branches, ["main", "tracr-12-checkout"]);
    assert_eq!(all.facets.models, ["claude-opus-5-5"]);
    assert_eq!(all.facets.tools, ["Bash"]);

    assert_eq!(
        found(SessionQuery {
            branch: Some("main".into()),
            ..q()
        }),
        [B]
    );
    assert_eq!(
        found(SessionQuery {
            text: Some("SHOP".into()),
            ..q()
        }),
        [A]
    );
    assert_eq!(
        found(SessionQuery {
            tool: Some("Bash".into()),
            ..q()
        }),
        [A]
    );
    assert_eq!(
        found(SessionQuery {
            model: Some("claude-opus-5-5".into()),
            ..q()
        }),
        [A]
    );
    assert_eq!(
        found(SessionQuery {
            subagents: Some(true),
            ..q()
        }),
        Vec::<String>::new()
    );
    assert!(
        found(SessionQuery {
            provider: Some("codex".into()),
            ..q()
        })
        .is_empty()
    );
    // Cost: B has no usage, so it never matches a minimum (unpriced is not 0).
    assert_eq!(
        found(SessionQuery {
            min_cost_nano: Some(0),
            ..q()
        }),
        [A]
    );
    // Another period finds nothing.
    let other = SessionQuery {
        from: "2026-09-29".into(),
        to: "2026-09-29".into(),
        ..q()
    };
    assert!(found(other).is_empty());

    // Project: by repository mapping, then by binding.
    let shop_repo = all
        .list
        .sessions
        .iter()
        .find(|s| s.native_session_id == A)
        .unwrap()
        .repository
        .clone()
        .unwrap();
    let mut app = {
        c.end("switch").unwrap();
        App::open_writer(&data, Box::new(clock.clone())).unwrap()
    };
    app.project_op(&ProjectOp::Map {
        repository: shop_repo,
        project: Some(shop.clone()),
    })
    .unwrap();
    assert_eq!(
        found(SessionQuery {
            project: Some(shop.clone()),
            ..q()
        }),
        [A]
    );
    app.binding_op(&BindingOp::Bind {
        subject: Subject::Session(B.into()),
        target: format!("project:{shop}"),
        valid_from: Some(at("09:00").0),
    })
    .unwrap();
    assert_eq!(
        found(SessionQuery {
            project: Some(shop.clone()),
            ..q()
        }),
        [A, B]
    );
    let facets = reader().search_sessions(&q()).unwrap().facets;
    assert_eq!(facets.projects.len(), 1);
    assert_eq!(facets.projects[0].name, "Shop");
    assert_eq!(facets.bound_to, [format!("project:{shop}")]);
    assert_eq!(
        found(SessionQuery {
            bound_to: Some(format!("project:{shop}")),
            ..q()
        }),
        [B]
    );
}
