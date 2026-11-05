# T-0019 Menu bar popover

User request (2026-11-02): "a nice menu bar app, like ClaudeBar". ClaudeBar is the reference for the kind of tool: a glanceable quota monitor that lives in the menu bar. tracr gets its own popover in its own style. No layout, assets or text are copied.

Today, the menu bar shows:
- the title: timer, "n waiting" and the tightest quota;
- a plain menu with the day line, collector state, quota lines, Open, Timer and Quit.

A WIP branch `feat/menubar-popup` (on an older base) has a frameless popup window and click handling, but no page. Its backend is reused here.

Huly: TRACR-39 (milestone 0.12).
