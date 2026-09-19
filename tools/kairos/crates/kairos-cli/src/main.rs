//! `kairos` — the fleet tool for the Kairos umbrella folder.
//!
//! Every package under the umbrella is an **independent** git repository and
//! cargo workspace. This binary is the one place that knows the fleet as a
//! whole. It reads `KAIROS.toml` and offers these verbs:
//!
//! * `status`  — what exists on disk, what is a git repo, what has a remote.
//! * `check`   — compile-gate every package: host workspace + each no_std crate
//!   on each listed bare-metal target (+ clippy, tests, deny, the hardening
//!   tables, and the Xtensa rungs on request).
//! * `new`     — stamp a new package directory from a template and register it.
//! * `patches` — write each package's umbrella-only `.cargo/config.toml` so the
//!   siblings it uses resolve to the local checkouts (replaces Janus's
//!   `gen-sibling-patches.py`).
//! * `harden`  — render the hardening-status block into each README from its
//!   `docs/plans/use-protection-please.md` (replaces the skill's
//!   `render_readme_table.py`; same block, byte for byte on a re-run).
//! * `deploy`  — commit, create the GitHub repo (public or private, always
//!   explicit) and push exactly one package.
//! * `secrets` — fan the sibling-fetch token out to every package repository
//!   as its Actions secret (stdin to `gh`, never a command line).
//!
//! It shells out to `cargo`, `git` and `gh`; it never reimplements them.
//! Copied from Janus's `tools/janus` (2026-09-09) and grown; generalising the
//! two into one binary is an upstream item in the mission plan.

mod conform;
mod harden;
mod oracle;
mod patches;
mod power;

use serde::{Deserialize, Serialize};
use std::{
    env, fmt, fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, ExitCode, Stdio},
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

#[global_allocator]
static ALLOC: kairos_alloc::Alloc = kairos_alloc::Alloc;

const MANIFEST: &str = "KAIROS.toml";

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug)]
struct Fail(String);

impl fmt::Display for Fail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Fail {}

fn fail<T>(msg: impl Into<String>) -> Result<T> {
    Err(Box::new(Fail(msg.into())))
}

// ----------------------------------------------------------------- manifest --

#[derive(Debug, Deserialize)]
pub(crate) struct Manifest {
    kairos: Fleet,
    #[serde(default, rename = "package")]
    packages: Vec<Package>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Fleet {
    org: String,
    #[serde(default = "default_branch")]
    default_branch: String,
    /// Intended visibility of the umbrella repository itself.
    #[serde(default = "default_visibility")]
    visibility: String,
    /// The umbrella repository's name on GitHub (default: this folder's
    /// name). Janus's umbrella is `janus`; Kairos's is `kairos`, whatever the
    /// local folder is called.
    #[serde(default)]
    umbrella: Option<String>,
    /// The Actions secret and environment variable carrying the sibling-fetch
    /// token.
    #[serde(default = "default_token")]
    token: String,
}

fn default_branch() -> String {
    "main".to_owned()
}

fn default_visibility() -> String {
    "private".to_owned()
}

fn default_token() -> String {
    "KAIROS_GIT_TOKEN".to_owned()
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Package {
    name: String,
    kind: String,
    status: String,
    visibility: String,
    #[serde(default)]
    description: String,
    /// The crates this package publishes, each at `crates/<crate>`. Empty
    /// means the one crate named like the package.
    #[serde(default)]
    crates: Vec<String>,
    #[serde(default)]
    no_std_crates: Vec<String>,
    /// The crates carrying `#[cfg(kani)]` proof harnesses, checked by
    /// `check --kani`. They need their own gate because nothing else
    /// compiles them: `cfg(kani)` is set by the model checker, so a
    /// harness can stop compiling and every other gate stays green.
    #[serde(default)]
    proofs: Vec<String>,
    /// Crates that must allocate NOTHING, checked on the linked artifact.
    ///
    /// The obvious check — "it compiles with `--no-default-features`" —
    /// does NOT work, and was believed for a while. `extern crate alloc`
    /// resolves from the sysroot on a bare-metal target whatever the
    /// feature flags say, so a `Box::new` added to the kernel compiled
    /// clean through every one of the eight `no_std` rungs. A claim about
    /// allocation has to be read off the object file.
    #[serde(default)]
    no_alloc_crates: Vec<String>,
    /// `--cfg` flags the no_std rungs are checked under (as `RUSTFLAGS`), for
    /// crates whose `no_std` build must be opted into: rusty_alloc refuses to
    /// build without `ra_single_threaded` and allocates nothing without
    /// `ra_small_profile`, by design. A firmware sets the same flags.
    #[serde(default)]
    cfgs: Vec<String>,
    #[serde(default)]
    targets: Vec<String>,
    /// Per-crate overrides of `targets`, for a crate that is not portable
    /// across the package's whole list. An architecture port is the case:
    /// `rusty_rtos_port-cortex-m` is ARMv7-M assembly and cannot build for
    /// RISC-V, so gating it on the package's RISC-V rungs would be asking
    /// it to be something it is not. Its own QEMU cell gates it instead.
    #[serde(default)]
    crate_targets: std::collections::BTreeMap<String, Vec<String>>,
    /// Sibling packages this package's graph reaches: `"pkg"` for every crate
    /// of the sibling, `"pkg/crate"` for one crate of it. Keep it honest: a
    /// sibling listed here but absent from the graph is a dead override row
    /// (`patches` reads the manifests and skips crates the graph never names).
    #[serde(default)]
    uses: Vec<String>,
    #[serde(default)]
    plan: Option<String>,
}

impl Package {
    /// The crate names, defaulting to the package name.
    pub(crate) fn crate_names(&self) -> Vec<String> {
        if self.crates.is_empty() {
            vec![self.name.clone()]
        } else {
            self.crates.clone()
        }
    }
}

/// Walk upward from the cwd until a `KAIROS.toml` is found.
fn find_root() -> Result<PathBuf> {
    let mut dir = env::current_dir()?;
    loop {
        if dir.join(MANIFEST).is_file() {
            return Ok(dir);
        }
        if !dir.pop() {
            return fail(format!(
                "no {MANIFEST} found in this directory or any parent"
            ));
        }
    }
}

fn load_manifest(root: &Path) -> Result<Manifest> {
    let text = fs::read_to_string(root.join(MANIFEST))?;
    let manifest: Manifest = toml::from_str(&text)?;
    if manifest.kairos.org.trim().is_empty() {
        return fail(format!("{MANIFEST}: [kairos].org must not be empty"));
    }
    for package in &manifest.packages {
        validate_name(&package.name)?;
        match package.visibility.as_str() {
            "public" | "private" => {}
            other => {
                return fail(format!(
                    "{MANIFEST}: package {} has visibility {other:?}; expected public or private",
                    package.name
                ));
            }
        }
        for used in &package.uses {
            let sibling = used.split('/').next().unwrap_or_default();
            if !manifest.packages.iter().any(|p| p.name == sibling) {
                return fail(format!(
                    "{MANIFEST}: package {} uses {used:?} but no package {sibling:?} is listed",
                    package.name
                ));
            }
        }
    }
    Ok(manifest)
}

/// Package names become directory names, crate names and GitHub repo names, so
/// they are kept to the intersection of what all three accept.
fn validate_name(name: &str) -> Result<()> {
    let ok = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && name.chars().next().is_some_and(|c| c.is_ascii_alphabetic());
    if ok {
        Ok(())
    } else {
        fail(format!(
            "invalid package name {name:?}: use [a-z][a-z0-9_-]* (letters, digits, _ and -)"
        ))
    }
}

fn find_package<'a>(manifest: &'a Manifest, name: &str) -> Result<&'a Package> {
    manifest
        .packages
        .iter()
        .find(|p| p.name == name)
        .map_or_else(
            || fail(format!("package {name:?} is not listed in {MANIFEST}")),
            Ok,
        )
}

// ------------------------------------------------------------------ process --

/// Run a command, echoing it first. In `dry` mode only the echo happens.
fn run(dry: bool, cwd: &Path, program: &str, args: &[&str]) -> Result<String> {
    run_env(dry, cwd, program, args, &[])
}

