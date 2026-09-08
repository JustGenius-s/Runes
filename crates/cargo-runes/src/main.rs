//! `cargo runes <subcommand>` — run a normal Cargo command with the Runes
//! tracing driver installed as `RUSTC_WRAPPER`.
//!
//! This is a thin orchestrator: it locates (and builds) the driver, sets the
//! environment Cargo needs, then execs `cargo` with the user's arguments. All
//! tracing happens inside the driver.

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

const USAGE: &str = "\
cargo runes — run a Cargo command through the Runes tracing driver

Usage:
  cargo runes <cargo-subcommand> [args...]     e.g. `cargo runes run`
  cargo runes --help

Environment:
  RUNES_DRIVER       path to a prebuilt runes-driver binary (skips auto-build)
  RUNES_CONFIG       path to Runes.toml (default: Runes.toml if present)
  RUNES_OUT          trace output path (default: trace.json)
";

fn main() -> ExitCode {
    let mut args: Vec<OsString> = env::args_os().skip(1).collect();
    // Cargo invokes us as `cargo-runes runes <args...>`.
    if args.first().is_some_and(|arg| arg == "runes") {
        args.remove(0);
    }

    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if args.is_empty() {
        eprint!("{USAGE}");
        return ExitCode::FAILURE;
    }

    let driver = match locate_driver() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("cargo-runes: {error}");
            eprintln!("hint: set RUNES_DRIVER to a prebuilt runes-driver binary");
            return ExitCode::FAILURE;
        }
    };

    // The driver links against `librustc_driver` from the nightly sysroot, and
    // Cargo execs RUSTC_WRAPPER directly (no rustup shim to inject the library
    // path), so we provide it ourselves.
    let lib_path_env = driver_library_path_env();

    // Bake the driver's identity into the crate metadata so changing the driver
    // forces a rebuild of the traced crate.
    let metadata = match fingerprint(&driver) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("cargo-runes: cannot stat driver: {error}");
            return ExitCode::FAILURE;
        }
    };

    let rustflags = match env::var("RUSTFLAGS") {
        Ok(existing) => format!("{existing} -C metadata=runes_{metadata}"),
        Err(_) => format!("-C metadata=runes_{metadata}"),
    };

    let mut command = Command::new(cargo_bin());
    command
        .args(&args)
        .env("RUSTC_WRAPPER", &driver)
        .env("RUNES_LOWER_KEYWORD", "1")
        .env("RUSTFLAGS", rustflags)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    for (key, value) in lib_path_env {
        command.env(key, value);
    }
    let status = command.status();

    match status {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => match status.code() {
            Some(code) => ExitCode::from(u8::try_from(code).unwrap_or(1)),
            None => ExitCode::FAILURE,
        },
        Err(error) => {
            eprintln!("cargo-runes: failed to run cargo: {error}");
            ExitCode::FAILURE
        }
    }
}

fn cargo_bin() -> OsString {
    env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"))
}

/// Returns the `DYLD_LIBRARY_PATH` / `LD_LIBRARY_PATH` values needed to load
/// `librustc_driver` from the driver's toolchain sysroot, preserving any
/// existing entries.
fn driver_library_path_env() -> Vec<(String, String)> {
    let Some(sysroot) = toolchain_sysroot() else {
        return Vec::new();
    };
    let lib = sysroot.join("lib");
    if !lib.is_dir() {
        return Vec::new();
    }
    ["DYLD_LIBRARY_PATH", "LD_LIBRARY_PATH"]
        .into_iter()
        .map(|key| {
            let mut value = lib.clone().into_os_string();
            if let Some(existing) = env::var_os(key) {
                value.push(":");
                value.push(existing);
            }
            (key.to_owned(), value.to_string_lossy().into_owned())
        })
        .collect()
}

/// Resolves the nightly sysroot via `rustc --print sysroot`.
fn toolchain_sysroot() -> Option<PathBuf> {
    let channel = env::var("RUNES_TOOLCHAIN").unwrap_or_else(|_| "nightly".to_owned());
    let output = Command::new("rustup")
        .args(["run", &channel, "rustc", "--print", "sysroot"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let sysroot = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    if sysroot.is_empty() {
        return None;
    }
    Some(PathBuf::from(sysroot))
}

/// Finds the driver: `RUNES_DRIVER` wins, else build the in-tree one.
fn locate_driver() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("RUNES_DRIVER") {
        let path = PathBuf::from(path);
        if !path.is_file() {
            return Err(format!("RUNES_DRIVER does not exist: {}", path.display()));
        }
        return Ok(path);
    }

    let dir = find_dir("compiler/runes-driver")
        .ok_or_else(|| "could not find compiler/runes-driver in this directory or its parents".to_owned())?;
    build_driver(&dir)
}

/// Builds the driver in place and returns the binary path.
///
/// The driver needs `rustc-dev`, so it must be built with the toolchain in its
/// own `rust-toolchain.toml`. `CARGO` usually points at the *current* (stable)
/// toolchain's cargo, which bypasses rustup's directory-based selection, so we
/// go through `rustup run` explicitly.
fn build_driver(dir: &Path) -> Result<PathBuf, String> {
    let channel = toolchain_channel(dir);
    let status = match Command::new("rustup")
        .args(["run", &channel, "cargo", "build"])
        .current_dir(dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
    {
        Ok(status) => status,
        Err(_) => {
            // Fall back to the ambient cargo, asking rustup for the channel.
            Command::new(cargo_bin())
                .arg("build")
                .current_dir(dir)
                .env("RUSTUP_TOOLCHAIN", &channel)
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .status()
                .map_err(|error| format!("failed to build driver: {error}"))?
        }
    };
    if !status.success() {
        return Err("driver build failed".to_owned());
    }
    let bin = dir.join("target/debug/runes-driver");
    if !bin.is_file() {
        return Err(format!("driver binary missing at {}", bin.display()));
    }
    Ok(bin)
}

/// Reads `channel` from the driver's `rust-toolchain.toml` (default `nightly`).
fn toolchain_channel(dir: &Path) -> String {
    let Ok(text) = std::fs::read_to_string(dir.join("rust-toolchain.toml")) else {
        return "nightly".to_owned();
    };
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("channel")
            && let Some(value) = rest.split('=').nth(1)
        {
            let value = value.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return value.to_owned();
            }
        }
    }
    "nightly".to_owned()
}

/// Walks up from the current directory looking for `relative`.
fn find_dir(relative: &str) -> Option<PathBuf> {
    let mut dir = env::current_dir().ok()?;
    loop {
        let candidate = dir.join(relative);
        if candidate.is_dir() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Cheap identity from mtime + size, used as crate metadata.
fn fingerprint(path: &Path) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|error| error.to_string())?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Ok(format!("{}_{}", mtime, meta.len()))
}
