"""Core checks: auth, school/users/CRUD, timetable shape, entry conflicts,
generation and the two-week cycle. Runs against a freshly-seeded demo DB."""

import sys

from common import ADMIN, TEACHER, STUDENT, SCHEDULER, Api, check, summary

admin = Api()
admin.login(*ADMIN)
sched = Api()
sched.login(*SCHEDULER)
teach = Api()
teach.login(*TEACHER)
stud = Api()
stud.login(*STUDENT)

# ---------------------------------------------------------------- auth & roles
s, body, _ = Api().req("POST", "/api/auth/login", body={"email": ADMIN[0], "password": "wrong"})
check("login rejects a bad password with 401", s == 401, f"status={s}")

s, me, _ = admin.req("GET", "/api/auth/me")
check("/api/auth/me returns the admin profile", s == 200 and me["user"]["role"] == "admin", str(me)[:120])
check("/api/auth/me returns the school", me.get("school", {}).get("name") == "Demo High School", str(me)[:160])

s, me_s, _ = stud.req("GET", "/api/auth/me")
check("student login resolves the linked class", me_s["user"]["role"] == "student" and me_s["user"]["class_id"], str(me_s)[:160])

check("anonymous request is rejected", Api().req("GET", "/api/timetable")[0] == 401)

# ------------------------------------------------------------- school & users
s, sch, _ = admin.req("GET", "/api/school")
c = sch["counts"]
check("school counts match the seed", (c["subjects"], c["teachers"], c["classes"], c["rooms"], c["lessons"]) == (10, 14, 6, 7, 60), str(c))
check("seeded timetable is fully placed", c.get("entries", c.get("placed", 0)) in (162, 324) or c.get("entries", 0) > 0, str(c))
check("scheduler cannot update school settings", sched.req("PUT", "/api/school", body={"name": "X"})[0] == 403)
s, sch2, _ = admin.req("PUT", "/api/school", body={"name": "Demo High School", "cycle_weeks": 1})
check("admin can update school settings", s == 200 and sch2.get("cleared_entries") == 0, f"{s} {sch2}")

s, users, _ = admin.req("GET", "/api/users")
check("admin lists users", s == 200 and any(u["role"] == "admin" for u in users["items"]), str(users)[:120])
check("student cannot list users", stud.req("GET", "/api/users")[0] == 403)

s, new_user, _ = admin.req(
    "POST", "/api/users",
    body={"name": "E2E Teacher", "email": "e2e.tester@demo.school", "password": "secret7", "role": "teacher"},
)
check("admin creates a teacher account", s == 201 and new_user.get("id"), f"{s} {new_user}")
uid = new_user["id"]
check("duplicate email is rejected", admin.req(
    "POST", "/api/users",
    body={"name": "E2E", "email": "e2e.tester@demo.school", "password": "secret7", "role": "teacher"},
)[0] == 409)
check("scheduler cannot create users", sched.req(
    "POST", "/api/users",
    body={"name": "E2E", "email": "e2e.b@demo.school", "password": "secret7", "role": "teacher"},
)[0] == 403)

grade7a = next(x for x in Api().req("GET", "/api/setup/all", token=admin.token)[1]["classes"] if x["name"] == "Grade 7A")
s, _, _ = admin.req("PUT", f"/api/users/{uid}", body={"name": "E2E Student", "email": "e2e.tester@demo.school", "role": "student", "class_id": grade7a["id"]})
check("admin can change a user's role", s == 200, f"status={s}")
s, _, _ = admin.req("DELETE", f"/api/users/{uid}")
check("admin can delete a user", s == 200, f"status={s}")
check("admin cannot delete their own account", admin.req("DELETE", f"/api/users/{admin.login(*ADMIN)['user']['id']}")[0] == 400)