/// `run`, with extra environment variables for the child (printed too, so the
/// log shows exactly what was executed).
fn run_env(
    dry: bool,
    cwd: &Path,
    program: &str,
    args: &[&str],
    env: &[(&str, String)],
) -> Result<String> {
    let prefix: String = env.iter().map(|(k, v)| format!("{k}=\"{v}\" ")).collect();
    println!("$ {prefix}{program} {}", args.join(" "));
    if dry {
        return Ok(String::new());
    }
    let output = Command::new(program)
        .args(args)
        .envs(env.iter().map(|(k, v)| (*k, v.as_str())))
        .current_dir(cwd)
        .output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    if output.status.success() {
        if !stdout.trim().is_empty() {
            println!("{}", stdout.trim_end());
        }
        Ok(stdout)
    } else {
        fail(format!(
            "`{program} {}` failed with {}\n{stdout}{stderr}",
            args.join(" "),
            output.status
        ))
    }
}

/// The package's `Cargo.lock`, if it is the one a **standalone clone**
/// resolves — every sibling still carrying its `source = "git+..."` line.
///
/// Inside the umbrella every package patches its siblings to local paths
/// (`kairos patches`), and a `[patch]` rewrites the lockfile: the patched
/// crate loses its git source. That lockfile is useless to anyone else, and
/// `Cargo.lock` is committed precisely so a fresh clone reproduces the
/// build (hardening gate H-07). So `check` reads the good one before it
/// runs cargo and puts it back afterwards
/// ([`restore_standalone_lockfile`]) — the working tree is always left in
/// the state that should be committed.
fn standalone_lockfile(dir: &Path, package: &Package, manifest: &Manifest) -> Option<String> {
    let _ = manifest;
    if package.uses.is_empty() {
        return None;
    }
    let text = fs::read_to_string(dir.join("Cargo.lock")).ok()?;
    // A standalone lockfile is one a FRESH CLONE can use, and the tell is a
    // sibling entry with no `source`.
    //
    // This used to look for `git+https://github.com/<org>/<sibling>`, which was
    // right while the siblings were git dependencies. They are crates.io
    // versions now -- `cargo publish` refuses a git dependency -- so that
    // string is never present and the check could only ever fail. The property
    // is unchanged; only its signature is.
    //
    // Under the umbrella's `[patch.crates-io]` a sibling is a path override,
    // and cargo records a path dependency as a `[[package]]` with NO `source`
    // line. A registry dependency always has one.
    if lockfile_has_pathed_sibling(&text, dir) {
        return None;
    }
    Some(text)
}

/// Does any `rusty_rtos*` entry in this lockfile lack a `source`?
///
/// Entries for the packages the lockfile's own workspace CONTAINS legitimately
/// have no source, so only siblings count -- and a sibling is any
/// `rusty_rtos*` package whose directory is not inside this repo. The cheap
/// form of that test: the workspace's own crates are listed in its
/// `[workspace] members`, and every other `rusty_rtos*` entry came from
/// outside.
fn lockfile_has_pathed_sibling(text: &str, dir: &Path) -> bool {
    let mut local = Vec::new();
    for block in text.split("[[package]]").skip(1) {
        let Some(name) = toml_string_field(block, "name") else {
            continue;
        };
        if !name.starts_with("rusty_rtos") {
            continue;
        }
        if block
            .lines()
            .any(|l| l.trim_start().starts_with("source = "))
        {
            continue;
        }
        local.push(name);
    }
    // Everything sourceless is either this workspace's own crate or a patched
    // sibling. A workspace crate has a `dependencies` entry naming it from
    // within, which is not worth parsing: compare against the crates on disk.
    local
        .iter()
        .any(|name| !dir.join("crates").join(name).join("Cargo.toml").is_file())
}

/// The value of `key = "..."` in a lockfile block.
fn toml_string_field(block: &str, key: &str) -> Option<String> {
    block.lines().find_map(|line| {
        let rest = line
            .trim()
            .strip_prefix(key)?
            .trim_start()
            .strip_prefix('=')?;
        Some(rest.trim().trim_matches('"').to_owned())
    })
}

/// Put back the lockfile cargo rewrote, if it rewrote it.
fn restore_standalone_lockfile(dir: &Path, before: Option<&String>) {
    let Some(before) = before else {
        return;
    };
    let lock = dir.join("Cargo.lock");
    if fs::read_to_string(&lock).is_ok_and(|now| now == *before) {
        return;
    }
    if fs::write(&lock, before).is_ok() {
        println!("  restored the standalone Cargo.lock (the [patch] table had rewritten it)");
    }
}

/// Does `crates/<krate>/Cargo.toml` declare `feature` under `[features]`?
fn crate_has_feature(dir: &Path, krate: &str, feature: &str) -> bool {
    let manifest = dir.join("crates").join(krate).join("Cargo.toml");
    let Ok(text) = fs::read_to_string(manifest) else {
        return false;
    };
    let mut in_features = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with('[') {
            in_features = line == "[features]";
            continue;
        }
        if in_features
            && line
                .split_once('=')
                .is_some_and(|(k, _)| k.trim().trim_matches('"') == feature)
        {
            return true;
        }
    }
    false
}

/// Run a command quietly and report only whether it succeeded, with its stdout.
fn probe(cwd: &Path, program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

// ------------------------------------------------------------------- status --

fn git_toplevel(dir: &Path) -> Option<PathBuf> {
    probe(dir, "git", &["rev-parse", "--show-toplevel"]).map(PathBuf::from)
}

/// True when `dir` is itself the root of a git work tree (not merely inside a
/// parent's repository).
fn is_own_repo(dir: &Path) -> bool {
    match (git_toplevel(dir), dir.canonicalize()) {
        (Some(top), Ok(here)) => {
            let top = top.canonicalize().unwrap_or(top);
            same_path(&top, &here)
        }
        _ => false,
    }
}

/// Compare two paths after normalising the Windows verbatim prefix and slashes.
fn same_path(a: &Path, b: &Path) -> bool {
    fn norm(p: &Path) -> String {
        let s = p.to_string_lossy();
        let s = s.strip_prefix(r"\\?\").unwrap_or(&s);
        s.replace('\\', "/").to_lowercase()
    }
    norm(a) == norm(b)
}

/// The conclusion of the latest GitHub Actions run of `org/name`, through
/// `gh`: `success`, `failure`, `in_progress`, `-` when there is none or `gh`
/// cannot see the repository.
fn ci_conclusion(org: &str, name: &str) -> String {
    let repo = format!("{org}/{name}");
    probe(
        Path::new("."),
        "gh",
        &[
            "run",
            "list",
            "--repo",
            &repo,
            "--limit",
            "1",
            "--json",
            "status,conclusion",
            "-q",
            ".[0] | (.conclusion // .status // \"-\")",
        ],
    )
    .filter(|s| !s.is_empty())
    .unwrap_or_else(|| "-".into())
}

fn status(root: &Path, manifest: &Manifest, args: &[String]) -> Result<()> {
    let with_ci = has_flag(args, "--ci");
    let as_json = has_flag(args, "--json");
    let mut rows = Vec::new();
    for package in &manifest.packages {
        let dir = root.join(&package.name);
        let exists = dir.join("Cargo.toml").is_file();
        let git = exists && is_own_repo(&dir);
        let clean =
            git && probe(&dir, "git", &["status", "--porcelain"]).is_some_and(|s| s.is_empty());
        let remote = if git {
            probe(&dir, "git", &["remote", "get-url", "origin"]).unwrap_or_else(|| "-".into())
        } else {
            "-".into()
        };
        let plan = package
            .plan
            .as_deref()
            .is_some_and(|p| dir.join(p).is_file() || root.join(p).is_file());
        let ci = if with_ci && git {
            Some(ci_conclusion(&manifest.kairos.org, &package.name))
        } else if with_ci {
            Some("-".to_owned())
        } else {
            None
        };
        rows.push(StatusRow {
            name: package.name.clone(),
            kind: package.kind.clone(),
            status: package.status.clone(),
            visibility: package.visibility.clone(),
            dir: exists,
            git,
            clean,
            plan,
            ci,
            remote,
        });
    }
    if as_json {
        let text = serde_json::to_string_pretty(&rows).map_err(|e| Fail(format!("json: {e}")))?;
        println!("{text}");
        return Ok(());
    }
    println!(
        "fleet: {} (branch {})\n",
        manifest.kairos.org, manifest.kairos.default_branch
    );
    println!(
        "{:<22} {:<11} {:<10} {:<8} {:<4} {:<4} {:<6} {:<5} {}remote",
        "package",
        "kind",
        "status",
        "vis",
        "dir",
        "git",
        "clean",
        "plan",
        if with_ci { "ci           " } else { "" }
    );
    for row in &rows {
        let ci = match &row.ci {
            Some(c) => format!("{c:<13}"),
            None => String::new(),
        };
        println!(
            "{:<22} {:<11} {:<10} {:<8} {:<4} {:<4} {:<6} {:<5} {}{}",
            row.name,
            row.kind,
            row.status,
            row.visibility,
            mark(row.dir),
            mark(row.git),
            mark(row.clean),
            mark(row.plan),
            ci,
            row.remote
        );
    }
    Ok(())
}

/// A status glyph from the house chrome crate (thoth): the same constant
/// every Remade UI prints, ASCII-safe in source.
fn mark(b: bool) -> &'static str {
    if b {
        thoth::status::OK
    } else {
        thoth::status::CROSS
    }
}

/// One row of `status`, the shape `--json` prints (the house JSON layer).
#[derive(Debug, Serialize)]
struct StatusRow {
    name: String,
    kind: String,
    status: String,
    visibility: String,
    dir: bool,
    git: bool,
    clean: bool,
    plan: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    ci: Option<String>,
    remote: String,
}

// -------------------------------------------------------------------- check --

/// The ESP32-S3's bare-metal target, checked by `check --xtensa`.
const XTENSA_S3: &str = "xtensa-esp32s3-none-elf";

/// The firmware cells under `<package>/firmware/` that need no hardware.
///
/// Discovered, not listed, for the reason the house-gate runner is:
/// a cell nobody runs is not a gate, and a hand-written list is how one
/// gets forgotten. The test is the cell's own runner — a `qemu-system-*`
/// one needs nothing but this box, while an `espflash` one wants a board
/// on a serial port and must never be started by a gate.
/// Firmware cells that need a BOARD on a serial port: their runner is
/// `espflash`.
///
/// Discovered the same way the emulator cells are — by reading the runner
/// out of the cell's own `.cargo/config.toml` — so this tool never keeps a
/// list that can fall behind the directory.
fn board_cells(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir.join("firmware")) else {
        return out;
    };
    for entry in entries.flatten() {
        let cell = entry.path();
        let config = cell.join(".cargo").join("config.toml");
        if let Ok(text) = fs::read_to_string(&config) {
            let runner = text
                .lines()
                .find(|l| l.trim_start().starts_with("runner"))
                .unwrap_or_default();
            if runner.contains("espflash") {
                let triple = text
                    .lines()
                    .find(|l| l.trim_start().starts_with("target"))
                    .and_then(|l| l.split('"').nth(1))
                    .unwrap_or("")
                    .to_owned();
                out.push((cell, triple));
            }
        }
    }
    out.sort();
    out
}

