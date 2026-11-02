# Design brief (apple-enterprise-design, before UI code)

## 1. The work
- **Job:** a glance at "where do I stand right now" without opening the app window.
- **User:** the developer, with several Claude Code and Codex sessions running and rate limits that matter.
- **Primary task:** see quota left, and who needs you.
- **Frequency:** many times an hour, a few seconds each time.
- **Consequence of error:**
  - A wrong quota figure makes you start a long task that hits the limit.
  - A missed "waiting" session idles while its cache expires.
- **Objects:**
  - quota windows (provider × 5 h or week);
  - live sessions (waiting, working);
  - today's time (counted, unclear, agent);
  - today's estimated cost;
  - the timer.

## 2. Workflow
- **Start:** a glance at the menu bar title (quota and "n waiting").
- **Click:** the popover opens under the icon.
- **What needs to be visible immediately:**
  - the tightest quota per provider with its reset time;
  - waiting sessions;
  - today's numbers.
- **Available on demand:**
  - every window as a bar (expand);
  - the full session board, which lives in the app.
- **Actions:**
  - open the app on a session or on Limits;
  - start or stop the timer;
  - refresh;
  - Settings;
  - Quit.
- **End state:** the popover closes on click-away or Esc. Nothing needs saving.

## 3. Native pattern
A **menu bar extra with a popover**, a focused utility (macOS: Wi-Fi, Battery, Control Center modules). It uses a translucent popover material and the system light or dark appearance, and dismisses on focus loss. Right-click keeps the plain native menu for keyboard and accessibility users.

## 4. Information hierarchy
1. **Header:** "tracr", the collector dot (recording or not) and today's date. A refresh button sits at the trailing edge.
2. **Limits** (primary): one compact row per provider.
   - It shows a ring for the tightest window, the provider name, the other window as a line, and the reset countdown.
   - The level color comes from the existing `levelOf` (pace aware, user preference), so it is the same as the app.
3. **Needs you:** waiting sessions (project, waiting time, cache time left). A row opens the session in the app. Shown only when there are any.
4. **Working:** a count plus the first three sessions (project, running time).
5. **Today:** four figures in one row (counted, unclear, agents, estimated cost). Unclear is amber when above 0. Clicking the row opens Today in the app.
6. **Footer:**
   - leading: the timer button, start or stop with the elapsed time;
   - trailing: "Open tracr" (⏎) and an overflow menu with Settings (⌘,) and Quit (⌘Q).

No destructive action except Quit, which sits in the overflow menu.

## 5. Layout and navigation
- **Size:** fixed width of 360 pt. The height follows the content, up to 600 pt, then scrolls.
- **Sections:** separated by hairlines. No cards inside cards.
- **No tabs:** one scrolling page.
- **Keyboard:**
  - Tab moves through the rows.
  - ⏎ opens the app; Esc closes.
  - ⌘R refreshes; ⌘, opens Settings; ⌘Q quits.
- **Data and refresh:**
  - Data is read when the popover opens, then every 5 s while it is visible.
  - Countdowns tick every 30 s.
- **Empty states:**
  - no limits yet: a line with a link to Limits;
  - no sessions: "No agent is running";
  - no data yet: "Starting…" during warm-up.
- **Appearance:** follows the system light or dark mode with the existing v2 tokens. The window background is translucent (macOS popover material), with the content on it.

## Deliberate differences from ClaudeBar
- No themes, provider pills or leaderboard.
- tracr adds what ClaudeBar has no way to know: your counted time today, waiting sessions, and the timer.
