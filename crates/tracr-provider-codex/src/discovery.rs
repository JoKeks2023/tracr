//! Which files under an approved Codex root are rollout streams (§4.2).
//!
//! Claimed: `rollout-<YYYY-MM-DDTHH-MM-SS>-<thread uuid>.jsonl` (or the same
//! name with `.zst`, zstd-compressed by Codex), either below
//! `sessions/YYYY/MM/DD/` or directly in `archived_sessions/`. The thread ID
//! in the file name is the stream identity (checked against `session_meta.id`
//! on every local rollout), so archiving a thread keeps its source
//! (walkthrough F5). Nothing else is claimed: `session_index.jsonl` and
//! `transcription-history.jsonl` hold content and are never read.

use tracr_core::adapter::{StreamRef, StreamRole};
use tracr_core::evidence::StreamKey;
use tracr_core::ids::MachineId;

fn is_uuid(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-',
            _ => c.is_ascii_hexdigit(),
        })
}

/// The thread ID of a rollout file name.
pub fn thread_of_file(name: &str) -> Option<&str> {
    let stem = name
        .strip_prefix("rollout-")?
        .strip_suffix(".jsonl.zst")
        .or_else(|| name.strip_suffix(".jsonl")?.strip_prefix("rollout-"))?;
    // `YYYY-MM-DDTHH-MM-SS-` is 20 bytes, then the 36-byte thread UUID.
    let (stamp, thread) = (stem.get(..20)?, stem.get(20..)?);
    let stamp_ok = stamp.bytes().enumerate().all(|(i, c)| match i {
        4 | 7 | 13 | 16 | 19 => c == b'-',
        10 => c == b'T',
        _ => c.is_ascii_digit(),
    });
    (stamp_ok && is_uuid(thread)).then_some(thread)
}

pub fn stream_key(machine: &MachineId, thread: &str) -> StreamKey {
    StreamKey::new(format!("codex:{machine}:{thread}"))
}

fn is_date_dirs(y: &str, m: &str, d: &str) -> bool {
    let digits = |s: &str, n: usize| s.len() == n && s.bytes().all(|c| c.is_ascii_digit());
    digits(y, 4) && digits(m, 2) && digits(d, 2)
}

pub fn classify(machine: &MachineId, rel: &[&str]) -> Option<StreamRef> {
    let file = match rel {
        [file] => file,
        [y, m, d, file] if is_date_dirs(y, m, d) => file,
        _ => return None,
    };
    let thread = thread_of_file(file)?;
    // A subagent thread is recognized from its `session_meta`, not its path:
    // linking it to its parent is the normalizer's (gated) decision.
    Some(StreamRef {
        stream_key: stream_key(machine, thread),
        role: StreamRole::Main,
        native_session: Some(thread.to_owned()),
        native_agent: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: &str = "019f0000-aaaa-7000-8000-00000000000a";

    #[test]
    fn a_compressed_rollout_is_the_same_stream_as_its_plain_name() {
        let m = MachineId::new("m");
        let plain = format!("rollout-2026-08-11T22-18-58-{T}.jsonl");
        let packed = format!("{plain}.zst");
        assert_eq!(thread_of_file(&packed), Some(T));
        assert_eq!(
            classify(&m, &[&packed]).unwrap().stream_key,
            classify(&m, &[&plain]).unwrap().stream_key
        );
        assert!(thread_of_file(&format!("{plain}.zst.bak")).is_none());
        assert!(thread_of_file(&format!("rollout-2026-08-11T22-18-58-{T}.zst")).is_none());
    }

    #[test]
    fn claims_rollouts_live_and_archived_as_one_stream() {
        let m = MachineId::new("m");
        let name = format!("rollout-2026-08-11T22-18-58-{T}.jsonl");
        let live = classify(&m, &["2026", "08", "11", &name]).unwrap();
        let archived = classify(&m, &[&name]).unwrap();
        assert_eq!(live.stream_key, archived.stream_key);
        assert_eq!(live.native_session.as_deref(), Some(T));
    }

    #[test]
    fn claims_nothing_else() {
        let m = MachineId::new("m");
        for rel in [
            vec!["session_index.jsonl"],
            vec!["transcription-history.jsonl"],
            vec!["2026", "08", "11", "notes.jsonl"],
            vec![
                "x",
                "y",
                "rollout-2026-08-11T22-18-58-019f0000-aaaa-7000-8000-00000000000a.jsonl",
            ],
            vec!["rollout-2026-08-11T22-18-58-not-a-uuid.jsonl"],
        ] {
            assert!(classify(&m, &rel).is_none(), "{rel:?}");
        }
    }
}
