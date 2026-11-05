# Validation report — T-0019 menu bar popover

Date: 2026-11-02, debug build of `feat/menubar`, data dir = a copy in the session scratchpad (`lsof` confirmed only the copy was open). Display 1920×1080 pt @2x.

## Checked at runtime
| Check | Result |
|---|---|
| Popover appears under the menu bar icon, 360 pt wide, height follows the content (160 → 292 → 329 → 421 pt) | ok |
| `--background` start: only the popover is on screen, the main window stays hidden | ok (CGWindowList) |
| Another app becomes active → popover hides | ok (CGWindowList shows no popover window) |
| Esc → popover hides | ok |
| Limits row: ring for the 5-hour window, weekly window as second line, reset countdown | ok after the label fix |
| Dark and light appearance: text readable on the material | ok (screenshots) |
| Debug app writes only to the copy | ok (`lsof`) |

## Click-through 2026-11-05 (debug build, copy of the data dir)
| Check | Result |
|---|---|
| Click on the icon opens the popover; figures match the Today page (counted, unknown, agents, cost) | ok |
| Timer start and stop from the popover | ok after the fix below |
| ⋯ menu: Settings, Limits, Quit tracr | opens ok (items not clicked) |
| Right-click: native menu (today line, Collecting, Open, Start timer, Quit) | ok; English until the main window has set the language |
| Popover over a full-screen app (Terminal in its own Space) | **failed**, fixed below, then ok |

### Fixes from the click-through
- **Full-screen Space (found 2026-11-05):** the popover stayed on the desktop Space, so a click on the icon showed nothing over a full-screen app. Collection behaviour `CanJoinAllSpaces | FullScreenAuxiliary` alone did not help, neither did switching to an accessory app while open (also with a delay) or `orderFrontRegardless`. What works: create the popover window while the app is an accessory app and switch back to regular right after (Dock icon stays). Checked with `isOnActiveSpace` and CGWindowList, plus a screenshot over the full-screen Terminal; app type afterwards regular (`background only` = false).
- **GH #33, timer seemed to ignore a click:** the button fired `popover_timer` without waiting and re-read after 400 ms, so a busy collector showed the old state and a second click could toggle twice. The popover now calls `manual` with an explicit start/stop and keeps the button disabled until done. Verified: start, immediate second click → one ledger revision; stop with one click → end written after ~1 s.

## Not verified
Rows that open sessions, Settings/Quit actually clicked, English UI, VoiceOver, the "Braucht dich" section with waiting sessions, several displays.

## Change after review
- German `pvResets` "zurückgesetzt in {time}" truncated the left text at 360 pt → "Reset in {time}".

## Design-review gate (apple-enterprise-design)
1. Calm first-party style: yes, system popover material, no branding copy.
2. Where am I / next action: yes, header names state and date; rows say what they open.
3. Layout fits workflow: yes, a glance view with progressive disclosure into the main window.
4. Commands: yes, footer + ⋯ menu + keys (Esc, ⏎, ⌘R, ⌘,, ⌘Q).
5. Hierarchy by type and spacing: yes, sections divided by hairlines, no cards.
6. Cards: none.
7. Color: yes, accent for ok, warn colour only for unresolved time and tight limits; checked in both appearances.
8. Data-heavy: not applicable (glance view, max 3 rows per section, rest via "n weitere").
9. Forms/destructive: not applicable; Quit is in the menu, not a button.
10. Keyboard/a11y: partly. Esc verified; focus rings and aria labels exist in code; VoiceOver not run. Intentional open item, not a blocker for a glance view.
11. Motion: yes, only the refresh spinner, off with reduced motion.
12. Generic SaaS look: no.
