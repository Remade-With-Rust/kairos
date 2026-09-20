//! `kairos conform` — the K1 gate: does the Rust kernel decide what the C
//! kernel decides?
//!
//! One scenario, run twice — once on `rusty_rtos_demo`'s sim and once on the
//! instrumented C oracle — and the two traces compared line for line. A pass
//! means every scheduling decision, in order, at the same tick, with the same
//! counters. A failure names the first line that differs and shows both
//! sides, because that line names the C statement that was mis-modelled.
//!
//! The comparison includes the harness's `KAIROS_RESULT` line, so the tick,
//! yield and critical-section-exit counters have to agree too. They are the
//! part that cannot be faked: the trace says *what* happened, the counters
//! say *when*, and sim time on both sides is a count of critical-section
//! exits (`ORACLES.md`, sim contract v1).
//!
//! With `--exits` both sides add the ` #<exits>` debug column, which is how
//! a divergence is actually diagnosed: the events usually agree for a long
//! while after the accounting has drifted.

use std::{fs, path::Path};

use crate::{Result, fail, has_flag, option};

/// The scenarios `rusty_rtos_demo` can run. The C oracle knows the same
/// names; `kairos oracle` refuses one it does not have.
const SCENARIOS: [&str; 21] = [
    "dynamic",
    "AbortDelay",
    "PollQ",
    "BlockQ",
    "semtest",
    "countsem",
    "recmutex",
    "blocktim",
    "QPeek",
    "GenQTest",
    "QueueOverwrite",
    "QueueSetPolling",
    "IntSemTest",
    "StreamBufferInterrupt",
    "TaskNotify",
    "TimerDemo",
    "EventGroupsDemo",
    "MessageBufferAMP",
    "PollQ-typed",
    "PollQ-async",
    "death",
];

/// The fewest ticks at which a scenario actually does its job.
///
/// The default 2,000 is enough for every scenario whose tasks are all
/// running by the first tick. `death` is not one of them: its creator waits
/// a whole second before it even counts the tasks, waits another before it
/// spawns, and the spawned tasks wait 200 ticks more before killing
/// anything — so at 2,000 ticks the run ends ON the first creation and
/// deletes NOTHING. It would pass, and the pass would mean nothing.
///
/// A floor rather than a fixed value, so `--ticks` can still be raised.
const MIN_TICKS: [(&str, u64); 1] = [("death", 4_000)];

/// The ticks a scenario must be run for at least.
fn min_ticks(scenario: &str) -> u64 {
    let mut floor = 0;
    for (name, ticks) in MIN_TICKS {
        if name == scenario {
            floor = ticks;
        }
    }
    floor
}

/// Scenarios that are built and do NOT conform, with the reason.
///
/// `--all` must mean "every scenario that is expected to agree", or it
/// stops being a gate the moment one is known not to. But a scenario
/// quietly dropped from a list is the defect this repository has paid for
/// more than once, so `--all` PRINTS these and what blocks them rather
/// than omitting them.
const BLOCKED: [(&str, &str); 0] = [];

// `AbortDelay` was the one entry here, for 1,945 of 2,549 lines, and it was
// never a kernel disagreement: all 2,549 events, ticks and arguments
// matched, and 8 lines differed only in a QUEUE ORDINAL. The trace contract
// named unnamed objects in a way that depended on the C allocator, which the
// Rust arena does not share. Both sides now number by creation order and it
// conforms in full. See `rusty_rtos_demo-core::trace`.

/// A scenario whose C arm is another scenario's.
///
/// `PollQ-typed` is `PollQ` written against the Rust face (mission plan,
/// K2.1). There is no separate C original and there must not be: the point
/// is that the *same* oracle trace comes out, exits included, so the face
/// is proved to cost nothing rather than asserted to.
fn oracle_scenario(scenario: &str) -> &str {
    scenario
        .strip_suffix("-typed")
        .or_else(|| scenario.strip_suffix("-async"))
        .unwrap_or(scenario)
}

/// The demo package, and the binary inside it.
const DEMO: &str = "rusty_rtos_demo";
const SIM_BIN: &str = "kairos-sim";