/// How long a board cell may run before it is judged to have hung.
/// How long a board cell may go SILENT before it is called hung.
///
/// Not how long it may take, and that distinction is the point: the gate's
/// own word is "hung", and a cell printing progress is not hung however long
/// it runs. `xiao-s3-signing` legitimately spends **196 seconds** in timed
/// batches -- it amortises 20,000 rounds because a 1 us clock cannot resolve
/// microseconds any other way -- and under a flat 300-second budget it passed
/// one run and missed the next. A flat budget prices the wrong thing.
///
/// A genuinely hung cell prints nothing, so it is still caught in two minutes
/// rather than five.
const BOARD_SILENCE: Duration = Duration::from_secs(120);

/// An absolute stop, so a cell that chatters for ever cannot hold the gate.
const BOARD_TIMEOUT: Duration = Duration::from_secs(900);

/// Build, flash and judge one board cell.
///
/// # Why the verdict is a printed LINE and not an exit code
///
/// An emulator cell ends in `debug::exit` and hands cargo the guest's
/// verdict, which is what lets `--qemu` gate. A board has no such channel:
/// the chip keeps running and the monitor never returns. So the contract
/// here is that a cell prints `RESULT: PASS` or `RESULT: FAIL`, and this
/// reads it.
///
/// **A cell that prints neither before the timeout FAILS.** That is not a
/// technicality — a hang is precisely how a broken context switch presents,
/// and a gate that treated silence as success would certify it.
fn run_board_cell(cell: &Path, triple: &str) -> Result<()> {
    let name = cell
        .file_name()
        .map_or_else(|| "?".to_string(), |n| n.to_string_lossy().into_owned());

    // The cell pins its own toolchain in `rust-toolchain.toml` -- `esp` for
    // Xtensa, which has no upstream rustc target -- and that pin must be
    // allowed to win.
    //
    // `RUSTUP_TOOLCHAIN` is REMOVED rather than set, and that is the whole
    // subtlety here: when this tool is itself run through `cargo run`, cargo
    // exports the toolchain that built it, the child cargo obeys the
    // environment over the file, and the cell is built with entirely the
    // wrong compiler. The symptom is a wall of "'esp32s3' is not a
    // recognized processor for this target", which reads like a broken
    // toolchain install and is not one. Removing the variable hands the
    // decision back to the cell, without this tool hardcoding a toolchain
    // name it would then have to keep in step.
    println!("$ cargo build --release   ({name}, toolchain from rust-toolchain.toml)");
    let status = Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(cell)
        .env_remove("RUSTUP_TOOLCHAIN")
        .status()?;
    if !status.success() {
        return fail(format!("board cell {name}: build failed with {status}"));
    }

    let elf = cell.join("target").join(triple).join("release").join(&name);
    if !elf.is_file() {
        return fail(format!(
            "board cell {name}: built, but no ELF at {}",
            elf.display()
        ));
    }

    // Two attempts, because the first can lose a race that has nothing to do
    // with the cell. These cells never exit -- they print a verdict and then
    // spin -- so the monitor has to be killed, and on Windows the COM port is
    // not always free by the time the next cell asks for it. The symptom is
    // `Failed to open serial port ... Access is denied`, and before this the
    // runner threw stderr away and reported it as "the cell HUNG ... a broken
    // context switch presents exactly this way" -- a confident diagnosis
    // pointing at the wrong thing. Three cells in a row produced two such
    // "hangs" that all passed when run singly.
    let mut last = None;
    for attempt in 1..=2 {
        let outcome = flash_and_watch(cell, &elf, &name)?;
        match outcome.verdict {
            Some(true) => return Ok(()),
            Some(false) => return fail(format!("board cell {name}: the cell reported FAIL")),
            None => {
                if attempt == 1 && outcome.port_busy() {
                    println!("  the serial port was busy; letting it settle and retrying once");
                    std::thread::sleep(Duration::from_secs(5));
                }
                last = Some(outcome);
                if attempt == 2 || !last.as_ref().is_some_and(BoardOutcome::port_busy) {
                    break;
                }
            }
        }
    }

    let outcome = last.unwrap_or_default();
    if outcome.port_busy() {
        // Say what actually happened. A gate that blames the code for an
        // environment fault teaches people to ignore the gate.
        return fail(format!(
            "board cell {name}: could not open the serial port, twice. This is NOT the \
             cell hanging -- espflash said: {}{}",
            outcome.stderr_tail(),
            outcome.timeline()
        ));
    }
    fail(format!(
        "board cell {name}: no RESULT line, and {}. The cell HUNG, which is a \
         failure and not a missing feature -- a broken context switch presents \
         exactly this way.{}{}",
        if outcome.went_quiet {
            format!("it went SILENT for {}s", BOARD_SILENCE.as_secs())
        } else {
            format!("it ran past the {}s hard stop", BOARD_TIMEOUT.as_secs())
        },
        outcome.stderr_note(),
        outcome.timeline()
    ))
}

/// What one flash-and-watch attempt saw.
#[derive(Default)]
struct BoardOutcome {
    /// `Some` once the cell printed `RESULT: PASS` or `RESULT: FAIL`.
    verdict: Option<bool>,
    /// Whatever espflash said on stderr. Kept because the reason a board cell
    /// produced no verdict is almost never visible on stdout.
    stderr: String,
    /// True when the cell stopped SAYING anything, rather than running long.
    /// The two are different failures and the message says which.
    went_quiet: bool,
    /// How many lines the monitor received, of any kind.
    lines: u64,
    /// When the first and last of them arrived, from the flash command.
    first_at: Option<Duration>,
    last_at: Option<Duration>,
    /// The longest silence between two consecutive lines, and when it began.
    ///
    /// This is the measurement the whole timeline exists for. "The cell
    /// produced two of eight rounds and then something for 900 seconds" is a
    /// description; "it went quiet for 412s starting at t=88s" is a fact that
    /// points at a cause.
    max_gap: Duration,
    max_gap_at: Duration,
    /// Every line, timestamped, written next to the cell's build output.
    transcript: Option<PathBuf>,
}

