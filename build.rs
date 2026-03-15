//! Build script for quoracle.
//!
//! Enforces mutually exclusive solver features (microlp or cbc).

fn main() {
    // Ensure only one solver feature is enabled
    let microlp = cfg!(feature = "microlp");
    let cbc = cfg!(feature = "cbc");

    if microlp && cbc {
        panic!(
            "Cannot enable both 'microlp' and 'cbc' features simultaneously. \
             Please use only one solver feature at a time:\n  \
             cargo test --no-default-features --features microlp\n  \
             cargo test --no-default-features --features cbc"
        );
    }

    if !microlp && !cbc {
        panic!(
            "At least one solver feature must be enabled: 'microlp' or 'cbc'.\n  \
             Default: cargo test\n  \
             Explicit: cargo test --features microlp"
        );
    }
}
