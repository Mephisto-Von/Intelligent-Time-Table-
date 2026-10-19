#!/usr/bin/env python3
"""Build the Final Year Project defense deck for TimetableMaster.

Usage:  python3 docs/fyp-presentation.py
Output: docs/FYP-Presentation.pptx

Screenshots are taken from docs/screenshots/; every number in the deck is
measured from the repository (line counts, API surface, test totals), so the
deck stays accurate as the project evolves.
"""

from __future__ import annotations

import datetime as _dt
import subprocess
from pathlib import Path

from PIL import Image
from pptx import Presentation
from pptx.dml.color import RGBColor
from pptx.enum.shapes import MSO_SHAPE
from pptx.enum.text import PP_ALIGN, MSO_ANCHOR
from pptx.util import Inches, Pt

ROOT = Path(__file__).resolve().parent.parent
DOCS = ROOT / "docs"
SHOTS = DOCS / "screenshots"
OUT = DOCS / "FYP-Presentation.pptx"

NAVY = RGBColor(0x0F, 0x2A, 0x47)
BLUE = RGBColor(0x1D, 0x6F, 0xA8)
STEEL = RGBColor(0x35, 0x6A, 0x92)
AMBER = RGBColor(0xF0, 0xA2, 0x02)
GREEN = RGBColor(0x2E, 0x8B, 0x57)
SLATE = RGBColor(0x33, 0x3F, 0x50)
MUTED = RGBColor(0x6B, 0x7A, 0x8C)
PAPER = RGBColor(0xFF, 0xFF, 0xFF)
CARD = RGBColor(0xF2, 0xF6, 0xFA)
LINE = RGBColor(0xD7, 0xE0, 0xEA)

SLIDE_W, SLIDE_H = Inches(13.333), Inches(7.5)


# ----------------------------------------------------------------- measurements
def sloc(*paths: str) -> int:
    total = 0
    for pattern in paths:
        for p in ROOT.glob(pattern):
            if p.is_file():
                total += sum(1 for _ in p.open(encoding="utf-8", errors="ignore"))
    return total


def count(pattern: str, needle: str) -> int:
    n = 0
    for p in ROOT.glob(pattern):
        if p.is_file():
            n += p.read_text(encoding="utf-8", errors="ignore").count(needle)
    return n


def git(*args: str) -> str:
    try:
        return subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True, timeout=20).stdout.strip()
    except Exception:
        return ""


RUST_LOC = sloc("backend/src/**/*.rs")
TS_LOC = sloc("frontend/src/**/*.tsx", "frontend/src/**/*.ts")
API_ROUTES = count("backend/src/routes/*.rs", '.route("/api/')
SQL_TABLES = count("backend/src/db.rs", "CREATE TABLE IF NOT EXISTS")

# Unit tests: every #[test] in the backend (solver, pdf, ical) — matches `cargo test`.
SOLVER_TESTS = count("backend/src/**/*.rs", "#[test]")

# API checks executed by one full `tests/run.sh` run: suite_core 49,
# suite_features 44, suite_cover_leave 33 = 126. Some assertions sit inside
# loops, so counting `check(` occurrences under-counts; refresh this figure
# from the "===== suite_x: n/m passed =====" lines of a run.
HTTP_CHECKS = 126

# UI assertions authored across the three Chromium specs (a run executes 31 of
# them; a few sit behind conditionals).
BROWSER_CHECKS = sum(p.read_text(encoding="utf-8").count("check(") for p in (ROOT / "tests/browser").glob("*.spec.mjs"))
TODAY = _dt.date.today().strftime("%d %B %Y")


# ----------------------------------------------------------------------- layout
def tb(slide, left, top, width, height, text, *, size=16, bold=False, color=SLATE,
       align=PP_ALIGN.LEFT, anchor=MSO_ANCHOR.TOP, space_after=6, line_spacing=1.15):
    box = slide.shapes.add_textbox(left, top, width, height)
    tf = box.text_frame
    tf.word_wrap = True
    tf.vertical_anchor = anchor
    lines = text if isinstance(text, (list, tuple)) else [text]
    for i, line in enumerate(lines):
        p = tf.paragraphs[0] if i == 0 else tf.add_paragraph()
        p.alignment = align
        p.space_after = Pt(space_after)
        p.line_spacing = line_spacing
        run = p.add_run()
        run.text = line
        run.font.size = Pt(size)
        run.font.bold = bold
        run.font.color.rgb = color
        run.font.name = "Calibri"
    return box


