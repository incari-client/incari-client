#!/usr/bin/env bash
# Fails if a feature has no Rust status in features/README.md (ADR 0068): every features/<name>
# has a row there, and every row's last column says Ported, Not yet or Won't port. Keeps the Rust
# build's gaps written down as features are added.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

table=features/README.md
problems=$(
  # Tracked modules only: a renamed module can leave an ignored build/ folder behind.
  for build in $(git ls-files 'features/*/build.gradle.kts'); do
    name=$(basename "$(dirname "$build")")
    grep -q "](${name}/README.md)" "$table" || echo "features/${name} has no row in ${table}"
  done
  awk -F'|' '/^\| \[/ {
    status = $(NF - 1); gsub(/^ +| +$/, "", status)
    if (status != "Ported" && status != "Not yet" && status != "Won'"'"'t port") print "no Rust status: " $0
  }' "$table"
)

if [ -n "$problems" ]; then
  echo "$problems"
  echo "Give each feature a Rust column in ${table}: Ported, Not yet or Won't port." >&2
  exit 1
fi
echo "Every feature has a Rust status."
