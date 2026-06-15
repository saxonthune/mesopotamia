#!/usr/bin/env bash
#
# Full smoke-test suite for mesopotamia. Three gates, in order:
#   1. unit tests        — cargo test (both binaries; field/elk/ui pure-fn suites)
#   2. build             — compile each binary's dev profile
#   3. runtime launch    — boot each app, confirm the window comes up with no
#                          panic and no wgpu scissor crash, then shut it down
#
# Every gate runs even if an earlier one fails; the script reports all results and
# exits non-zero if any failed — so a red bar means "something broke," and the
# section that printed FAIL says what.
#
# Usage: ./smoke.sh            # full suite
#        ./smoke.sh --no-run   # skip the runtime launch (headless / CI)
#
# The runtime launch opens a real window per binary, so run it on a machine with a
# display.

set -u

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

BINS=(demo1 mesopotamia)
RUN=1
[ "${1:-}" = "--no-run" ] && RUN=0

fail=0
step() { printf '\n\033[1m=== %s ===\033[0m\n' "$1"; }

# --- Gate 1: unit tests -----------------------------------------------------
step "unit tests (cargo test)"
if cargo test --quiet; then
  echo "unit tests: PASS"
else
  echo "unit tests: FAIL"
  fail=1
fi

# --- Gate 2: build ----------------------------------------------------------
step "build (cargo build)"
for bin in "${BINS[@]}"; do
  if cargo build --quiet --bin "$bin"; then
    echo "build $bin: PASS"
  else
    echo "build $bin: FAIL"
    fail=1
  fi
done

# --- Gate 3: runtime launch smoke ------------------------------------------
# Boot the already-built binary directly (so we get its real pid for a clean
# kill, and the window appears in ~1s rather than after a recompile), wait for
# the window-created log line, let a few frames render, then stop it.
smoke_run() {
  local bin="$1"
  local exe="$ROOT/target/debug/$bin"
  local log; log="$(mktemp -t "meso_smoke_${bin}.XXXXXX")"
  step "runtime launch: $bin"

  if [ ! -x "$exe" ]; then
    echo "$bin: FAIL — binary not built ($exe)"
    fail=1
    return
  fi

  "$exe" >"$log" 2>&1 &
  local pid=$!

  local i=0
  while [ $i -lt 30 ]; do
    grep -qiE 'Creating new window' "$log" && break
    grep -qiE 'panic'               "$log" && break
    kill -0 "$pid" 2>/dev/null || break   # process died on its own
    i=$((i + 1)); sleep 1
  done
  sleep 3   # let a few frames render so a render-time crash has a chance to fire

  kill "$pid" 2>/dev/null
  wait "$pid" 2>/dev/null

  if grep -qiE 'Creating new window' "$log" && ! grep -qiE 'panic|scissor|not contained' "$log"; then
    echo "$bin: PASS (window up, no panic/scissor)"
  else
    echo "$bin: FAIL — see $log"
    grep -iE 'panic|scissor|not contained|error' "$log" | head -10
    fail=1
  fi
}

if [ "$RUN" -eq 1 ]; then
  for bin in "${BINS[@]}"; do
    smoke_run "$bin"
  done
else
  step "runtime launch"
  echo "skipped (--no-run)"
fi

# --- Verdict ----------------------------------------------------------------
if [ "$fail" -eq 0 ]; then
  printf '\n\033[1;32mALL SMOKE CHECKS PASSED\033[0m\n'
else
  printf '\n\033[1;31mSMOKE CHECKS FAILED\033[0m\n'
fi
exit "$fail"
