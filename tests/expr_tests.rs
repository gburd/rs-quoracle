//! Integration tests for the expression algebra module

#![allow(clippy::expect_used, clippy::unwrap_used)]

use quoracle::expr::{choose, majority, Node};
use quoracle::Expr;
use std::collections::HashSet;

fn n(x: &str) -> Expr<String> {
    Expr::Node(Node::new(x.to_string()))
}

fn set(items: &[&str]) -> HashSet<String> {
    items.iter().map(|s| (*s).to_string()).collect()
}

fn quorum_set(e: &Expr<String>) -> HashSet<Vec<String>> {
    e.quorums()
        .map(|q| {
            let mut v: Vec<String> = q.into_iter().collect();
            v.sort();
            v
        })
        .collect()
}

fn sorted_set(items: &[&str]) -> Vec<String> {
    let mut v: Vec<String> = items.iter().map(|s| (*s).to_string()).collect();
    v.sort();
    v.dedup();
    v
}

fn assert_quorums(e: &Expr<String>, expected: &[&[&str]]) {
    let got = quorum_set(e);
    let want: HashSet<Vec<String>> = expected.iter().map(|s| sorted_set(s)).collect();
    assert_eq!(got, want, "quorums mismatch");
}

// -- quorums tests --

#[test]
fn test_quorums_or() {
    let e = n("a") + n("b") + n("c");
    assert_quorums(&e, &[&["a"], &["b"], &["c"]]);
}

#[test]
fn test_quorums_and() {
    let e = n("a") * n("b") * n("c");
    assert_quorums(&e, &[&["a", "b", "c"]]);
}

#[test]
fn test_quorums_mixed() {
    let e = n("a") + n("b") * n("c");
    assert_quorums(&e, &[&["a"], &["b", "c"]]);
}

#[test]
fn test_quorums_dup_and() {
    let e = n("a") * n("a") * n("a");
    assert_quorums(&e, &[&["a"]]);
}

#[test]
fn test_quorums_dup_or() {
    let e = n("a") + n("a") + n("a");
    assert_quorums(&e, &[&["a"]]);
}

#[test]
fn test_quorums_node_times_or() {
    let e = n("a") * (n("a") + n("b"));
    assert_quorums(&e, &[&["a"], &["a", "b"]]);
}

#[test]
fn test_quorums_choose_1() {
    let e = choose(1, vec![n("a"), n("b"), n("c")]).expect("valid");
    assert_quorums(&e, &[&["a"], &["b"], &["c"]]);
}

#[test]
fn test_quorums_choose_2() {
    let e = choose(2, vec![n("a"), n("b"), n("c")]).expect("valid");
    assert_quorums(&e, &[&["a", "b"], &["a", "c"], &["b", "c"]]);
}

#[test]
fn test_quorums_choose_3() {
    let e = choose(3, vec![n("a"), n("b"), n("c")]).expect("valid");
    assert_quorums(&e, &[&["a", "b", "c"]]);
}

#[test]
fn test_quorums_cross_product() {
    let e = (n("a") + n("b")) * (n("c") + n("d"));
    assert_quorums(&e, &[&["a", "c"], &["a", "d"], &["b", "c"], &["b", "d"]]);
}

#[test]
fn test_quorums_cross_product_dup() {
    let e = (n("a") + n("b")) * (n("a") + n("c"));
    assert_quorums(&e, &[&["a"], &["a", "c"], &["a", "b"], &["b", "c"]]);
}

#[test]
fn test_quorums_nested_choose() {
    let e = choose(
        2,
        vec![
            choose(2, vec![n("a"), n("b"), n("c")]).expect("valid"),
            choose(2, vec![n("d"), n("e"), n("f")]).expect("valid"),
            choose(2, vec![n("a"), n("c"), n("e")]).expect("valid"),
        ],
    )
    .expect("valid");

    let qs = quorum_set(&e);
    assert!(qs.contains(&sorted_set(&["a", "b", "d", "e"])));
    assert!(qs.contains(&sorted_set(&["b", "c", "d", "f"])));
    assert!(qs.contains(&sorted_set(&["a", "c", "e", "f"])));
}