/// Build the sim once, so a `--all` sweep does not rebuild per scenario.
fn build_sim(root: &Path) -> Result<()> {
    let dir = root.join(DEMO);
    if !dir.join("Cargo.toml").is_file() {
        return fail(format!("{DEMO} is not checked out; nothing to conform"));
    }
    // Keep the standalone `Cargo.lock`, exactly as `check` does.
    //
    // This build runs with the umbrella's `[patch]` table in scope, which
    // rewrites the lockfile: every sibling loses its `source = "git+..."`
    // line and a fresh clone can no longer reproduce the build (H-07).
    // `check` has snapshotted and restored around its cargo calls for a
    // while; `conform` did not, and the omission was invisible because
    // `conform` is usually run alone. It surfaced on 2026-09-11 when a
    // `conform --all` between two `check --board` runs left the demo's
    // lockfile rewritten, and the SECOND check reported an H-07 failure
    // caused by the conform in between.
    let before = lockfile_if_standalone(&dir);
    let result = crate::run(
        false,
        &dir,
        "cargo",
        &["build", "--release", "--bin", SIM_BIN],
    );
    restore_lockfile(&dir, before.as_ref());
    result?;
    Ok(())
}

/// The package's `Cargo.lock`, but only when it is the one a standalone
/// clone resolves — i.e. it still names a sibling by git URL.
///
/// Returning `None` for an already-rewritten lockfile is deliberate and
/// matches `check`: this restores what was there, it does not invent a
/// correct lockfile. The tooling is self-protecting, not self-healing.
fn lockfile_if_standalone(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("Cargo.lock")).ok()?;
    text.contains("source = \"git+").then_some(text)
}

/// Put back the lockfile cargo rewrote, if it rewrote it.
fn restore_lockfile(dir: &Path, before: Option<&String>) {
    let Some(before) = before else {
        return;
    };
    let lock = dir.join("Cargo.lock");
    if std::fs::read_to_string(&lock).is_ok_and(|now| now == *before) {
        return;
    }
    if std::fs::write(&lock, before).is_ok() {
        println!("  restored the standalone Cargo.lock (the [patch] table had rewritten it)");
    }
}

/// Run the Rust sim and return its trace.
///
/// The built binary is invoked directly rather than through `cargo run`:
/// the trace is stderr, and so is any warning cargo decides to print, so a
/// single unrelated lint would otherwise appear as line 1 of the trace and
/// "diverge" against the oracle's first event. Build once, then run the
/// artefact.
fn run_sim(root: &Path, scenario: &str, ticks: u64, exits: bool) -> Result<String> {
    let dir = root.join(DEMO);
    let bin = dir
        .join("target")
        .join("release")
        .join(format!("{SIM_BIN}{}", std::env::consts::EXE_SUFFIX));
    if !bin.is_file() {
        return fail(format!("{} was not built", bin.display()));
    }
    let ticks = ticks.to_string();
    let mut command = std::process::Command::new(&bin);
    command.args([scenario, &ticks]).current_dir(&dir);
    if exits {
        command.env("KAIROS_TRACE_EXITS", "1");
    }
    println!("$ {} {scenario} {ticks}", bin.display());
    let output = command.output()?;
    let trace = String::from_utf8_lossy(&output.stderr).into_owned();
    let out = root
        .join("oracle")
        .join("traces")
        .join(format!("{scenario}.sim"));
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&out, &trace)?;
    Ok(trace)
}

/// Run the C oracle and return its trace.
fn run_oracle(root: &Path, scenario: &str, ticks: u64, exits: bool) -> Result<String> {
    // A `-typed` arm has no C original: it is another scenario written
    // against the Rust face, and its whole point is to come out identical
    // to that scenario's own oracle trace.
    let scenario = oracle_scenario(scenario);
    // `MessageBufferAMP` comes out of the second binary: it redefines
    // `sbSEND_COMPLETED`, which is global (see `oracle::binary_for`).
    let bin_name = if scenario == "MessageBufferAMP" {
        "corpus-amp"
    } else {
        "corpus"
    };
    let bin = root.join("oracle").join("build").join(bin_name);
    if !bin.is_file() {
        return fail(format!(
            "oracle/build/{bin_name} is not built; run `kairos oracle build`"
        ));
    }
    let env = if exits { "KAIROS_TRACE_EXITS=1 " } else { "" };
    let script = format!(
        "ulimit -f 4194304; {env}timeout 900 ./oracle/build/{bin_name} {scenario} {ticks} 2> oracle/traces/{scenario}.oracle; echo exit=$?"
    );
    let (ok, _stdout, stderr) = crate::oracle::host_shell(root, &script)?;
    if !ok {
        return fail(format!("running the oracle failed:\n{stderr}"));
    }
    Ok(fs::read_to_string(
        root.join("oracle")
            .join("traces")
            .join(format!("{scenario}.oracle")),
    )?)
}

