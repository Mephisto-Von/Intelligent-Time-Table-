#!/usr/bin/env bash
# TimetableMaster end-to-end runner.
#
#   tests/run.sh                # build + HTTP suites + browser suites
#   SKIP_BUILD=1 tests/run.sh   # reuse existing builds
#   SKIP_BROWSER=1 tests/run.sh # HTTP suites only (no Playwright)
#
# Boots the backend on PORT (default 8126) against a throwaway SQLite DB, with
# NOTIFY_WEBHOOK_URL pointing at the local webhook receiver, seeds the demo
# school, runs all suites, then tears everything down.
set -uo pipefail

PORT="${PORT:-8126}"
BASE="http://localhost:$PORT"
WEBHOOK_PORT="${WEBHOOK_PORT:-9098}"
SKIP_BUILD="${SKIP_BUILD:-0}"
SKIP_BROWSER="${SKIP_BROWSER:-0}"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/backend/target/debug/intelligent-timetable-backend"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/ttm-e2e.XXXXXX")"
WEBHOOK_LOG="$WORK/webhook.log"
BACKEND_LOG="$WORK/backend.log"

declare -a PIDS=()
cleanup() {
  for pid in ${PIDS[@]:-}; do
    kill "$pid" 2>/dev/null || true
  done
  wait 2>/dev/null || true
}
trap cleanup EXIT INT TERM

step() { printf '\n\033[1;36m== %s ==\033[0m\n' "$*"; }
fail() { printf '\n\033[0;31m== FAIL: %s ==\033[0m\n' "$*"; exit 1; }

# --------------------------------------------------------------------- builds
if [[ "$SKIP_BUILD" != 1 ]]; then
  step "Building backend"
  (cd "$ROOT/backend" && cargo build) || fail "cargo build"
  step "Building frontend"
  [[ -d "$ROOT/frontend/node_modules" ]] \
    || { (cd "$ROOT/frontend" && npm ci --no-fund --no-audit) || fail "npm ci (frontend)"; }
  (cd "$ROOT/frontend" && npm run build) || fail "npm run build"
else
  [[ -x "$BIN" ]] || fail "SKIP_BUILD=1 but $BIN is missing (run without SKIP_BUILD once)"
  [[ -f "$ROOT/frontend/dist/index.html" ]] || fail "SKIP_BUILD=1 but frontend/dist is missing"
fi

# ------------------------------------------------------------------- services
# A leftover server from an earlier run would answer our readiness probe and the
# suites would silently test the wrong process, against a database this run does
# not own. Refuse to start instead of reporting someone else's results.
for probe in "$BASE/api/audit" "http://localhost:$WEBHOOK_PORT/"; do
  if curl -s -o /dev/null -m 1 "$probe" 2>/dev/null; then
    fail "something is already listening on ${probe#http://localhost} — a previous run left a server behind (stop it, or rerun with PORT=… WEBHOOK_PORT=…)"
  fi
done

step "Starting services (API :$PORT, webhook stub :$WEBHOOK_PORT)"
node "$ROOT/tests/webhook-receiver.mjs" "$WEBHOOK_PORT" "$WEBHOOK_LOG" &
PIDS+=($!)

# exec so $! is the server itself: killing the subshell instead would leave the
# backend holding the port, and the next run would test that stale process.
(
  cd "$ROOT/backend" || exit 1
  exec env TIMETABLE_DB="$WORK/timetable.db" \
    TIMETABLE_JWT_SECRET="e2e-runner-secret-0123456789abcdef" \
    PORT="$PORT" \
    NOTIFY_WEBHOOK_URL="http://localhost:$WEBHOOK_PORT/hook" \
    "$BIN" --seed-demo
) >"$BACKEND_LOG" 2>&1 &
PIDS+=($!)

# No /api/health route: any HTTP response (even 401 from /api/audit) proves the
# server is up. Seeding happens before the listener binds, so ready == seeded.
for _ in $(seq 1 90); do
  curl -s -o /dev/null -m 2 "$BASE/api/audit" && break
  grep -q 'panicked\|demo seed failed\|Address already in use' "$BACKEND_LOG" 2>/dev/null && { cat "$BACKEND_LOG"; fail "backend failed to start"; }
  sleep 1
  [[ $_ == 90 ]] && { cat "$BACKEND_LOG"; fail "backend did not start within 90s"; }
done
echo "API ready at $BASE (webhook deliveries -> $WEBHOOK_LOG)"

export BASE
export WEBHOOK_LOG

# --------------------------------------------------------------- HTTP suites
# Full output goes to a per-suite log so a failure shows every check, while a
# passing suite stays short. The suite's own exit status is what decides.
run_suite() {
  local log="$WORK/$1.log"
  printf '\n-- %s --\n' "$1"
  (cd "$ROOT/tests/http" && python3 "$1" "$BASE") >"$log" 2>&1
  local rc=$?
  if (( rc != 0 )); then cat "$log"; else tail -3 "$log"; fi
  return "$rc"
}
run_suite suite_core.py        || fail "suite_core"
run_suite suite_features.py    || fail "suite_features"
run_suite suite_cover_leave.py || fail "suite_cover_leave"

# ------------------------------------------------------------- browser suites
if [[ "$SKIP_BROWSER" != 1 ]]; then
  step "Browser suites (headless Chromium via Playwright)"
  cd "$ROOT/tests/browser" || fail "tests/browser missing"
  [[ -d node_modules/playwright ]] || { npm install --no-fund --no-audit || fail "npm install (tests/browser)"; }
  npx playwright install chromium >/dev/null 2>&1 || true

  for spec in drag-and-drop public-page settings; do
    printf '\n-- %s --\n' "$spec"
    node "$spec.spec.mjs" || fail "browser suite $spec"
  done
  cd "$ROOT"
fi

# ------------------------------------------------------------------ summary
step "All suites finished"
echo "Webhook deliveries recorded: $(grep -c . "$WEBHOOK_LOG" 2>/dev/null || echo 0)"
echo "Artifacts kept in $WORK (backend.log, webhook.log)"
