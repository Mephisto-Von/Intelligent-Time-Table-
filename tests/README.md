# TimetableMaster end-to-end tests

Black-box suites that exercise the running app the way users do: over HTTP and
in a real headless Chromium. No unit tests here — see `backend/src/solver/tests.rs`
for the solver's own tests.

## Quick start

```sh
tests/run.sh                 # builds backend + frontend, runs everything
SKIP_BUILD=1 tests/run.sh    # reuse an existing build (much faster)
SKIP_BROWSER=1 tests/run.sh  # HTTP suites only
```

The runner:

1. builds `backend` (cargo) and `frontend` (vite),
2. starts `tests/webhook-receiver.mjs` on `:9098` and the backend on `:8126`
   (override with `PORT` / `WEBHOOK_PORT`) against a throwaway SQLite DB in
   `$TMPDIR/ttm-e2e.*`, seeded with the demo school (`--seed-demo`) and
   `NOTIFY_WEBHOOK_URL` pointed at the stub,
3. runs the three HTTP suites, then the three browser suites (stopping at the
   first failing suite — fix it and re-run, or run a single suite by hand as
   described below),
4. prints per-suite pass/fail and keeps logs (`backend.log`, `webhook.log`) in
   the work dir it prints at the end.

To iterate on one suite while a server is already running:

```sh
(cd tests/http && python3 suite_core.py http://localhost:8126)
(cd tests/browser && BASE=http://localhost:8126 node settings.spec.mjs)
```

Nothing outside `$TMPDIR` is touched — the real `data/timetable.db` and
`jwt_secret.txt` are never used (the DB path and `TIMETABLE_JWT_SECRET` are
both overridden).

## Layout

| Path | What it covers |
| --- | --- |
| `run.sh` | Runner described above. |
| `webhook-receiver.mjs` | Zero-dependency Node HTTP stub that records every webhook POST as one JSON line to `webhook.log`. |
| `http/common.py` | Stdlib-only helpers: tiny JSON client, `check`/`summary` assertion helpers, demo credentials, webhook log reader. Suites take the base URL as `argv[1]` or `BASE` env. |
| `http/suite_core.py` | Auth (login/roles), school + user CRUD, RBAC, setup CRUD, timetable shape, entry conflicts, auto-generation, two-week cycle. |
| `http/suite_features.py` | Audit log, notification status/test (verified against `webhook.log`), public share links (create/resolve/revoke), CSV/ICS/PDF exports, calendar events. |
| `http/suite_cover_leave.py` | Teacher leave request → admin approve → cover suggestions → assigning a substitute, with role checks and seeded-data sanity checks. |
| `browser/package.json` | Local Playwright dependency (installed on first `run.sh` invocation). |
| `browser/drag-and-drop.spec.mjs` | Timetable page: logs in as the demo admin, opens a class view, drags a lesson onto an empty legal slot, then verifies the move reached the API, that exactly one move request was sent per attempt, and that the grid repainted both cells. |
| `browser/public-page.spec.mjs` | Creates a share link through the API, opens `/public/:token` with no session, checks the read-only grid shows exactly the shared class's lessons and is not editable, then revokes the link and verifies the page dies. |
| `browser/settings.spec.mjs` | Settings page: notification channel and event list, "send test notification", saving the school settings appears in the activity log (and the demo name is restored afterwards), then creating, copying and revoking a public link from the timetable Share dialog. |

## Requirements

- Rust + cargo, Node + npm (backend and browser suites)
- Python 3 (stdlib only — no pip installs needed)
- Playwright Chromium (auto-installed by `run.sh` into `tests/browser`)
