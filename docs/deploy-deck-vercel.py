#!/usr/bin/env python3
"""Deploy the self-hosted deck (docs/deck/) to Vercel as a static preview.

Usage:
    VERCEL_TOKEN=… python3 docs/deploy-deck-vercel.py             # deploy + verify
    VERCEL_TOKEN=… python3 docs/deploy-deck-vercel.py --dry-run  # show the plan only
    python3 docs/deploy-deck-vercel.py --verify URL               # re-check a live deployment

Requires VERCEL_TOKEN (a token with deploy rights on the target team).
Optional: VERCEL_TEAM (team id; defaults to the token's own team),
VERCEL_PROJECT (defaults to intelligent-time-table-engine).

Creates a non-production preview deployment on the named project — nothing
existing is touched: no project is created, no production alias is moved, no
project settings are changed.

Two Vercel API quirks this script works around (both verified live):

1. files[].data is stored VERBATIM unless `encoding: "base64"` is also sent —
   without it the site serves the base64 text instead of the file.
2. The project's rootDirectory ("frontend") makes the build sandbox match only
   paths under frontend/; files anywhere else are silently not deployed.
   Hence both files are uploaded as frontend/… with a single static `**` build,
   which serves them at the site root.

After the deployment turns READY it fetches "/" and the .pptx anonymously and
compares their sha1 against the local files (the HTML check tolerates the
<script> tag Vercel appends to HTML responses).
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import os
import sys
import time
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DECK_DIR = ROOT / "docs" / "deck"
FILES = ["index.html", "FYP-Presentation.pptx"]
API = "https://api.vercel.com"


def call(token: str, url: str, data: dict | None = None) -> tuple[int, dict]:
    req = urllib.request.Request(
        url,
        data=json.dumps(data).encode() if data is not None else None,
        method="POST" if data is not None else "GET",
        headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            return resp.status, json.loads(resp.read().decode())
    except urllib.error.HTTPError as e:
        body = e.read().decode()
        try:
            return e.code, json.loads(body)
        except ValueError:
            return e.code, {"raw": body[:400]}


def fetch(url: str) -> tuple[int, str, bytes]:
    with urllib.request.urlopen(url, timeout=60) as resp:
        return resp.status, resp.headers.get("Content-Type", ""), resp.read()


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--dry-run", action="store_true", help="print the plan and exit")
    ap.add_argument("--verify", metavar="URL", help="verify an already-deployed deck and exit")
    args = ap.parse_args()

    if args.verify:
        ok = verify_bytes(args.verify.removeprefix("https://").rstrip("/"))
        print(f"result  : https://{args.verify.rstrip('/')}  {'OK' if ok else 'FAILED'}")
        return 0 if ok else 3

    team = os.environ.get("VERCEL_TEAM", "")
    project = os.environ.get("VERCEL_PROJECT", "intelligent-time-table-engine")
    scope = f"?teamId={team}" if team else ""

    payload = {
        "name": project,
        # frontend/ prefix: project rootDirectory is "frontend" (quirk 2).
        # encoding: base64: data is stored verbatim otherwise (quirk 1).
        "files": [
            {
                "file": f"frontend/{name}",
                "data": base64.b64encode((DECK_DIR / name).read_bytes()).decode(),
                "encoding": "base64",
            }
            for name in FILES
        ],
        "builds": [{"src": "**", "use": "@vercel/static"}],
    }

    print(f"project : {project}")
    print(f"team    : {team or '(token default)'}")
    for name in FILES:
        raw = (DECK_DIR / name).read_bytes()
        print(f"upload  : frontend/{name}  {len(raw)} bytes  sha1={hashlib.sha1(raw).hexdigest()}")
    if args.dry_run:
        print("dry run: no deployment created")
        return 0

    token = os.environ.get("VERCEL_TOKEN")
    if not token:
        print("error: set VERCEL_TOKEN", file=sys.stderr)
        return 64

    status, resp = call(token, f"{API}/v13/deployments{scope}", payload)
    if status not in (200, 201):
        print(f"create failed (HTTP {status}): {json.dumps(resp.get('error', resp))[:400]}")
        return 1
    dpl, url = resp.get("id") or resp.get("uid"), resp.get("url")
    print(f"created : {dpl}  https://{url}")

    state = None
    detail: dict = {}
    for _ in range(40):
        time.sleep(3)
        _, detail = call(token, f"{API}/v13/deployments/{dpl}{scope}")
        state = detail.get("readyState") or detail.get("state")
        if state in ("READY", "ERROR", "CANCELED"):
            break
    print(f"state   : {state}")
    if state != "READY":
        print(f"errorCode   : {detail.get('errorCode')}")
        print(f"errorMessage: {detail.get('errorMessage')}")
        return 2

    ok = verify_bytes(url)

    print(f"result  : https://{url}  {'OK' if ok else 'FAILED'}")
    return 0 if ok else 3


def verify_bytes(url: str) -> bool:
    ok = True
    for path, name in [("/", "index.html"), ("/FYP-Presentation.pptx", "FYP-Presentation.pptx")]:
        expected = (DECK_DIR / name).read_bytes()
        try:
            st, ctype, body = fetch(f"https://{url}{path}")
        except OSError as e:
            print(f"verify  : {path} -> FETCH ERROR {e}")
            ok = False
            continue
        if body == expected:
            verdict = "byte-exact"
        elif not body.startswith(expected):
            verdict = "MISMATCH"
            ok = False
        else:
            tail = body[len(expected):].strip()
            if tail.startswith(b"<script") and tail.endswith(b"</script>"):
                verdict = f"byte-exact + {len(tail)}B vercel script"
            else:
                verdict = "MISMATCH (unexpected trailing bytes)"
                ok = False
        print(f"verify  : {path} -> http={st} {ctype} {len(body)} bytes [{verdict}]")
    return ok


if __name__ == "__main__":
    sys.exit(main())