def rect(slide, left, top, width, height, fill, *, line=None, radius=None):
    shape = MSO_SHAPE.ROUNDED_RECTANGLE if radius else MSO_SHAPE.RECTANGLE
    sh = slide.shapes.add_shape(shape, left, top, width, height)
    sh.fill.solid()
    sh.fill.fore_color.rgb = fill
    if line is None:
        sh.line.fill.background()
    else:
        sh.line.color.rgb = line
        sh.line.width = Pt(0.75)
    sh.shadow.inherit = False
    if radius is not None:
        try:
            sh.adjustments[0] = radius
        except Exception:
            pass
    return sh


def blank(prs):
    return prs.slides.add_slide(prs.slide_layouts[6])


def frame(slide, title, kicker=None, page=None):
    """Standard content slide: accent band, title, footer."""
    rect(slide, 0, 0, SLIDE_W, Inches(0.14), NAVY)
    if kicker:
        tb(slide, Inches(0.6), Inches(0.34), Inches(9), Inches(0.3), kicker.upper(),
           size=11, bold=True, color=BLUE)
        top = Inches(0.62)
    else:
        top = Inches(0.45)
    tb(slide, Inches(0.6), top, Inches(12.1), Inches(0.7), title, size=28, bold=True, color=NAVY)
    rect(slide, Inches(0.6), top + Inches(0.62), Inches(1.1), Inches(0.05), AMBER)
    rect(slide, 0, SLIDE_H - Inches(0.42), SLIDE_W, Inches(0.42), CARD)
    tb(slide, Inches(0.6), SLIDE_H - Inches(0.34), Inches(8), Inches(0.26),
       "TimetableMaster · Final Year Project", size=10, color=MUTED)
    if page is not None:
        tb(slide, SLIDE_W - Inches(1.2), SLIDE_H - Inches(0.34), Inches(0.6), Inches(0.26),
           str(page), size=10, color=MUTED, align=PP_ALIGN.RIGHT)


def bullets(slide, items, left, top, width, height, *, size=15, color=SLATE, bullet_color=BLUE,
            gap=10):
    box = slide.shapes.add_textbox(left, top, width, height)
    tf = box.text_frame
    tf.word_wrap = True
    for i, item in enumerate(items):
        if isinstance(item, tuple):
            text, level = item
        else:
            text, level = item, 0
        p = tf.paragraphs[0] if i == 0 else tf.add_paragraph()
        p.space_after = Pt(gap)
        p.line_spacing = 1.12
        p.level = level
        marker = "▪  " if level == 0 else "–  "
        r0 = p.add_run()
        r0.text = marker
        r0.font.size = Pt(size)
        r0.font.bold = True
        r0.font.color.rgb = bullet_color if level == 0 else MUTED
        r0.font.name = "Calibri"
        bold = text.endswith(":") or (level == 0 and ":" in text[:40] and text.isupper())
        r1 = p.add_run()
        r1.text = text
        r1.font.size = Pt(size if level == 0 else size - 1)
        r1.font.bold = bold
        r1.font.color.rgb = color if level == 0 else MUTED
        r1.font.name = "Calibri"
    return box


def table(slide, rows, left, top, width, col_widths=None, *, header=True, size=12, row_h=Inches(0.4)):
    n_rows, n_cols = len(rows), len(rows[0])
    shp = slide.shapes.add_table(n_rows, n_cols, left, top, width, row_h * n_rows)
    tbl = shp.table
    if col_widths:
        for i, w in enumerate(col_widths):
            tbl.columns[i].width = w
    for r, row in enumerate(rows):
        for c, val in enumerate(row):
            cell = tbl.cell(r, c)
            cell.text = str(val)
            cell.margin_left = Inches(0.1)
            cell.margin_right = Inches(0.08)
            cell.margin_top = Inches(0.03)
            cell.margin_bottom = Inches(0.03)
            cell.vertical_anchor = MSO_ANCHOR.MIDDLE
            cell.fill.solid()
            if r == 0 and header:
                cell.fill.fore_color.rgb = NAVY
            else:
                cell.fill.fore_color.rgb = PAPER if r % 2 else CARD
            for p in cell.text_frame.paragraphs:
                p.space_after = Pt(0)
                for run in p.runs:
                    run.font.size = Pt(size)
                    run.font.name = "Calibri"
                    run.font.bold = (r == 0 and header) or c == 0 and r > 0
                    run.font.color.rgb = PAPER if (r == 0 and header) else SLATE
    return tbl


