#!/usr/bin/env python3
"""Reduce real Claude Code transcripts to structure-only fixtures (architecture §12.1).

Usage: sanitize.py <claude projects root> <fixtures dir>

For each producer version in VERSIONS, writes into <fixtures dir>/<version>/:
  main.jsonl      contiguous prefix of one real session transcript
  records.jsonl   up to 2 samples per structural signature across all sessions of that version
  subagent.jsonl  prefix of one subagent (or workflow agent) transcript
  expected.json   outcome counts computed by the independent reference rule below

Sanitizing keeps structure and non-content metadata, and replaces everything else:
  * IDs are remapped consistently to fake IDs (prefixes such as msg_/toolu_/req_ kept);
  * paths and branch names become fixture values;
  * enum-like fields keep identifier values; tool names keep builtin names;
  * every other string becomes a unique identifier-shaped sentinel "cx<n>zz". It is
    identifier-shaped on purpose: it passes the extractor's identifier validators, so a
    leak through a marker, link or tool-name channel is detected too. A leading envelope
    tag from the adapter's allowlist is preserved around the sentinel;
  * non-identifier dict keys become "kx<n>zz".
tests/extract.rs asserts that no retained fact contains a sentinel.

The reference classifier below re-implements the adapter's documented rule in Python.
It checks that the Rust code implements that rule; it is not an independent oracle of
what is human (the rule itself was validated against a session with known prompts).
"""

import collections
import glob
import hashlib
import json
import os
import re
import sys