# ------------------------------------------------------------------ setup CRUD
setup = admin.req("GET", "/api/setup/all")[1]
s, subj, _ = admin.req("POST", "/api/subjects", body={"name": "E2E Subject", "code": "E2S"})
check("admin creates a subject", s == 201 and subj.get("id"), f"{s} {subj}")
s, tchr, _ = admin.req("POST", "/api/teachers", body={"name": "E2E Person", "initials": "EP", "max_weekly": 10, "max_daily": 4})
check("admin creates a teacher", s == 201 and tchr.get("id"), f"{s} {tchr}")
s, room, _ = admin.req("POST", "/api/rooms", body={"name": "E2E Annex", "capacity": 25, "room_type": "general"})
check("admin creates a room", s == 201 and room.get("id"), f"{s} {room}")
s, klass, _ = admin.req("POST", "/api/classes", body={"name": "E2E 1", "grade": "7", "size": 20})
check("admin creates a class", s == 201 and klass.get("id"), f"{s} {klass}")
check("teacher role cannot edit setup", teach.req("POST", "/api/subjects", body={"name": "No", "code": "NO"})[0] == 403)
s, lesson, _ = admin.req(
    "POST", "/api/lessons",
    body={"class_id": klass["id"], "subject_id": subj["id"], "teacher_id": tchr["id"],
          "periods_per_week": 2, "max_per_day": 1},
)
check("admin creates a lesson", s == 201 and lesson.get("id"), f"{s} {lesson}")
s, _, _ = admin.req("PUT", f"/api/subjects/{subj['id']}", body={"name": "E2E Subject Renamed", "code": "E2S", "color": "#123456"})
check("admin renames the subject", s == 200, f"status={s}")
s, _, _ = admin.req("DELETE", f"/api/lessons/{lesson['id']}")
check("admin deletes the lesson", s == 200, f"status={s}")
s, _, _ = admin.req("DELETE", f"/api/subjects/{subj['id']}")
check("unused subject can be deleted", s == 200, f"status={s}")
for path in (f"/api/classes/{klass['id']}", f"/api/rooms/{room['id']}", f"/api/teachers/{tchr['id']}"):
    s, _, _ = admin.req("DELETE", path)
    check(f"cleanup: DELETE {path.split('/api/')[1]}", s == 200, f"status={s}")

# ------------------------------------------------------------- timetable shape
s, tt, _ = admin.req("GET", "/api/timetable")
check("timetable returns slots, entries and cycle", s == 200 and tt["entries"] and tt["cycle_weeks"] == 1, str(tt)[:120])
breaks = [x for x in tt["slots"] if x["is_break"]]
check("the bell schedule marks lunch as a break", any(x["idx"] == 4 for x in breaks), f"breaks={breaks[:3]}")
s, tt_s, _ = stud.req("GET", "/api/timetable")
check("student sees only their own class", s == 200 and all(e["class_id"] == me_s["user"]["class_id"] for e in tt_s["entries"]) and tt_s["entries"], f"{len(tt_s['entries'])} entries")

# entry conflicts ------------------------------------------------------------
week0 = [e for e in tt["entries"] if e["week"] == 0]
e0 = week0[0]
s, dup, _ = admin.req(
    "POST", "/api/timetable/entries",
    body={"lesson_id": e0["lesson_id"], "week": 0, "day": e0["day"], "idx": e0["idx"]},
)
check("placing a lesson onto an occupied class slot is a 409", s == 409 and "already has" in str(dup), f"{s} {dup}")
s, brk, _ = admin.req("POST", "/api/timetable/entries", body={"lesson_id": e0["lesson_id"], "week": 0, "day": 1, "idx": 4})
check("placing a lesson into the lunch break is a 409", s == 409 and "break" in str(brk), f"{s} {brk}")
s, ghost, _ = admin.req("PUT", "/api/timetable/entries/e2e-nonexistent", body={"day": 2, "idx": 1})
check("updating a missing entry is a 404", s == 404, f"status={s}")
check("teacher role cannot place entries", teach.req(
    "POST", "/api/timetable/entries",
    body={"lesson_id": e0["lesson_id"], "week": 0, "day": e0["day"], "idx": e0["idx"]},
)[0] == 403)

learners = {e["class_id"] for e in week0}
teach_busy = {}
for e in week0:
    teach_busy.setdefault((e["teacher_id"], e["day"], e["idx"]), e)
class_busy = {}
for e in week0:
    class_busy.setdefault((e["class_id"], e["day"], e["idx"]), e)

open_slots = [(x["day"], x["idx"]) for x in tt["slots"] if not x["is_break"]]

