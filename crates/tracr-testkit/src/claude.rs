//! Schema-faithful Claude Code transcript lines (shape of producer 2.1.281).

use serde_json::json;

use crate::collector::at;

pub struct ClaudeSession {
    pub session_id: String,
    pub cwd: String,
    pub branch: String,
    pub version: String,
    counter: u64,
}

impl ClaudeSession {
    /// `session_id` should be a UUID, as Claude Code names its transcripts.
    pub fn new(session_id: &str) -> Self {
        Self {
            session_id: session_id.to_owned(),
            cwd: "/Users/dev/project".into(),
            branch: "main".into(),
            version: "2.1.281".into(),
            counter: 0,
        }
    }

    fn uuid(&mut self) -> String {
        self.counter += 1;
        format!(
            "{}-0000-4000-8000-{:012x}",
            &self.session_id[..8],
            self.counter
        )
    }

    fn base(
        &mut self,
        record_type: &str,
        hhmm: &str,
    ) -> serde_json::Map<String, serde_json::Value> {
        let v = json!({
            "type": record_type,
            "uuid": self.uuid(),
            "sessionId": self.session_id,
            "timestamp": at(hhmm),
            "version": self.version,
            "entrypoint": "cli",
            "userType": "external",
            "isSidechain": false,
            "cwd": self.cwd,
            "gitBranch": self.branch,
        });
        v.as_object().unwrap().clone()
    }

    /// A prompt the human typed.
    pub fn prompt(&mut self, hhmm: &str) -> String {
        let mut o = self.base("user", hhmm);
        o.insert("promptSource".into(), json!("typed"));
        o.insert("turnOrigin".into(), json!("human"));
        o.insert("origin".into(), json!({"kind": "human"}));
        o.insert(
            "message".into(),
            json!({"role": "user", "content": format!("prompt text at {hhmm}")}),
        );
        serde_json::Value::Object(o).to_string()
    }

    /// An automated task notification (not a human prompt).
    pub fn task_notification(&mut self, hhmm: &str) -> String {
        let mut o = self.base("user", hhmm);
        o.insert("promptSource".into(), json!("system"));
        o.insert("turnOrigin".into(), json!("task_notification"));
        o.insert("origin".into(), json!({"kind": "task-notification"}));
        o.insert(
            "message".into(),
            json!({"role": "user", "content": "<task-notification>done</task-notification>"}),
        );
        serde_json::Value::Object(o).to_string()
    }

    /// An assistant record carrying usage for `message_id` (streaming
    /// re-emissions reuse the same message and request IDs).
    pub fn assistant(&mut self, hhmm: &str, message_id: &str, output_tokens: u64) -> String {
        let mut o = self.base("assistant", hhmm);
        o.insert("requestId".into(), json!(format!("req_{message_id}")));
        o.insert(
            "message".into(),
            json!({
                "id": message_id,
                "role": "assistant",
                "model": "claude-opus-5-5",
                "stop_reason": "tool_use",
                "content": [{"type": "tool_use", "id": format!("toolu_{message_id}"), "name": "Bash", "input": {"command": "ls"}}],
                "usage": {
                    "input_tokens": 10,
                    "cache_creation_input_tokens": 100,
                    "cache_read_input_tokens": 1000,
                    "output_tokens": output_tokens,
                    "service_tier": "standard"
                }
            }),
        );
        serde_json::Value::Object(o).to_string()
    }

    pub fn tool_result(&mut self, hhmm: &str, tool_use_id: &str) -> String {
        let mut o = self.base("user", hhmm);
        o.insert("toolUseResult".into(), json!({"stdout": "output"}));
        o.insert(
            "message".into(),
            json!({"role": "user", "content": [{"type": "tool_result", "tool_use_id": tool_use_id, "content": "output"}]}),
        );
        serde_json::Value::Object(o).to_string()
    }

    /// Claude Code's cumulative cost figure (no uuid, no timestamp).
    pub fn cost_state(&self, total_usd: &str) -> String {
        format!(
            r#"{{"type":"cost-state","sessionId":"{}","totalCostUSD":{total_usd},"hasUnknownModelCost":false}}"#,
            self.session_id
        )
    }

    /// A UI-state record the manifest ignores.
    pub fn mode(&self) -> String {
        format!(
            r#"{{"type":"mode","mode":"normal","sessionId":"{}"}}"#,
            self.session_id
        )
    }
}
