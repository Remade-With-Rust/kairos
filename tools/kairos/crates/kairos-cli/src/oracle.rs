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

/// The K7 libraries, pinned in `ORACLES.md` under `FreeRTOS/FreeRTOS-LTS`
/// at `0b25dc50bae4cb971c7a459b109e52ab2f01a6b8`.
///
/// Each is its own repository rather than a directory of the LTS manifest,
/// which is why they are cloned one at a time and pinned one at a time. They
/// are fetched only when asked for — `fetch` takes the kernel and the classic
/// distribution always, because the conformance corpus needs them on every
/// run, and a K7 library is needed by one package.
struct Library {
    /// The directory under `oracle/`, and the name printed.
    name: &'static str,
    url: &'static str,
    tag: &'static str,
    sha: &'static str,
    /// Which Kairos package this is the oracle for.
    package: &'static str,
}

const LIBRARIES: [Library; 6] = [
    Library {
        name: "backoffAlgorithm",
        url: "https://github.com/FreeRTOS/backoffAlgorithm.git",
        tag: "v1.4.2",
        sha: "14f4c88b33dd554be30a00a312c88d3986d457d0",
        package: "rusty_rtos_backoff",
    },
    Library {
        name: "coreJSON",
        url: "https://github.com/FreeRTOS/coreJSON.git",
        tag: "v3.3.1",
        sha: "cffa492da18c890181d64462f8af63992a69d3b0",
        package: "rusty_rtos_json",
    },
    Library {
        name: "coreSNTP",
        url: "https://github.com/FreeRTOS/coreSNTP.git",
        tag: "v2.0.0",
        sha: "50f5f96f4c33b14c0358f404ff4ff2a29d422ad7",
        package: "rusty_rtos_sntp",
    },
    Library {
        name: "coreMQTT",
        url: "https://github.com/FreeRTOS/coreMQTT.git",
        tag: "v5.0.2",
        sha: "04845c6a8e5f9cf2d232f1c6e80baeb81302e690",
        package: "rusty_rtos_mqtt",
    },
    Library {
        name: "coreHTTP",
        url: "https://github.com/FreeRTOS/coreHTTP.git",
        tag: "v3.1.3",
        sha: "3c4a5838658cd6d0ff8fb7c3a14e30baafcbcd28",
        package: "rusty_rtos_http",
    },
    Library {
        name: "FreeRTOS-Plus-TCP",
        url: "https://github.com/FreeRTOS/FreeRTOS-Plus-TCP.git",
        tag: "V4.4.1",
        sha: "c12361095aca68aeed858f45d14395fbffa92c0d",
        package: "rusty_rtos_tcp",
    },
];

/// Clone one K7 library at its pin, or verify the clone that is there.
fn fetch_library(root: &Path, library: &Library) -> Result<()> {
    let at = root.join("oracle").join(library.name);
    if at.join(".git").exists() {
        match head_sha(&at).as_deref() {
            Some(sha) if sha == library.sha => {
                println!(
                    "{}: {} = {sha} (pinned, for {})",
                    library.name, library.tag, library.package
                );
                return Ok(());
            }
            Some(sha) => {
                return fail(format!(
                    "{} is at {sha}, not the pinned {} ({}); delete oracle/{} and re-fetch, or re-pin in ORACLES.md with a decision-log row",
                    library.name, library.sha, library.tag, library.name
                ));
            }
            None => return fail(format!("{}: cloned but has no HEAD", library.name)),
        }
    }

    println!(
        "{}: cloning {} for {}",
        library.name, library.tag, library.package
    );
    git(
        &root.join("oracle"),
        &[
            "clone",
            "--depth",
            "1",
            "--branch",
            library.tag,
            library.url,
            library.name,
        ],
    )?;
    match head_sha(&at).as_deref() {
        Some(sha) if sha == library.sha => {
            println!("{}: {} = {sha} (pinned)", library.name, library.tag);
            Ok(())
        }
        Some(sha) => fail(format!(
            "{} cloned at {sha}, not the pinned {} ({}) — the tag has moved; re-pin in ORACLES.md with a decision-log row",
            library.name, library.sha, library.tag
        )),
        None => fail(format!("{}: cloned but has no HEAD", library.name)),
    }
}

/// A demo scenario: its name, the standard demo files it needs, and
/// whether it has to come out of the second binary.
struct Scenario {
    name: &'static str,
    demo_files: &'static [&'static str],
    /// `MessageBufferAMP` replaces `sbSEND_COMPLETED`, which is a global
    /// macro: with it defined every other stream-buffer scenario behaves
    /// differently, so the AMP demo gets a binary of its own.
    amp: bool,
}