impl BoardOutcome {
    /// Did this fail because something else held the port, rather than
    /// because the cell misbehaved?
    fn port_busy(&self) -> bool {
        let e = self.stderr.to_ascii_lowercase();
        e.contains("access is denied")
            || e.contains("failed to open serial port")
            || e.contains("device or resource busy")
            || e.contains("permission denied")
    }

    fn stderr_tail(&self) -> String {
        self.stderr
            .lines()
            .rfind(|l| !l.trim().is_empty())
            .unwrap_or("(nothing)")
            .trim()
            .to_string()
    }

    /// Appended to a genuine-hang message, so even then the reader gets what
    /// espflash said rather than nothing.
    fn stderr_note(&self) -> String {
        if self.stderr.trim().is_empty() {
            String::new()
        } else {
            format!(" espflash said: {}", self.stderr_tail())
        }
    }

    /// What the monitor actually saw, in seconds.
    ///
    /// A failing board cell used to report only that it failed. The first
    /// question anyone then asks is "when did it stop, and what was it
    /// saying?", and answering it meant re-running by hand and hoping the
    /// flake recurred. This answers it from the run that failed.
    fn timeline(&self) -> String {
        let secs = |d: Duration| format!("{:.1}s", d.as_secs_f64());
        let mut out = format!("\n      timeline: {} line(s)", self.lines);
        if let (Some(first), Some(last)) = (self.first_at, self.last_at) {
            out.push_str(&format!(
                ", first at {}, last at {}",
                secs(first),
                secs(last)
            ));
        }
        if self.max_gap > Duration::from_secs(1) {
            out.push_str(&format!(
                "\n      longest silence: {} beginning at {}",
                secs(self.max_gap),
                secs(self.max_gap_at)
            ));
        }
        if let Some(path) = &self.transcript {
            out.push_str(&format!("\n      transcript: {}", path.display()));
        }
        out
    }
}

