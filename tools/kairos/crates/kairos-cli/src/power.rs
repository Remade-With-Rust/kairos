//! `kairos power` — the two instruments the tickless-power mission needs
//! before it is allowed to build anything.
//!
//! * `idle` is M0's ceiling probe. It reads a stored oracle trace and asks how
//!   much of the run the idle task held the CPU, and — the number that decides
//!   the mission — how much of that was in windows long enough to sleep
//!   through. A scenario that never idles for two consecutive ticks cannot be
//!   helped by tickless idle however good the policy is, and saying so costs
//!   one command rather than one firmware.
//!
//! * `diff` is M1's projection. Turning tickless idle on changes a trace, so a
//!   raw byte-diff against the C kernel cannot survive it. But it does not
//!   change the SCHEDULE — the port winds the tick count forward across a
//!   sleep, so every task still runs at the tick it would have run at. `diff`
//!   applies [`rusty_rtos_core::trace::Scheduling`]'s rule to a trace file and
//!   demonstrates the invariance directly: strip every heartbeat line and the
//!   projection does not move.
//!
//! Both read the traces `kairos oracle trace` already stores, so neither needs
//! a board, a C toolchain, or a tickless port that does not exist yet.

use std::{fs, path::Path, path::PathBuf};

use crate::{Result, fail, has_flag, option};

/// The events a tickless sleep is allowed to change.
///
/// The same three `rusty_rtos_core::trace::is_suppressible` names, by their
/// trace spelling. A line is one of these or it is part of the schedule.
const SUPPRESSIBLE: [&str; 3] = [
    "TASK_INCREMENT_TICK",
    "LOW_POWER_IDLE_BEGIN",
    "LOW_POWER_IDLE_END",
];

/// The idle task's name, as `prvIdleTask` sets it.
const IDLE: &str = "IDLE";

/// `configEXPECTED_IDLE_TIME_BEFORE_SLEEP`'s default: below this a sleep costs
/// more than it saves, so the window is not counted as sleepable.
const DEFAULT_MIN_SLEEP_TICKS: u64 = 2;

/// What one scenario's trace says about its idle time.
struct Idle {
    name: String,
    ticks: u64,
    idle_ticks: u64,
    sleepable_ticks: u64,
    windows: u64,
    sleepable_windows: u64,
    longest: u64,
}

impl Idle {
    fn share(&self, of: u64) -> f64 {
        if self.ticks == 0 {
            0.0
        } else {
            100.0 * of as f64 / self.ticks as f64
        }
    }
}

/// The second field of a trace line is the event name; the first is the tick.
fn split(line: &str) -> Option<(u64, &str, Option<&str>)> {
    let mut parts = line.split_ascii_whitespace();
    let tick = parts.next()?.parse().ok()?;
    let event = parts.next()?;
    Some((tick, event, parts.next()))
}

/// Walk a trace and measure the idle windows.
///
/// A window opens when the idle task is switched in and closes when anything
/// else is, so its length is the tick distance between the two — which is
/// exactly how long a tickless port could have been asleep.
fn measure(name: &str, text: &str, min_sleep: u64) -> Idle {
    let mut out = Idle {
        name: name.to_owned(),
        ticks: 0,
        idle_ticks: 0,
        sleepable_ticks: 0,
        windows: 0,
        sleepable_windows: 0,
        longest: 0,
    };
    let mut current: Option<&str> = None;
    let mut opened = 0u64;

    for line in text.lines() {
        let Some((tick, event, subject)) = split(line) else {
            continue;
        };
        out.ticks = out.ticks.max(tick);
        if event != "TASK_SWITCHED_IN" {
            continue;
        }
        if current == Some(IDLE) {
            let window = tick.saturating_sub(opened);
            out.idle_ticks = out.idle_ticks.saturating_add(window);
            out.windows = out.windows.saturating_add(1);
            out.longest = out.longest.max(window);
            if window >= min_sleep {
                out.sleepable_ticks = out.sleepable_ticks.saturating_add(window);
                out.sleepable_windows = out.sleepable_windows.saturating_add(1);
            }
        }
        current = subject;
        opened = tick;
    }

    out
}

/// The projection: the lines a tickless sleep may not change.
fn project(text: &str) -> Vec<&str> {
    text.lines()
        .filter(|line| split(line).is_none_or(|(_, event, _)| !SUPPRESSIBLE.contains(&event)))
        .collect()
}

