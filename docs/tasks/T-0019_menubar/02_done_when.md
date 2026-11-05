# Done when
- [~] Left click on the menu bar icon opens the popover under the icon. Click-away or Esc closes it. Right click shows the native menu.
  Verified 2026-11-02: the popover opens under the icon (via `TRACR_SHOW_POPOVER`), closes when another app becomes active and on Esc. Not verified: a real click on the icon, the right-click menu.
- [~] The popover shows the header, limits, needs you, working, today and the footer, as in the brief, in German and English.
  Verified in German with real limits (synthetic statusLine figures in the copy). English not looked at. "Braucht dich" was empty in the copy.
- [ ] Rows open the main window on the right place (session, Limits, Today). Not verified (needs clicks).
- [ ] The timer starts and stops from the popover. Settings and Quit work. Not verified (needs clicks).
- [x] Light and dark mode both work. Nothing overflows at 360 pt. (The German reset label overflowed; it is now "Reset in …".)
- [~] fmt, clippy and typecheck are green. The full workspace test suite was not rerun on this branch.
- [x] A screenshot of the popover from a debug instance with a throwaway data dir.
- [x] Design-review gate run (05_validation_report.md).
