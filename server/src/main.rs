//! Runtime entry point. All logic lives in the `svelte-rust-test` library; this
//! binary just starts the async runtime and calls `run()`. The library is also
//! linked by `build.rs` as a build-dependency so that a plain `cargo build`
//! regenerates `openapi.json` and the frontend bundle before producing this
//! binary.

#[tokio::main]
async fn main() {
    svelte_rust_test::run().await;
}
