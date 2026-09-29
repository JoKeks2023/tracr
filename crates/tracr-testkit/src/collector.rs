//! Scenario time. Walkthrough scenarios happen on one reporting day; tests
//! write provider lines and drive clocks at scenario times.

/// The walkthroughs' reporting day.
pub const SCENARIO_DAY: &str = "2026-09-28";

/// `"14:05"` → `"2026-09-28T14:05:00.000Z"`.
pub fn at(hhmm: &str) -> String {
    format!("{SCENARIO_DAY}T{hhmm}:00.000Z")
}
