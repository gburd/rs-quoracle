//! Error types for the Quoracle library

use thiserror::Error;

/// Result type alias for Quoracle operations
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur when working with quorum systems
#[derive(Error, Debug, Clone, PartialEq)]
pub enum Error {
    /// Read and write quorums do not overlap
    #[error("Read and write quorums do not overlap")]
    NonOverlappingQuorums,

    /// No strategy could be found satisfying the constraints
    #[error("No strategy found satisfying the constraints")]
    NoStrategyFound,

    /// No quorum system could be found satisfying the requirements
    #[error("No quorum system found satisfying the requirements")]
    NoQuorumSystemFound,

    /// Invalid distribution specification
    #[error("Invalid distribution: {0}")]
    InvalidDistribution(String),

    /// Linear programming solver error
    #[error("LP solver error: {0}")]
    LpError(String),

    /// Invalid quorum system configuration
    #[error("Invalid quorum system: {0}")]
    InvalidQuorumSystem(String),

    /// Invalid expression
    #[error("Invalid expression: {0}")]
    InvalidExpression(String),
}