VERSIONS = ["2.1.231", "2.1.236", "2.1.267", "2.1.281"]
IDENT = re.compile(r"^[A-Za-z0-9_.:\-]{1,64}$")
UUID = re.compile(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$")
ENUM_KEYS = {
    "type", "subtype", "entrypoint", "userType", "promptSource", "turnOrigin", "sessionKind",
    "permissionMode", "level", "operation", "reason", "role", "stop_reason", "service_tier",
    "model", "version", "queuePriority", "speed", "inference_geo", "kind", "media_type",
}
ID_KEYS = {
    "uuid", "parentUuid", "logicalParentUuid", "sessionId", "session_id", "agentId", "requestId",
    "id", "tool_use_id", "promptId", "sourceToolUseID", "sourceToolAssistantUUID",
    "continuedInSessionId", "interruptedMessageId", "leafUuid", "messageId", "snapshotMessageId",
    "toolUseID", "taskId", "senderTaskId", "from", "originalHeadCommit", "bridgeSessionId",
}
PATH_KEYS = {"cwd", "relocatedCwd", "originalCwd", "preEnterOriginalCwd", "worktreePath", "trackingPath"}
BRANCH_KEYS = {"gitBranch", "originalBranch", "worktreeBranch"}
CONTENT_TAGS = [
    "command-name", "command-message", "command-args", "local-command-caveat", "local-command-stdout",
    "local-command-stderr", "bash-input", "bash-stdout", "bash-stderr", "task-notification", "system-reminder",
]
HUMAN_TAGS = {"command-name", "command-message", "bash-input"}
HUMAN_PROMPT_SOURCES = {"typed", "queued", "suggestion_accepted"}
INTERACTIVE = {"cli"}
IGNORED = {
    "attachment", "last-prompt", "ai-title", "summary", "mode", "permission-mode", "atis-latch",
    "agent-name", "agent-setting", "file-history-snapshot", "file-history-delta", "bridge-session",
}
KNOWN = {"user", "assistant", "system", "cost-state", "pr-link", "continued-in", "relocated", "worktree-state", "queue-operation"}


class Sanitizer:
    def __init__(self):
        self.ids = {}
        self.n = 0

    def sentinel(self):
        self.n += 1
        return f"cx{self.n}zz"

    def fake_id(self, value):
        if value not in self.ids:
            h = hashlib.sha256(("fixture:" + value).encode()).hexdigest()
            if UUID.match(value):
                fake = f"{h[:8]}-{h[8:12]}-4{h[13:16]}-8{h[17:20]}-{h[20:32]}"
            elif re.match(r"^[0-9a-f]{40}$", value):
                fake = h[:40]
            else:
                m = re.match(r"^([A-Za-z]+_)", value)
                prefix = m.group(1) if m else ""
                fake = prefix + "fx" + h[: max(8, min(24, len(value) - len(prefix)))]
            self.ids[value] = fake
        return self.ids[value]

    def text(self, s):
        m = re.match(r"^\s*<([a-z][a-z0-9_-]{0,40})>", s)
        if m and m.group(1) in CONTENT_TAGS:
            tag = m.group(1)
            return f"<{tag}>{self.sentinel()}</{tag}>"
        return self.sentinel()

    def value(self, key, v, parent_key=None):
        if isinstance(v, dict):
            out = {}
            for k, x in v.items():
                nk = k if IDENT.match(k) and parent_key not in ("snapshot", "trackedFileBackups") else "k" + self.sentinel()
                out[nk] = self.value(k, x, key)
            return out
        if isinstance(v, list):
            return [self.value(key, x, parent_key) for x in v]
        if not isinstance(v, str):
            return v
        if key in ID_KEYS:
            return self.fake_id(v)
        if key in PATH_KEYS:
            return "/Users/fixture/project" if key != "worktreePath" else "/Users/fixture/project-worktree"
        if key in BRANCH_KEYS:
            return "fixture-branch"
        if key == "prRepository":
            return "example-org/example-repo"
        if key == "name" and parent_key in ("content", None):
            return v if IDENT.match(v) and not v.startswith("mcp__") else "mcp__example__tool"
        if key == "timestamp":
            return v
        if key in ("content", "text"):
            return self.text(v)
        if key in ENUM_KEYS and IDENT.match(v):
            return v
        if key == "model" and re.match(r"^[A-Za-z0-9<>_.:/@\-\[\]]{1,128}$", v):
            return v
        return self.sentinel()

    def line(self, record):
        return json.dumps(self.value(None, record), ensure_ascii=False, separators=(",", ":"))


def leading_tag(s):
    m = re.match(r"^\s*<([a-z][a-z0-9_-]{0,40})>", s or "")
    return m.group(1) if m and m.group(1) in CONTENT_TAGS else None


def reference_outcome(r, role):
    """Independent reference implementation of the §13.2 classification."""
    t = r.get("type")
    if not isinstance(t, str):
        return "quarantined/missing_type"
    if t in IGNORED:
        return "ignored/" + t
    if t not in KNOWN:
        return "quarantined/unknown_record_type"
    if t == "assistant":
        return "agent_message"
    if t == "system":
        st = r.get("subtype")
        return "system_event" + ("/" + st if isinstance(st, str) and IDENT.match(st) else "")
    if t == "cost-state":
        return "reported_cost_snapshot"
    if t == "pr-link":
        return "pr_observed"
    if t == "continued-in":
        return "session_continued"
    if t == "relocated":
        return "session_relocated"
    if t == "worktree-state":
        return "worktree_observed"
    if t == "queue-operation":
        return "queue_operation/" + r.get("operation", "")
    # user
    msg = r.get("message") or {}
    c = msg.get("content")
    has_tr = isinstance(c, list) and any(isinstance(b, dict) and b.get("type") == "tool_result" for b in c)
    if "toolUseResult" in r or has_tr:
        return "tool_finished"
    if "interruptedMessageId" in r:
        return "turn_interrupted"
    tag = None
    kind = "absent"
    if isinstance(c, str):
        kind, tag = "text", leading_tag(c)
    elif isinstance(c, list):
        kind = "blocks"
        for b in c:
            if isinstance(b, dict) and b.get("type") == "text":
                tag = leading_tag(b.get("text"))
                break
    elif c is not None:
        kind = "other"
    origin = r.get("origin")
    origin_kind = origin.get("kind") if isinstance(origin, dict) else None
    if role == "subagent":
        reason = "subagent_stream"
    elif r.get("isSidechain") or "agentId" in r:
        reason = "sidechain"
    elif r.get("entrypoint") not in INTERACTIVE:
        reason = "non_interactive_entrypoint"
    elif r.get("isMeta"):
        reason = "meta"
    elif r.get("isCompactSummary"):
        reason = "compact_summary"
    elif r.get("isVisibleInTranscriptOnly"):
        reason = "transcript_only"
    elif "scheduledTaskId" in r or "scheduledFireId" in r:
        reason = "scheduled"
    elif (origin_kind is not None and origin_kind != "human") or r.get("turnOrigin") not in (None, "human") or (
        r.get("promptSource") is not None and r.get("promptSource") not in HUMAN_PROMPT_SOURCES
    ):
        reason = "automated_origin"
    elif tag is not None and tag not in HUMAN_TAGS:
        reason = "tool_or_system_output"
    elif kind == "absent":
        reason = "no_content"
    else:
        return "user_interaction/prompt_submitted"
    return "agent_input/" + reason


def structural_signature(r):
    """Record shape: what the classification rule reads, not what it concludes."""
    msg = r.get("message") if isinstance(r.get("message"), dict) else {}
    c = msg.get("content")
    if isinstance(c, str):
        shape = "text:" + str(leading_tag(c))
    elif isinstance(c, list):
        types = sorted({b.get("type") for b in c if isinstance(b, dict)})
        first_text = next((b.get("text") for b in c if isinstance(b, dict) and b.get("type") == "text"), None)
        shape = "blocks:" + ",".join(map(str, types)) + ":" + str(leading_tag(first_text))
    else:
        shape = str(type(c).__name__)
    origin = r.get("origin")
    return (
        r.get("type"), r.get("subtype"), shape, bool(r.get("isMeta")), r.get("promptSource"), r.get("turnOrigin"),
        origin.get("kind") if isinstance(origin, dict) else None, "toolUseResult" in r, bool(r.get("isSidechain")),
        "agentId" in r, r.get("entrypoint"), "scheduledTaskId" in r or "scheduledFireId" in r,
        "interruptedMessageId" in r, bool(r.get("isCompactSummary")), bool(r.get("isVisibleInTranscriptOnly")),
        r.get("operation"), r.get("worktreeSession") is None if "worktreeSession" in r else None,
    )


def file_version(path):
    c = collections.Counter()
    for line in open(path, errors="replace"):
        try:
            v = json.loads(line).get("version")
        except Exception:
            continue
        if v:
            c[v] += 1
    return c.most_common(1)[0][0] if c else None


def main():
    root, out = sys.argv[1], sys.argv[2]
    mains = glob.glob(os.path.join(root, "*", "*.jsonl"))
    subs = glob.glob(os.path.join(root, "*", "*", "subagents", "**", "agent-*.jsonl"), recursive=True)
    by_version = collections.defaultdict(lambda: {"main": [], "sub": []})
    for p in mains:
        by_version[file_version(p)]["main"].append(p)
    for p in subs:
        by_version[file_version(p)]["sub"].append(p)
    for version in VERSIONS:
        files = by_version[version]
        if not files["main"]:
            print(f"skip {version}: no main transcript")
            continue
        d = os.path.join(out, version)
        os.makedirs(d, exist_ok=True)
        san = Sanitizer()
        expected = {}

        def emit(name, records, role):
            counts = collections.Counter(reference_outcome(r, role) for r in records)
            with open(os.path.join(d, name), "w") as f:
                for r in records:
                    f.write(san.line(r) + "\n")
            expected[name] = {"role": role, "lines": len(records), "outcomes": dict(sorted(counts.items()))}

        # main.jsonl: contiguous prefix of the smallest session with at least 40 lines.
        candidates = sorted((os.path.getsize(p), p) for p in files["main"])
        chosen = next((p for _, p in candidates if sum(1 for _ in open(p, errors="replace")) >= 40), candidates[0][1])
        emit("main.jsonl", [json.loads(l) for l in list(open(chosen, errors="replace"))[:200]], "main")

        # records.jsonl: up to 2 samples per structural signature across all main sessions.
        per_sig = collections.defaultdict(list)
        for p in sorted(files["main"]):
            for l in open(p, errors="replace"):
                r = json.loads(l)
                sig = structural_signature(r)
                if len(per_sig[sig]) < 2:
                    per_sig[sig].append(r)
        emit("records.jsonl", [r for sig in sorted(per_sig, key=repr) for r in per_sig[sig]], "main")

        if files["sub"]:
            sub = sorted((os.path.getsize(p), p) for p in files["sub"])[len(files["sub"]) // 2][1]
            emit("subagent.jsonl", [json.loads(l) for l in list(open(sub, errors="replace"))[:60]], "subagent")

        with open(os.path.join(d, "expected.json"), "w") as f:
            json.dump(expected, f, indent=2, sort_keys=True)
            f.write("\n")
        print(version, {k: v["lines"] for k, v in expected.items()})


if __name__ == "__main__":
    main()
