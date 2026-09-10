//! `kairos oracle` — the instrumented C kernel that every scheduling decision
//! is measured against.
//!
//! * `fetch` clones FreeRTOS-Kernel and the classic FreeRTOS repository at
//!   the commits `ORACLES.md` pins (and refuses anything else).
//! * `patch` applies the deterministic-tick edits to the Posix port in place:
//!   exact-anchor textual replacements on the freshly restored pinned file,
//!   each asserted to match exactly once, marked `KAIROS`.
//! * `build <scenario>` compiles the kernel, the patched port, the harness
//!   and the scenario's demo task file with the system C compiler — under
//!   WSL on Windows (the Posix port needs pthreads and signals), directly on
//!   a Unix host. No CMake, no make: one compiler invocation this tool owns.
//! * `trace <scenario> [--ticks N]` runs the binary twice, refuses a trace
//!   that differs between the runs, and stores it under `oracle/traces/` as
//!   one zstd frame (`<scenario>.trace.zst`, the house compressor,
//!   round-tripped before it is kept); the plain file stays beside it for
//!   humans and is not tracked.
//! * `cat <scenario>` decompresses the stored trace to stdout (for a diff).
//!
//! The C toolchain is a dev-only oracle dependency, never a build dependency
//! of anything that ships.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use crate::{Result, fail, has_flag, option, probe};

/// FreeRTOS-Kernel, the pinned tag and commit (`ORACLES.md`).
const KERNEL_URL: &str = "https://github.com/FreeRTOS/FreeRTOS-Kernel.git";
const KERNEL_TAG: &str = "V11.3.1";
const KERNEL_SHA: &str = "3a22924e0a9ddbbc8b0758881c33b3422a5cc20d";

/// The classic distribution: the standard demo tasks and the proofs.
const CLASSIC_URL: &str = "https://github.com/FreeRTOS/FreeRTOS.git";
const CLASSIC_SHA: &str = "f4fcc3b228643144727e9257ba12db1cb632b6e6";
const CLASSIC_SPARSE: [&str; 5] = [
    "FreeRTOS/Demo/Common/Minimal",
    "FreeRTOS/Demo/Common/include",
    "FreeRTOS/Demo/Posix_GCC",
    "FreeRTOS/Test/CBMC",
    "FreeRTOS/Test/VeriFast",
];

/// A demo scenario: its name and the standard demo files it needs.
struct Scenario {
    name: &'static str,
    demo_files: &'static [&'static str],
}

const SCENARIOS: [Scenario; 15] = [
    Scenario {
        name: "dynamic",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/dynamic.c"],
    },
    Scenario {
        name: "PollQ",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/PollQ.c"],
    },
    Scenario {
        name: "BlockQ",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/BlockQ.c"],
    },
    Scenario {
        name: "semtest",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/semtest.c"],
    },
    Scenario {
        name: "countsem",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/countsem.c"],
    },
    Scenario {
        name: "recmutex",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/recmutex.c"],
    },
    Scenario {
        name: "blocktim",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/blocktim.c"],
    },
    Scenario {
        name: "QPeek",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/QPeek.c"],
    },
    Scenario {
        name: "GenQTest",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/GenQTest.c"],
    },
    Scenario {
        name: "QueueOverwrite",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/QueueOverwrite.c"],
    },
    Scenario {
        name: "QueueSetPolling",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/QueueSetPolling.c"],
    },
    Scenario {
        name: "IntSemTest",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/IntSemTest.c"],
    },
    Scenario {
        name: "StreamBufferInterrupt",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/StreamBufferInterrupt.c"],
    },
    Scenario {
        name: "TimerDemo",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/TimerDemo.c"],
    },
    Scenario {
        name: "EventGroupsDemo",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/EventGroupsDemo.c"],
    },
];

/// One binary serves the whole corpus: the harness's table names every
/// scenario, so building per scenario would compile the kernel seven times
/// for seven identical binaries.
const ORACLE_BIN: &str = "corpus";

const KERNEL_SOURCES: [&str; 6] = [
    "tasks.c",
    "queue.c",
    "list.c",
    "timers.c",
    "event_groups.c",
    "stream_buffer.c",
];

fn kernel_dir(root: &Path) -> PathBuf {
    root.join("oracle").join("FreeRTOS-Kernel")
}

fn classic_dir(root: &Path) -> PathBuf {
    root.join("oracle").join("FreeRTOS")
}