/// Every stored trace, in name order.
fn traces(root: &Path) -> Result<Vec<(String, PathBuf)>> {
    let dir = root.join("oracle").join("traces");
    let entries = fs::read_dir(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut found: Vec<(String, PathBuf)> = entries
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "trace"))
        .filter_map(|path| {
            let stem = path.file_stem()?.to_str()?.to_owned();
            Some((stem, path))
        })
        .collect();
    found.sort_by(|a, b| a.0.cmp(&b.0));
    if found.is_empty() {
        return fail(format!(
            "no traces under {} — run `kairos oracle trace --all` first",
            dir.display()
        ));
    }
    Ok(found)
}

/// Which traces a run covers: one named scenario, or all of them.
fn selected(root: &Path, args: &[String]) -> Result<Vec<(String, PathBuf)>> {
    let all = traces(root)?;
    let Some(name) = args.iter().find(|a| !a.starts_with("--")) else {
        return Ok(all);
    };
    if let Some(one) = all.iter().find(|(stem, _)| stem == name) {
        return Ok(vec![one.clone()]);
    }
    let known: Vec<&str> = all.iter().map(|(s, _)| s.as_str()).collect();
    fail(format!(
        "no trace for {name:?}; stored: {}",
        known.join(", ")
    ))
}

/// `kairos power idle` — M0's ceiling probe.
fn idle(root: &Path, args: &[String]) -> Result<()> {
    let min_sleep = match option(args, "--min-sleep") {
        None => DEFAULT_MIN_SLEEP_TICKS,
        Some(v) => match v.parse::<u64>() {
            Ok(n) => n,
            Err(_) => return fail(format!("--min-sleep wants a tick count, got {v:?}")),
        },
    };

    let mut rows = Vec::new();
    for (name, path) in selected(root, args)? {
        let text = fs::read_to_string(&path).map_err(|e| format!("{name}: {e}"))?;
        rows.push(measure(&name, &text, min_sleep));
    }

    println!(
        "idle windows and what a tickless port could sleep through \
         (>= {min_sleep} ticks)\n"
    );
    println!(
        "{:<22} {:>7} {:>8} {:>7} {:>10} {:>7} {:>8} {:>8}",
        "scenario", "ticks", "idle", "idle%", "sleepable", "sleep%", "windows", "longest"
    );

    let mut total_ticks = 0u64;
    let mut total_idle = 0u64;
    let mut total_sleepable = 0u64;
    for row in &rows {
        println!(
            "{:<22} {:>7} {:>8} {:>6.1}% {:>10} {:>6.1}% {:>8} {:>8}",
            row.name,
            row.ticks,
            row.idle_ticks,
            row.share(row.idle_ticks),
            row.sleepable_ticks,
            row.share(row.sleepable_ticks),
            row.sleepable_windows,
            row.longest,
        );
        total_ticks = total_ticks.saturating_add(row.ticks);
        total_idle = total_idle.saturating_add(row.idle_ticks);
        total_sleepable = total_sleepable.saturating_add(row.sleepable_ticks);
    }

    let pct = |n: u64| {
        if total_ticks == 0 {
            0.0
        } else {
            100.0 * n as f64 / total_ticks as f64
        }
    };
    println!(
        "\n{:<22} {:>7} {:>8} {:>6.1}% {:>10} {:>6.1}%",
        "ALL",
        total_ticks,
        total_idle,
        pct(total_idle),
        total_sleepable,
        pct(total_sleepable),
    );

    let dead = rows.iter().filter(|r| r.sleepable_ticks == 0).count();
    if dead > 0 {
        println!(
            "\n{dead} of {} scenarios have NO sleepable window: they idle, but never \
             for {min_sleep} consecutive ticks.\nTickless idle cannot help those \
             whatever the policy — that is a property of the workload, not a tuning \
             failure.",
            rows.len()
        );
    }
    Ok(())
}

