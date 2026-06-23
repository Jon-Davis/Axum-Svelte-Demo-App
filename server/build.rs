//! Build-time frontend pipeline. Because this crate links the `svelte-rust-test`
//! library as a *build-dependency*, the route tree is already compiled when this
//! script runs, so we can call `openapi_document()` here — the step that a
//! `build.rs` on the library itself could never do (it would run before the
//! crate compiled). That's what lets a plain `cargo build` regenerate the spec
//! and the Svelte bundle in one shot.
//!
//! Producing the spec is the only app-specific part and lives here (it needs the
//! route tree). Everything downstream — writing `openapi.json`, regenerating the
//! TypeScript types from it, and building the Svelte bundle, all content-gated —
//! lives in the `svelte-rust` build-dependency, driven by the `Frontend` builder.

use std::path::PathBuf;

fn main() {
    let root = workspace_root();

    let spec = svelte_rust_test::openapi_document()
        .to_pretty_json()
        .expect("serialize OpenAPI document");

    svelte_rust::build::Frontend::new(&root, svelte_rust_test::BUILD_DIR)
        .openapi(format!("{spec}\n"), "openapi.json", "src/lib/api/gen")
        .run();
}

fn workspace_root() -> PathBuf {
    PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .expect("server crate has a parent (the workspace root)")
        .to_path_buf()
}
