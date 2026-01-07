#!/usr/bin/env bash
set -euo pipefail

CRITERION_DIR="${1:-target/criterion}"

if [[ ! -d "$CRITERION_DIR" ]]; then
  echo "error: criterion dir not found: $CRITERION_DIR" >&2
  echo "usage: $0 [criterion_dir]" >&2
  exit 2
fi

## Yes i got lazy sue me

python3 - "$CRITERION_DIR" <<'PY'
import signal
import json
import os
import sys

signal.signal(signal.SIGPIPE, signal.SIG_DFL)

criterion_dir = sys.argv[1]

records = []

for root, dirs, files in os.walk(criterion_dir):
    if os.path.basename(root) != "new":
        continue
    if "benchmark.json" not in files or "estimates.json" not in files:
        continue

    bench_path = os.path.join(root, "benchmark.json")
    est_path = os.path.join(root, "estimates.json")

    try:
        with open(bench_path, "r", encoding="utf-8") as f:
            bench = json.load(f)
        with open(est_path, "r", encoding="utf-8") as f:
            est = json.load(f)
    except Exception:
        continue

    full_id = bench.get("full_id") or bench.get("title") or ""
    group_id = bench.get("group_id") or ""

    mean = est.get("mean") or {}
    ci = (mean.get("confidence_interval") or {})
    point = mean.get("point_estimate")
    lo = ci.get("lower_bound")
    hi = ci.get("upper_bound")

    if point is None:
        continue

    records.append(
        {
            "group_id": group_id,
            "full_id": full_id,
            "mean_ns": float(point),
            "ci_lo_ns": float(lo) if lo is not None else None,
            "ci_hi_ns": float(hi) if hi is not None else None,
        }
    )


pairs = {}  # (group_id, key) -> {"old": rec, "new": rec}
singles = []

for rec in records:
    full_id = rec["full_id"]
    group_id = rec["group_id"]
    parts = full_id.split("/") if full_id else []

    if not parts:
        singles.append(rec)
        continue

    if group_id and parts[0] == group_id:
        rest = parts[1:]
    else:
        rest = parts

    if not rest:
        singles.append(rec)
        continue

    variant = rest[0]
    if variant in ("old", "new"):
        key = "/".join(rest[1:])
        bucket = pairs.setdefault((group_id or parts[0], key), {})
        bucket[variant] = rec
    else:
        singles.append(rec)

print(f"Criterion summary from: {criterion_dir}\n")

pair_rows = []
for (group_id, key), bucket in pairs.items():
    if "old" in bucket and "new" in bucket:
        old = bucket["old"]
        new = bucket["new"]
        speedup = old["mean_ns"] / new["mean_ns"] if new["mean_ns"] else float("inf")
        delta_pct = (new["mean_ns"] - old["mean_ns"]) / old["mean_ns"] * 100.0 if old["mean_ns"] else 0.0
        pair_rows.append((group_id, key, old["mean_ns"], new["mean_ns"], speedup, delta_pct))

pair_rows.sort(key=lambda r: (r[0], r[1]))

if pair_rows:
    print("OLD vs NEW (mean time; lower is better)")
    print(f"{'group':<18} {'case':<44} {'old(ns)':>10} {'new(ns)':>10} {'speedup':>9} {'delta%':>8}")
    for group, key, old_ns, new_ns, speedup, delta_pct in pair_rows:
        print(f"{group:<18} {key:<44} {old_ns:>10.4f} {new_ns:>10.4f} {speedup:>9.3f} {delta_pct:>8.2f}")
    print("")

unmatched = []
for (group_id, key), bucket in pairs.items():
    if "old" in bucket and "new" not in bucket:
        unmatched.append((group_id, key, "missing new"))
    if "new" in bucket and "old" not in bucket:
        unmatched.append((group_id, key, "missing old"))

unmatched.sort(key=lambda r: (r[0], r[1]))
if unmatched:
    print("Unmatched variants (likely due to timeout):")
    for group, key, why in unmatched[:50]:
        print(f"- {group}/{key}: {why}")
    if len(unmatched) > 50:
        print(f"  ... {len(unmatched) - 50} more")
    print("")

if singles:
    singles.sort(key=lambda r: (r["group_id"], r["full_id"]))
    print("Other benchmarks (no old/new pairing detected):")
    print(f"{'group':<18} {'id':<60} {'mean(ns)':>10}")
    for rec in singles[:200]:
        print(f"{rec['group_id']:<18} {rec['full_id']:<60} {rec['mean_ns']:>10.4f}")
    if len(singles) > 200:
        print(f"  ... {len(singles) - 200} more")
PY
