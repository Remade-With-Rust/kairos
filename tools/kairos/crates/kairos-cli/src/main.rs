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

use serde::{Deserialize, Serialize};
use std::{
    env, fmt, fs,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
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
    if package.uses.is_empty() {
        return None;
    }
    let text = fs::read_to_string(dir.join("Cargo.lock")).ok()?;
    for used in &package.uses {
        let sibling = used.split('/').next().unwrap_or(used);
        let url = format!("git+https://github.com/{}/{sibling}", manifest.kairos.org);
        if !text.contains(&url) {
            return None;
        }
    }
    Some(text)
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

fn check(root: &Path, manifest: &Manifest, args: &[String]) -> Result<()> {
    let with_tests = has_flag(args, "--test");
    let with_clippy = has_flag(args, "--clippy");
    let with_fmt = has_flag(args, "--fmt");
    let with_deny = has_flag(args, "--deny");
    let with_harden = has_flag(args, "--harden");
    let with_xtensa = has_flag(args, "--xtensa");
    let with_kani = has_flag(args, "--kani");
    let with_qemu = has_flag(args, "--qemu");
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
  kairos check [PACKAGE ...] [--fmt] [--clippy] [--test] [--deny] [--harden] [--xtensa] [--kani] [--qemu]
  kairos new NAME [--kind function] [--description TEXT] [--tier critical-path|standard|utility] [--date YYYY-MM-DD] [--dry-run]
  kairos patches [--dry-run]
  kairos harden [--all | --plan FILE [--readme FILE]] [--check] [--link URL] [--architect NAME] [--quiet]
  kairos deploy PACKAGE (--public | --private) [--message TEXT] [--dry-run] [--override-visibility]
  kairos deploy --umbrella (--public | --private) [--message TEXT] [--dry-run]
  kairos secrets [--from-env VAR] [--dry-run]
  kairos oracle fetch | patch | build [SCENARIO] | trace [SCENARIO] [--ticks N] [--all] | cat [SCENARIO]
  kairos conform [SCENARIO] [--ticks N] [--all] [--exits]

`status --ci` adds the latest GitHub Actions conclusion per package (through
`gh`). `check` is the compile gate — host workspace plus each no_std crate on
each listed target — and `--fmt` / `--clippy` / `--test` / `--deny` widen it
to the format check, the lint, the suites and the dependency policy;
`--harden` verifies every README's hardening block is current with its plan
file. `--xtensa` adds each no_std crate on xtensa-esp32s3-none-elf through the
esp toolchain (`cargo +esp`, `-Z build-std`), a gate CI cannot run.
`--qemu` runs every firmware cell that needs no hardware: any
`<package>/firmware/*/` whose own `.cargo/config.toml` names a
`qemu-system-*` runner. The cells are DISCOVERED rather than listed, and
the runner is the test — an `espflash` cell wants a board on a serial port
and is never started by a gate. A cell that ends in `debug::exit` hands
cargo the guest's verdict as an exit code, which is what lets an emulator
gate at all.
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
