//! Example demonstrating heuristic search for optimal quorum systems
//!
//! Run with: cargo run --example search

use quoracle::search::{search, SearchConfig};
use quoracle::*;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Quoracle Search Example ===\n");

    // Create nodes
    let a = Node::new("a");
    let b = Node::new("b");
    let c = Node::new("c");
    let d = Node::new("d");

    println!("Searching for optimal 4-node quorum system...");
    println!("Objective: Minimize load");
    println!("Read fraction: 50%");
    println!("Timeout: 5 seconds\n");

    // Configure search
    let config = SearchConfig {
        optimize: Objective::Load,
        resilience: 1, // Require at least 1-resilience
        read_fraction: Some(Distribution::Fixed(0.5)),
        timeout: Duration::from_secs(5),
        ..Default::default()
    };

    // Run search
    let result = search(vec![a, b, c, d], config)?;

    println!("Found optimal system!");
    println!("Resilience: {}", result.quorum_system.resilience());

    let load = result
        .strategy
        .load(Some(&Distribution::Fixed(0.5)), None)?;
    println!("Load: {:.4}", load);
    println!("Capacity: {:.4}", 1.0 / load);

    println!("\n=== Search complete ===");
    Ok(())
}
