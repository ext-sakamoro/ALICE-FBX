#!/usr/bin/env bash
# scripts/preflight.sh — the CI steps of this repository, run locally before a push.
#
# usage: scripts/preflight.sh [--quick]
#   --quick: everything except the fuzz smoke runs and cargo audit / deny (network)
set -euo pipefail
cd "$(dirname "$0")/.."
quick=0
[[ "${1:-}" == "--quick" ]] && quick=1
export CARGO_TERM_COLOR=always

step() { printf '\n== %s\n' "$1"; }

step "ci.yml / lint: fmt"
cargo fmt --all -- --check
step "ci.yml / lint: clippy"
cargo clippy --all-targets -- -D warnings

step "ci.yml / test: fixtures are the writer's output"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
python3 scripts/fbx_fixture.py "$tmp" >/dev/null
n=0
for f in "$tmp"/*.fbx; do
  cmp "$f" "tests/data/$(basename "$f")"
  n=$((n + 1))
done
[ "$n" -ge 14 ] || { echo "only $n fixtures compared"; exit 1; }
echo "$n fixtures equal"

step "ci.yml / test: tests with per-binary minimums"
python3 scripts/test_min_passed.py
cargo test 2>&1 | python3 scripts/min_passed.py tree=9 scene=7 parity=1

step "ci.yml / msrv: build with rust-version 1.85"
if rustup toolchain list | grep -q '^1\.85'; then
  cargo +1.85 build --lib
else
  echo "toolchain 1.85 not installed here (rustup toolchain install 1.85 --profile minimal): skipped, CI runs it"
fi

step "security-audit.yml / stub-guard"
if grep -rnE 'todo!|unimplemented!|panic!\(".*(stub|not implemented)' src; then exit 1; fi
step "security-audit.yml / machete"
cargo machete

step "public-text.yml"
python3 scripts/test_public_text.py
PUBLIC_TEXT_BASE=${PUBLIC_TEXT_BASE:-$(git rev-list --max-parents=0 HEAD | tail -1)} scripts/public_text.sh

if [[ "$quick" == 1 ]]; then
  echo
  echo "preflight --quick OK (fuzz, audit and deny skipped)"
  exit 0
fi

step "security-audit.yml / audit + deny"
cargo generate-lockfile
cargo audit
cargo deny check all

step "fuzz.yml: 30 s per target"
for t in fuzz_read_tree fuzz_read_tree_versioned fuzz_read_scene; do
  mkdir -p "fuzz/corpus/$t"
  for f in tests/data/*.fbx; do
    if [ "$t" = fuzz_read_tree ]; then cp "$f" "fuzz/corpus/$t/"; else tail -c +28 "$f" > "fuzz/corpus/$t/$(basename "$f")"; fi
  done
  cargo +nightly fuzz run "$t" -- -max_total_time=30 -rss_limit_mb=2048
done

echo
echo "preflight OK"
