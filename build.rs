use std::fs;
use std::path::Path;
use std::process::Command;

#[cfg(windows)]
const NPM: &str = "npm.cmd";
#[cfg(not(windows))]
const NPM: &str = "npm";

fn main() {
    // tokio_unstable is required for tokio-console's task instrumentation.
    // Only emit it when the feature is active so production builds pay no cost.
    if std::env::var("CARGO_FEATURE_TOKIO_CONSOLE").is_ok() {
        println!("cargo:rustc-cfg=tokio_unstable");
    }

    // Re-run when any frontend config changes
    println!("cargo:rerun-if-changed=svelte.config.js");
    println!("cargo:rerun-if-changed=vite.config.js");
    println!("cargo:rerun-if-changed=package.json");
    println!("cargo:rerun-if-changed=src/app.html");
    // Stamp file: missing or older than source files → triggers rebuild
    // (Written only after a successful npm build, so it never bounces.)
    println!("cargo:rerun-if-changed=.frontend-stamp");

    // Re-run when any Svelte/JS/CSS file in src/routes changes (recursive)
    watch_dir(Path::new("src/routes"));

    // Install node_modules if missing
    if !Path::new("node_modules").exists() {
        run(NPM, &["install"]);
    }

    // Build the frontend, then write the stamp so Cargo knows we're up to date
    run(NPM, &["run", "build"]);
    fs::write(".frontend-stamp", "").expect("failed to write .frontend-stamp");
}

fn watch_dir(dir: &Path) {
    // Watching the directory itself catches newly added files
    println!("cargo:rerun-if-changed={}", dir.display());
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            watch_dir(&path);
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("svelte" | "js" | "ts" | "css" | "html")
        ) {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}

fn run(cmd: &str, args: &[&str]) {
    let status = Command::new(cmd)
        .args(args)
        .status()
        .unwrap_or_else(|e| panic!("failed to execute `{cmd}`: {e}"));
    assert!(status.success(), "`{cmd}` exited with {status}");
}
