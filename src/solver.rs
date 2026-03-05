//! Linear programming solver selection and abstraction
//!
//! Quoracle supports three LP solver backends:
//! - **Clarabel** (default): Pure Rust, silent, good performance
//! - **CBC**: Industry standard, verbose output, best performance
//! - **Microlp**: Pure Rust, simple, slower
//!
//! Enable solvers via Cargo features: `clarabel`, `cbc`, `microlp`, or `all-solvers`.

use crate::error::{Error, Result};

/// LP solver backend selection.
///
/// Choose the solver based on your requirements:
///
/// - **Clarabel** (default, pure Rust):
///   - ✅ Silent by default (no verbose solver output)
///   - ✅ Pure Rust (easy deployment, WASM support)
///   - ✅ Good performance (competitive with CBC)
///   - ✅ Apache 2.0 license
///   - ⚠️ Only supports continuous variables (sufficient for quoracle)
///   - **Recommendation**: Best for production use
///
/// - **CBC** (C library):
///   - ✅ Industry-standard solver (COIN-OR)
///   - ✅ Excellent performance
///   - ✅ Supports integer variables
///   - ⚠️ Verbose output to stdout/stderr (cannot be suppressed easily)
///   - ⚠️ Requires CBC C library at build time
///   - ⚠️ EPL 2.0 license (copyleft)
///   - **Recommendation**: Use if you need absolute best performance and don't mind verbose output
///
/// - **Microlp** (pure Rust):
///   - ✅ Pure Rust (no external dependencies)
///   - ✅ Simple and lightweight
///   - ✅ Silent by default
///   - ⚠️ Slower than CBC/Clarabel
///   - ⚠️ Best for small problems only
///   - **Recommendation**: Use for simple cases or when deployment simplicity is critical
///
/// # Examples
///
/// ```
/// use quoracle::{Solver, QuorumSystem, Expr, Node, Distribution, Objective};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let a = Expr::Node(Node::new("a"));
/// let b = Expr::Node(Node::new("b"));
/// let c = Expr::Node(Node::new("c"));
/// let qs = QuorumSystem::from_reads(a + b + c);
/// let dist = Distribution::fixed(0.5)?;
///
/// // Use default solver (Clarabel)
/// let strategy = qs.strategy_with_solver(
///     Solver::default(),
///     Objective::Load,
///     Some(&dist),
///     None, None, None, None, 0
/// )?;
///
/// // Explicitly choose CBC for best performance
/// #[cfg(feature = "cbc")]
/// let strategy = qs.strategy_with_solver(
///     Solver::Cbc,
///     Objective::Load,
///     Some(&dist),
///     None, None, None, None, 0
/// )?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Solver {
    /// CBC solver (COIN-OR)
    #[cfg(feature = "cbc")]
    Cbc,

    /// Clarabel solver (pure Rust, default)
    #[cfg(feature = "clarabel")]
    Clarabel,

    /// Microlp solver (pure Rust, simple)
    #[cfg(feature = "microlp")]
    Microlp,
}

impl Default for Solver {
    fn default() -> Self {
        // Default priority: Clarabel > CBC > Microlp
        #[cfg(feature = "clarabel")]
        {
            Self::Clarabel
        }
        #[cfg(all(not(feature = "clarabel"), feature = "cbc"))]
        {
            Self::Cbc
        }
        #[cfg(all(not(feature = "clarabel"), not(feature = "cbc"), feature = "microlp"))]
        {
            Self::Microlp
        }
        #[cfg(not(any(feature = "clarabel", feature = "cbc", feature = "microlp")))]
        compile_error!("At least one solver feature must be enabled: clarabel, cbc, or microlp");
    }
}

impl std::fmt::Display for Solver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            #[cfg(feature = "cbc")]
            Self::Cbc => write!(f, "CBC"),
            #[cfg(feature = "clarabel")]
            Self::Clarabel => write!(f, "Clarabel"),
            #[cfg(feature = "microlp")]
            Self::Microlp => write!(f, "Microlp"),
        }
    }
}

impl Solver {
    /// Get a list of all available solvers (based on enabled features).
    #[must_use]
    pub fn available() -> Vec<Self> {
        let mut solvers = Vec::new();
        #[cfg(feature = "clarabel")]
        solvers.push(Self::Clarabel);
        #[cfg(feature = "cbc")]
        solvers.push(Self::Cbc);
        #[cfg(feature = "microlp")]
        solvers.push(Self::Microlp);
        solvers
    }

