fn main() {
    if std::env::var("CARGO_FEATURE_TOKIO_CONSOLE").is_ok() {
        println!("cargo:rustc-cfg=tokio_unstable");
    }
    svelte_rust::build::frontend("build");
}