fn port_c(root: &Path) -> PathBuf {
    kernel_dir(root)
        .join("portable")
        .join("ThirdParty")
        .join("GCC")
        .join("Posix")
        .join("port.c")
}

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    crate::run(false, dir, "git", args)
}

fn head_sha(dir: &Path) -> Option<String> {
    probe(dir, "git", &["rev-parse", "HEAD"])
}

// ------------------------------------------------------------------ fetch --

fn fetch(root: &Path) -> Result<()> {
    let oracle = root.join("oracle");
    fs::create_dir_all(&oracle)?;

    let kernel = kernel_dir(root);
    if kernel.join(".git").exists() {
        println!("FreeRTOS-Kernel: already cloned");
    } else {
        git(
            &oracle,
            &[
                "clone",
                "--depth",
                "1",
                "--branch",
                KERNEL_TAG,
                KERNEL_URL,
                "FreeRTOS-Kernel",
            ],
        )?;
    }
    match head_sha(&kernel) {
        Some(sha) if sha == KERNEL_SHA => {
            println!("FreeRTOS-Kernel: {KERNEL_TAG} = {sha} (pinned)")
        }
        Some(sha) => {
            return fail(format!(
                "FreeRTOS-Kernel is at {sha}, not the pinned {KERNEL_SHA} ({KERNEL_TAG}); delete oracle/FreeRTOS-Kernel and re-fetch, or re-pin in ORACLES.md with a decision-log row"
            ));
        }
        None => return fail("FreeRTOS-Kernel: cannot read HEAD"),
    }

    let classic = classic_dir(root);
    if !classic.join(".git").exists() {
        git(
            &oracle,
            &[
                "clone",
                "--depth",
                "1",
                "--filter=blob:none",
                "--sparse",
                CLASSIC_URL,
                "FreeRTOS",
            ],
        )?;
        let mut args = vec!["sparse-checkout", "set", "--no-cone"];
        args.extend(CLASSIC_SPARSE);
        git(&classic, &args)?;
    }
    if head_sha(&classic).as_deref() != Some(CLASSIC_SHA) {
        git(&classic, &["fetch", "--depth", "1", "origin", CLASSIC_SHA])?;
        git(&classic, &["checkout", "--detach", CLASSIC_SHA])?;
    }
    match head_sha(&classic) {
        Some(sha) if sha == CLASSIC_SHA => println!("FreeRTOS (classic): {sha} (pinned)"),
        Some(sha) => {
            return fail(format!(
                "FreeRTOS (classic) is at {sha}, not the pinned {CLASSIC_SHA}"
            ));
        }
        None => return fail("FreeRTOS (classic): cannot read HEAD"),
    }
    let minimal = classic.join("FreeRTOS/Demo/Common/Minimal");
    let n = fs::read_dir(&minimal).map(|d| d.count()).unwrap_or(0);
    println!("FreeRTOS/Demo/Common/Minimal: {n} files");
    Ok(())
}

// ------------------------------------------------------------------ patch --

const MARKER: &str = "KAIROS";

/// One exact-anchor edit: `from` must occur exactly once.
struct Edit {
    what: &'static str,
    from: &'static str,
    to: &'static str,
}

