//! Comprehensive tutorial ported from the Python quoracle tutorial.
//!
//! Covers: quorum systems, resilience, strategies, load,
//! capacity, workload distributions, heterogeneous nodes,
//! f-resilient strategies, latency, and network load.
//!
//! Run with: cargo run --example tutorial

#![allow(
    clippy::many_single_char_names,
    clippy::print_stdout,
    clippy::wildcard_imports,
    clippy::uninlined_format_args,
    clippy::too_many_lines,
    clippy::stable_sort_primitive
)]

use hashbrown::HashSet;
use quoracle::*;
use std::collections::BTreeMap;
use std::time::Duration;

fn set<'a>(items: &[&'a str]) -> HashSet<&'a str> {
    items.iter().copied().collect()
}

fn quorum<'a>(items: &[&'a str]) -> Vec<&'a str> {
    let mut v: Vec<&'a str> = items.to_vec();
    v.sort();
    v
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // ---- Quorum Systems ----

    println!("=== Quorum Systems ===\n");

    let a = Expr::Node(Node::new("a"));
    let b = Expr::Node(Node::new("b"));
    let c = Expr::Node(Node::new("c"));
    let d = Expr::Node(Node::new("d"));
    let e = Expr::Node(Node::new("e"));
    let f = Expr::Node(Node::new("f"));

    // reads = (a*b*c) + (d*e*f)
    let grid = QuorumSystem::from_reads(
        a.clone() * b.clone() * c.clone() + d.clone() * e.clone() * f.clone(),
    );

    println!("Read quorums:");
    for rq in grid.read_quorums() {
        let mut v: Vec<_> = rq.into_iter().collect();
        v.sort();
        println!("  {:?}", v);
    }

    println!("\nWrite quorums:");
    for wq in grid.write_quorums() {
        let mut v: Vec<_> = wq.into_iter().collect();
        v.sort();
        println!("  {:?}", v);
    }

    // Quorum membership checks.
    println!(
        "\nis_read_quorum({{a,b,c}}): {}",
        grid.is_read_quorum(&set(&["a", "b", "c"]))
    );
    println!(
        "is_read_quorum({{a,b,d}}): {}",
        grid.is_read_quorum(&set(&["a", "b", "d"]))
    );
    println!(
        "is_write_quorum({{a,d}}):  {}",
        grid.is_write_quorum(&set(&["a", "d"]))
    );
    println!(
        "is_write_quorum({{a,b}}):  {}",
        grid.is_write_quorum(&set(&["a", "b"]))
    );

    // ---- Resilience ----

    println!("\n=== Resilience ===\n");
    println!("Read resilience:  {}", grid.read_resilience());
    println!("Write resilience: {}", grid.write_resilience());
    println!("Overall:          {}", grid.resilience());

    // ---- Strategies ----

    println!("\n=== Strategies ===\n");

    let mut sigma_r = BTreeMap::new();
    sigma_r.insert(quorum(&["a", "b", "c"]), 2.0);
    sigma_r.insert(quorum(&["d", "e", "f"]), 1.0);

    let mut sigma_w = BTreeMap::new();
    sigma_w.insert(quorum(&["a", "d"]), 1.0);
    sigma_w.insert(quorum(&["b", "e"]), 1.0);
    sigma_w.insert(quorum(&["c", "f"]), 1.0);

    let strategy = grid.make_strategy(sigma_r, sigma_w)?;

    println!("Sample read quorums:");
    for _ in 0..4 {
        let mut rq: Vec<_> = strategy.get_read_quorum().into_iter().collect();
        rq.sort();
        println!("  {:?}", rq);
    }
    println!("Sample write quorums:");
    for _ in 0..4 {
        let mut wq: Vec<_> = strategy.get_write_quorum().into_iter().collect();
        wq.sort();
        println!("  {:?}", wq);
    }

    // ---- Load and Capacity ----

    println!("\n=== Load and Capacity ===\n");

    let fr1 = Distribution::fixed(1.0)?;
    let fr0 = Distribution::fixed(0.0)?;
    let fr025 = Distribution::fixed(0.25)?;

    println!("load(fr=1.0):   {:.6}", strategy.load(Some(&fr1), None)?);
    println!("load(fw=1.0):   {:.6}", strategy.load(None, Some(&fr1))?);
    println!("load(fr=0.25):  {:.6}", strategy.load(Some(&fr025), None)?);

    println!("\nPer-node load at fr=0.25:");
    for name in ["a", "b", "c", "d", "e", "f"] {
        let node = grid.node(&name)?;
        let nl = strategy.node_load(node, Some(&fr025), None)?;
        println!("  {name}: {nl:.6}");
    }

    // Optimal strategy for fr=0.25.
    let limits = StrategyLimits::default();
    let opt = grid.strategy(Objective::Load, Some(&fr025), None, &limits, 0)?;
    println!("\nOptimal load(fr=0.25): {:.6}", opt.load(Some(&fr025), None)?);
    println!("load(fr=0.0):          {:.6}", opt.load(Some(&fr0), None)?);
    let fr05 = Distribution::fixed(0.5)?;
    println!("load(fr=0.5):          {:.6}", opt.load(Some(&fr05), None)?);
    println!("load(fr=1.0):          {:.6}", opt.load(Some(&fr1), None)?);

    println!("\ncapacity(fr=0.25): {:.4}", opt.capacity(Some(&fr025), None)?);

    // ---- Workload Distributions ----

    println!("\n=== Workload Distributions ===\n");

    let dist = Distribution::weighted(&[(0.1, 0.5), (0.75, 0.5)])?;
    let sigma =
        grid.strategy(Objective::Load, Some(&dist), None, &limits, 0)?;
    println!("load(distribution): {:.6}", sigma.load(Some(&dist), None)?);

    // ---- Heterogeneous Nodes ----

    println!("\n=== Heterogeneous Nodes ===\n");

    let a = Expr::Node(Node::new("a").with_capacity(1000.0));
    let b = Expr::Node(Node::new("b").with_capacity(500.0));
    let c = Expr::Node(Node::new("c").with_capacity(1000.0));
    let d = Expr::Node(Node::new("d").with_capacity(500.0));
    let e = Expr::Node(Node::new("e").with_capacity(1000.0));
    let f = Expr::Node(Node::new("f").with_capacity(500.0));

    let grid2 = QuorumSystem::from_reads(
        a.clone() * b.clone() * c.clone() + d.clone() * e.clone() * f.clone(),
    );
    let fr075 = Distribution::fixed(0.75)?;
    let s2 = grid2.strategy(Objective::Load, Some(&fr075), None, &limits, 0)?;
    println!("load(fr=0.75):     {:.6}", s2.load(Some(&fr075), None)?);
    println!("capacity(fr=0.75): {:.2}", s2.capacity(Some(&fr075), None)?);

    // Asymmetric read/write capacities.
    let a =
        Expr::Node(Node::new("a").with_read_write_capacity(10000.0, 1000.0));
    let b = Expr::Node(Node::new("b").with_read_write_capacity(5000.0, 500.0));
    let c =
        Expr::Node(Node::new("c").with_read_write_capacity(10000.0, 1000.0));
    let d = Expr::Node(Node::new("d").with_read_write_capacity(5000.0, 500.0));
    let e =
        Expr::Node(Node::new("e").with_read_write_capacity(10000.0, 1000.0));
    let f = Expr::Node(Node::new("f").with_read_write_capacity(5000.0, 500.0));
    let grid3 = QuorumSystem::from_reads(
        a.clone() * b.clone() * c.clone() + d.clone() * e.clone() * f.clone(),
    );

    println!(
        "\ncapacity(fr=1.0): {:.2}",
        grid3
            .strategy(Objective::Load, Some(&fr1), None, &limits, 0)?
            .capacity(Some(&fr1), None)?
    );
    println!(
        "capacity(fr=0.5): {:.2}",
        grid3
            .strategy(Objective::Load, Some(&fr05), None, &limits, 0)?
            .capacity(Some(&fr05), None)?
    );
    println!(
        "capacity(fr=0.0): {:.2}",
        grid3
            .strategy(Objective::Load, Some(&fr0), None, &limits, 0)?
            .capacity(Some(&fr0), None)?
    );

    // ---- Latency ----

    println!("\n=== Latency ===\n");

    let secs = |s| Duration::from_secs(s);
    let a = Expr::Node(
        Node::new("a")
            .with_read_write_capacity(10000.0, 1000.0)
            .with_latency(secs(1)),
    );
    let b = Expr::Node(
        Node::new("b")
            .with_read_write_capacity(5000.0, 500.0)
            .with_latency(secs(2)),
    );
    let c = Expr::Node(
        Node::new("c")
            .with_read_write_capacity(10000.0, 1000.0)
            .with_latency(secs(3)),
    );
    let d = Expr::Node(
        Node::new("d")
            .with_read_write_capacity(5000.0, 500.0)
            .with_latency(secs(4)),
    );
    let e = Expr::Node(
        Node::new("e")
            .with_read_write_capacity(10000.0, 1000.0)
            .with_latency(secs(5)),
    );
    let f = Expr::Node(
        Node::new("f")
            .with_read_write_capacity(5000.0, 500.0)
            .with_latency(secs(6)),
    );
    let grid4 = QuorumSystem::from_reads(a * b * c + d * e * f);

    let lat_opt =
        grid4.strategy(Objective::Latency, Some(&fr05), None, &limits, 0)?;
    println!("latency(fr=1.0): {:?}", lat_opt.latency(Some(&fr1), None)?);
    println!("latency(fr=0.0): {:?}", lat_opt.latency(Some(&fr0), None)?);
    println!("latency(fr=0.5): {:?}", lat_opt.latency(Some(&fr05), None)?);

    // Latency-optimal with load constraint.
    let load_constrained_limits =
        StrategyLimits { load: Some(1.0 / 1500.0), ..Default::default() };
    let lat_constrained = grid4.strategy(
        Objective::Latency,
        Some(&fr05),
        None,
        &load_constrained_limits,
        0,
    )?;
    println!("\nLatency-optimal with load <= 1/1500:");
    println!("  capacity: {:.2}", lat_constrained.capacity(Some(&fr05), None)?);
    println!("  latency:  {:?}", lat_constrained.latency(Some(&fr05), None)?);

    // Load-optimal with latency constraint.
    let latency_constrained_limits =
        StrategyLimits { latency: Some(secs(4)), ..Default::default() };
    let load_constrained = grid4.strategy(
        Objective::Load,
        Some(&fr05),
        None,
        &latency_constrained_limits,
        0,
    )?;
    println!("\nLoad-optimal with latency <= 4s:");
    println!(
        "  capacity: {:.2}",
        load_constrained.capacity(Some(&fr05), None)?
    );
    println!("  latency:  {:?}", load_constrained.latency(Some(&fr05), None)?);

    // ---- Network Load ----

    println!("\n=== Network Load ===\n");

    let net_opt =
        grid4.strategy(Objective::Network, Some(&fr05), None, &limits, 0)?;
    println!(
        "network_load(fr=0.5): {:.4}",
        net_opt.network_load(Some(&fr05), None)?
    );

    println!("\n=== Tutorial complete ===");
    Ok(())
}