const SCENARIOS: [Scenario; 19] = [
    Scenario {
        name: "death",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/death.c"],
        amp: false,
    },
    Scenario {
        name: "AbortDelay",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/AbortDelay.c"],
        amp: false,
    },
    Scenario {
        name: "TaskNotify",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/TaskNotify.c"],
        amp: false,
    },
    Scenario {
        name: "dynamic",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/dynamic.c"],
        amp: false,
    },
    Scenario {
        name: "PollQ",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/PollQ.c"],
        amp: false,
    },
    Scenario {
        name: "BlockQ",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/BlockQ.c"],
        amp: false,
    },
    Scenario {
        name: "semtest",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/semtest.c"],
        amp: false,
    },
    Scenario {
        name: "countsem",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/countsem.c"],
        amp: false,
    },
    Scenario {
        name: "recmutex",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/recmutex.c"],
        amp: false,
    },
    Scenario {
        name: "blocktim",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/blocktim.c"],
        amp: false,
    },
    Scenario {
        name: "QPeek",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/QPeek.c"],
        amp: false,
    },
    Scenario {
        name: "GenQTest",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/GenQTest.c"],
        amp: false,
    },
    Scenario {
        name: "QueueOverwrite",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/QueueOverwrite.c"],
        amp: false,
    },
    Scenario {
        name: "QueueSetPolling",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/QueueSetPolling.c"],
        amp: false,
    },
    Scenario {
        name: "IntSemTest",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/IntSemTest.c"],
        amp: false,
    },
    Scenario {
        name: "StreamBufferInterrupt",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/StreamBufferInterrupt.c"],
        amp: false,
    },
    Scenario {
        name: "TimerDemo",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/TimerDemo.c"],
        amp: false,
    },
    Scenario {
        name: "EventGroupsDemo",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/EventGroupsDemo.c"],
        amp: false,
    },
    Scenario {
        name: "MessageBufferAMP",
        demo_files: &["FreeRTOS/Demo/Common/Minimal/MessageBufferAMP.c"],
        amp: true,
    },
];

/// One binary serves the whole corpus: the harness's table names every
/// scenario, so building per scenario would compile the kernel sixteen
/// times for sixteen identical binaries.
const ORACLE_BIN: &str = "corpus";

/// The second binary, built with `-DKAIROS_AMP=1`. See [`Scenario::amp`].
const ORACLE_BIN_AMP: &str = "corpus-amp";

/// Which binary a scenario has to be run from.
fn binary_for(name: &str) -> &'static str {
    if SCENARIOS.iter().any(|s| s.name == name && s.amp) {
        ORACLE_BIN_AMP
    } else {
        ORACLE_BIN
    }
}

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