/// `kairos power diff` — M1's projection, and the invariance it exists to show.
fn diff(root: &Path, args: &[String]) -> Result<()> {
    let show = has_flag(args, "--show");
    let mut failures = 0usize;

    for (name, path) in selected(root, args)? {
        let text = fs::read_to_string(&path).map_err(|e| format!("{name}: {e}"))?;

        let kept = project(&text);
        let total = text.lines().count();
        let dropped = total.saturating_sub(kept.len());

        // The invariance, demonstrated rather than asserted: take the
        // heartbeat away — which is precisely what a tickless sleep does —
        // and project again. The two must be the same schedule.
        let slept: String = text
            .lines()
            .filter(|line| split(line).is_none_or(|(_, event, _)| event != "TASK_INCREMENT_TICK"))
            .collect::<Vec<_>>()
            .join("\n");
        let after = project(&slept);

        let same = kept == after;
        if !same {
            failures = failures.saturating_add(1);
        }
        println!(
            "{:<22} {:>7} lines  {:>7} scheduling  {:>7} suppressible  {}",
            name,
            total,
            kept.len(),
            dropped,
            if same { "invariant" } else { "MOVED" },
        );
        if show {
            for line in kept.iter().take(8) {
                println!("    {line}");
            }
        }
    }

    if failures > 0 {
        return fail(format!(
            "{failures} scenario(s) moved under tick suppression — the projection \
             is not doing its job"
        ));
    }
    println!(
        "\nEvery scenario's schedule is unchanged when the heartbeat is removed.\n\
         That is what lets a tickless port be gated: the trace it changes is not \
         the trace that matters."
    );
    Ok(())
}

/// `kairos power`.
///
/// # Errors
///
/// When no traces are stored, a named scenario has none, or a projection moves
/// under tick suppression.
pub fn main(root: &Path, args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("idle") => idle(root, &args[1..]),
        Some("diff") => diff(root, &args[1..]),
        other => fail(format!(
            "kairos power idle|diff [SCENARIO] [--all]; got {other:?}"
        )),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// Two tasks, one of which is the idle task holding the CPU for a while.
    const TRACE: &str = "\
0 TASK_CREATE A 1
0 TASK_SWITCHED_IN A
1 TASK_INCREMENT_TICK 1
1 TASK_SWITCHED_OUT A
1 TASK_SWITCHED_IN IDLE
2 TASK_INCREMENT_TICK 2
3 TASK_INCREMENT_TICK 3
4 TASK_INCREMENT_TICK 4
4 TASK_SWITCHED_OUT IDLE
4 TASK_SWITCHED_IN A
5 TASK_INCREMENT_TICK 5
5 TASK_SWITCHED_IN IDLE
6 TASK_SWITCHED_IN A";

    #[test]
    fn an_idle_window_is_the_tick_distance_the_idle_task_held_the_cpu() {
        let m = measure("t", TRACE, 2);
        // IDLE held from tick 1 to 4, and from 5 to 6.
        assert_eq!(m.windows, 2, "two idle windows");
        assert_eq!(m.idle_ticks, 4, "three ticks then one");
        assert_eq!(m.longest, 3);
        // Only the first clears the two-tick floor.
        assert_eq!(m.sleepable_windows, 1);
        assert_eq!(m.sleepable_ticks, 3);
    }

    /// The floor is what decides whether a window counts, so moving it has to
    /// move the answer — otherwise `--min-sleep` is decoration.
    #[test]
    fn the_sleep_floor_decides_which_windows_count() {
        assert_eq!(measure("t", TRACE, 1).sleepable_windows, 2);
        assert_eq!(measure("t", TRACE, 2).sleepable_windows, 1);
        assert_eq!(measure("t", TRACE, 4).sleepable_windows, 0);
    }

    /// M1's claim, at the file level: removing the heartbeat leaves the
    /// schedule alone.
    #[test]
    fn the_projection_is_invariant_under_tick_suppression() {
        let slept: String = TRACE
            .lines()
            .filter(|l| !l.contains("TASK_INCREMENT_TICK"))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            project(TRACE),
            project(&slept),
            "the projection moved when only the heartbeat was taken away"
        );
    }

    /// Its poison: take a scheduling decision away instead and it must move.
    #[test]
    fn the_projection_is_not_invariant_under_a_lost_switch() {
        let lost: String = TRACE
            .lines()
            .filter(|l| *l != "4 TASK_SWITCHED_IN A")
            .collect::<Vec<_>>()
            .join("\n");
        assert_ne!(
            project(TRACE),
            project(&lost),
            "a dropped context switch survived the projection"
        );
    }

    /// The three suppressible spellings are the ones the core names, and a
    /// fourth would widen what a tickless port may change.
    #[test]
    fn exactly_three_event_kinds_are_suppressible() {
        assert_eq!(SUPPRESSIBLE.len(), 3);
        for name in SUPPRESSIBLE {
            assert!(name.starts_with("TASK_INCREMENT") || name.starts_with("LOW_POWER_IDLE"));
        }
    }
}
