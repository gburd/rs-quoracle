use criterion::{black_box, criterion_group, criterion_main, Criterion};
use quoracle::search::{search, SearchConfig};
use quoracle::*;
use std::time::Duration;

fn benchmark_quorum_enumeration(c: &mut Criterion) {
    // Majority quorum of 5 nodes: any 3 nodes
    c.bench_function("quorum_enum_majority_5", |b| {
        let nodes: Vec<_> = (0..5).map(|i| Expr::Node(Node::new(i))).collect();
        let expr = majority(nodes).unwrap();
        b.iter(|| {
            let qs = QuorumSystem::from_reads(expr.clone());
            let quorums: Vec<_> = qs.read_quorums().collect();
            black_box(quorums)
        });
    });

    // Grid quorum: 3x3 grid
    c.bench_function("quorum_enum_grid_3x3", |b| {
        let row1 = (0..3).map(|i| Expr::Node(Node::new(i))).collect::<Vec<_>>();
        let row2 = (3..6).map(|i| Expr::Node(Node::new(i))).collect::<Vec<_>>();
        let row3 = (6..9).map(|i| Expr::Node(Node::new(i))).collect::<Vec<_>>();

        let r1 = row1[0].clone() * row1[1].clone() * row1[2].clone();
        let r2 = row2[0].clone() * row2[1].clone() * row2[2].clone();
        let r3 = row3[0].clone() * row3[1].clone() * row3[2].clone();
        let expr = r1 + r2 + r3;

        b.iter(|| {
            let qs = QuorumSystem::from_reads(expr.clone());
            let quorums: Vec<_> = qs.read_quorums().collect();
            black_box(quorums)
        });
    });

    // Choose 2 of 5 nodes
    c.bench_function("quorum_enum_choose_2_of_5", |b| {
        let nodes: Vec<_> = (0..5).map(|i| Expr::Node(Node::new(i))).collect();
        let expr = choose(2, nodes).unwrap();
        b.iter(|| {
            let qs = QuorumSystem::from_reads(expr.clone());
            let quorums: Vec<_> = qs.read_quorums().collect();
            black_box(quorums)
        });
    });
}

fn benchmark_resilience(c: &mut Criterion) {
    // Grid quorum resilience
    c.bench_function("resilience_grid_3x3", |b| {
        let row1 = (0..3).map(|i| Expr::Node(Node::new(i))).collect::<Vec<_>>();
        let row2 = (3..6).map(|i| Expr::Node(Node::new(i))).collect::<Vec<_>>();
        let row3 = (6..9).map(|i| Expr::Node(Node::new(i))).collect::<Vec<_>>();

        let r1 = row1[0].clone() * row1[1].clone() * row1[2].clone();
        let r2 = row2[0].clone() * row2[1].clone() * row2[2].clone();
        let r3 = row3[0].clone() * row3[1].clone() * row3[2].clone();
        let expr = r1 + r2 + r3;
        let qs = QuorumSystem::from_reads(expr);

        b.iter(|| {
            let res = qs.resilience();
            black_box(res)
        });
    });

    // Majority quorum resilience
    c.bench_function("resilience_majority_5", |b| {
        let nodes: Vec<_> = (0..5).map(|i| Expr::Node(Node::new(i))).collect();
        let expr = majority(nodes).unwrap();
        let qs = QuorumSystem::from_reads(expr);

        b.iter(|| {
            let res = qs.resilience();
            black_box(res)
        });
    });

    // Complex expression resilience
    c.bench_function("resilience_complex", |bencher| {
        let a = Expr::Node(Node::new('a'));
        let b = Expr::Node(Node::new('b'));
        let c = Expr::Node(Node::new('c'));
        let d = Expr::Node(Node::new('d'));
        let e = Expr::Node(Node::new('e'));
        let f = Expr::Node(Node::new('f'));

        let expr = (a.clone() * b.clone() + c.clone() * d.clone()) * (e.clone() + f.clone());
        let qs = QuorumSystem::from_reads(expr);

        bencher.iter(|| {
            let res = qs.resilience();
            black_box(res)
        });
    });
}

fn benchmark_strategy_optimization(c: &mut Criterion) {
    // Grid quorum strategy optimization
    c.bench_function("strategy_grid_3x3", |b| {
        let row1 = (0..3).map(|i| Expr::Node(Node::new(i))).collect::<Vec<_>>();
        let row2 = (3..6).map(|i| Expr::Node(Node::new(i))).collect::<Vec<_>>();
        let row3 = (6..9).map(|i| Expr::Node(Node::new(i))).collect::<Vec<_>>();

        let r1 = row1[0].clone() * row1[1].clone() * row1[2].clone();
        let r2 = row2[0].clone() * row2[1].clone() * row2[2].clone();
        let r3 = row3[0].clone() * row3[1].clone() * row3[2].clone();
        let expr = r1 + r2 + r3;
        let qs = QuorumSystem::from_reads(expr);
        let dist = Distribution::Fixed(0.5);

        b.iter(|| {
            let strategy = qs.strategy(
                Objective::Load,
                Some(&dist),
                None,
                None,
                None,
                None,
                0,
            ).unwrap();
            black_box(strategy)
        });
    });

    // Majority quorum strategy
    c.bench_function("strategy_majority_7", |b| {
        let nodes: Vec<_> = (0..7).map(|i| Expr::Node(Node::new(i))).collect();
        let expr = majority(nodes).unwrap();
        let qs = QuorumSystem::from_reads(expr);
        let dist = Distribution::Fixed(0.5);

        b.iter(|| {
            let strategy = qs.strategy(
                Objective::Load,
                Some(&dist),
                None,
                None,
                None,
                None,
                0,
            ).unwrap();
            black_box(strategy)
        });
    });
}

fn benchmark_load_calculation(c: &mut Criterion) {
    // Grid quorum load calculation
    c.bench_function("load_grid_3x3", |b| {
        let row1 = (0..3).map(|i| Expr::Node(Node::new(i))).collect::<Vec<_>>();
        let row2 = (3..6).map(|i| Expr::Node(Node::new(i))).collect::<Vec<_>>();
        let row3 = (6..9).map(|i| Expr::Node(Node::new(i))).collect::<Vec<_>>();

        let r1 = row1[0].clone() * row1[1].clone() * row1[2].clone();
        let r2 = row2[0].clone() * row2[1].clone() * row2[2].clone();
        let r3 = row3[0].clone() * row3[1].clone() * row3[2].clone();
        let expr = r1 + r2 + r3;
        let qs = QuorumSystem::from_reads(expr);
        let dist = Distribution::Fixed(0.5);
        let strategy = qs.strategy(
            Objective::Load,
            Some(&dist),
            None,
            None,
            None,
            None,
            0,
        ).unwrap();

        b.iter(|| {
            let load = strategy.load(Some(&dist), None).unwrap();
            black_box(load)
        });
    });
}

fn benchmark_search(c: &mut Criterion) {
    // Heuristic search for 4 nodes
    c.bench_function("search_4_nodes", |b| {
        let nodes: Vec<_> = (0..4).map(Node::new).collect();
        let config = SearchConfig {
            optimize: Objective::Load,
            resilience: 1,
            read_fraction: Some(Distribution::Fixed(0.5)),
            timeout: Duration::from_secs(2),
            ..Default::default()
        };

        b.iter(|| {
            let result = search(nodes.clone(), config.clone());
            black_box(result)
        });
    });
}

criterion_group!(
    benches,
    benchmark_quorum_enumeration,
    benchmark_resilience,
    benchmark_strategy_optimization,
    benchmark_load_calculation,
    benchmark_search
);
criterion_main!(benches);
