# T-0015 step 1 done when

- Sidebar lists Heute, Sitzungen, Arbeitszeit; ⌘1/⌘2/⌘3 switch; ⌘K opens a palette with the same destinations and the actions available.
- Sitzungen lists sessions of a day/week/month newest first with provider, repo·branch, agent time and estimated cost; search and provider filter; detail beside it.
- Arbeitszeit is the former day view (overview / agent timeline), unchanged in content.
- `Reader::sessions` matches `day_agents` for the same day (test) and refuses a reversed or malformed range (test).
- fmt, clippy `-D warnings`, tests, typecheck green; real window checked or the gap stated.
