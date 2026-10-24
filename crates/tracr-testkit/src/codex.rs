//! Schema-faithful Codex rollout lines (shape of Codex Desktop 0.145–0.158).
//!
//! The shapes reproduce what a structure-only profile of local rollouts
//! found (record kinds, key paths and enum values); every text is a
//! placeholder. `SECRET` appears in each content field so tests can prove
//! that no content is retained.

use serde_json::{Value, json};

use crate::collector::at;

/// Placeholder text in every content-bearing field.
pub const SECRET: &str = "FIXTURE-SECRET-TEXT do not retain";

#[derive(Debug, Clone)]
pub enum Origin {
    /// A thread the human works in (`source: "vscode"`).
    Interactive,
    /// An automatic reviewer thread (`source.subagent.other: "guardian"`).
    Guardian { parent: String },
    /// A spawned subagent (`source.subagent.thread_spawn`).
    Spawned { parent: String },
}

pub struct CodexThread {
    pub thread_id: String,
    pub cwd: String,
    pub cli_version: String,
    pub model: String,
    /// `service_tier` reported by `thread_settings_applied` (`default` in real rollouts); `None` omits it.
    pub service_tier: Option<String>,
    pub origin: Origin,
    ordinal: u64,
}

impl CodexThread {
    pub fn new(thread_id: &str, origin: Origin) -> Self {
        Self {
            thread_id: thread_id.to_owned(),
            cwd: "/Users/dev/project".into(),
            cli_version: "0.153.4".into(),
            model: "gpt-5.6-luna".into(),
            service_tier: None,
            origin,
            ordinal: 0,
        }
    }

    /// The rollout file name, e.g. for `sessions/2026/09/28/<name>`.
    pub fn file_name(&self) -> String {
        format!("rollout-2026-09-28T09-00-00-{}.jsonl", self.thread_id)
    }

    fn line(&mut self, hhmm: &str, record_type: &str, payload: Value) -> String {
        let v = json!({
            "timestamp": at(hhmm),
            "type": record_type,
            "payload": payload,
            "ordinal": self.ordinal,
        });
        self.ordinal += 1;
        v.to_string()
    }

    fn passthrough(turn: &str) -> Value {
        json!({"turn_id": turn, "create_time": 1_790_000_000.5})
    }

    pub fn meta(&mut self, hhmm: &str) -> String {
        let mut p = json!({
            "id": self.thread_id,
            "session_id": self.thread_id,
            "timestamp": at(hhmm),
            "cwd": self.cwd,
            "originator": "Codex Desktop",
            "cli_version": self.cli_version,
            "model_provider": "openai",
            "history_mode": "paginated",
            "context_window": {"window_id": "w1"},
            "base_instructions": {"text": SECRET, "provenance": {"type": "model", "model": self.model}},
            "git": {"branch": "main", "commit_hash": "abc123", "repository_url": "https://github.com/dev/project.git"},
        });
        let o = p.as_object_mut().unwrap();
        match &self.origin {
            Origin::Interactive => {
                o.insert("source".into(), json!("vscode"));
                o.insert("thread_source".into(), json!("user"));
            }
            Origin::Guardian { parent } => {
                o.insert("source".into(), json!({"subagent": {"other": "guardian"}}));
                o.insert("thread_source".into(), json!("guardian_review"));
                o.insert("parent_thread_id".into(), json!(parent));
                o.insert("session_id".into(), json!(parent));
                o.insert("subagent_history_start_ordinal".into(), json!(40));
            }
            Origin::Spawned { parent } => {
                o.insert(
                    "source".into(),
                    json!({"subagent": {"thread_spawn": {"parent_thread_id": parent, "depth": 1, "agent_nickname": "Meitner", "agent_path": "/x", "agent_role": null}}}),
                );
                o.insert("thread_source".into(), json!("subagent"));
                o.insert("parent_thread_id".into(), json!(parent));
                o.insert("forked_from_id".into(), json!(parent));
                o.insert("session_id".into(), json!(parent));
            }
        }
        self.line(hhmm, "session_meta", p)
    }

    /// `task_started`, then the turn's `turn_context`.
    pub fn turn(&mut self, hhmm: &str, turn: &str) -> Vec<String> {
        let started = self.line(
            hhmm,
            "event_msg",
            json!({"type": "task_started", "turn_id": turn, "started_at": 1_790_000_000, "model_context_window": 258_400, "collaboration_mode_kind": "default"}),
        );
        let model = self.model.clone();
        let cwd = self.cwd.clone();
        let ctx = self.line(
            hhmm,
            "turn_context",
            json!({"turn_id": turn, "cwd": cwd, "model": model, "effort": "medium", "summary": "auto", "approval_policy": "on-request", "user_instructions": SECRET, "developer_instructions": SECRET}),
        );
        let mut lines = vec![started, ctx];
        if let Some(tier) = self.service_tier.clone() {
            let model = self.model.clone();
            lines.push(self.line(
                hhmm,
                "event_msg",
                json!({"type": "thread_settings_applied", "thread_settings": {"model": model, "service_tier": tier}}),
            ));
        }
        lines
    }

    pub fn user_message(&mut self, hhmm: &str, turn: &str, id: &str) -> String {
        let thread = self.thread_id.clone();
        self.line(
            hhmm,
            "event_msg",
            json!({"type": "item_completed", "thread_id": thread, "turn_id": turn, "completed_at_ms": 1_790_000_000_000_i64,
                   "item": {"type": "UserMessage", "id": id, "content": [{"type": "text", "text": SECRET}]}}),
        )
    }