/// Flash the cell and watch its output until it reports or the deadline passes.
///
/// Every line is timestamped and kept, whether it matched anything or not.
/// That is the difference between "it emitted something for 900 seconds" and
/// knowing what, and when it stopped.
fn flash_and_watch(cell: &Path, elf: &Path, name: &str) -> Result<BoardOutcome> {
    println!("$ espflash flash --monitor   ({name})");
    let started = Instant::now();
    let mut child = Command::new("espflash")
        .args(["flash", "--monitor", "--non-interactive"])
        .arg(elf)
        .current_dir(cell)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let Some(stdout) = child.stdout.take() else {
        let _ = child.kill();
        return fail(format!("board cell {name}: no output from espflash"));
    };
    let stderr = child.stderr.take();

    let errors = Arc::new(Mutex::new(String::new()));
    if let Some(stderr) = stderr {
        let errors = Arc::clone(&errors);
        std::thread::spawn(move || {
            for line in BufReader::new(stderr)
                .lines()
                .map_while(std::result::Result::ok)
            {
                if let Ok(mut buf) = errors.lock() {
                    buf.push_str(&line);
                    buf.push('\n');
                }
            }
        });
    }

    // Timestamp in the READER, not in the consumer: the channel is fast but
    // the consumer wakes on a 2-second tick, and a gap measured there would
    // be the tick rather than the board.
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout)
            .lines()
            .map_while(std::result::Result::ok)
        {
            if tx.send((started.elapsed(), line)).is_err() {
                break;
            }
        }
    });

    let hard_stop = started + BOARD_TIMEOUT;
    let mut last_output = Instant::now();
    let mut verdict: Option<bool> = None;
    let mut transcript: Vec<(Duration, String)> = Vec::new();
    let mut first_at = None;
    let mut last_at = None;
    let mut max_gap = Duration::ZERO;
    let mut max_gap_at = Duration::ZERO;

    while Instant::now() < hard_stop && last_output.elapsed() < BOARD_SILENCE {
        match rx.recv_timeout(Duration::from_secs(2)) {
            Ok((at, line)) => {
                // Any output at all means the cell is alive. The deadline
                // measures SILENCE, not duration.
                last_output = Instant::now();
                if first_at.is_none() {
                    first_at = Some(at);
                }
                if let Some(previous) = last_at {
                    let gap = at.saturating_sub(previous);
                    if gap > max_gap {
                        max_gap = gap;
                        max_gap_at = previous;
                    }
                }
                last_at = Some(at);
                transcript.push((at, line.clone()));

                // The cell's own report, passed through: a reader wants the
                // counters, not just the verdict.
                if line.starts_with("RESULT:")
                    || line.contains(" ok    ")
                    || line.starts_with("SWITCH ")
                    || line.starts_with("SIGN ")
                {
                    println!("  {}", line.trim_end());
                }
                if line.contains("RESULT: PASS") {
                    verdict = Some(true);
                    break;
                }
                if line.contains("RESULT: FAIL") {
                    verdict = Some(false);
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                // espflash can exit early -- a port it cannot open, a board
                // that went away -- and then there is nothing to wait for.
                if verdict.is_none() && matches!(child.try_wait(), Ok(Some(_))) {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    let _ = child.kill();
    let _ = child.wait();
    // The port is not always free the instant the monitor dies.
    std::thread::sleep(Duration::from_millis(750));

    // Keep the transcript only when it is wanted: on a pass nobody reads it,
    // and writing one per cell per run would be litter.
    let path = cell.join("target").join(format!("kairos-board-{name}.log"));
    let transcript_path = if verdict == Some(true) {
        let _ = fs::remove_file(&path);
        None
    } else {
        let body: String = transcript
            .iter()
            .map(|(at, line)| format!("[{:>9.3}s] {line}\n", at.as_secs_f64()))
            .collect();
        fs::write(&path, body).ok().map(|()| path)
    };

    let stderr = errors.lock().map(|b| b.clone()).unwrap_or_default();
    let went_quiet = last_output.elapsed() >= BOARD_SILENCE;
    Ok(BoardOutcome {
        verdict,
        stderr,
        went_quiet,
        lines: transcript.len() as u64,
        first_at,
        last_at,
        max_gap,
        max_gap_at,
        transcript: transcript_path,
    })
}

fn qemu_cells(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir.join("firmware")) else {
        return out;
    };
    for entry in entries.flatten() {
        let cell = entry.path();
        let config = cell.join(".cargo").join("config.toml");
        if let Ok(text) = fs::read_to_string(&config) {
            // The runner names the emulator, so the cell says which one it
            // needs and this tool never has to keep a list.
            if let Some(program) = text
                .split(|c: char| c.is_whitespace() || c == '"')
                .find(|word| word.starts_with("qemu-system-"))
            {
                // The cell's `[build] target` is deliberately NOT read.
                // It used to be, to find the linked ELF for a
                // static-allocation check -- and that check was removed
                // because it could not fail (see the long note at the
                // caller). Carrying the triple for a consumer that no
                // longer exists is how a vestige becomes a warning.
                out.push((cell, program.to_owned()));
            }
        }
    }
    out.sort();
    out
}

/// Somewhere `program` can actually be executed from, or `None`.
///
/// A cell's runner names `qemu-system-arm` and nothing more, so the gate
/// inherits whatever `PATH` the operator's shell happened to have. That is
/// how a gate quietly becomes something that only works in one terminal:
/// both QEMU cells reported "the cell failed, exit 101" on a clean `PATH`
/// when the truth was that QEMU was not on it — a tooling failure wearing a
/// test failure's clothes, which is the most expensive kind.
///
/// So look for it. `KAIROS_QEMU_DIR` wins if set, then `PATH`, then the
/// places an installer puts it.
fn qemu_dir(program: &str) -> Option<PathBuf> {
    let exe = format!("{program}{}", std::env::consts::EXE_SUFFIX);
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(dir) = std::env::var("KAIROS_QEMU_DIR") {
        roots.push(PathBuf::from(dir));
    }
    if let Ok(path) = std::env::var("PATH") {
        roots.extend(std::env::split_paths(&path));
    }
    roots.extend(
        [
            r"C:\Program Files\qemu",
            r"C:\Program Files (x86)\qemu",
            "/usr/bin",
            "/usr/local/bin",
            "/opt/homebrew/bin",
        ]
        .iter()
        .map(PathBuf::from),
    );
    roots.into_iter().find(|dir| dir.join(&exe).is_file())
}

/// Does this rlib reference the Rust allocator at all?
///
/// `llvm-nm` prints an undefined `U` line per referenced symbol, so a crate
/// that never allocates has none. Poison-proven both ways: a `Box::new`
/// added to `rusty_rtos_kernel-core` turns 0 references into 2
/// (`__rust_alloc`, `__rust_alloc_zeroed`).
fn nm_mentions_allocator(rlib: &Path) -> Result<bool> {
    if !rlib.is_file() {
        return fail(format!("no rlib at {}", rlib.display()));
    }
    let out = Command::new("llvm-nm").arg(rlib).output();
    let out = match out {
        Ok(o) => o,
        // Missing tool and failing check are different states, and saying
        // so is the difference between "install LLVM" and an afternoon
        // reading a kernel diff.
        Err(_) => {
            return fail(
                "`llvm-nm` not found. It ships with LLVM and with the Rust                  `llvm-tools` component. The check did NOT run, so this is                  not a verdict about the code."
                    .to_string(),
            );
        }
    };
    let text = String::from_utf8_lossy(&out.stdout);
    Ok(text.contains("__rust_alloc") || text.contains("__rust_dealloc"))
}

/// Every cargo project directly under `tools/`.
fn tool_crates(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(root.join("tools")) else {
        return out;
    };
    for entry in entries.flatten() {
        let dir = entry.path();
        // A tool that ships its own runner owns its own pass/fail policy
        // and is not this gate's business. `house-gate` is the case that
        // forces it: its workspace deliberately contains an EXPECTED
        // failure (`gate-xml`, the category nothing in Kairos will ever
        // parse), so building its members from here would make `check`
        // fail for ever, on purpose, for the wrong reason.
        if dir.join("run.sh").is_file() {
            continue;
        }
        if dir.join("Cargo.toml").is_file() {
            out.push(dir);
        }
    }
    out.sort();
    out
}

fn check(root: &Path, manifest: &Manifest, args: &[String]) -> Result<()> {
    let with_tests = has_flag(args, "--test");
    let with_clippy = has_flag(args, "--clippy");
    let with_fmt = has_flag(args, "--fmt");
    let with_deny = has_flag(args, "--deny");
    let with_harden = has_flag(args, "--harden");
    let with_xtensa = has_flag(args, "--xtensa");
    let with_kani = has_flag(args, "--kani");
    let with_qemu = has_flag(args, "--qemu");
    let with_board = has_flag(args, "--board");
    let with_soak = has_flag(args, "--soak");
    let only: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let mut failures = Vec::new();
    let mut ran = 0usize;
    for package in &manifest.packages {
        if !only.is_empty() && !only.contains(&&package.name) {
            continue;
        }
        let dir = root.join(&package.name);
        if !dir.join("Cargo.toml").is_file() {
            if !only.is_empty() {
                failures.push(format!("{}: no Cargo.toml on disk", package.name));
            }
            continue;
        }
        println!("== {} ==", package.name);
        ran += 1;
        // Every cargo command below runs with the umbrella's `[patch]` table
        // in place, which rewrites `Cargo.lock`. Keep the standalone one.
        let lock_before = standalone_lockfile(&dir, package, manifest);
        if lock_before.is_none() && !package.uses.is_empty() && dir.join("Cargo.lock").is_file() {
            failures.push(format!(
                "{}: Cargo.lock is not the standalone one — it was written with the umbrella's [patch] table in place, so a fresh clone cannot use it (hardening gate H-07). Restore it: `mv .cargo/config.toml .cargo/config.toml.off && cargo generate-lockfile && mv .cargo/config.toml.off .cargo/config.toml`",
                package.name
            ));
        }
        if with_fmt {
            if let Err(e) = run(false, &dir, "cargo", &["fmt", "--all", "--check"]) {
                failures.push(format!("{}: fmt: {e}", package.name));
            }
        }
        if let Err(e) = run(false, &dir, "cargo", &["check", "--workspace"]) {
            failures.push(format!("{}: host workspace: {e}", package.name));
        }
        if with_clippy {
            let args = [
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ];
            if let Err(e) = run(false, &dir, "cargo", &args) {
                failures.push(format!("{}: clippy: {e}", package.name));
            }
        }
        if with_tests {
            if let Err(e) = run(false, &dir, "cargo", &["test", "--workspace"]) {
                failures.push(format!("{}: tests: {e}", package.name));
            }
        }
        if with_deny {
            if let Err(e) = run(false, &dir, "cargo", &["deny", "check"]) {
                failures.push(format!("{}: deny: {e}", package.name));
            }
        }
        if with_harden {
            let plan = dir.join("docs").join("plans").join(harden::PLAN_NAME);
            if plan.is_file() {
                if let Err(e) = harden::process(&plan, &dir.join("README.md"), true, "", "") {
                    failures.push(format!("{}: hardening table: {e}", package.name));
                }
            } else {
                failures.push(format!(
                    "{}: no docs/plans/{}",
                    package.name,
                    harden::PLAN_NAME
                ));
            }
        }
        // The feature ladder above `core`: each rung only where the crate
        // declares the feature (the allocator seam has `small-metal`, not
        // `alloc`).
        let rustflags: Vec<(&str, String)> = if package.cfgs.is_empty() {
            Vec::new()
        } else {
            let flags: Vec<String> = package.cfgs.iter().map(|c| format!("--cfg {c}")).collect();
            vec![("RUSTFLAGS", flags.join(" "))]
        };
        for krate in &package.no_std_crates {
            let rungs: Vec<Option<&str>> = std::iter::once(None)
                .chain(
                    ["alloc", "small-metal"]
                        .into_iter()
                        .filter(|f| crate_has_feature(&dir, krate, f))
                        .map(Some),
                )
                .collect();
            // A crate with an override is checked on its own list and
            // nothing else; without one it takes the package's.
            let targets = package.crate_targets.get(krate).unwrap_or(&package.targets);
            for target in targets {
                for extra in rungs.iter().copied() {
                    let mut args = vec!["check", "-p", krate, "--no-default-features"];
                    if let Some(feature) = extra {
                        args.extend(["--features", feature]);
                    }
                    args.extend(["--target", target]);
                    if let Err(e) = run_env(false, &dir, "cargo", &args, &rustflags) {
                        failures.push(format!(
                            "{}: {krate} on {target} (features {}): {e}",
                            package.name,
                            extra.unwrap_or("none")
                        ));
                    }
                }
            }
            restore_standalone_lockfile(&dir, lock_before.as_ref());
            if with_xtensa {
                // The S3's own bare-metal target, through the esp toolchain
                // (rustup's `+esp`), with `core` and `alloc` built from source:
                // the esp toolchain ships no prebuilt Xtensa sysroot. Local
                // only — CI runners have no esp toolchain.
                for extra in [None, Some("alloc")] {
                    let mut args = vec!["+esp", "check", "-p", krate, "--no-default-features"];
                    if let Some(feature) = extra {
                        args.extend(["--features", feature]);
                    }
                    args.extend(["--target", XTENSA_S3, "-Z", "build-std=core,alloc"]);
                    if let Err(e) = run(false, &dir, "cargo", &args) {
                        failures.push(format!(
                            "{}: {krate} on {XTENSA_S3} (features {}): {e}",
                            package.name,
                            extra.unwrap_or("none")
                        ));
                    }
                }
            }
        }
        if with_board {
            // A board cell is never started by the ordinary gate: it wants a
            // part on a serial port. This flag is how the hardware claims in
            // `docs/LEDGER.md` are re-run, rather than re-read.
            for (cell, triple) in board_cells(&dir) {
                if let Err(e) = run_board_cell(&cell, &triple) {
                    failures.push(format!("{}: {e}", package.name));
                }
            }
        }
        if with_qemu {
            for (cell, program) in qemu_cells(&dir) {
                let name = cell
                    .file_name()
                    .map_or_else(|| "?".to_string(), |n| n.to_string_lossy().into_owned());
                // Missing tool and failing cell are different states, and
                // saying so is the difference between "install QEMU" and an
                // afternoon reading a kernel diff.
                let Some(qemu) = qemu_dir(&program) else {
                    failures.push(format!(
                        "{}: qemu cell {name}: `{program}` not found. Install QEMU,                          put it on PATH, or set KAIROS_QEMU_DIR to the directory                          holding it. The cell did NOT run, so this is not a verdict                          about the code.",
                        package.name
                    ));
                    continue;
                };
                // Prepend rather than replace: the child still needs cargo,
                // rustc and the linker from the inherited PATH.
                let path = std::env::var("PATH").unwrap_or_default();
                let joined =
                    std::env::join_paths(std::iter::once(qemu).chain(std::env::split_paths(&path)))
                        .map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or(path);
                println!("$ cargo run --release   ({name}, {program})");
                // The cell's own `.cargo/config.toml` names the runner, so
                // this is `qemu-system-* -kernel <elf>` and nothing here has
                // to know which machine. A cell that ends by calling
                // `debug::exit` gives cargo the guest's verdict as an exit
                // code, which is the whole reason it can be a gate.
                if let Err(e) = run_env(
                    false,
                    &cell,
                    "cargo",
                    &["run", "--release"],
                    &[("PATH", joined)],
                ) {
                    failures.push(format!("{}: qemu cell {name}: {e}", package.name));
                    continue;
                }

                // K4's static-allocation clause is NOT checked here, and
                // the reason is worth more than the check was.
                //
                // `nm` on a linked, LTO'd bare-metal ELF cannot answer "does
                // this contain a heap". A cell was poisoned with a real
                // `#[global_allocator]` and a live `Box::new`, and the
                // binary still carried **zero** allocator symbols: the shim
                // is inlined away. The check reported "static allocation
                // only" against a cell that had a heap, which is a check
                // that cannot fail.
                //
                // What does hold, and is gated:
                //
                // * `no_alloc_crates` reads the same question off an RLIB,
                //   where the allocator appears as an UNDEFINED symbol and
                //   cannot be optimised out. Poison-proven both ways.
                // * A bare-metal binary that declares no global allocator
                //   cannot link an allocation at all — poisoning a cell with
                //   `Box::new` and no allocator fails the build, which the
                //   cell gate already catches.
                //
                // Together those are the guarantee; a symbol scan of the ELF
                // adds nothing to it.
            }
        }
        // A crate that claims to allocate nothing is checked on its
        // rlib, not on its source. This runs with the ordinary gate
        // rather than behind a flag: it is fast, and it is the kind of
        // property that rots silently.
        for krate in &package.no_alloc_crates {
            let Some(target) = package.targets.first() else {
                continue;
            };
            ran += 1;
            if let Err(e) = run(
                false,
                &dir,
                "cargo",
                &[
                    "build",
                    "-p",
                    krate,
                    "--no-default-features",
                    "--target",
                    target,
                ],
            ) {
                failures.push(format!("{}: no-alloc build {krate}: {e}", package.name));
                continue;
            }
            let rlib = dir
                .join("target")
                .join(target)
                .join("debug")
                .join(format!("lib{}.rlib", krate.replace('-', "_")));
            match nm_mentions_allocator(&rlib) {
                Err(e) => failures.push(format!("{}: no-alloc {krate}: {e}", package.name)),
                Ok(true) => failures.push(format!(
                    "{}: {krate} references the allocator (`__rust_alloc`) in                      {}. It is declared allocation-free.",
                    package.name,
                    rlib.display()
                )),
                Ok(false) => println!("  {krate}: allocates nothing (0 allocator symbols)"),
            }
        }
        if with_soak {
            // The long-run tests, which are `#[ignore]`d precisely so the
            // ordinary gate stays fast. A test that is ignored by default
            // and run by nothing is a test that has stopped existing, so
            // this flag is what keeps them alive.
            for krate in &package.no_std_crates {
                if !dir.join("crates").join(krate).join("tests").is_dir() {
                    continue;
                }
                ran += 1;
                if let Err(e) = run(
                    false,
                    &dir,
                    "cargo",
                    // `--tests` and not a bare `cargo test`: without it
                    // `--ignored` also reaches the DOCTEST harness, where
                    // it means "compile the ```ignore blocks too" — and
                    // those are marked ignore because they are
                    // illustrative fragments that were never meant to
                    // compile. The first run of this flag failed on one,
                    // which is a gate reporting a fault in its own
                    // invocation.
                    &[
                        "test",
                        "-p",
                        krate,
                        "--release",
                        "--tests",
                        "--",
                        "--ignored",
                        "--nocapture",
                    ],
                ) {
                    failures.push(format!("{}: soak: {e}", package.name));
                }
            }
        }
        if with_kani {
            for krate in &package.proofs {
                // Codegen only: this compiles every harness and verifies
                // none, which is the cheap half and the half that rots.
                // Its own target directory, because a Windows cargo and a
                // WSL cargo sharing one is a fight (and Kani is Linux-only).
                // `host_shell` runs a login shell, which already has cargo
                // on PATH. Do not set it here: WSL's interop PATH carries
                // the Windows one, which contains `Program Files (x86)`,
                // and an unquoted assignment splits on the parentheses.
                let script = format!(
                    "CARGO_TARGET_DIR=/tmp/kani-codegen cargo kani --only-codegen -p {krate}"
                );
                println!("$ cargo kani --only-codegen -p {krate}");
                match oracle::host_shell(&dir, &script) {
                    Ok((true, _, _)) => {}
                    Ok((false, out, err)) => {
                        let why = err
                            .lines()
                            .chain(out.lines())
                            .find(|l| l.starts_with("error"))
                            .unwrap_or("see the output above");
                        failures.push(format!(
                            "{}: the {krate} proof harnesses do not compile: {why}",
                            package.name
                        ));
                    }
                    Err(e) => failures.push(format!("{}: cargo kani: {e}", package.name)),
                }
            }
        }
        // Restore AGAIN, at the very end of the package.
        //
        // The earlier call is not enough, and that was a real defect: the
        // `--board` cells, the `--qemu` cells, the no-alloc rlib builds and
        // `--soak` all run cargo AFTER it, each with the umbrella's `[patch]`
        // in scope, and each rewrites the lockfile the first restore had just
        // put back. `standalone_lockfile`'s own doc promises the working tree
        // is always left in the state that should be committed; with those
        // flags it was not, so a `check --board` left a lockfile that the
        // NEXT `check` reported as an H-07 failure. A gate whose own run
        // breaks the next one teaches people to ignore it.
        //
        // Idempotent: it compares before writing, so the ordinary path pays
        // nothing for this.
        restore_standalone_lockfile(&dir, lock_before.as_ref());
    }
    // The umbrella's own tools, on a full run.
    //
    // Nothing built these. `house-gate/run.sh` discovers `gate-*/` inside
    // its own workspace and `check` walks the manifest's packages, so a
    // cargo project under `tools/` was covered by neither — which is
    // exactly how `gate-xml`'s `host/` and `hostgit/` went unbuilt for
    // weeks. Discovered off the filesystem rather than listed, for the
    // same reason: a hand-written list is how one gets forgotten.
    if only.is_empty() {
        for tool in tool_crates(root) {
            let name = tool
                .file_name()
                .map_or_else(|| "?".to_string(), |n| n.to_string_lossy().into_owned());
            ran += 1;
            // Name it: a bare `cargo check --all-targets` in the log says
            // nothing about which tool it checked, and two of them look
            // identical.
            println!("$ cargo check --all-targets   (tools/{name})");
            if let Err(e) = run(false, &tool, "cargo", &["check", "--all-targets"]) {
                failures.push(format!("tools/{name}: {e}"));
            }
        }
    }
    if ran == 0 {
        return fail("nothing to check: no listed package has a Cargo.toml on disk");
    }
    if failures.is_empty() {
        println!("\ncheck: {ran} package(s) passed");
        Ok(())
    } else {
        fail(format!(
            "check: {} failure(s)\n{}",
            failures.len(),
            failures.join("\n")
        ))
    }
}

// ---------------------------------------------------------------------- new --

/// The bare-metal targets a stamped package is held to in CI: the two
/// Cortex-M classes the house ships to (M4F/M7 and M33) and the two RISC-V
/// classes (ESP32-C6 and ESP32-P4).
const DEFAULT_TARGETS: [&str; 4] = [
    "thumbv7em-none-eabihf",
    "thumbv8m.main-none-eabihf",
    "riscv32imac-unknown-none-elf",
    "riscv32imafc-unknown-none-elf",
];

#[allow(clippy::too_many_arguments)]
fn scaffold(
    root: &Path,
    manifest: &Manifest,
    name: &str,
    kind: &str,
    description: &str,
    tier: &str,
    date: &str,
    dry: bool,
) -> Result<()> {
    validate_name(name)?;
    let dest = root.join(name);
    if dest.exists() {
        return fail(format!(
            "{} already exists; refusing to overwrite",
            dest.display()
        ));
    }
    let template = root
        .join("tools")
        .join("kairos")
        .join("templates")
        .join(kind);
    if !template.is_dir() {
        return fail(format!(
            "no template for kind {kind:?} at {}",
            template.display()
        ));
    }
    match tier {
        "critical-path" | "standard" | "utility" => {}
        other => {
            return fail(format!(
                "unknown tier {other:?}: critical-path, standard or utility"
            ));
        }
    }
    let description = if description.is_empty() {
        manifest
            .packages
            .iter()
            .find(|p| p.name == name)
            .map(|p| p.description.clone())
            .unwrap_or_default()
    } else {
        description.to_owned()
    };
    let subs = [
        ("__NAME__", name.to_owned()),
        ("__NAME_IDENT__", name.replace('-', "_")),
        ("__ORG__", manifest.kairos.org.clone()),
        ("__TOKEN__", manifest.kairos.token.clone()),
        ("__DESCRIPTION__", description.clone()),
        ("__TIER__", tier.to_owned()),
        ("__DATE__", date.to_owned()),
    ];
    println!("scaffold {kind} -> {}", dest.display());
    copy_template(&template, &dest, &subs, dry)?;
    if !dry {
        run(
            false,
            &dest,
            "git",
            &["init", "-b", &manifest.kairos.default_branch],
        )?;
    }
    if manifest.packages.iter().any(|p| p.name == name) {
        println!("{MANIFEST}: {name} already registered");
    } else {
        let targets = DEFAULT_TARGETS
            .iter()
            .map(|t| format!("\"{t}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let block = format!(
            "\n[[package]]\nname = \"{name}\"\nkind = \"{kind}\"\nstatus = \"scaffold\"\nvisibility = \"private\"\ndescription = \"{}\"\ncrates = [\"{name}\", \"{name}-core\"]\nno_std_crates = [\"{name}-core\"]\ntargets = [{targets}]\nuses = [\"rusty_rtos_core\"]\nplan = \"docs/plans/{name}.md\"\n",
            description.replace('"', "\\\"")
        );
        if dry {
            println!("would append to {MANIFEST}:{block}");
        } else {
            let path = root.join(MANIFEST);
            let mut text = fs::read_to_string(&path)?;
            text.push_str(&block);
            fs::write(&path, text)?;
            println!("{MANIFEST}: registered {name} (visibility private until you change it)");
        }
    }
    println!(
        "\nnext: edit {name}/README.md and {name}/docs/plans/{name}.md, then `kairos patches` and `kairos check {name}`"
    );
    Ok(())
}

fn copy_template(src: &Path, dest: &Path, subs: &[(&str, String)], dry: bool) -> Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_name = entry.file_name().to_string_lossy().into_owned();
        let target = dest.join(substitute(&file_name, subs));
        if entry.file_type()?.is_dir() {
            if !dry {
                fs::create_dir_all(&target)?;
            }
            copy_template(&entry.path(), &target, subs, dry)?;
        } else {
            let body = substitute(&fs::read_to_string(entry.path())?, subs);
            println!("  write {}", target.display());
            if !dry {
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&target, body)?;
            }
        }
    }
    Ok(())
}

fn substitute(text: &str, subs: &[(&str, String)]) -> String {
    subs.iter()
        .fold(text.to_owned(), |acc, (from, to)| acc.replace(from, to))
}

// ------------------------------------------------------------------- deploy --

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Visibility {
    Public,
    Private,
}

impl Visibility {
    fn flag(self) -> &'static str {
        match self {
            Self::Public => "--public",
            Self::Private => "--private",
        }
    }

    fn word(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
        }
    }
}

