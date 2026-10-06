//! Quoracle: construct, analyze, and optimize read-write quorum systems.
//!
//! A *read-write quorum system* says which sets of nodes may serve a read
//! and which may serve a write, such that every read set overlaps every
//! write set. Quoracle lets you:
//!
//! - describe quorum systems with an expression algebra
//!   (`a + b` = either, `a * b` = both, [`choose`]/[`majority`] = k of n);
//! - compute fault tolerance ([`QuorumSystem::resilience`]);
//! - find the strategy (how often to pick each quorum) that minimizes
//!   load, network traffic, or latency for a given read/write mix, by
//!   linear programming ([`QuorumSystem::strategy`]);
//! - evaluate any strategy's load, capacity, latency and per-node load;
//! - search for the best quorum system over a set of nodes ([`search()`]).
//!
//! This is a Rust port of the Python [Quoracle] library described in
//! Whittaker et al., "Read-Write Quorum Systems Made Practical"
//! (`PaPoC` 2021).
//!
//! [Quoracle]: https://github.com/mwhittaker/quoracle
//!
//! # Example
//!
//! ```
//! use quoracle::{Distribution, Expr, Node, Objective, QuorumSystem,
//!                StrategyLimits};
//!
//! # fn main() -> Result<(), quoracle::Error> {
//! // A 2x2 grid: read a full row, write one node from each row.
//! let [a, b, c, d] = ["a", "b", "c", "d"].map(|x| Expr::Node(Node::new(x)));
//! let qs = QuorumSystem::from_reads(a * b + c * d);
//! assert_eq!(qs.resilience(), 1);
//!
//! // The load-optimal strategy for a 75%-read workload.
//! let fr = Distribution::fixed(0.75)?;
//! let strategy = qs.strategy(
//!     Objective::Load, Some(&fr), None, &StrategyLimits::default(), 0,
//! )?;
//! let load = strategy.load(Some(&fr), None)?;
//! assert!((load - 0.5).abs() < 1e-6);
//! // The system can serve 1 / load = 2x the throughput of a single node.
//! assert!((strategy.capacity(Some(&fr), None)? - 2.0).abs() < 1e-6);
//! # Ok(())
//! # }
//! ```
//!
//! # LP solvers
//!
//! Strategy optimization uses [`good_lp`]. The default `microlp` feature is
//! pure Rust; the `cbc` feature uses COIN-OR CBC (faster on large problems,
//! needs the system CBC library). Resilience never needs a solver.

#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(not(any(feature = "microlp", feature = "cbc")))]
compile_error!(
    "quoracle needs an LP solver: enable `microlp` (default) or `cbc`"
);

pub mod distribution;
pub mod error;
pub mod expr;
pub mod geometry;
pub mod quorum_system;
pub mod search;

/// The `hashbrown` version used in this crate's public API (quorums are
/// `hashbrown::HashSet`s). Use it to build sets that are passed in.
pub use hashbrown;

pub use distribution::Distribution;
pub use error::Error;
pub use expr::{choose, majority, And, Choose, Element, Expr, Node, Or};
pub use quorum_system::{
    Objective, Quorum, QuorumSystem, Strategy, StrategyLimits,
};
pub use search::{search, SearchConfig, SearchResult};

// Compile and run the code blocks in the README and the user guide.
#[cfg(doctest)]
mod doc_tests {
    #[doc = include_str!("../README.md")]
    struct Readme;
    #[doc = include_str!("../docs/src/quick-start.md")]
    struct QuickStart;
    #[doc = include_str!("../docs/src/guide.md")]
    struct Guide;
}
