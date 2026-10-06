# Quick start

```toml
[dependencies]
quoracle = "2"
```

```rust
use quoracle::{Distribution, Expr, Node, Objective, QuorumSystem, StrategyLimits};

fn main() -> Result<(), quoracle::Error> {
    // A 2x2 grid: reads contact a full row, writes one node per row.
    let [a, b, c, d] = ["a", "b", "c", "d"].map(|x| Expr::Node(Node::new(x)));
    let grid = QuorumSystem::from_reads(a * b + c * d);
    assert_eq!(grid.resilience(), 1);

    let fr = Distribution::fixed(0.75)?; // 75% reads
    let strategy = grid.strategy(
        Objective::Load, Some(&fr), None, &StrategyLimits::default(), 0,
    )?;
    assert!((strategy.capacity(Some(&fr), None)? - 2.0).abs() < 1e-6);

    // Pick replicas for a request.
    let replicas = strategy.get_read_quorum();
    assert!(grid.is_read_quorum(&replicas));
    Ok(())
}
```

Next: the [guide](./guide.md) walks through every feature.