struct DeployOpts {
    visibility: Visibility,
    dry: bool,
    message: String,
    override_visibility: bool,
}

/// Deploy one package directory as its own GitHub repository.
fn deploy(root: &Path, manifest: &Manifest, name: &str, opts: &DeployOpts) -> Result<()> {
    let package = find_package(manifest, name)?;
    let dir = root.join(name);
    for required in ["Cargo.toml", "README.md"] {
        if !dir.join(required).is_file() {
            return fail(format!("{name}: missing {required}; not deployable"));
        }
    }
    let description = if package.description.is_empty() {
        format!("{name} — part of the Kairos family (FreeRTOS remade in Rust, Remade With Rust)")
    } else {
        package.description.clone()
    };
    publish(
        &dir,
        manifest,
        name,
        &package.visibility,
        &description,
        opts,
    )
}

/// Deploy the umbrella folder itself (plans, manifest, tool). The package
/// directories inside it are ignored by its `.gitignore` and travel separately.
fn deploy_umbrella(root: &Path, manifest: &Manifest, opts: &DeployOpts) -> Result<()> {
    let name = match manifest.kairos.umbrella.clone() {
        Some(n) => n,
        None => root
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .ok_or_else(|| Fail("cannot name the umbrella directory".into()))?,
    };
    validate_name(&name)?;
    for required in [MANIFEST, "README.md", ".gitignore"] {
        if !root.join(required).is_file() {
            return fail(format!("umbrella: missing {required}; not deployable"));
        }
    }
    let description = "Kairos — umbrella for the rusty_rtos_* family (FreeRTOS remade in memory-safe Rust): the plan, the fleet manifest, the fleet tool, the C oracle harness.";
    publish(
        root,
        manifest,
        &name,
        &manifest.kairos.visibility,
        description,
        opts,
    )
}