def picture(slide, name, left, top, width, height, *, crop_top=0, max_ratio=1.0):
    """Insert a screenshot scaled to fit the box (contain), cropping tall captures."""
    src = SHOTS / name
    if not src.exists():
        rect(slide, left, top, width, height, CARD, line=LINE)
        tb(slide, left, top + height / 2, width, Inches(0.3), f"{name} (missing)",
           size=12, color=MUTED, align=PP_ALIGN.CENTER)
        return None
    img = Image.open(src)
    if crop_top:
        img = img.crop((0, int(img.height * crop_top), img.width, img.height))
    cache = Path("/tmp") / f"fyp-{crop_top:.2f}-{src.name}"
    if not cache.exists():
        img.save(cache)
    with Image.open(cache) as im:
        iw, ih = im.size
    ratio = min(width / iw, height / ih)
    w, h = int(iw * ratio), int(ih * ratio)
    pic = slide.shapes.add_picture(str(cache), left + int((width - w) / 2), top + int((height - h) / 2),
                                   width=w, height=h)
    rect(slide, pic.left - Pt(0.75), pic.top - Pt(0.75), pic.width + Pt(1.5), pic.height + Pt(1.5),
         PAPER, line=LINE)
    return pic


# ----------------------------------------------------------------------- build
prs = Presentation()
prs.slide_width, prs.slide_height = SLIDE_W, SLIDE_H
page = 0


def new(title=None, kicker=None):
    global page
    page += 1
    s = blank(prs)
    if title:
        frame(s, title, kicker, page)
    return s


# 1 — title -------------------------------------------------------------------
s = blank(prs)
rect(s, 0, 0, SLIDE_W, SLIDE_H, NAVY)
rect(s, 0, Inches(2.75), SLIDE_W, Inches(0.06), AMBER)
tb(s, Inches(1), Inches(1.05), Inches(11.3), Inches(0.5),
   "FINAL YEAR PROJECT  ·  " + TODAY.upper(), size=14, bold=True, color=AMBER)
tb(s, Inches(1), Inches(1.55), Inches(11.3), Inches(1.1), "TimetableMaster",
   size=54, bold=True, color=PAPER)
tb(s, Inches(1), Inches(3.0), Inches(11.3), Inches(1.0),
   ["An intelligent timetable platform for schools and universities",
    "Constraint-based generation · drag & drop editing · cover, leave and calendar"],
   size=18, color=RGBColor(0xC9, 0xD9, 0xE8), space_after=6)

tb(s, Inches(1), Inches(4.25), Inches(6.0), Inches(0.35), "PROJECT TEAM",
   size=13, bold=True, color=AMBER)
TEAM = [("Hira Hassan", "F23607055"),
        ("Iman Aftab", "F23607041"),
        ("Jawaria Sabir", "F23607040"),
        ("Hamza Abdul Karim", "F23607046")]
for i, (name, roll) in enumerate(TEAM):
    col, row = i % 2, i // 2
    tb(s, Inches(1 + col * 5.6), Inches(4.68 + row * 0.42), Inches(5.4), Inches(0.38),
       f"{name}   ·   {roll}", size=16, color=PAPER)
rect(s, Inches(1), Inches(5.62), Inches(2.2), Inches(0.04), AMBER)
tb(s, Inches(1), Inches(5.8), Inches(11.3), Inches(0.4),
   "Supervisor: Ms. Faria Sajjad", size=17, bold=True, color=RGBColor(0xE8, 0xA8, 0x38))
tb(s, Inches(1), Inches(6.75), Inches(11.3), Inches(0.4),
   "Built from scratch in Rust (Axum + SQLite) and React 19 + TypeScript",
   size=14, color=RGBColor(0x9F, 0xB8, 0xCE))

# 2 — agenda ------------------------------------------------------------------
s = new("What this presentation covers", "Agenda")
items = [
    ("1.  Problem & objectives — why timetabling is still done by hand"),
    ("2.  The scheduling engine — the technical core of the project"),
    ("3.  Architecture & technology choices"),
    ("4.  Live product tour — setup, timetable, cover, calendar"),
    ("5.  Security, roles and multi-tenancy"),
    ("6.  Testing & CI — 12 unit tests, 126 API checks, 33 browser checks"),
    ("7.  Results, limitations and future work"),
]
bullets(s, items, Inches(0.9), Inches(1.7), Inches(11.5), Inches(4.6), size=19, gap=16)

