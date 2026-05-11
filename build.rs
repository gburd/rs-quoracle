//! Build script: validates LP solver feature configuration.

fn main() {
    let microlp = cfg!(feature = "microlp");
    let cbc = cfg!(feature = "cbc");

    if microlp && cbc {
        // Both features enabled (e.g. via --all-features in CI).
        // Silently prefer microlp (the default) — no warning needed since
        // --all-features is a valid CI invocation pattern.
        return;
    }

    if !microlp && !cbc {
        // No solver configured — this is always an error.
        println!(
            "cargo:warning=quoracle: no solver feature enabled; add \
             'microlp' (default) or 'cbc' to your dependency features."
        );
        // Trigger a compile error via a non-existent cfg.
        println!("cargo:rustc-cfg=quoracle_no_solver_enabled");
    }
}