/// The shared commit → create-or-push flow.
fn publish(
    dir: &Path,
    manifest: &Manifest,
    name: &str,
    intended_visibility: &str,
    description: &str,
    opts: &DeployOpts,
) -> Result<()> {
    if intended_visibility != opts.visibility.word() && !opts.override_visibility {
        return fail(format!(
            "{name}: {MANIFEST} says visibility = {intended_visibility:?} but you asked for {}. Change the manifest or pass --override-visibility.",
            opts.visibility.word()
        ));
    }
    let branch = manifest.kairos.default_branch.as_str();
    let repo = format!("{}/{}", manifest.kairos.org, name);
    println!(
        "deploy {name} -> github.com/{repo} ({}){}",
        opts.visibility.word(),
        if opts.dry { " [dry run]" } else { "" }
    );

    if !is_own_repo(dir) {
        run(opts.dry, dir, "git", &["init", "-b", branch])?;
    }
    let dirty = probe(dir, "git", &["status", "--porcelain"]).is_none_or(|s| !s.is_empty());
    if dirty {
        run(opts.dry, dir, "git", &["add", "-A"])?;
        run(opts.dry, dir, "git", &["commit", "-m", &opts.message])?;
    } else {
        println!("work tree clean; nothing to commit");
    }

    let has_remote = probe(dir, "git", &["remote", "get-url", "origin"]).is_some();
    if has_remote {
        run(opts.dry, dir, "git", &["push", "-u", "origin", branch])?;
    } else {
        run(
            opts.dry,
            dir,
            "gh",
            &[
                "repo",
                "create",
                &repo,
                opts.visibility.flag(),
                "--source=.",
                "--remote=origin",
                "--push",
                "--description",
                description,
            ],
        )?;
    }
    println!("\nhttps://github.com/{repo}");
    Ok(())
}

// ------------------------------------------------------------------ secrets --