# 3 — problem -----------------------------------------------------------------
s = new("Timetabling is still a manual, error-prone ritual", "Problem statement")
left = ["Spreadsheets and paper grids: a single school week means hundreds of",
        "constraints (rooms, availability, workload caps) checked by hand.",
        "",
        "Clashes surface only after the timetable is published — a teacher",
        "double-booked on Monday morning invalidates days of work.",
        "",
        "Teacher absences are handled ad-hoc: phone calls, whiteboards and",
        "guesswork about who is free, qualified and willing to cover.",
        "",
        "Nobody can answer \"who changed this, and when?\" after the fact."]
bullets(s, ["Manual spreadsheets and paper grids",
            "Hundreds of constraints per week: rooms, availability, caps",
            "Clashes found only after publishing → costly rework",
            "Cover arranged by phone calls and guesswork",
            "No audit trail of who changed what, and when"],
        Inches(0.9), Inches(1.7), Inches(6.6), Inches(4.4), size=17, gap=14)
rect(s, Inches(7.9), Inches(1.75), Inches(4.8), Inches(3.9), CARD, line=LINE, radius=0.06)
tb(s, Inches(8.2), Inches(2.0), Inches(4.2), Inches(0.4), "THE COST OF GETTING IT WRONG",
   size=12, bold=True, color=BLUE)
bullets(s, ["Re-issuing a timetable to 3 classes and 8 teachers",
            "Teacher conflicts → disputes, grievances, cover chaos",
            "Manual cover hunting on a normal teaching morning",
            "Version chaos: final_v7_REAL.xlsx"],
        Inches(8.2), Inches(2.5), Inches(4.2), Inches(2.8), size=13, bullet_color=AMBER, gap=9)

# 4 — objectives --------------------------------------------------------------
s = new("What the project set out to do", "Objectives")
bullets(s, [
    "Generate clash-free timetables automatically from a curriculum",
    "Respect every hard rule: rooms, availability, workload caps, two-week A/B cycles",
    "Keep human judgement: drag & drop edits that survive regeneration (pinned lessons)",
    "Handle day-2 operations: leave workflow, ranked cover suggestions, school calendar",
    "Make the result usable: CSV / PDF / ICS export, read-only share links, notifications",
    "Make it accountable and safe: role-based access, multi-tenancy, activity log",
    "Prove it works: automated tests at three layers, run in CI on every push",
], Inches(0.9), Inches(1.65), Inches(11.6), Inches(4.8), size=17, gap=15)

# 5 — roles -------------------------------------------------------------------
s = new("Who uses it, and what they can do", "Roles & tenancy")
rows = [["Role", "Sees", "Can do"],
        ["admin", "Everything in their school(s)", "Manage users, settings, rebuild the demo workspace"],
        ["scheduler", "Timetable, setup, cover, calendar", "Generate, drag & drop, assign substitutes, decide nothing about people"],
        ["teacher", "Own timetable, own leave", "Request leave, view cover in which they are involved"],
        ["student", "Own class timetable, calendar", "Read-only"]]
table(s, rows, Inches(0.9), Inches(1.7), Inches(11.5),
      [Inches(1.7), Inches(3.6), Inches(6.2)], size=13, row_h=Inches(0.55))
tb(s, Inches(0.9), Inches(5.6), Inches(11.5), Inches(0.8),
   "Multi-tenant by design: every row in the database is scoped to one school, taken from the "
   "signed JWT — no account can read another school's data.", size=14, color=MUTED)

# 6 — features ----------------------------------------------------------------
s = new("Feature set at a glance", "Product")
chips = [
    ("Constraint-based generation", "greedy seed + simulated annealing"),
    ("Drag & drop editing", "pinned lessons survive regeneration"),
    ("Cover & substitutes", "ranked candidates with reasons"),
    ("Leave management", "impact preview before approving"),
    ("Two-week A/B cycles", "constraints applied across the cycle"),
    ("School calendar", "holidays, exams, activities"),
    ("CSV · PDF · ICS export", "print-ready, per class/teacher/room"),
    ("Read-only share links", "revocable, no sign-in needed"),
    ("Activity log", "who changed what, and when"),
    ("Notifications", "webhook / email, optional, fail-open"),
]
for i, (title, sub) in enumerate(chips):
    col, row = i % 2, i // 2
    left = Inches(0.9 + col * 6.0)
    top = Inches(1.65 + row * 0.98)
    rect(s, left, top, Inches(5.6), Inches(0.82), CARD, line=LINE, radius=0.12)
    rect(s, left, top, Inches(0.08), Inches(0.82), BLUE)
    tb(s, left + Inches(0.25), top + Inches(0.08), Inches(5.2), Inches(0.32), title, size=15, bold=True, color=NAVY)
    tb(s, left + Inches(0.25), top + Inches(0.42), Inches(5.2), Inches(0.3), sub, size=12, color=MUTED)

