//! Builds the web client (ADR 0076) with Node: copies `client/` to
//! `OUT_DIR`, installs its pinned tools from the lockfile with `npm ci`
//! (again only when the lockfile changes), and runs `client/build.mjs`,
//! which type-checks and bundles it. `src/assets.rs` embeds the result.
//! Reruns only when the client's sources change. `PIGEON_NODE` and
//! `PIGEON_NPM` name other programs than `node` and `npm` on the `PATH`.
#![allow(clippy::panic, clippy::print_stdout, reason = "a build script")]

use std::path::{Path, PathBuf};
use std::process::Command;

/// The oldest Node that builds the client, as `client/package.json` says.
const NODE_MIN: (u32, u32) = (22, 12);

fn main() {
    let manifest = PathBuf::from(env("CARGO_MANIFEST_DIR"));
    let client = manifest.join("client");
    for watched in [
        "package.json",
        "package-lock.json",
        "tsconfig.json",
        "build.mjs",
        "src",
        "test",
    ] {
        println!("cargo::rerun-if-changed={}", client.join(watched).display());
    }
    println!("cargo::rerun-if-env-changed=PIGEON_NODE");
    println!("cargo::rerun-if-env-changed=PIGEON_NPM");
    let node = std::env::var("PIGEON_NODE").unwrap_or_else(|_| "node".into());
    let npm = std::env::var("PIGEON_NPM").unwrap_or_else(|_| "npm".into());
    check_node(&node);

    let out = PathBuf::from(env("OUT_DIR"));
    let work = out.join("client");
    for dir in ["src", "test"] {
        let _ = std::fs::remove_dir_all(work.join(dir));
        copy_dir(&client.join(dir), &work.join(dir));
    }
    for file in [
        "package.json",
        "package-lock.json",
        "tsconfig.json",
        "build.mjs",
    ] {
        copy(&client.join(file), &work.join(file));
    }

    let lock = read(&client.join("package-lock.json")) + &read(&client.join("package.json"));
    let stamp = work.join("node_modules").join(".pigeon-lock");
    if std::fs::read_to_string(&stamp).ok().as_deref() != Some(lock.as_str()) {
        run(
            Command::new(&npm)
                .args([
                    "ci",
                    "--ignore-scripts",
                    "--no-audit",
                    "--no-fund",
                    "--loglevel=error",
                ])
                .current_dir(&work),
            &format!(
                "`{npm} ci` failed in {}: it installs the web client's pinned tools (esbuild, TypeScript) from client/package-lock.json, which needs the npm registry or npm's cache",
                work.display()
            ),
        );
        if let Err(e) = std::fs::write(&stamp, &lock) {
            panic!("cannot write {}: {e}", stamp.display());
        }
    }
    run(
        Command::new(&node)
            .arg("build.mjs")
            .arg(work.join("dist"))
            .current_dir(&work),
        "the web client did not build: see the errors above",
    );
    let dist = work.join("dist");
    let names: serde_json::Value = serde_json::from_str(&read(&dist.join("manifest.json")))
        .unwrap_or_else(|e| panic!("client/build.mjs wrote no manifest: {e}"));
    let mut rust = String::new();
    for (key, constant) in [("script", "SCRIPT"), ("style", "STYLE"), ("icon", "ICON")] {
        let Some(name) = names[key].as_str() else {
            panic!("client/build.mjs named no {key}");
        };
        let path = dist.join(name);
        rust.push_str(&format!(
            "/// The client's {key}, by its content-hashed name.\npub(crate) const {constant}: Asset = Asset {{ name: {name:?}, bytes: include_bytes!({path:?}) }};\n",
            path = path.display().to_string(),
        ));
    }
    if let Err(e) = std::fs::write(out.join("assets.rs"), rust) {
        panic!("cannot write assets.rs: {e}");
    }
    println!(
        "cargo::rustc-env=PIGEON_WEB_CLIENT_TESTS={}",
        dist.join("test").display()
    );
    println!("cargo::rustc-env=PIGEON_WEB_NODE={node}");
}

fn env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("cargo sets {key}"))
}

/// Stops the build with a sentence naming what is missing.
fn check_node(node: &str) {
    let help = "pigeon-web builds its web client with Node.js 22.12 or later and npm \
                (client/.nvmrc names the version CI uses): install them from \
                https://nodejs.org or your system's packages, or set PIGEON_NODE \
                and PIGEON_NPM to their paths, then build again";
    let version = match Command::new(node).arg("--version").output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_owned(),
        Ok(_) | Err(_) => panic!("`{node}` was not found or does not run. {help}"),
    };
    let mut parts = version.trim_start_matches('v').split('.');
    let mut next = || {
        parts
            .next()
            .and_then(|p| p.parse::<u32>().ok())
            .unwrap_or(0)
    };
    let found = (next(), next());
    if found < NODE_MIN {
        panic!("`{node}` is Node.js {version}, too old. {help}");
    }
}

fn run(command: &mut Command, failed: &str) {
    match command.status() {
        Ok(s) if s.success() => {}
        Ok(_) => panic!("{failed}"),
        Err(e) => panic!("{failed} ({e}; is npm installed with Node.js?)"),
    }
}

fn read(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

fn copy(from: &Path, to: &Path) {
    if let Some(parent) = to.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::copy(from, to) {
        panic!("cannot copy {} to {}: {e}", from.display(), to.display());
    }
}

fn copy_dir(from: &Path, to: &Path) {
    let entries =
        std::fs::read_dir(from).unwrap_or_else(|e| panic!("cannot read {}: {e}", from.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        let target = to.join(entry.file_name());
        if path.is_dir() {
            copy_dir(&path, &target);
        } else {
            copy(&path, &target);
        }
    }
}