/// Fan the sibling-fetch token out to every package repository as its
/// Actions secret.
///
/// The token is read from the environment (the manifest's `token` name, or
/// whatever `--from-env` names) and handed to `gh secret set` on stdin: it
/// never appears on a command line, in this tool's output, or in a shell
/// history beyond the caller's own assignment. Repositories that do not
/// exist yet are skipped and named. `--dry-run` lists what would be set
/// without reading the token.
fn secrets(manifest: &Manifest, args: &[String]) -> Result<()> {
    use std::io::Write as _;
    use std::process::Stdio;

    let secret = manifest.kairos.token.as_str();
    let var = option(args, "--from-env").unwrap_or_else(|| secret.to_owned());
    let dry = has_flag(args, "--dry-run");
    let token = env::var(&var).ok().filter(|t| !t.trim().is_empty());
    if token.is_none() && !dry {
        return fail(format!(
            "{var} is not set; put the fine-grained token there (README, \"Visibility policy\": Contents: Read on every rusty_rtos_* repo)"
        ));
    }
    let org = &manifest.kairos.org;
    let (mut set, mut skipped) = (0usize, 0usize);
    for package in &manifest.packages {
        let repo = format!("{org}/{}", package.name);
        let exists = Command::new("gh")
            .args(["repo", "view", &repo, "--json", "name"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !exists {
            println!("  {repo}: no repository yet, skipped");
            skipped += 1;
            continue;
        }
        println!("$ gh secret set {secret} --repo {repo}   (value from ${var}, on stdin)");
        if dry {
            continue;
        }
        let mut child = Command::new("gh")
            .args(["secret", "set", secret, "--repo", &repo])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| "gh secret set: no stdin".to_owned())?;
            stdin.write_all(token.as_deref().unwrap_or_default().as_bytes())?;
        }
        let output = child.wait_with_output()?;
        if !output.status.success() {
            return fail(format!(
                "`gh secret set` failed for {repo}: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        set += 1;
    }
    if dry {
        println!(
            "dry run: {} repositories would get the secret, {skipped} skipped",
            manifest.packages.len() - skipped
        );
    } else {
        println!("{set} secret(s) set, {skipped} skipped");
    }
    Ok(())
}

// --------------------------------------------------------------------- main --

const USAGE: &str = "kairos — fleet tool for the Kairos umbrella folder

USAGE
  kairos status [--ci] [--json]
  kairos check [PACKAGE ...] [--fmt] [--clippy] [--test] [--deny] [--harden] [--xtensa] [--kani] [--qemu] [--board]
  kairos new NAME [--kind function] [--description TEXT] [--tier critical-path|standard|utility] [--date YYYY-MM-DD] [--dry-run]
  kairos patches [--dry-run]
  kairos harden [--all | --plan FILE [--readme FILE]] [--check] [--link URL] [--architect NAME] [--quiet]
  kairos deploy PACKAGE (--public | --private) [--message TEXT] [--dry-run] [--override-visibility]
  kairos deploy --umbrella (--public | --private) [--message TEXT] [--dry-run]
  kairos secrets [--from-env VAR] [--dry-run]
  kairos oracle fetch | patch | build [SCENARIO] | trace [SCENARIO] [--ticks N] [--all] | cat [SCENARIO]
  kairos conform [SCENARIO] [--ticks N] [--all] [--exits]
  kairos power idle [SCENARIO] [--min-sleep N] | diff [SCENARIO] [--show]

`status --ci` adds the latest GitHub Actions conclusion per package (through
`gh`). `check` is the compile gate — host workspace plus each no_std crate on
each listed target — and `--fmt` / `--clippy` / `--test` / `--deny` widen it
to the format check, the lint, the suites and the dependency policy;
`--harden` verifies every README's hardening block is current with its plan
file. `--xtensa` adds each no_std crate on xtensa-esp32s3-none-elf through the
esp toolchain (`cargo +esp`, `-Z build-std`), a gate CI cannot run.
`--soak` runs the `#[ignore]`d long-run tests — K3's one-hour clause is one,
and a test nothing runs is a test that has stopped existing.

`--qemu` runs every firmware cell that needs no hardware: any
`<package>/firmware/*/` whose own `.cargo/config.toml` names a
`qemu-system-*` runner. The cells are DISCOVERED rather than listed, and
the runner is the test — an `espflash` cell wants a board on a serial port
and is never started by a gate. A cell that ends in `debug::exit` hands
cargo the guest's verdict as an exit code, which is what lets an emulator
gate at all.
`--board` runs every firmware cell that needs a PART on a serial port: any
`<package>/firmware/*/` whose runner is `espflash`. Discovered the same way
the emulator cells are, so the list cannot fall behind the directory. A board
has no exit code to hand back, so the contract is that a cell prints
`RESULT: PASS` or `RESULT: FAIL` and this reads it -- and a cell that prints
NEITHER before the timeout fails, because a hang is exactly how a broken
context switch presents. This is how the hardware rows in `docs/LEDGER.md`
are re-run rather than re-read.
`--kani` compiles each package's proof harnesses (`cargo kani
--only-codegen`, under WSL on Windows) without verifying them. They are
`#[cfg(kani)]`, so no other gate compiles them at all and they can rot
while everything stays green — which is what happened when the `Raw` trait
grew and the symbolic fake did not.
`patches` writes each package's umbrella-only, gitignored `.cargo/config.toml`
so the sibling crates it depends on (from `uses`, cross-checked against its
manifests) resolve to the local checkouts through cargo's `paths` override,
which leaves Cargo.lock in its standalone form (a `[patch]` table would
rewrite it and make `--locked` fail outside the umbrella). `harden` renders the hardening-status block into a README
from its docs/plans/use-protection-please.md — the plan file is the source,
the block is generated, and a re-run is byte-identical so `--check` is a gate.
Every package is its own git repository and cargo workspace. `deploy` commits
the work tree, creates github.com/<org>/<PACKAGE> with the visibility you name
(never a default), and pushes. `--umbrella` does the same for this folder
itself (plans, manifest, tool); packages inside it are ignored by .gitignore.
`secrets` fans the sibling-fetch token in the manifest's token variable (or
the variable --from-env names) out to every package repository as its Actions
secret, on stdin, never on a command line.
`oracle` is the instrumented C kernel: `fetch` clones the commits ORACLES.md
pins, `patch` applies the deterministic-tick edits to the Posix port, `build`
compiles a scenario with the system C compiler (under WSL on Windows), and
`trace` runs it twice and refuses a trace that differs between the runs.
`conform` is the K1 gate: it runs a scenario on the Rust kernel and on the
C oracle and compares the traces line for line, counters included.
Run from anywhere inside the Kairos folder.";

fn main() -> ExitCode {
    match real_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn real_main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();
    let Some(verb) = args.first() else {
        println!("{USAGE}");
        return Ok(());
    };
    if matches!(verb.as_str(), "-h" | "--help" | "help") {
        println!("{USAGE}");
        return Ok(());
    }
    if verb == "--version" {
        println!(
            "kairos {} (rusty_alloc {})",
            env!("CARGO_PKG_VERSION"),
            kairos_alloc::VERSION
        );
        return Ok(());
    }
    let root = find_root()?;
    let manifest = load_manifest(&root)?;
    match verb.as_str() {
        "status" => status(&root, &manifest, &args[1..]),
        "check" => check(&root, &manifest, &args[1..]),
        "new" => {
            let name = positional(&args, 1, "NAME")?;
            let kind = option(&args, "--kind").unwrap_or_else(|| "function".to_owned());
            let description = option(&args, "--description").unwrap_or_default();
            let tier = option(&args, "--tier").unwrap_or_else(|| "critical-path".to_owned());
            let date = option(&args, "--date").unwrap_or_else(|| "unrecorded".to_owned());
            scaffold(
                &root,
                &manifest,
                &name,
                &kind,
                &description,
                &tier,
                &date,
                has_flag(&args, "--dry-run"),
            )
        }
        "patches" => patches::write_all(&root, &manifest, has_flag(&args, "--dry-run")),
        "harden" => harden::main(&root, &manifest, &args[1..]),
        "deploy" => {
            let umbrella = has_flag(&args, "--umbrella");
            let name = if umbrella {
                "umbrella".to_owned()
            } else {
                positional(&args, 1, "PACKAGE")?
            };
            let visibility = match (has_flag(&args, "--public"), has_flag(&args, "--private")) {
                (true, false) => Visibility::Public,
                (false, true) => Visibility::Private,
                _ => return fail("deploy needs exactly one of --public or --private"),
            };
            let opts = DeployOpts {
                visibility,
                dry: has_flag(&args, "--dry-run"),
                message: option(&args, "--message").unwrap_or_else(|| {
                    format!("{name}: initial scaffold from the Kairos umbrella")
                }),
                override_visibility: has_flag(&args, "--override-visibility"),
            };
            if umbrella {
                deploy_umbrella(&root, &manifest, &opts)
            } else {
                deploy(&root, &manifest, &name, &opts)
            }
        }
        "secrets" => secrets(&manifest, &args[1..]),
        "oracle" => oracle::main(&root, &args[1..]),
        "conform" => conform::main(&root, &args[1..]),
        "power" => power::main(&root, &args[1..]),
        other => fail(format!("unknown verb {other:?}\n\n{USAGE}")),
    }
}

fn positional(args: &[String], index: usize, what: &str) -> Result<String> {
    match args.get(index) {
        Some(value) if !value.starts_with("--") => Ok(value.clone()),
        _ => fail(format!("missing {what}\n\n{USAGE}")),
    }
}

pub(crate) fn option(args: &[String], flag: &str) -> Option<String> {
    let index = args.iter().position(|a| a == flag)?;
    args.get(index + 1).cloned()
}

pub(crate) fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}