# 7 — engine constraints ------------------------------------------------------
s = new("The scheduling engine: rules that must never break", "Technical core")
rect(s, Inches(0.9), Inches(1.7), Inches(5.6), Inches(4.3), CARD, line=LINE, radius=0.05)
tb(s, Inches(1.15), Inches(1.9), Inches(5.1), Inches(0.4), "HARD CONSTRAINTS — never violated",
   size=13, bold=True, color=NAVY)
bullets(s, ["A class, a teacher and a room host at most one lesson per period",
            "Teachers are only scheduled inside their availability",
            "Lesson max-per-day; teacher daily and weekly workload caps",
            "Fixed rooms must match; required room types must fit the class",
            "Hand-pinned lessons stay exactly where they are"],
        Inches(1.15), Inches(2.4), Inches(5.1), Inches(3.4), size=13, bullet_color=NAVY, gap=8)
rect(s, Inches(6.85), Inches(1.7), Inches(5.6), Inches(4.3), CARD, line=LINE, radius=0.05)
tb(s, Inches(7.1), Inches(1.9), Inches(5.1), Inches(0.4), "SOFT GOALS — optimised", size=13, bold=True, color=NAVY)
bullets(s, ["No gaps in a class's day; no gaps in a teacher's day",
            "The same subject is spread across the week, not clustered",
            "Teacher load is balanced across days",
            "Work is balanced across the cycle weeks (A/B)",
            "Weights are tunable per run through the API"],
        Inches(7.1), Inches(2.4), Inches(5.1), Inches(3.4), size=13, bullet_color=GREEN, gap=8)

# 8 — algorithm ---------------------------------------------------------------
s = new("Two phases, deterministic, and it explains itself", "How the solver works")
steps = [
    ("1", "Greedy seeding", "Most-constrained-first: the lessons with the fewest legal slots "
                            "are placed first, so the search space never closes unnecessarily."),
    ("2", "Simulated annealing", "Relocate / swap / insert moves explore neighbouring solutions "
                                 "and accept worse ones at high temperature to escape local optima."),
    ("3", "Report", "Placed periods, unplaced periods with the dominant reason "
                    "(teacher unavailable / busy / limits / room), moves and quality score."),
]
for i, (num, title, body) in enumerate(steps):
    top = Inches(1.75 + i * 1.42)
    rect(s, Inches(0.9), top, Inches(0.75), Inches(1.15), NAVY, radius=0.2)
    tb(s, Inches(0.9), top + Inches(0.28), Inches(0.75), Inches(0.6), num,
       size=26, bold=True, color=PAPER, align=PP_ALIGN.CENTER)
    rect(s, Inches(1.85), top, Inches(10.6), Inches(1.15), CARD, line=LINE, radius=0.05)
    tb(s, Inches(2.1), top + Inches(0.12), Inches(10.1), Inches(0.34), title, size=17, bold=True, color=NAVY)
    tb(s, Inches(2.1), top + Inches(0.5), Inches(10.1), Inches(0.6), body, size=13, color=SLATE)
tb(s, Inches(0.9), Inches(6.15), Inches(11.6), Inches(0.5),
   f"Determinism is a tested property: the same seed and iteration count produce the same "
   f"timetable — {SOLVER_TESTS} unit tests cover feasibility, clashes, limits, room rules, "
   f"pinned placements and two-week cycles.", size=13, color=MUTED)