/// Compare two traces; `Ok(lines)` when they are identical.
///
/// A `-typed` arm names itself in its own `KAIROS_RESULT` line, and that
/// one word is the only thing about it that is allowed to differ: the
/// counters on that line are compared like every other, so a face that
/// cost a single critical-section exit would still be caught here.
fn compare(scenario: &str, ours: &str, theirs: &str) -> Result<usize> {
    let ours = &ours.replace(
        &format!("KAIROS_RESULT {scenario} "),
        &format!("KAIROS_RESULT {} ", oracle_scenario(scenario)),
    );
    let mut ours_lines = ours.lines();
    let mut theirs_lines = theirs.lines();
    let mut n = 0usize;
    loop {
        match (ours_lines.next(), theirs_lines.next()) {
            (None, None) => return Ok(n),
            (a, b) if a == b => n = n.saturating_add(1),
            (a, b) => {
                let line = n.saturating_add(1);
                return fail(format!(
                    "{scenario}: traces diverge at line {line}\n  ours:   {}\n  oracle: {}\n\nThe kernel agreed with the C kernel for {n} lines. Re-run with\n`--exits` to see the critical-section-exit column on both sides: the\nevents usually agree for a while after the sim-time accounting has\ndrifted, and the column shows where it drifted.",
                    a.unwrap_or("<end of trace>"),
                    b.unwrap_or("<end of trace>")
                ));
            }
        }
    }
}

/// One scenario, both kernels, one verdict.
fn conform_one(root: &Path, scenario: &str, ticks: u64, exits: bool) -> Result<()> {
    if !SCENARIOS.contains(&scenario) {
        return fail(format!(
            "unknown scenario {scenario:?}; known: {}",
            SCENARIOS.join(", ")
        ));
    }
    // A scenario run for too few ticks is not a weaker gate, it is a
    // vacuous one: `death` at 2,000 never reaches a single `vTaskDelete`.
    let ticks = ticks.max(min_ticks(scenario));
    let ours = run_sim(root, scenario, ticks, exits)?;
    let theirs = run_oracle(root, scenario, ticks, exits)?;
    let verdict = ours
        .lines()
        .rev()
        .find(|l| l.starts_with("KAIROS_RESULT"))
        .unwrap_or("KAIROS_RESULT <missing>");
    let lines = compare(scenario, &ours, &theirs)?;
    if !verdict.contains(" pass ") {
        return fail(format!(
            "{scenario}: the traces are identical but the scenario failed its own check: {verdict}"
        ));
    }
    println!("{scenario}: {lines} lines identical, {ticks} ticks");
    println!("  {verdict}");
    Ok(())
}

pub(crate) fn main(root: &Path, args: &[String]) -> Result<()> {
    let ticks = option(args, "--ticks")
        .map(|t| t.parse::<u64>())
        .transpose()
        .map_err(|e| format!("--ticks: {e}"))?
        .unwrap_or(2000);
    let exits = has_flag(args, "--exits");
    let all = has_flag(args, "--all");
    build_sim(root)?;
    if all {
        for scenario in SCENARIOS {
            conform_one(root, scenario, ticks, exits)?;
        }
        println!(
            "\nconform: {} scenario(s) identical to the C kernel",
            SCENARIOS.len()
        );
        for (scenario, why) in BLOCKED {
            println!();
            println!("NOT RUN -- {scenario}: {why}");
        }
        return Ok(());
    }
    let scenario = args
        .first()
        .filter(|a| !a.starts_with("--"))
        .map_or("dynamic", String::as_str);
    conform_one(root, scenario, ticks, exits)
}
