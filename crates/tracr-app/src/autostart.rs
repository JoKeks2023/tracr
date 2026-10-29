//! Launch at login (macOS): an opt-in user LaunchAgent that opens the app.
//!
//! The device sensor, which says when you were at the computer, only runs
//! while the app runs. Without a login item, every stretch with the app closed
//! stays unknown. The agent is one small plist in the user's own
//! `~/Library/LaunchAgents`; turning it off deletes that file. It opens the app
//! in the background (no window), and opening an app that already runs does
//! nothing.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};

/// The agent's label and file name stem.
pub const LABEL: &str = "app.tracr.desktop.autostart";

/// Where the agent file lives for this home directory.
pub fn agent_path(home: &Path) -> PathBuf {
    home.join("Library")
        .join("LaunchAgents")
        .join(format!("{LABEL}.plist"))
}

/// The `.app` bundle that contains `exe`, or `None` when it does not run from a bundle
/// (a development build): there is nothing stable to open at login.
pub fn bundle_of(exe: &Path) -> Option<PathBuf> {
    exe.ancestors()
        .find(|p| p.extension().is_some_and(|e| e == "app"))
        .map(Path::to_path_buf)
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn plist(bundle: &Path) -> String {
    format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" ",
            "\"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n",
            "<plist version=\"1.0\">\n<dict>\n",
            "  <key>Label</key><string>{label}</string>\n",
            "  <key>ProgramArguments</key>\n  <array>\n",
            "    <string>/usr/bin/open</string>\n",
            "    <string>-g</string>\n",
            "    <string>-a</string>\n",
            "    <string>{bundle}</string>\n",
            "    <string>--args</string>\n",
            "    <string>--background</string>\n",
            "  </array>\n",
            "  <key>RunAtLoad</key><true/>\n",
            "</dict>\n</plist>\n"
        ),
        label = LABEL,
        bundle = escape(&bundle.display().to_string()),
    )
}

/// Whether the agent exists and opens this very bundle.
pub fn enabled(home: &Path, bundle: &Path) -> bool {
    fs::read_to_string(agent_path(home))
        .is_ok_and(|text| text.contains(&escape(&bundle.display().to_string())))
}

/// Turns launch at login on or off. Off removes the agent file; on writes it.
pub fn set(home: &Path, bundle: &Path, on: bool) -> Result<()> {
    let path = agent_path(home);
    if !on {
        return match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(anyhow!(e).context(format!("remove {}", path.display()))),
        };
    }
    if !bundle.is_dir() {
        return Err(anyhow!("{} is not an app bundle", bundle.display()));
    }
    let dir = path.parent().context("agent directory")?;
    fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    fs::write(&path, plist(bundle)).with_context(|| format!("write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_writes_the_agent_for_this_bundle_and_off_removes_it() {
        let home = tempfile::TempDir::new().unwrap();
        let bundle = home.path().join("Apps & Co").join("tracr.app");
        fs::create_dir_all(&bundle).unwrap();
        assert!(!enabled(home.path(), &bundle));

        set(home.path(), &bundle, true).unwrap();
        assert!(enabled(home.path(), &bundle));
        let text = fs::read_to_string(agent_path(home.path())).unwrap();
        assert!(text.contains("Apps &amp; Co"), "paths are escaped");
        assert!(text.contains("--background") && text.contains("RunAtLoad"));
        assert!(
            !enabled(home.path(), &home.path().join("other.app")),
            "another bundle is not this agent"
        );

        set(home.path(), &bundle, false).unwrap();
        assert!(!enabled(home.path(), &bundle));
        set(home.path(), &bundle, false).unwrap();
    }

    #[test]
    fn only_an_app_bundle_can_be_opened_at_login() {
        assert_eq!(
            bundle_of(Path::new(
                "/Applications/tracr.app/Contents/MacOS/tracr-desktop"
            )),
            Some(PathBuf::from("/Applications/tracr.app"))
        );
        assert_eq!(bundle_of(Path::new("/x/target/debug/tracr-desktop")), None);
        let home = tempfile::TempDir::new().unwrap();
        assert!(set(home.path(), &home.path().join("missing.app"), true).is_err());
    }
}