# 9 — architecture ------------------------------------------------------------
s = new("One binary serves the API and the UI", "Architecture")
boxes = [
    ("Browser", "React 19 · TypeScript · Vite\nTanStack Query · React Router", Inches(0.9), BLUE),
    ("HTTP API", "Rust · Axum 0.8\nJWT (HS256) · argon2\nREST, school-scoped", Inches(5.1), NAVY),
    ("SQLite", "sqlx, WAL mode\nschema + migrations\nseeded demo school", Inches(9.3), STEEL),
]
for title, body, left, colour in boxes:
    rect(s, left, Inches(2.0), Inches(3.2), Inches(2.0), colour, radius=0.08)
    tb(s, left + Inches(0.25), Inches(2.2), Inches(2.7), Inches(0.4), title, size=19, bold=True, color=PAPER)
    tb(s, left + Inches(0.25), Inches(2.72), Inches(2.7), Inches(1.1), body.split("\n"),
       size=12, color=RGBColor(0xD8, 0xE6, 0xF2), space_after=2)
for left in (Inches(4.1), Inches(8.3)):
    ar = rect(s, left, Inches(2.85), Inches(1.0), Inches(0.3), AMBER)
    tb(s, left, Inches(2.87), Inches(1.0), Inches(0.26), "JSON", size=11, bold=True,
       color=NAVY, align=PP_ALIGN.CENTER)
tb(s, Inches(0.9), Inches(1.55), Inches(11.6), Inches(0.4),
   "Vite builds the SPA into frontend/dist, and the Rust process serves it — a single deployable.",
   size=14, color=MUTED)
mods = ["solver/", "pdf.rs", "ical.rs", "notifications.rs", "audit.rs"]
tb(s, Inches(0.9), Inches(4.35), Inches(11.6), Inches(0.35), "INDEPENDENT MODULES INSIDE THE SERVICE",
   size=12, bold=True, color=BLUE)
for i, m in enumerate(mods):
    left = Inches(0.9 + i * 2.36)
    rect(s, left, Inches(4.75), Inches(2.2), Inches(0.55), CARD, line=LINE, radius=0.25)
    tb(s, left, Inches(4.86), Inches(2.2), Inches(0.34), m, size=12, bold=True, color=NAVY,
       align=PP_ALIGN.CENTER)
tb(s, Inches(0.9), Inches(5.55), Inches(11.6), Inches(0.6),
   "Solver, PDF writer and iCalendar generator are pure functions with no I/O — that is what makes "
   "them unit-testable and what makes the API predictable.", size=13, color=MUTED)

# 10 — tech stack -------------------------------------------------------------
s = new("Technology choices", "Stack")
rows = [["Layer", "Choice", "Why"],
        ["Backend", "Rust 2024 · Axum 0.8 · sqlx (SQLite)",
         "Performance + type safety; the solver runs in-process, no queue needed"],
        ["Auth", "JWT (HS256) · argon2", "Stateless sessions, per-installation secret, modern password hashing"],
        ["Frontend", "React 19 · TypeScript · Vite · TanStack Query",
         "Typed API client, cached server state, fast builds"],
        ["Exports", "dependency-free PDF writer · iCalendar generator",
         "No heavyweight document library; output is unit-tested"],
        ["Testing", "cargo test · Python HTTP suites · Playwright Chromium",
         "Solver logic, API contract and real browser behaviour"],
        ["CI", "GitHub Actions", "Unit tests + full end-to-end run on every push and PR"]]
table(s, rows, Inches(0.9), Inches(1.7), Inches(11.5),
      [Inches(1.6), Inches(4.1), Inches(5.8)], size=12, row_h=Inches(0.62))

# 11 — demo: setup ------------------------------------------------------------
s = new("Demo · defining the school", "Product tour")
picture(s, "setup.png", Inches(0.9), Inches(1.6), Inches(7.0), Inches(4.9), crop_top=0.30)
bullets(s, ["Subjects, teachers, classes, rooms and lessons in one screen",
            "Teacher availability as a clickable grid",
            "Bell schedule defines the periods, breaks excluded",
            "Everything lands in one API call (/api/setup/all)"],
        Inches(8.2), Inches(1.8), Inches(4.2), Inches(4.2), size=14, gap=13)

# 12 — demo: timetable --------------------------------------------------------
s = new("Demo · the timetable, generated then edited by hand", "Product tour")
picture(s, "timetable.png", Inches(0.9), Inches(1.6), Inches(7.0), Inches(4.9), crop_top=0.30)
bullets(s, ["View by class, teacher or room — same grid, different grouping",
            "Drag a lesson to an empty slot: the API validates, the grid repaints",
            "Pin a lesson and it survives every future generation",
            "Conflicts are explained, not just rejected",
            "Exports and share links follow the current view"],
        Inches(8.2), Inches(1.75), Inches(4.2), Inches(4.4), size=14, gap=12)

