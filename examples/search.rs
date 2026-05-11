//! Example demonstrating heuristic search for optimal quorum systems
//!
//! Run with: cargo run --example search

#![allow(clippy::many_single_char_names, clippy::print_stdout)]

use quoracle::search::{search, SearchConfig};
use quoracle::{Distribution, Node, Objective};
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
    let nodes = vec![a, b, c, d];
    let result = search(&nodes, &config)?;

    println!("Found optimal system!");
    println!("Resilience: {}", result.quorum_system.resilience());

    let load = result
        .strategy
        .load(Some(&Distribution::Fixed(0.5)), None)?;
    println!("Load: {load:.4}");
    let capacity = 1.0 / load;
    println!("Capacity: {capacity:.4}");

    println!("\n=== Search complete ===");
    Ok(())
}