const EDITS: [Edit; 6] = [
    Edit {
        what: "declare the counters the harness reports",
        from: "static uint64_t prvStartTimeNs;\n",
        to: "static uint64_t prvStartTimeNs;\n/* KAIROS sim contract v1: counters the harness reports. */\nunsigned long ulKairosYields = 0UL;\nunsigned long ulKairosTicks = 0UL;\nunsigned long ulKairosExits = 0UL;\n",
    },
    Edit {
        what: "remove the timer tick thread",
        from: "    xTimerTickThreadShouldRun = true;\n    pthread_create( &hTimerTickThread, NULL, prvTimerTickHandler, NULL );\n",
        to: "    /* KAIROS sim contract v1: no timer thread. Ticks come from vPortKairosTick()\n     * (the idle hook) and from every 16th outermost vPortExitCritical(). */\n    ( void ) hTimerTickThread;\n    ( void ) prvTimerTickHandler;\n",
    },
    Edit {
        what: "do not join the thread that was never started",
        from: "    /* Stop the timer tick thread. */\n    xTimerTickThreadShouldRun = false;\n    pthread_join( hTimerTickThread, NULL );\n",
        to: "    /* KAIROS: there is no timer tick thread to stop. */\n",
    },
    Edit {
        what: "deliver a tick on every 16th outermost critical-section exit",
        from: "    /* If we have reached 0 then re-enable the interrupts. */\n    if( uxCriticalNesting == 0 )\n    {\n        vPortEnableInterrupts();\n    }\n",
        to: "    /* If we have reached 0 then re-enable the interrupts. */\n    if( uxCriticalNesting == 0 )\n    {\n        /* KAIROS sim contract v1: a tick that arrived during a critical section\n         * fires where the hardware would fire it, at the section's exit. Every\n         * 16th outermost exit on a FreeRTOS thread delivers one. */\n        if( prvIsFreeRTOSThread() == pdTRUE )\n        {\n            ulKairosExits++;\n\n            if( ( ulKairosExits % 16UL ) == 0UL )\n            {\n                vPortSystemTickHandler( SIGALRM );\n            }\n        }\n\n        vPortEnableInterrupts();\n    }\n",
    },
    Edit {
        what: "count yields",
        from: "    vPortEnterCritical();\n\n    prvPortYieldFromISR();\n\n    vPortExitCritical();\n",
        to: "    vPortEnterCritical();\n\n    ulKairosYields++; /* KAIROS: reported, not a tick source. */\n\n    prvPortYieldFromISR();\n\n    vPortExitCritical();\n",
    },
    Edit {
        what: "count ticks",
        from: "        if( xTaskIncrementTick() != pdFALSE )\n",
        to: "        ulKairosTicks++; /* KAIROS */\n\n        if( xTaskIncrementTick() != pdFALSE )\n",
    },
];

const APPENDIX: &str = "
/*-----------------------------------------------------------*/

/* KAIROS sim contract v1: the deterministic tick, called from the idle hook.
 * Does what the SIGALRM handler did, inside a critical section so the switch
 * it may cause happens with signals blocked. */
void vPortKairosTick( void )
{
    vPortEnterCritical();
    vPortSystemTickHandler( SIGALRM );
    vPortExitCritical();
}
";

fn patch(root: &Path) -> Result<()> {
    let path = port_c(root);
    if !path.is_file() {
        return fail(format!(
            "{}: not found (run `kairos oracle fetch` first)",
            path.display()
        ));
    }
    // Start from the pinned file every time: the edits are then applied
    // exactly once whatever state the checkout was in.
    git(
        &kernel_dir(root),
        &["checkout", "--", "portable/ThirdParty/GCC/Posix/port.c"],
    )?;
    let text = fs::read_to_string(&path)?;
    if text.contains(MARKER) {
        return fail(
            "port.c still carries KAIROS edits after `git checkout`; the checkout is not the pinned tree",
        );
    }
    // A Windows git checkout carries CRLF endings; the anchors are LF. Match
    // on LF and write back in the style the checkout had.
    let crlf = text.contains("\r\n");
    let mut out = if crlf {
        text.replace("\r\n", "\n")
    } else {
        text
    };
    for edit in &EDITS {
        let n = out.matches(edit.from).count();
        if n != 1 {
            return fail(format!(
                "port.c patch \"{}\": anchor matched {n} times, expected exactly 1 — the pinned port.c has changed; re-derive the anchors",
                edit.what
            ));
        }
        out = out.replacen(edit.from, edit.to, 1);
        println!("  patched: {}", edit.what);
    }
    out.push_str(APPENDIX);
    if crlf {
        out = out.replace('\n', "\r\n");
    }
    fs::write(&path, out)?;
    println!("{}: patched (6 edits + vPortKairosTick)", path.display());
    Ok(())
}

// ------------------------------------------------------------------ build --

/// `F:\x\y` -> `/mnt/f/x/y` for WSL; a Unix path is returned as is.
fn wsl_path(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let s = s.strip_prefix("//?/").unwrap_or(&s).to_owned();
    let mut chars = s.chars();
    match (chars.next(), chars.next(), chars.next()) {
        (Some(d), Some(':'), Some('/')) if d.is_ascii_alphabetic() => {
            format!("/mnt/{}/{}", d.to_ascii_lowercase(), &s[3..])
        }
        _ => s,
    }
}

