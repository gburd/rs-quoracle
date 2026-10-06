# Quoracle

Quoracle is a Rust library for designing and analyzing **read-write quorum
systems**. A quorum system decides which replicas a read must contact and
which a write must contact, so that every read sees the latest write.

Given a quorum system and a workload, Quoracle computes its fault
tolerance, load, capacity, network cost, and latency. It finds the
strategy (the probability of picking each quorum) that optimizes any of
those, and it can search for the best quorum system over a set of nodes.

It is a Rust port of the Python [Quoracle](https://github.com/mwhittaker/quoracle)
library from [*Read-Write Quorum Systems Made
Practical*](https://mwhittaker.github.io/publications/quoracle.pdf)
(PaPoC 2021).

## Why not just use majorities?

Majority quorums are safe and simple, but they are rarely the
highest-throughput choice. With 9 identical nodes:

| Read fraction | Majority | 3×3 grid | Read-one / write-all |
|---|---|---|---|
| 50% | 1.80× | **3.00×** | 1.80× |
| 90% | 1.80× | 3.00× | **5.00×** |
| 99% | 1.80× | 3.00× | **8.33×** |

(Capacity relative to one node, using each system's optimal strategy.)
Quoracle computes numbers like these for your nodes and your workload.

## Links

- Source: [codeberg.org/gregburd/rs-quoracle](https://codeberg.org/gregburd/rs-quoracle)
  (mirror: [github.com/gburd/rs-quoracle](https://github.com/gburd/rs-quoracle))
- Crate: [crates.io/crates/quoracle](https://crates.io/crates/quoracle)
- API: [docs.rs/quoracle](https://docs.rs/quoracle)
