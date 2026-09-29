//! Which files under an approved Claude root are transcript streams (§4.2).
//!
//! Relative to `~/.claude/projects`:
//!
//! * `<project dir>/<session uuid>.jsonl`, the session's own transcript;
//! * `<project dir>/<session uuid>/subagents/**/agent-<id>.jsonl`, subagent
//!   transcripts, including workflow agents under `subagents/workflows/wf_*/`.
//!
//! Everything else is not claimed, in particular the workflow
//! `journal.jsonl` files next to workflow agents (tool-internal, not a
//! transcript). Stream identity comes from the path; the path itself is only
//! a locator.

use tracr_core::adapter::{StreamRef, StreamRole};
use tracr_core::evidence::{StreamKey, native_id};
use tracr_core::ids::MachineId;

fn is_session_uuid(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-',
            _ => c.is_ascii_hexdigit(),
        })
}

pub fn main_stream_key(machine: &MachineId, session: &str) -> StreamKey {
    StreamKey::new(format!("claude:{machine}:{session}"))
}

pub fn subagent_stream_key(machine: &MachineId, session: &str, agent: &str) -> StreamKey {
    StreamKey::new(format!("claude:{machine}:{session}:agent-{agent}"))
}

pub fn classify(machine: &MachineId, rel: &[&str]) -> Option<StreamRef> {
    match rel {
        [_project, file] => {
            let session = file.strip_suffix(".jsonl")?;
            is_session_uuid(session).then(|| StreamRef {
                stream_key: main_stream_key(machine, session),
                role: StreamRole::Main,
                native_session: Some(session.to_owned()),
                native_agent: None,
            })
        }
        [_project, session, "subagents", .., file] => {
            if !is_session_uuid(session) {
                return None;
            }
            let agent = file.strip_prefix("agent-")?.strip_suffix(".jsonl")?;
            let agent = native_id(agent)?;
            Some(StreamRef {
                stream_key: subagent_stream_key(machine, session, agent),
                role: StreamRole::Subagent,
                native_session: Some((*session).to_owned()),
                native_agent: Some(agent.to_owned()),
            })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: &str = "320666a0-e817-428d-865a-43bbeaac7053";

    fn m() -> MachineId {
        MachineId::new("m")
    }

    #[test]
    fn main_transcript() {
        let s = classify(&m(), &["-Users-x-proj", &format!("{S}.jsonl")]).unwrap();
        assert_eq!(s.role, StreamRole::Main);
        assert_eq!(s.stream_key.as_str(), format!("claude:m:{S}"));
        assert_eq!(s.native_session.as_deref(), Some(S));
    }

    #[test]
    fn subagent_and_workflow_agent_transcripts() {
        let a = classify(
            &m(),
            &["p", S, "subagents", "agent-a5d67caeb325d41bb.jsonl"],
        )
        .unwrap();
        assert_eq!(a.role, StreamRole::Subagent);
        assert_eq!(a.native_agent.as_deref(), Some("a5d67caeb325d41bb"));
        assert_eq!(
            a.stream_key.as_str(),
            format!("claude:m:{S}:agent-a5d67caeb325d41bb")
        );

        let w = classify(
            &m(),
            &[
                "p",
                S,
                "subagents",
                "workflows",
                "wf_2928cd94-239",
                "agent-abc123.jsonl",
            ],
        )
        .unwrap();
        assert_eq!(w.role, StreamRole::Subagent);
        assert_eq!(w.native_agent.as_deref(), Some("abc123"));
    }

    #[test]
    fn journals_and_other_files_are_not_claimed() {
        assert!(
            classify(
                &m(),
                &["p", S, "subagents", "workflows", "wf_1", "journal.jsonl"]
            )
            .is_none()
        );
        assert!(classify(&m(), &["p", "notes.jsonl"]).is_none());
        assert!(classify(&m(), &["p", S, "tool-results", "x.jsonl"]).is_none());
        assert!(classify(&m(), &[&format!("{S}.jsonl")]).is_none());
        assert!(classify(&m(), &["p", S, "subagents", "agent-has space.jsonl"]).is_none());
    }
}