/// Run a shell command line on the oracle host: WSL on Windows, `sh` elsewhere.
pub(crate) fn host_shell(cwd: &Path, script: &str) -> Result<(bool, String, String)> {
    let output = if cfg!(windows) {
        let cd = wsl_path(cwd);
        Command::new("wsl")
            .args([
                "-d",
                "Ubuntu",
                "--",
                "bash",
                "-lc",
                &format!("cd '{cd}' && {script}"),
            ])
            .output()?
    } else {
        Command::new("sh")
            .args(["-c", script])
            .current_dir(cwd)
            .output()?
    };
    Ok((
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    ))
}

fn scenario(name: &str) -> Result<&'static Scenario> {
    SCENARIOS.iter().find(|s| s.name == name).map_or_else(
        || {
            let known: Vec<&str> = SCENARIOS.iter().map(|s| s.name).collect();
            fail(format!(
                "unknown scenario {name:?}; known: {}",
                known.join(", ")
            ))
        },
        Ok,
    )
}

fn build(root: &Path, name: &str) -> Result<()> {
    let sc = scenario(name)?;
    if !port_c(root).is_file() {
        return fail("oracle/FreeRTOS-Kernel is not fetched; run `kairos oracle fetch`");
    }
    if !fs::read_to_string(port_c(root))?.contains(MARKER) {
        return fail("the Posix port is not patched; run `kairos oracle patch`");
    }
    let build_dir = root.join("oracle").join("build");
    fs::create_dir_all(&build_dir)?;

    // Paths relative to the umbrella root, so the same command line works
    // under WSL (cwd = the root) and on a Unix host.
    let mut sources: Vec<String> = KERNEL_SOURCES
        .iter()
        .map(|s| format!("oracle/FreeRTOS-Kernel/{s}"))
        .collect();
    sources.push("oracle/FreeRTOS-Kernel/portable/ThirdParty/GCC/Posix/port.c".into());
    sources
        .push("oracle/FreeRTOS-Kernel/portable/ThirdParty/GCC/Posix/utils/wait_for_event.c".into());
    sources.push("oracle/FreeRTOS-Kernel/portable/MemMang/heap_3.c".into());
    sources.push("oracle/harness/main.c".into());
    sources.push("oracle/harness/kairos_trace.c".into());
    // Every scenario's demo file goes into every binary: the harness's
    // table names them all, and one build serves the whole corpus.
    let _ = sc;
    for scenario in &SCENARIOS {
        for f in scenario.demo_files {
            sources.push(format!("oracle/FreeRTOS/{f}"));
        }
    }
    let includes = [
        "oracle/harness",
        "oracle/FreeRTOS-Kernel/include",
        "oracle/FreeRTOS-Kernel/portable/ThirdParty/GCC/Posix",
        "oracle/FreeRTOS-Kernel/portable/ThirdParty/GCC/Posix/utils",
        "oracle/FreeRTOS/FreeRTOS/Demo/Common/include",
    ];
    let mut cmd = String::from("cc -O0 -g -Wall -Wno-unused-parameter -DprojCOVERAGE_TEST=0");
    for inc in includes {
        cmd.push_str(&format!(" -I{inc}"));
    }
    for s in &sources {
        cmd.push_str(&format!(" {s}"));
    }
    cmd.push_str(&format!(" -o oracle/build/{ORACLE_BIN} -pthread"));
    println!("$ {cmd}");
    let (ok, stdout, stderr) = host_shell(root, &cmd)?;
    print!("{stdout}");
    if !ok {
        return fail(format!("the oracle build failed:\n{stderr}"));
    }
    if !stderr.trim().is_empty() {
        println!("{}", stderr.trim_end());
    }
    let bin = build_dir.join(ORACLE_BIN);
    let size = fs::metadata(&bin).map(|m| m.len()).unwrap_or(0);
    println!(
        "built oracle/build/{ORACLE_BIN} ({size} bytes), serving {} scenario(s)",
        SCENARIOS.len()
    );
    Ok(())
}

// ------------------------------------------------------------------ trace --

