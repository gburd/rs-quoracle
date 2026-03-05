//! Simple example demonstrating basic Quoracle usage.
//!
//! Run with: cargo run --example simple

#![allow(
    clippy::many_single_char_names,
    clippy::print_stdout,
    clippy::wildcard_imports,
    clippy::uninlined_format_args
)]

use quoracle::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Quoracle Simple Example ===\n");

    // Create node expressions.
    let a = Expr::Node(Node::new("a"));
    let b = Expr::Node(Node::new("b"));
    let c = Expr::Node(Node::new("c"));
    let d = Expr::Node(Node::new("d"));
    let e = Expr::Node(Node::new("e"));
    let f = Expr::Node(Node::new("f"));

    // Create a grid quorum system: reads = (a*b*c) + (d*e*f)
    //   reads require {a,b,c} OR {d,e,f}
    //   writes (the dual) require one from each row
    println!("Creating grid quorum system...");
    let grid = QuorumSystem::from_reads(
        (a.clone() * b.clone() * c.clone()) + (d.clone() * e.clone() * f.clone()),
    );

    // Resilience: how many node failures can the system
    // tolerate?
    println!("Read resilience:  {}", grid.read_resilience());
    println!("Write resilience: {}", grid.write_resilience());
    println!("Overall:          {}\n", grid.resilience());

    // Find the load-optimal strategy for 50% reads.
    println!("Finding optimal strategy for 50% reads...");
    let fr = Distribution::fixed(0.5)?;
    let strategy = grid.strategy(Objective::Load, Some(&fr), None, None, None, None, 0)?;

    let load = strategy.load(Some(&fr), None)?;
    println!("Optimal load: {:.4}", load);
    println!("Capacity:     {:.4}\n", 1.0 / load);

    // Sweep over different read fractions.
    println!("Load at different read fractions:");
    for frac in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let d = Distribution::fixed(frac)?;
        let s = grid.strategy(Objective::Load, Some(&d), None, None, None, None, 0)?;
        let l = s.load(Some(&d), None)?;
        println!("  fr={:.2}: load={:.4}", frac, l);
    }

    println!("\n=== Example complete ===");
    Ok(())
}