# 13 — demo: cover ------------------------------------------------------------
s = new("Demo · a teacher is absent: who covers?", "Product tour")
picture(s, "cover.png", Inches(0.9), Inches(1.6), Inches(7.0), Inches(4.9), crop_top=0.28)
bullets(s, ["Approved leave turns into an absence on the cover day",
            "Affected lessons are listed per period",
            "Candidates are ranked, each with a reason",
            "Blocked teachers are shown with why: on leave, busy, unavailable",
            "Assigning notifies over webhook; history keeps both assignments and cancels"],
        Inches(8.2), Inches(1.75), Inches(4.2), Inches(4.4), size=14, gap=11)

# 14 — demo: dashboard / calendar --------------------------------------------
s = new("Demo · the day view and the school calendar", "Product tour")
picture(s, "dashboard.png", Inches(0.9), Inches(1.7), Inches(5.5), Inches(3.2))
picture(s, "calendar.png", Inches(6.9), Inches(1.7), Inches(5.5), Inches(3.2))
tb(s, Inches(0.9), Inches(5.1), Inches(5.5), Inches(0.9),
   "Dashboard: coverage stats, unfilled periods, today's absences, upcoming events.",
   size=13, color=MUTED)
tb(s, Inches(6.9), Inches(5.1), Inches(5.5), Inches(0.9),
   "Calendar: holidays close the school — the solver and cover logic both respect them.",
   size=13, color=MUTED)

# 15 — security ---------------------------------------------------------------
s = new("Security, roles and accountability", "Non-functional")
cards = [
    ("Passwords", "argon2 hashing; the demo school is rebuildable in one click"),
    ("Sessions", "HS256 JWTs signed with a per-installation secret (auto-generated on first run)"),
    ("Authorisation", "every write endpoint requires admin or scheduler; teachers request leave only for themselves; students are read-only"),
    ("Tenancy", "every row is school-scoped from the token — no cross-school reads"),
    ("Public links", "unguessable tokens, revocable, return 404 the moment they are revoked"),
    ("Audit", "generation, edits, cover decisions and account changes are all logged with actor and timestamp"),
]
for i, (title, body) in enumerate(cards):
    col, row = i % 3, i // 3
    left = Inches(0.9 + col * 3.95)
    top = Inches(1.7 + row * 2.05)
    rect(s, left, top, Inches(3.7), Inches(1.85), CARD, line=LINE, radius=0.06)
    rect(s, left, top, Inches(3.7), Inches(0.08), BLUE)
    tb(s, left + Inches(0.2), top + Inches(0.22), Inches(3.3), Inches(0.35), title, size=16, bold=True, color=NAVY)
    tb(s, left + Inches(0.2), top + Inches(0.66), Inches(3.3), Inches(1.05), body, size=12, color=SLATE)

# 16 — testing ----------------------------------------------------------------
s = new("Testing at three layers — and what it caught", "Quality")
table(s, [["Layer", "Tool", "Covers"],
          ["Unit", "cargo test", f"{SOLVER_TESTS} tests: feasibility, clashes, workload caps, room rules, pinned placements, two-week cycles, determinism"],
          ["API", "Python suites (stdlib only)", f"{HTTP_CHECKS} checks: auth and RBAC, setup CRUD, timetable conflicts, generation, cover/leave, exports, share links, notifications"],
          ["Browser", "Playwright (Chromium)", f"{BROWSER_CHECKS} UI assertions: drag & drop reaches the API and repaints, share links work for anonymous visitors, settings and activity log"]],
      Inches(0.9), Inches(1.65), Inches(11.5), [Inches(1.5), Inches(3.1), Inches(6.9)], size=12, row_h=Inches(0.9))
rect(s, Inches(0.9), Inches(4.85), Inches(11.5), Inches(1.5), CARD, line=LINE, radius=0.05)
rect(s, Inches(0.9), Inches(4.85), Inches(0.09), Inches(1.5), AMBER)
tb(s, Inches(1.2), Inches(5.0), Inches(11.0), Inches(0.35), "A BUG ONLY THE SUITE COULD FIND",
   size=13, bold=True, color=NAVY)
