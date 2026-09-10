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
const SCENARIOS: [&str; 19] = [
    "dynamic",
    "PollQ",
    "BlockQ",
    "semtest",
    "countsem",
    "recmutex",
    "blocktim",
    "AbortDelay",
    "QPeek",
    "GenQTest",
    "QueueOverwrite",
    "QueueSetPolling",
    "IntSemTest",
    "StreamBufferInterrupt",
    "TimerDemo",
    "EventGroupsDemo",
    "MessageBufferAMP",
    "PollQ-typed",
    "PollQ-async",
];

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
    crate::run(
        false,
        &dir,
        "cargo",
        &["build", "--release", "--bin", SIM_BIN],
    )?;
    Ok(())
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
        return Ok(());
    }
    let scenario = args
        .first()
        .filter(|a| !a.starts_with("--"))
        .map_or("dynamic", String::as_str);
    conform_one(root, scenario, ticks, exits)
}
