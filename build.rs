//! Build script: fail early, with a clear message, when no LP solver
//! feature is enabled (otherwise `good_lp` fails with a less obvious error).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let microlp = std::env::var_os("CARGO_FEATURE_MICROLP").is_some();
    let cbc = std::env::var_os("CARGO_FEATURE_CBC").is_some();
    if !microlp && !cbc {
        println!(
            "cargo:warning=quoracle: no LP solver feature enabled; enable \
             `microlp` (default) or `cbc`."
        );
    }
}
