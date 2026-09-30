# Scope

**In scope:** architecture §4.5–§4.8, §7.1 (coverage, human timeline, counting), §7.3, §7.4, and §7.5 (structural attribution); §13.1 S1.4. Split into three milestones, see [00_brief](00_brief.md).

**Deviations and choices, explicit:**

1. **No IOKit power-notification thread in M1.** Sleep and wake come from the `kern.sleeptime` / `kern.waketime` sysctls, read at every 15 s sample. The architecture lists these as the fallback for a missed notification. The kernel times are exact; detection lags by at most one sample. Several sleep/wake cycles between two samples (e.g. Power Nap dark wakes) would show only the last one.
2. **Full rebuild in the runtime** at most every 5 minutes while changes are pending, until dirty-unit derivation exists (M3). A rebuild peaks at about 435 MB RSS on the current history.
3. **The desktop app is a writer while it runs** (AD-03): it hosts the collector, closing the window keeps collecting, and the tray menu offers Open and Quit. Root approval in the app shows only proposed roots that exist (PS-02). Other roots stay a CLI action.
4. **Not in S1.4:** a login item (autostart), and a menu-bar status with minutes, which needs M2.
