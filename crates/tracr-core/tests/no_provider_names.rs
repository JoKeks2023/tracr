//! INV-8 / §1.2 enforcement: the core names no provider or transport and
//! depends on no workspace crate.

use std::fs;
use std::path::{Path, PathBuf};

const FORBIDDEN: &[&str] = &["claude", "codex", "mcp"];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read src dir") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn core_source_names_no_provider_or_transport() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rust_files(&src, &mut files);
    assert!(!files.is_empty());
    let mut violations = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file).expect("read source");
        for (n, line) in text.lines().enumerate() {
            if line.trim_start().starts_with("//") {
                continue;
            }
            let lower = line.to_lowercase();
            for word in FORBIDDEN {
                if lower.contains(word) {
                    violations.push(format!("{}:{}: {word}", file.display(), n + 1));
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "provider/transport names in tracr-core: {violations:#?}"
    );
}

#[test]
fn core_has_no_workspace_dependencies() {
    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .expect("read Cargo.toml");
    let mut in_deps = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_deps = trimmed.contains("dependencies");
            continue;
        }
        if in_deps {
            assert!(
                !trimmed.contains("path") && !trimmed.starts_with("tracr-"),
                "tracr-core must not depend on workspace crates: {trimmed}"
            );
        }
    }
}