    /// Check if a specific solver is available (feature-enabled).
    #[must_use]
    pub fn is_available(self) -> bool {
        match self {
            #[cfg(feature = "cbc")]
            Self::Cbc => true,
            #[cfg(feature = "clarabel")]
            Self::Clarabel => true,
            #[cfg(feature = "microlp")]
            Self::Microlp => true,
        }
    }

    /// Get the solver's name as a string.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            #[cfg(feature = "cbc")]
            Self::Cbc => "CBC",
            #[cfg(feature = "clarabel")]
            Self::Clarabel => "Clarabel",
            #[cfg(feature = "microlp")]
            Self::Microlp => "Microlp",
        }
    }

    /// Check if the solver is pure Rust (no C/C++ dependencies).
    #[must_use]
    pub fn is_pure_rust(self) -> bool {
        match self {
            #[cfg(feature = "cbc")]
            Self::Cbc => false,
            #[cfg(feature = "clarabel")]
            Self::Clarabel => true,
            #[cfg(feature = "microlp")]
            Self::Microlp => true,
        }
    }

    /// Check if the solver produces verbose output.
    #[must_use]
    pub fn is_verbose(self) -> bool {
        match self {
            #[cfg(feature = "cbc")]
            Self::Cbc => true,
            #[cfg(feature = "clarabel")]
            Self::Clarabel => false,
            #[cfg(feature = "microlp")]
            Self::Microlp => false,
        }
    }
}

/// Internal helper to dispatch to the appropriate solver backend.
///
/// This function exists to work around good_lp's design where each solver
/// is a separate type. We use a macro to avoid code duplication.
#[doc(hidden)]
pub fn solve_with_backend<F, R>(solver: Solver, f: F) -> Result<R>
where
    F: FnOnce(&dyn SolverBackend) -> Result<R>,
{
    match solver {
        #[cfg(feature = "cbc")]
        Solver::Cbc => {
            use good_lp_cbc::coin_cbc;
            f(&CbcBackend)
        }
        #[cfg(feature = "clarabel")]
        Solver::Clarabel => {
            use good_lp_clarabel::clarabel;
            f(&ClarabelBackend)
        }
        #[cfg(feature = "microlp")]
        Solver::Microlp => {
            use good_lp_microlp::microlp;
            f(&MicrolpBackend)
        }
    }
}

/// Trait abstracting over different solver backends.
#[doc(hidden)]
pub trait SolverBackend {
    fn name(&self) -> &'static str;
}

#[cfg(feature = "cbc")]
struct CbcBackend;
#[cfg(feature = "cbc")]
impl SolverBackend for CbcBackend {
    fn name(&self) -> &'static str {
        "CBC"
    }
}

#[cfg(feature = "clarabel")]
struct ClarabelBackend;
#[cfg(feature = "clarabel")]
impl SolverBackend for ClarabelBackend {
    fn name(&self) -> &'static str {
        "Clarabel"
    }
}

#[cfg(feature = "microlp")]
struct MicrolpBackend;
#[cfg(feature = "microlp")]
impl SolverBackend for MicrolpBackend {
    fn name(&self) -> &'static str {
        "Microlp"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_solver() {
        let solver = Solver::default();
        assert!(solver.is_available());
    }

    #[test]
    fn test_available_solvers() {
        let solvers = Solver::available();
        assert!(!solvers.is_empty(), "At least one solver must be enabled");
        for solver in solvers {
            assert!(solver.is_available());
        }
    }

    #[test]
    fn test_solver_properties() {
        for solver in Solver::available() {
            // All solvers should have names
            assert!(!solver.name().is_empty());

            // Display should work
            let display = format!("{solver}");
            assert!(!display.is_empty());

            // CBC is the only non-pure-rust solver
            #[cfg(feature = "cbc")]
            if matches!(solver, Solver::Cbc) {
                assert!(!solver.is_pure_rust());
                assert!(solver.is_verbose());
            }

            // Clarabel and Microlp are pure Rust and quiet
            #[cfg(feature = "clarabel")]
            if matches!(solver, Solver::Clarabel) {
                assert!(solver.is_pure_rust());
                assert!(!solver.is_verbose());
            }

            #[cfg(feature = "microlp")]
            if matches!(solver, Solver::Microlp) {
                assert!(solver.is_pure_rust());
                assert!(!solver.is_verbose());
            }
        }
    }

    #[test]
    #[cfg(feature = "clarabel")]
    fn test_clarabel_is_default() {
        assert_eq!(Solver::default(), Solver::Clarabel);
    }
}