/// The notification demo, whose PRNG seed has to be pinned. See
/// [`patch_task_notify`].
fn task_notify_c(root: &Path) -> PathBuf {
    classic_dir(root)
        .join("FreeRTOS")
        .join("Demo")
        .join("Common")
        .join("Minimal")
        .join("TaskNotify.c")
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

fn fetch(root: &Path, args: &[String]) -> Result<()> {
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

    // The K7 libraries, on request. `--libs` takes all six; `--lib <name>`
    // takes one, which is what a package working on itself wants -- cloning
    // FreeRTOS-Plus-TCP to write a backoff differential is a slow way to
    // prove nothing.
    let one = option(args, "--lib");
    let all = has_flag(args, "--libs");
    if all || one.is_some() {
        let mut matched = 0usize;
        for library in &LIBRARIES {
            if let Some(ref name) = one {
                if name != library.name {
                    continue;
                }
            }
            matched = matched.saturating_add(1);
            fetch_library(root, library)?;
        }
        if matched == 0 {
            let names: Vec<&str> = LIBRARIES.iter().map(|l| l.name).collect();
            return fail(format!(
                "no K7 library named {:?}; ORACLES.md pins {}",
                one.unwrap_or_default(),
                names.join(", ")
            ));
        }
    }
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
    patch_task_notify(root)
}

/// Pin `TaskNotify.c`'s PRNG seed, which upstream takes from a function
/// ADDRESS.
///
/// `vStartTaskNotifyTask` does `uxNextRand = ( uint32_t ) prvRand`, and
/// `prvRand()` then chooses TIMER PERIODS (TaskNotify.c:557 and :570), which
/// decide when the daemon wakes, which is all over the trace. ASLR varies
/// that address per run, so the demo does not reproduce ITSELF:
///
/// ```text
///   3 runs, ASLR on, as vendored   3,421 / 3,336 / 3,530 lines
///   3 runs, setarch -R             identical, 3,530
///   3 runs, ASLR on, seeded        identical, 3,519
/// ```
///
/// A differential needs BOTH arms to reproduce, and no Rust port can know a
/// C function's load address, so the scenario is unusable until this is
/// pinned. That is the real reason `TaskNotify` is one of the two K1
/// scenarios never built -- the ledger records only that registering it
/// exposed a use-after-free in the oracle's tracing, and somebody picking it
/// up would write the whole port before finding out.
///
/// The constant changes nothing the demo tests: the value only chooses which
/// periods the notifying timer runs at.
///
/// The middle row above is worth keeping for its own sake -- it says the C
/// oracle is deterministic on real pthreads, which is the assumption the
/// whole differential rests on.
fn patch_task_notify(root: &Path) -> Result<()> {
    let path = task_notify_c(root);
    if !path.is_file() {
        return fail(format!(
            "{}: not found (run `kairos oracle fetch` first)",
            path.display()
        ));
    }
    // From the pinned file every time, as `patch` does for port.c.
    git(
        &classic_dir(root),
        &["checkout", "--", "FreeRTOS/Demo/Common/Minimal/TaskNotify.c"],
    )?;
    let text = fs::read_to_string(&path)?;
    if text.contains(MARKER) {
        return fail(
            "TaskNotify.c still carries KAIROS edits after `git checkout`; the checkout is not the pinned tree",
        );
    }
    let crlf = text.contains("\r\n");
    let mut out = if crlf {
        text.replace("\r\n", "\n")
    } else {
        text
    };
    const FROM: &str = "    uxNextRand = ( uint32_t ) prvRand;";
    const TO: &str = r"    /* KAIROS: upstream seeds from the ADDRESS of prvRand, which ASLR
     * varies per run, and the values it makes become timer periods -- so the
     * demo does not reproduce itself and cannot be diffed against anything.
     * A constant changes nothing the demo tests. */
    uxNextRand = ( uint32_t ) 0x0dc0ffeeUL;";
    let n = out.matches(FROM).count();
    if n != 1 {
        return fail(format!(
            "TaskNotify.c patch \"pin the PRNG seed\": anchor matched {n} times, expected exactly 1 — the pinned TaskNotify.c has changed; re-derive the anchor"
        ));
    }
    out = out.replacen(FROM, TO, 1);
    if crlf {
        out = out.replace('\n', "\r\n");
    }
    fs::write(&path, out)?;
    println!("{}: patched (pin the PRNG seed)", path.display());
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

/// One `cc` line, and what it produced.
fn compile(
    root: &Path,
    build_dir: &Path,
    sources: &[String],
    includes: &[&str],
    bin_name: &str,
    extra: &str,
    serves: usize,
) -> Result<()> {
    let mut cmd = String::from("cc -O0 -g -Wall -Wno-unused-parameter -DprojCOVERAGE_TEST=0");
    cmd.push_str(extra);
    for inc in includes {
        cmd.push_str(&format!(" -I{inc}"));
    }
    for s in sources {
        cmd.push_str(&format!(" {s}"));
    }
    cmd.push_str(&format!(" -o oracle/build/{bin_name} -pthread"));
    println!("$ {cmd}");
    let (ok, stdout, stderr) = host_shell(root, &cmd)?;
    print!("{stdout}");
    if !ok {
        return fail(format!("the oracle build failed:\n{stderr}"));
    }
    if !stderr.trim().is_empty() {
        println!("{}", stderr.trim_end());
    }
    let size = fs::metadata(build_dir.join(bin_name))
        .map(|m| m.len())
        .unwrap_or(0);
    println!("built oracle/build/{bin_name} ({size} bytes), serving {serves} scenario(s)");
    Ok(())
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
    // Two binaries, identical but for one `-D`: see `Scenario::amp`.
    let plain = SCENARIOS.iter().filter(|s| !s.amp).count();
    let amp = SCENARIOS.len().saturating_sub(plain);
    compile(root, &build_dir, &sources, &includes, ORACLE_BIN, "", plain)?;
    if amp > 0 {
        compile(
            root,
            &build_dir,
            &sources,
            &includes,
            ORACLE_BIN_AMP,
            " -DKAIROS_AMP=1",
            amp,
        )?;
    }
    Ok(())
}

// ------------------------------------------------------------------ trace --

fn trace(root: &Path, name: &str, ticks: u64) -> Result<()> {
    scenario(name)?;
    let bin_name = binary_for(name);
    let bin = root.join("oracle").join("build").join(bin_name);
    if !bin.is_file() {
        return fail(format!(
            "oracle/build/{bin_name} is not built; run `kairos oracle build`"
        ));
    }
    let traces = root.join("oracle").join("traces");
    fs::create_dir_all(&traces)?;
    let run = |suffix: &str| -> Result<(bool, String, usize)> {
        // `timeout` and `ulimit -f` (1 GiB) keep a scenario that never
        // reaches max_ticks from filling the disk.
        let script = format!(
            "ulimit -f 1048576; timeout 300 ./oracle/build/{bin_name} {name} {ticks} 2> oracle/traces/{name}.trace{suffix}; echo exit=$?"
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
        "fetch" => fetch(root, &args[1..]),
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