fn trace(root: &Path, name: &str, ticks: u64) -> Result<()> {
    scenario(name)?;
    let bin = root.join("oracle").join("build").join(ORACLE_BIN);
    if !bin.is_file() {
        return fail("oracle/build/corpus is not built; run `kairos oracle build`");
    }
    let traces = root.join("oracle").join("traces");
    fs::create_dir_all(&traces)?;
    let run = |suffix: &str| -> Result<(bool, String, usize)> {
        // `timeout` and `ulimit -f` (1 GiB) keep a scenario that never
        // reaches max_ticks from filling the disk.
        let script = format!(
            "ulimit -f 1048576; timeout 300 ./oracle/build/{ORACLE_BIN} {name} {ticks} 2> oracle/traces/{name}.trace{suffix}; echo exit=$?"
        );
        let (ok, stdout, stderr) = host_shell(root, &script)?;
        if !ok {
            return fail(format!("running the oracle failed:\n{stderr}"));
        }
        let text = fs::read_to_string(traces.join(format!("{name}.trace{suffix}")))?;
        let lines = text.lines().count();
        let verdict = text
            .lines()
            .rev()
            .find(|l| l.starts_with("KAIROS_RESULT"))
            .unwrap_or("KAIROS_RESULT ? missing")
            .to_owned();
        let pass = verdict.contains(" pass ");
        println!("run{suffix}: {} lines; {verdict}; {}", lines, stdout.trim());
        Ok((pass, verdict, lines))
    };
    let (pass1, verdict1, lines1) = run("")?;
    let (pass2, _verdict2, lines2) = run(".2")?;
    let a = fs::read(traces.join(format!("{name}.trace")))?;
    let b = fs::read(traces.join(format!("{name}.trace.2")))?;
    if a != b {
        return fail(format!(
            "the trace is NOT deterministic: run 1 has {lines1} lines, run 2 has {lines2}; compare oracle/traces/{name}.trace and {name}.trace.2"
        ));
    }
    let _ = fs::remove_file(traces.join(format!("{name}.trace.2")));
    if !(pass1 && pass2) {
        return fail(format!(
            "the scenario did not pass its own check: {verdict1}"
        ));
    }
    // The tracked form: one zstd frame, at the archival level (a trace is
    // repetitive text, and it is read far more often than written), and
    // never kept without a round trip proving it decodes to the bytes seen.
    let packed = rusty_zstd::compress(&a, 19).map_err(|e| format!("rusty_zstd: {e}"))?;
    let back = rusty_zstd::decompress(&packed).map_err(|e| format!("rusty_zstd: {e}"))?;
    if back != a {
        return fail("rusty_zstd round trip did not reproduce the trace; nothing stored");
    }
    fs::write(traces.join(format!("{name}.trace.zst")), &packed)?;
    println!(
        "oracle/traces/{name}.trace.zst: {lines1} lines, byte-identical across two runs, scenario pass ({ticks} ticks); {} -> {} bytes, round trip verified",
        a.len(),
        packed.len()
    );
    Ok(())
}

/// The stored trace, decompressed to stdout.
fn cat(root: &Path, name: &str) -> Result<()> {
    use std::io::Write as _;
    scenario(name)?;
    let path = root
        .join("oracle")
        .join("traces")
        .join(format!("{name}.trace.zst"));
    let packed = fs::read(&path)
        .map_err(|e| format!("{}: {e} (run `kairos oracle trace {name}`)", path.display()))?;
    let text = rusty_zstd::decompress(&packed).map_err(|e| format!("rusty_zstd: {e}"))?;
    let mut out = std::io::stdout().lock();
    out.write_all(&text)?;
    out.flush()?;
    Ok(())
}

// ------------------------------------------------------------------- main --

pub(crate) fn main(root: &Path, args: &[String]) -> Result<()> {
    let verb = args.first().map(String::as_str).unwrap_or("");
    match verb {
        "fetch" => fetch(root),
        "patch" => patch(root),
        "build" => {
            let name = args.get(1).map(String::as_str).unwrap_or("dynamic");
            build(root, name)
        }
        "cat" => {
            let name = args.get(1).map_or("dynamic", String::as_str);
            cat(root, name)
        }
        "trace" => {
            let name = args
                .get(1)
                .filter(|a| !a.starts_with("--"))
                .map_or("dynamic", String::as_str);
            let ticks = option(args, "--ticks")
                .map(|t| t.parse::<u64>())
                .transpose()
                .map_err(|e| format!("--ticks: {e}"))?
                .unwrap_or(2000);
            if has_flag(args, "--all") {
                for sc in &SCENARIOS {
                    trace(root, sc.name, ticks)?;
                }
                Ok(())
            } else {
                trace(root, name, ticks)
            }
        }
        _ => fail(
            "oracle needs one of: fetch | patch | build [SCENARIO] | trace [SCENARIO] [--ticks N] [--all] | cat [SCENARIO]",
        ),
    }
}