// -- is_quorum tests --

#[test]
fn test_is_quorum_or() {
    let expr = n("a") + n("b") + n("c");
    assert!(expr.is_quorum(&set(&["a"])));
    assert!(expr.is_quorum(&set(&["b"])));
    assert!(expr.is_quorum(&set(&["c"])));
    assert!(expr.is_quorum(&set(&["a", "b"])));
    assert!(expr.is_quorum(&set(&["a", "c"])));
    assert!(expr.is_quorum(&set(&["b", "c"])));
    assert!(expr.is_quorum(&set(&["a", "b", "c"])));
    assert!(expr.is_quorum(&set(&["a", "x"])));
    assert!(!expr.is_quorum(&set(&[])));
    assert!(!expr.is_quorum(&set(&["x"])));
}

#[test]
fn test_is_quorum_and() {
    let expr = n("a") * n("b") * n("c");
    assert!(expr.is_quorum(&set(&["a", "b", "c"])));
    assert!(expr.is_quorum(&set(&["a", "b", "c", "x"])));
    assert!(!expr.is_quorum(&set(&[])));
    assert!(!expr.is_quorum(&set(&["a"])));
    assert!(!expr.is_quorum(&set(&["b"])));
    assert!(!expr.is_quorum(&set(&["c"])));
    assert!(!expr.is_quorum(&set(&["a", "b"])));
    assert!(!expr.is_quorum(&set(&["a", "c"])));
    assert!(!expr.is_quorum(&set(&["b", "c"])));
    assert!(!expr.is_quorum(&set(&["x"])));
    assert!(!expr.is_quorum(&set(&["a", "x"])));
}

#[test]
fn test_is_quorum_choose() {
    let expr = choose(2, vec![n("a"), n("b"), n("c")]).expect("valid");
    assert!(expr.is_quorum(&set(&["a", "b"])));
    assert!(expr.is_quorum(&set(&["a", "c"])));
    assert!(expr.is_quorum(&set(&["b", "c"])));
    assert!(expr.is_quorum(&set(&["a", "b", "c"])));
    assert!(expr.is_quorum(&set(&["a", "b", "c", "x"])));
    assert!(!expr.is_quorum(&set(&["a"])));
    assert!(!expr.is_quorum(&set(&["b"])));
    assert!(!expr.is_quorum(&set(&["c"])));
    assert!(!expr.is_quorum(&set(&["x"])));
}

#[test]
fn test_is_quorum_cross_product() {
    let expr = (n("a") + n("b")) * (n("c") + n("d"));
    assert!(expr.is_quorum(&set(&["a", "c"])));
    assert!(expr.is_quorum(&set(&["a", "d"])));
    assert!(expr.is_quorum(&set(&["b", "c"])));
    assert!(expr.is_quorum(&set(&["b", "d"])));
    assert!(expr.is_quorum(&set(&["a", "b", "d"])));
    assert!(expr.is_quorum(&set(&["b", "c", "d"])));
    assert!(expr.is_quorum(&set(&["a", "c", "d"])));
    assert!(expr.is_quorum(&set(&["a", "b", "c", "d"])));
    assert!(!expr.is_quorum(&set(&["a"])));
    assert!(!expr.is_quorum(&set(&["b"])));
    assert!(!expr.is_quorum(&set(&["c"])));
    assert!(!expr.is_quorum(&set(&["d"])));
    assert!(!expr.is_quorum(&set(&["a", "b"])));
    assert!(!expr.is_quorum(&set(&["c", "d"])));
    assert!(!expr.is_quorum(&set(&["a", "b", "x"])));
}

// -- resilience tests --

#[test]
fn test_resilience_single() {
    assert_eq!(n("a").resilience(), 0);
}

#[test]
fn test_resilience_or() {
    assert_eq!((n("a") + n("b")).resilience(), 1);
    assert_eq!((n("a") + n("b") + n("c")).resilience(), 2);
    assert_eq!((n("a") + n("b") + n("c") + n("d")).resilience(), 3);
}