    pub fn agent_message(&mut self, hhmm: &str, turn: &str, id: &str) -> String {
        let thread = self.thread_id.clone();
        self.line(
            hhmm,
            "event_msg",
            json!({"type": "item_completed", "thread_id": thread, "turn_id": turn, "completed_at_ms": 1_790_000_000_000_i64,
                   "item": {"type": "AgentMessage", "id": id, "phase": "final_answer", "content": [{"type": "Text", "text": SECRET}]}}),
        )
    }

    /// A command execution item: activity detail tracr does not read.
    pub fn command_item(&mut self, hhmm: &str, turn: &str) -> String {
        let thread = self.thread_id.clone();
        self.line(
            hhmm,
            "event_msg",
            json!({"type": "item_completed", "thread_id": thread, "turn_id": turn, "completed_at_ms": 1_790_000_000_000_i64,
                   "item": {"type": "CommandExecution", "id": "cmd1", "command": [SECRET], "aggregated_output": SECRET, "exit_code": 0, "status": "completed"}}),
        )
    }

    /// Content records tracr never reads: user and assistant messages, reasoning.
    pub fn content(&mut self, hhmm: &str, turn: &str) -> Vec<String> {
        vec![
            self.line(
                hhmm,
                "response_item",
                json!({"type": "message", "id": "msg_u", "role": "user", "content": [{"type": "input_text", "text": SECRET}], "internal_chat_message_metadata_passthrough": Self::passthrough(turn)}),
            ),
            self.line(
                hhmm,
                "response_item",
                json!({"type": "reasoning", "id": "rs_1", "summary": [{"type": "summary_text", "text": SECRET}], "encrypted_content": SECRET, "internal_chat_message_metadata_passthrough": Self::passthrough(turn)}),
            ),
        ]
    }

    pub fn tool_call(&mut self, hhmm: &str, turn: &str, call: &str) -> String {
        self.line(
            hhmm,
            "response_item",
            json!({"type": "custom_tool_call", "id": format!("ctc_{call}"), "call_id": call, "name": "exec", "input": SECRET, "status": "completed", "internal_chat_message_metadata_passthrough": Self::passthrough(turn)}),
        )
    }

    pub fn tool_output(&mut self, hhmm: &str, turn: &str, call: &str) -> String {
        self.line(
            hhmm,
            "response_item",
            json!({"type": "custom_tool_call_output", "id": format!("ctco_{call}"), "call_id": call, "output": [{"type": "input_text", "text": SECRET}], "internal_chat_message_metadata_passthrough": Self::passthrough(turn)}),
        )
    }

    fn usage(input: u64, cached: u64, output: u64) -> Value {
        json!({"input_tokens": input, "cached_input_tokens": cached, "cache_write_input_tokens": 0,
               "output_tokens": output, "reasoning_output_tokens": output / 2, "total_tokens": input + output})
    }

    /// Per-response usage (current producers).
    pub fn usage_record(
        &mut self,
        hhmm: &str,
        turn: &str,
        response: &str,
        input: u64,
        cached: u64,
        output: u64,
    ) -> String {
        let thread = self.thread_id.clone();
        let u = Self::usage(input, cached, output);
        self.line(
            hhmm,
            "token_usage_record",
            json!({"response_id": response, "turn_id": turn, "root_turn_id": turn, "thread_id": thread, "session_id": thread,
                   "usage": u, "turn_token_usage": u, "thread_token_usage": u}),
        )
    }

    /// Cumulative thread totals (all producers; the only usage in older ones).
    pub fn token_count(&mut self, hhmm: &str, input: u64, cached: u64, output: u64) -> String {
        self.line(
            hhmm,
            "event_msg",
            json!({"type": "token_count", "info": {"total_token_usage": Self::usage(input, cached, output), "last_token_usage": Self::usage(0, 0, 0), "model_context_window": 258_400},
                   "rate_limits": {"limit_id": "codex", "plan_type": "prolite"}}),
        )
    }

    pub fn task_complete(&mut self, hhmm: &str, turn: &str, failed: bool) -> String {
        let mut p = json!({"type": "task_complete", "turn_id": turn, "started_at": 1_790_000_000, "completed_at": 1_790_000_060, "duration_ms": 60_000, "last_agent_message": SECRET});
        if failed {
            p["error"] = json!({"message": SECRET, "codex_error_info": "server_error"});
        }
        self.line(hhmm, "event_msg", p)
    }

    pub fn turn_aborted(&mut self, hhmm: &str, turn: &str) -> String {
        self.line(
            hhmm,
            "event_msg",
            json!({"type": "turn_aborted", "turn_id": turn, "reason": "interrupted", "started_at": 1_790_000_000, "completed_at": 1_790_000_030, "duration_ms": 30_000}),
        )
    }

    pub fn world_state(&mut self, hhmm: &str) -> String {
        self.line(
            hhmm,
            "world_state",
            json!({"state": {"model": "gpt-5.6-luna"}}),
        )
    }

    /// A whole human turn: prompt, a tool call, an answer, usage, completion.
    pub fn human_turn(
        &mut self,
        start: &str,
        end: &str,
        turn: &str,
        response: &str,
        input: u64,
        output: u64,
    ) -> Vec<String> {
        let mut v = self.turn(start, turn);
        v.push(self.user_message(start, turn, &format!("um_{turn}")));
        v.extend(self.content(start, turn));
        v.push(self.tool_call(start, turn, &format!("call_{turn}")));
        v.push(self.tool_output(end, turn, &format!("call_{turn}")));
        v.push(self.agent_message(end, turn, &format!("am_{turn}")));
        v.push(self.usage_record(end, turn, response, input, input / 2, output));
        v.push(self.task_complete(end, turn, false));
        v
    }
}