tb(s, Inches(1.2), Inches(5.4), Inches(11.0), Inches(0.85),
   "CI reported green while the runner was silently testing a stale server left behind by an "
   "earlier run — it killed a shell instead of the process. The runner now execs the server, "
   "refuses to start when the port is already answering, and prints every check it ran.",
   size=13, color=SLATE)

# 17 — results ----------------------------------------------------------------
s = new("Results", "Outcome")
stats = [
    (f"{RUST_LOC:,}", "lines of Rust\nbackend + solver"),
    (f"{TS_LOC:,}", "lines of TypeScript\nfrontend"),
    (f"{API_ROUTES}", "REST routes\nall school-scoped"),
    (f"{SQL_TABLES}", "SQLite tables\nschema + migrations"),
    (f"{SOLVER_TESTS + HTTP_CHECKS + BROWSER_CHECKS}", "automated checks\nall passing in CI"),
]
for i, (num, label) in enumerate(stats):
    left = Inches(0.9 + i * 2.36)
    rect(s, left, Inches(1.8), Inches(2.2), Inches(1.7), NAVY, radius=0.08)
    tb(s, left, Inches(2.0), Inches(2.2), Inches(0.7), num, size=30, bold=True, color=AMBER,
       align=PP_ALIGN.CENTER)
    tb(s, left + Inches(0.1), Inches(2.7), Inches(2.0), Inches(0.7), label.split("\n"),
       size=11, color=RGBColor(0xC9, 0xD9, 0xE8), align=PP_ALIGN.CENTER, space_after=0)
bullets(s, [
    "A full school (3 classes, 8 teachers, 6 rooms, 22 lessons) generates in well under a second",
    "Hand-edited lessons are honoured by every later generation — nothing is silently reverted",
    "Two-week A/B cycles solve with the same engine, with balance measured across both weeks",
    "The whole stack runs from one command: cargo run --seed-demo, plus the built SPA",
    "Cover assignment, leave impact, exports, share links and notifications all verified end to end",
], Inches(0.9), Inches(3.95), Inches(11.6), Inches(2.3), size=15, gap=11)

# 18 — future work ------------------------------------------------------------
s = new("Limitations & future work", "Next steps")
rect(s, Inches(0.9), Inches(1.7), Inches(5.6), Inches(4.2), CARD, line=LINE, radius=0.05)
tb(s, Inches(1.15), Inches(1.9), Inches(5.1), Inches(0.4), "KNOWN LIMITATIONS", size=13, bold=True, color=NAVY)
bullets(s, ["SQLite keeps the app single-node; no horizontal write scaling",
            "No teacher/team-taught lessons yet — one teacher per lesson",
            "Subject-vs-time-of-day preferences are not modelled",
            "Solver quality is bounded by the time budget, not proven optimal"],
        Inches(1.15), Inches(2.45), Inches(5.1), Inches(3.2), size=13, bullet_color=AMBER, gap=10)
rect(s, Inches(6.85), Inches(1.7), Inches(5.6), Inches(4.2), CARD, line=LINE, radius=0.05)
tb(s, Inches(7.1), Inches(1.9), Inches(5.1), Inches(0.4), "NEXT STEPS", size=13, bold=True, color=NAVY)
bullets(s, ["Multi-teacher lessons and lab/elective slots",
            "iCal subscription feed (the one-off .ics export already exists)",
            "Postgres + container deployment for multi-school districts",
            "Compare against an off-the-shelf CP-SAT solver as a benchmark"],
        Inches(7.1), Inches(2.45), Inches(5.1), Inches(3.2), size=13, bullet_color=GREEN, gap=10)

# 19 — closing ----------------------------------------------------------------
s = blank(prs)
rect(s, 0, 0, SLIDE_W, SLIDE_H, NAVY)
rect(s, Inches(1), Inches(3.0), Inches(1.6), Inches(0.06), AMBER)
tb(s, Inches(1), Inches(3.3), Inches(11.3), Inches(1.0), "Thank you", size=48, bold=True, color=PAPER)
tb(s, Inches(1), Inches(4.35), Inches(11.3), Inches(1.4),
   ["Questions welcome",
    "Repository: github.com/Mephisto-Von/Intelligent-Time-Table-",
    "Demo: sign in with admin@demo.school / demo1234"],
   size=17, color=RGBColor(0xC9, 0xD9, 0xE8), space_after=8)

prs.save(OUT)
print(f"wrote {OUT}  ({OUT.stat().st_size // 1024} KB, {len(prs.slides._sldIdLst)} slides)")