#[test]
fn test_resilience_and() {
    assert_eq!((n("a") * n("b")).resilience(), 0);
    assert_eq!((n("a") * n("b") * n("c")).resilience(), 0);
    assert_eq!((n("a") * n("b") * n("c") * n("d")).resilience(), 0);
}

#[test]
fn test_resilience_mixed() {
    assert_eq!(((n("a") + n("b")) * (n("c") + n("d"))).resilience(), 1);
    assert_eq!(
        ((n("a") + n("b") + n("c")) * (n("d") + n("e") + n("f"))).resilience(),
        2
    );
}

#[test]
fn test_resilience_dup() {
    assert_eq!(
        ((n("a") + n("b") + n("c")) * (n("a") + n("e") + n("f"))).resilience(),
        2
    );
    assert_eq!(
        ((n("a") + n("a") + n("c")) * (n("d") + n("e") + n("f"))).resilience(),
        1
    );
    assert_eq!(
        ((n("a") + n("a") + n("a")) * (n("d") + n("e") + n("f"))).resilience(),
        0
    );
    assert_eq!(
        (n("a") * n("b") + n("b") * n("c") + n("a") * n("d") + n("a") * n("d") * n("e"))
            .resilience(),
        1
    );
}

#[test]
fn test_resilience_choose() {
    let ch2_3 = choose(2, vec![n("a"), n("b"), n("c")]).expect("valid");
    assert_eq!(ch2_3.resilience(), 1);

    let ch2_5 = choose(2, vec![n("a"), n("b"), n("c"), n("d"), n("e")]).expect("valid");
    assert_eq!(ch2_5.resilience(), 3);

    let ch3_5 = choose(3, vec![n("a"), n("b"), n("c"), n("d"), n("e")]).expect("valid");
    assert_eq!(ch3_5.resilience(), 2);

    let ch4_5 = choose(4, vec![n("a"), n("b"), n("c"), n("d"), n("e")]).expect("valid");
    assert_eq!(ch4_5.resilience(), 1);
}

#[test]
fn test_resilience_choose_compound() {
    let e1 = choose(2, vec![n("a") + n("b") + n("c"), n("d") + n("e"), n("f")]).expect("valid");
    assert_eq!(e1.resilience(), 2);

    let e2 = choose(2, vec![n("a") * n("b"), n("a") * n("c"), n("d")]).expect("valid");
    assert_eq!(e2.resilience(), 0);

    let e3 = choose(2, vec![n("a") + n("b"), n("a") + n("c"), n("a") + n("d")]).expect("valid");
    assert_eq!(e3.resilience(), 2);
}

// -- dual tests --

fn assert_dual(x: &Expr<String>, y: &Expr<String>) {
    let x_dual = x.dual();
    let x_qs = quorum_set(&x_dual);
    let y_qs = quorum_set(y);
    assert_eq!(x_qs, y_qs, "dual mismatch");
}

#[test]
fn test_dual_node() {
    assert_dual(&n("a"), &n("a"));
}

#[test]
fn test_dual_or_and() {
    assert_dual(&(n("a") + n("b")), &(n("a") * n("b")));
}

#[test]
fn test_dual_dup() {
    assert_dual(&(n("a") + n("a")), &(n("a") * n("a")));
}

#[test]
fn test_dual_compound() {
    assert_dual(
        &((n("a") + n("b")) * (n("c") + n("d"))),
        &((n("a") * n("b")) + (n("c") * n("d"))),
    );
    assert_dual(
        &((n("a") + n("b")) * (n("a") + n("d"))),
        &((n("a") * n("b")) + (n("a") * n("d"))),
    );
    assert_dual(
        &((n("a") + n("b")) * (n("a") + n("a"))),
        &((n("a") * n("b")) + (n("a") * n("a"))),
    );
    assert_dual(
        &((n("a") + n("a")) * (n("a") + n("a"))),
        &((n("a") * n("a")) + (n("a") * n("a"))),
    );
}

#[test]
fn test_dual_nested() {
    assert_dual(
        &((n("a") + (n("a") * n("b"))) + ((n("c") * n("d")) + n("a"))),
        &((n("a") * (n("a") + n("b"))) * ((n("c") + n("d")) * n("a"))),
    );
}

