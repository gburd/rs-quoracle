#!/usr/bin/env bash
# Measure coverage for every solver configuration and enforce the floor.
# Each configuration must reach the floor on its own, and so must the
# merged report. Needs a nightly toolchain (for branch coverage),
# cargo-llvm-cov, and the CBC library.
set -euo pipefail

FLOOR="${COVERAGE_FLOOR:-85}"
IGNORE='(tests|examples|benches)/'
OUT=target/coverage
mkdir -p "$OUT"

configs=("--no-default-features --features microlp"
         "--no-default-features --features cbc"
         "--all-features")

check() {
  local name=$1
  echo "== coverage: $name"
  cargo +nightly llvm-cov report --branch --ignore-filename-regex "$IGNORE"
  cargo +nightly llvm-cov report --branch --ignore-filename-regex "$IGNORE" \
    --json --summary-only --output-path "$OUT/$name.json"
  python3 - "$OUT/$name.json" "$FLOOR" <<'PY'
import json, sys
t = json.load(open(sys.argv[1]))["data"][0]["totals"]
floor = float(sys.argv[2])
bad = []
for k in ("lines", "functions", "branches"):
    pct = t[k]["percent"] if t[k]["count"] else 100.0
    print(f"  {k:9} {pct:6.2f}%")
    if pct < floor:
        bad.append(f"{k} {pct:.2f}% < {floor}%")
if bad:
    sys.exit("coverage below floor: " + ", ".join(bad))
PY
}

# Each configuration on its own.
for i in "${!configs[@]}"; do
  cargo +nightly llvm-cov clean --workspace
  # shellcheck disable=SC2086
  cargo +nightly llvm-cov --branch --no-report ${configs[$i]}
  check "config-$i"
done

# All configurations merged (profiles accumulate until the next clean).
cargo +nightly llvm-cov clean --workspace
for c in "${configs[@]}"; do
  # shellcheck disable=SC2086
  cargo +nightly llvm-cov --branch --no-report $c
done
check merged
cargo +nightly llvm-cov report --branch --ignore-filename-regex "$IGNORE" \
  --lcov --output-path "$OUT/lcov.info"
