"""Shared helpers for the TimetableMaster HTTP suites (stdlib only).

Run from tests/http with:

    python3 suite_core.py [BASE]          # BASE defaults to http://localhost:8126
"""

import datetime as dt
import json
import os
import sys
import urllib.error
import urllib.request

BASE = sys.argv[1].rstrip("/") if len(sys.argv) > 1 else os.environ.get("BASE", "http://localhost:8126").rstrip("/")

# Demo accounts seeded by `seed_demo` (see backend/src/seed.rs).
ADMIN = ("admin@demo.school", "demo1234")
SCHEDULER = ("scheduler@demo.school", "demo1234")
TEACHER = ("david.chen@demo.school", "demo1234")
STUDENT = ("student@demo.school", "demo1234")

WEBHOOK_LOG = os.environ.get("WEBHOOK_LOG", "")

_failures: list[str] = []
_count = 0


def check(name: str, cond: bool, extra: str = "") -> bool:
    """Record one assertion. All suites must end with summary()."""
    global _count
    _count += 1
    if cond:
        print(f"PASS {name}")
    else:
        _failures.append(name)
        print(f"FAIL {name}" + (f"  -> {extra}" if extra else ""))
    return bool(cond)


def summary(label: str) -> None:
    print(f"\n===== {label}: {_count - len(_failures)}/{_count} passed =====")
    if _failures:
        print("FAILED:")
        for f in _failures:
            print(f"  - {f}")
        sys.exit(1)
    sys.exit(0)


class Api:
    """Tiny JSON client that keeps the response status, body and content type."""

    def __init__(self, base: str = BASE):
        self.base = base.rstrip("/")
        self.token: str | None = None

    def login(self, email: str, password: str) -> dict:
        status, body, _ = self.req("POST", "/api/auth/login", body={"email": email, "password": password})
        assert status == 200, f"login failed for {email}: {status} {body}"
        self.token = body["token"]
        return body

    def as_admin(self) -> "Api":
        api = Api(self.base)
        api.login(*ADMIN)
        return api

    def req(self, method: str, path: str, token: str | None = None, body: dict | None = None, raw: bool = False):
        """Returns (status, parsed_json_or_text, content_type); raw=True keeps bytes."""
        headers: dict[str, str] = {}
        use_token = token if token is not None else self.token
        if use_token:
            headers["Authorization"] = f"Bearer {use_token}"
        data = None
        if body is not None:
            data = json.dumps(body).encode()
            headers["Content-Type"] = "application/json"
        r = urllib.request.Request(self.base + path, data=data, headers=headers, method=method)
        try:
            with urllib.request.urlopen(r, timeout=60) as resp:
                status, content, ctype = resp.status, resp.read(), resp.headers.get("Content-Type", "")
        except urllib.error.HTTPError as e:
            status, content, ctype = e.code, e.read(), e.headers.get("Content-Type", "")
        if raw:
            return status, content, ctype
        raw = content.decode("utf-8", "replace")
        parsed: object
        if "application/json" in ctype:
            try:
                parsed = json.loads(raw)
            except json.JSONDecodeError:
                parsed = {}
        else:
            parsed = raw
        return status, parsed, ctype


def webhook_events() -> list[dict]:
    """Reads the JSON payloads captured by tests/webhook-receiver.mjs."""
    if not WEBHOOK_LOG or not os.path.exists(WEBHOOK_LOG):
        return []
    events = []
    with open(WEBHOOK_LOG) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                events.append(json.loads(line))
            except json.JSONDecodeError:
                pass
    return events


def next_weekday() -> str:
    """The coming Monday as YYYY-MM-DD (today itself excluded)."""
    t = dt.date.today()
    days = 7 - t.weekday()
    if days == 7:
        days = 7
    t += dt.timedelta(days=days)
    return t.isoformat()


def plus_days(iso_date: str, n: int) -> str:
    d = dt.date.fromisoformat(iso_date) + dt.timedelta(days=n)
    return d.isoformat()