#[test]
fn test_dual_choose() {
    let ch2_3 = choose(2, vec![n("a"), n("b"), n("c")]).expect("valid");
    let ch2_3b = choose(2, vec![n("a"), n("b"), n("c")]).expect("valid");
    assert_dual(&ch2_3, &ch2_3b);

    let ch2_ab_cd_e = choose(2, vec![n("a") + n("b"), n("c") + n("d"), n("e")]).expect("valid");
    let ch2_ab_cd_e_dual =
        choose(2, vec![n("a") * n("b"), n("c") * n("d"), n("e")]).expect("valid");
    assert_dual(&ch2_ab_cd_e, &ch2_ab_cd_e_dual);

    let ch3_5 = choose(3, vec![n("a"), n("b"), n("c"), n("d"), n("e")]).expect("valid");
    let ch3_5b = choose(3, vec![n("a"), n("b"), n("c"), n("d"), n("e")]).expect("valid");
    assert_dual(&ch3_5, &ch3_5b);

    let ch2_5 = choose(2, vec![n("a"), n("b"), n("c"), n("d"), n("e")]).expect("valid");
    let ch4_5 = choose(4, vec![n("a"), n("b"), n("c"), n("d"), n("e")]).expect("valid");
    assert_dual(&ch2_5, &ch4_5);
    assert_dual(&ch4_5, &ch2_5);
}

// -- dup_free tests --

#[test]
fn test_dup_free() {
    assert!(n("a").dup_free());
    assert!((n("a") + n("b")).dup_free());
    assert!((n("a") * n("b")).dup_free());
    assert!((n("a") * n("b") + n("c")).dup_free());

    let ch = choose(2, vec![n("a"), n("b"), n("c")]).expect("valid");
    assert!(ch.dup_free());

    let ch2 = choose(2, vec![n("a") * n("b"), n("c"), n("d") + n("e") + n("f")]).expect("valid");
    assert!(ch2.dup_free());

    let ch3 = choose(3, vec![n("a"), n("b"), n("c"), n("d"), n("e")]).expect("valid");
    assert!(ch3.dup_free());

    assert!(((n("a") + n("b")) * (n("c") + (n("d") * n("e")))).dup_free());
}

#[test]
fn test_not_dup_free() {
    assert!(!(n("a") + n("a")).dup_free());
    assert!(!(n("a") * n("a")).dup_free());
    assert!(!(n("a") * (n("b") + n("a"))).dup_free());

    let ch = choose(2, vec![n("a"), n("b"), n("a")]).expect("valid");
    assert!(!ch.dup_free());

    let ch2 = choose(3, vec![n("a"), n("b"), n("c"), n("d"), n("a")]).expect("valid");
    assert!(!ch2.dup_free());

    assert!(!((n("a") + n("b")) * (n("c") + (n("d") * n("a")))).dup_free());
}

// -- choose/majority helper tests --

#[test]
fn test_choose_returns_or_for_k1() {
    let e = choose(1, vec![n("a"), n("b"), n("c")]).expect("valid");
    assert!(matches!(e, Expr::Or(_)));
}

#[test]
fn test_choose_returns_and_for_k_eq_n() {
    let e = choose(3, vec![n("a"), n("b"), n("c")]).expect("valid");
    assert!(matches!(e, Expr::And(_)));
}

#[test]
fn test_choose_returns_choose_for_middle_k() {
    let e = choose(2, vec![n("a"), n("b"), n("c")]).expect("valid");
    assert!(matches!(e, Expr::Choose(_)));
}

#[test]
fn test_choose_errors() {
    assert!(choose::<String>(0, vec![]).is_err());
    assert!(choose(0, vec![n("a")]).is_err());
    assert!(choose(2, vec![n("a")]).is_err());
}

#[test]
fn test_majority() {
    let e = majority(vec![n("a"), n("b"), n("c")]).expect("valid");
    assert_quorums(&e, &[&["a", "b"], &["a", "c"], &["b", "c"]]);
}
