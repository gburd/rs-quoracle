//! Error types for the Quoracle library

use thiserror::Error;

/// Result type alias for Quoracle operations
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur when working with quorum systems
#[derive(Error, Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// Some write quorum does not intersect some read quorum.
    #[error("not all read quorums intersect all write quorums")]
    NonOverlappingQuorums,

    /// The constraints are infeasible, or there are no f-resilient
    /// quorums for the requested `f`.
    #[error("no strategy found satisfying the constraints")]
    NoStrategyFound,

    /// Search found no quorum system satisfying the requirements.
    #[error("no quorum system found satisfying the requirements")]
    NoQuorumSystemFound,

    /// Invalid read/write distribution.
    #[error("invalid distribution: {0}")]
    InvalidDistribution(String),

    /// The LP solver failed for a reason other than infeasibility.
    #[error("LP solver error: {0}")]
    LpError(String),

    /// Invalid arguments to a quorum system or strategy operation.
    #[error("invalid quorum system: {0}")]
    InvalidQuorumSystem(String),

    /// Invalid expression or geometry arguments.
    #[error("invalid expression: {0}")]
    InvalidExpression(String),
}

impl From<good_lp::ResolutionError> for Error {
    fn from(e: good_lp::ResolutionError) -> Self {
        match e {
            good_lp::ResolutionError::Infeasible => Self::NoStrategyFound,
            other => Self::LpError(other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolution_error_mapping() {
        assert_eq!(
            Error::from(good_lp::ResolutionError::Infeasible),
            Error::NoStrategyFound
        );
        assert!(matches!(
            Error::from(good_lp::ResolutionError::Unbounded),
            Error::LpError(_)
        ));
        assert!(Error::NonOverlappingQuorums.to_string().contains("intersect"));
    }
}