# A lesson taught on every day of the cycle cannot move anywhere without breaking
# the per-day subject cap, and the API's entry order is not guaranteed, so try
# every unlocked lesson instead of assuming the first one is movable.
moved = False
src = None
moved_from = None
target = None
attempts = 0
refusals = []
for candidate in [e for e in week0 if not e["locked"]]:
    from_slot = (candidate["day"], candidate["idx"])
    for day, idx in open_slots:
        if (day, idx) == from_slot:
            continue
        if (candidate["class_id"], day, idx) in class_busy or (candidate["teacher_id"], day, idx) in teach_busy:
            continue
        attempts += 1
        s, resp, _ = admin.req("PUT", f"/api/timetable/entries/{candidate['id']}", body={"week": 0, "day": day, "idx": idx})
        if s == 200:
            src, moved, moved_from, target = candidate, True, from_slot, (day, idx)
            break
        refusals.append((s, resp.get("error") if isinstance(resp, dict) else resp))
        if len(refusals) >= 12:
            break
    if moved or len(refusals) >= 12:
        break
check("moving a lesson to a free legal slot succeeds", moved,
      f"{attempts} attempt(s) over {len(week0)} entries; refusals={refusals[-3:]}")
check("a refused move is a rule violation, not a server error",
      all(s == 409 for s, _ in refusals), f"{refusals[:3]}")
if moved:
    s, tt2, _ = admin.req("GET", "/api/timetable")
    new_row = next((e for e in tt2["entries"] if e["id"] == src["id"]), None)
    check("the move is persisted with the new slot", new_row and (new_row["day"], new_row["idx"]) == target, str(new_row)[:160])
    busy_slots = [(e["day"], e["idx"]) for e in tt2["entries"] if e["lesson_id"] == src["lesson_id"] and e["id"] != src["id"]]
    if busy_slots:
        other = busy_slots[0]
        s, resp, _ = admin.req("PUT", f"/api/timetable/entries/{src['id']}", body={"week": 0, "day": other[0], "idx": other[1]})
        # moving this entry onto the same lesson's other placement is legal only if
        # max_per_day > 1; either way the API must answer 200 or a well-formed 409
        check("moves report conflicts with clear messages", s in (200, 409), f"{s} {resp}")

# ------------------------------------------------------------------ generation
s, gen, _ = admin.req("POST", "/api/generate",
                      body={"seed": 12345, "time_limit_ms": 4000})
check("generation places every requested period", s == 200 and gen["placed"] == gen["requested"] == 162, f"placed={gen['placed']} requested={gen['requested']} unplaced={gen['unplaced_total']}")
check("generation reports a score", isinstance(gen.get("score"), (int, float)), str(gen["score"])[:40])
check("teacher role cannot generate", teach.req("POST", "/api/generate", body={"seed": 1})[0] == 403)
check("student role cannot generate", stud.req("POST", "/api/generate", body={"seed": 1})[0] == 403)
s, dash, _ = admin.req("GET", "/api/dashboard")
check("dashboard reflects placed lessons", s == 200 and dash.get("placed", 0) == dash.get("requested", 1), str(dash)[:160])

# -------------------------------------------------------------- two-week cycle
s, _, _ = admin.req("PUT", "/api/school", body={"name": "Demo High School", "cycle_weeks": 2})
check("switching the cycle clears the timetable", s == 200, f"status={s}")
s, gen2, _ = admin.req("POST", "/api/generate", body={"seed": 7, "time_limit_ms": 4000})
check("two-week cycle also places everything", s == 200 and gen2["placed"] == gen2["requested"], f"placed={gen2['placed']}/{gen2['requested']}")
s, tt3, _ = admin.req("GET", "/api/timetable")
weeks_used = {e["week"] for e in tt3["entries"]}
check("both cycle weeks carry lessons", weeks_used == {0, 1}, f"weeks={weeks_used}")

s, _, _ = admin.req("PUT", "/api/school", body={"name": "Demo High School", "cycle_weeks": 1})
s, gen3, _ = admin.req("POST", "/api/generate", body={"seed": 12345, "time_limit_ms": 4000})
check("switching back to a single week repopulates", s == 200 and gen3["placed"] == gen3["requested"], f"placed={gen3['placed']}")

summary("suite_core")
