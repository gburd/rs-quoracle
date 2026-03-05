//! Quoracle: A library for constructing and analyzing read-write quorum systems
//!
//! This library provides tools for defining, optimizing, and analyzing quorum systems
//! used in distributed systems research. It supports:
//!
//! - **Expression algebra** for defining quorum systems using operators (OR, AND, Choose)
//! - **Linear programming-based optimization** for finding optimal read/write strategies
//! - **Multi-metric analysis** including load, capacity, network overhead, and latency
//! - **Resilience calculation** for fault tolerance analysis
//! - **Heuristic search** for discovering optimal quorum configurations
//!
//! # Examples
//!
//! ```rust
//! use quoracle::*;
//! use std::time::Duration;
//!
//! // Create nodes
//! let a = Expr::Node(Node::new('a'));
//! let b = Expr::Node(Node::new('b'));
//! let c = Expr::Node(Node::new('c'));
//!
//! // Build a quorum system where reads need any single node
//! // and writes (the dual) need all nodes.
//! let qs = QuorumSystem::from_reads(a + b + c);
//!
//! assert_eq!(qs.read_resilience(), 2);
//! assert_eq!(qs.write_resilience(), 0);
//!
//! // Find the load-optimal strategy for 50% reads.
//! let fr = Distribution::fixed(0.5).unwrap();
//! let strategy = qs.strategy(
//!     Objective::Load,
//!     Some(&fr), None, None, None, None, 0,
//! ).unwrap();
//!
//! let load = strategy.load(Some(&fr), None).unwrap();
//! assert!(load > 0.0);
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod distribution;
pub mod error;
pub mod expr;
pub mod geometry;
pub mod lp;
pub mod quorum_system;
pub mod search;

// Re-export main types
pub use distribution::Distribution;
pub use error::Error;
pub use expr::{choose, majority, And, Choose, Element, Expr, Node, Or};
pub use quorum_system::{Objective, QuorumSystem, Strategy};
pub use search::search;
