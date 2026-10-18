"""Leave and cover checks: requesting, approving/rejecting, impact, the
cover-day view, substitute assignment (with conflict rules) and history.
Must run after suite_core/suite_features on the same seeded DB."""

import sys

from common import ADMIN, SCHEDULER, STUDENT, TEACHER, Api, check, next_weekday, plus_days, summary, webhook_events

admin = Api()
admin.login(*ADMIN)
sched = Api()
sched.login(*SCHEDULER)
teach = Api()
teach.login(*TEACHER)
stud = Api()
stud.login(*STUDENT)

monday = next_weekday()  # always in the future -> independent of the weekday today
setup = admin.req("GET", "/api/setup/all")[1]
teachers = setup["teachers"]
by_name = {t["name"]: t for t in teachers}
priya = by_name["Priya Sharma"]
priya_id = priya["id"]
david_id = by_name["David Chen"]["id"]

log_len = len(webhook_events())

# ------------------------------------------------------------------ leave CRUD
s, leaves, _ = admin.req("GET", "/api/leave")
check("staff sees the seeded leave requests", s == 200 and len(leaves["items"]) >= 3, f"{len(leaves.get('items', []))} items")
check("seeded leave contains a pending request", any(i["status"] == "pending" for i in leaves["items"]))
check("student cannot view leave", stud.req("GET", "/api/leave")[0] == 403)

s, l1, _ = admin.req("POST", "/api/leave", body={
    "teacher_id": priya_id, "start_date": monday, "end_date": plus_days(monday, 1),
    "part": "full", "reason": "E2E leave",
})
check("staff creates leave for a named teacher", s == 201 and l1.get("id"), f"{s} {l1}")
check("leave requests notify staff over the webhook", any(e.get("event") == "leave-requested" for e in webhook_events()[log_len:]), "no leave-requested event seen")
log_len = len(webhook_events())

s, l2, _ = teach.req("POST", "/api/leave", body={
    "start_date": plus_days(monday, 2), "end_date": plus_days(monday, 2),
    "part": "am", "reason": "E2E own leave",
})
check("a teacher can request their own leave", s == 201 and l2.get("id"), f"{s} {l2}")
s, mine, _ = teach.req("GET", "/api/leave?status=pending")
check("teachers only see their own requests", all(i["teacher_id"] == david_id for i in mine["items"]) and mine["items"], str(mine)[:160])
check("an invalid leave part is rejected", admin.req("POST", "/api/leave", body={
    "teacher_id": priya_id, "start_date": monday, "end_date": monday, "part": "night",
})[0] == 400)

# decide ----------------------------------------------------------------------
s, _, _ = admin.req("POST", f"/api/leave/{l1['id']}/decide", body={"approve": True})
check("staff approves the requested leave", s == 200, f"status={s}")
check("approvals notify the teacher over the webhook", any(e.get("event") == "leave-decided" for e in webhook_events()[log_len:]), "no leave-decided event seen")
log_len = len(webhook_events())
check("deciding twice is rejected", admin.req("POST", f"/api/leave/{l1['id']}/decide", body={"approve": True})[0] == 400)
check("teacher role cannot decide leave", teach.req("POST", f"/api/leave/{l2['id']}/decide", body={"approve": True})[0] == 403)
s, rejected, _ = admin.req("POST", f"/api/leave/{l2['id']}/decide", body={"approve": False})
check("staff can reject a request", s == 200, f"{s} {rejected}")

s, impact, _ = admin.req("GET", f"/api/leave/{l1['id']}/impact")
check("impact lists affected sessions on Mon+Tue", s == 200 and impact["sessions"], f"{len(impact.get('sessions', []))} sessions")
check("impact sessions carry day and lesson info", all(("day" in sess and "lesson_id" in sess) for sess in impact["sessions"][:3]), str(impact["sessions"][:2]))
check("student cannot read impact", stud.req("GET", f"/api/leave/{l1['id']}/impact")[0] == 403)

# ---------------------------------------------------------------- cover views
s, cover, _ = admin.req("GET", f"/api/cover?date={monday}")
check("cover day lists the absent teacher", s == 200 and any(a["teacher_name"] == "Priya Sharma" for a in cover["absences"]), str(cover["absences"])[:160])
periods = cover["periods"]
check("affected cover periods are listed with lesson info", periods and all("lesson_id" in p and "class_name" in p for p in periods), f"{len(periods)} periods")
s, cover_s, _ = sched.req("GET", f"/api/cover?date={monday}")
check("scheduler (staff) can read the cover view", s == 200 and cover_s["date"] == monday, f"{s} {cover_s.get('date')}")
check("student cannot read the cover view", stud.req("GET", "/api/cover")[0] == 403)

# Periods cover every absent teacher that day, and their order is not guaranteed,
# so pick one of Priya's own periods — assigning her as substitute only proves a
# rule about self-cover if the lesson really is hers.
p0 = next((p for p in periods if p.get("absent_teacher_id") == priya_id), None)
check("the cover view lists periods for the absent teacher", p0 is not None,
      f"{len(periods)} periods; absent={sorted({p.get('absent_teacher_name') for p in periods})}")
if p0 is None:
    summary("suite_cover_leave")
candidates = p0["candidates"]
check("candidates are ranked and annotated", candidates and all("score" in c and "reason" in c for c in candidates), str(candidates[:2]))
absent_ids = {a["teacher_id"] for a in cover["absences"]}
check("the absent teacher is not offered as their own candidate", all(c["teacher_id"] != priya_id for c in candidates), str(candidates[:2]))

# assign -----------------------------------------------------------------------
sub = candidates[0]
s, assigned, _ = admin.req("POST", "/api/cover/assign", body={
    "date": monday, "lesson_id": p0["lesson_id"], "day": p0["day"], "idx": p0["idx"],
    "substitute_teacher_id": sub["teacher_id"],
})
check("staff assigns a substitute", s == 201 and assigned.get("id") and assigned.get("notified", 0) >= 1, f"{s} {assigned}")
check("cover assignment notifies over the webhook", any(e.get("event") == "cover-assigned" for e in webhook_events()[log_len:]), "no cover-assigned event seen")
log_len = len(webhook_events())

s, reassigned, _ = admin.req("POST", "/api/cover/assign", body={
    "date": monday, "lesson_id": p0["lesson_id"], "day": p0["day"], "idx": p0["idx"],
    "substitute_teacher_id": sub["teacher_id"],
})
check("re-assigning the same slot updates in place", s == 200 and reassigned.get("updated") is True, f"{s} {reassigned}")

s, own, _ = admin.req("POST", "/api/cover/assign", body={
    "date": monday, "lesson_id": p0["lesson_id"], "day": p0["day"], "idx": p0["idx"],
    "substitute_teacher_id": priya_id,
})
check("the absent teacher cannot cover their own lesson", s == 400, f"{s} {own}")
check("cover for a lesson not on that timetable is rejected", admin.req("POST", "/api/cover/assign", body={
    "date": monday, "lesson_id": "no-such-lesson", "day": p0["day"], "idx": p0["idx"],
    "substitute_teacher_id": sub["teacher_id"],
})[0] == 400)
check("cover on a mismatched weekday is rejected", admin.req("POST", "/api/cover/assign", body={
    "date": monday, "lesson_id": p0["lesson_id"], "day": 3, "idx": p0["idx"],
    "substitute_teacher_id": sub["teacher_id"],
})[0] == 400)

# Pick a teacher the API says is busy at this same period, then expect a 409.
# It must come from p0's own blocked list and not be the substitute we already
# assigned: re-posting that slot with them would just update it in place (200).
s, cover2, _ = admin.req("GET", f"/api/cover?date={monday}")
p0_now = next((p for p in cover2["periods"]
               if p.get("absent_teacher_id") == priya_id and p["lesson_id"] == p0["lesson_id"]
               and p["idx"] == p0["idx"]), {"blocked": []})
busy_teacher = next((b["teacher_id"] for b in p0_now["blocked"]
                     if b["reason"] == "teaching another class at this time"
                     and b["teacher_id"] not in (sub["teacher_id"], priya_id)), None)
if busy_teacher:
    s, busy, _ = admin.req("POST", "/api/cover/assign", body={
        "date": monday, "lesson_id": p0["lesson_id"], "day": p0["day"], "idx": p0["idx"],
        "substitute_teacher_id": busy_teacher,
    })
    check("a busy substitute is a 409 conflict", s == 409, f"{s} {busy}")
else:
    print("PASS (skipped) a busy substitute is a 409 conflict -- no busy teacher at P%s" % p0["idx"])

# history & cancel --------------------------------------------------------------
s, hist, _ = admin.req("GET", f"/api/cover/history?from={monday}&to={plus_days(monday, 7)}")
check("cover history records the assignment", s == 200 and any(i["substitute_name"] == sub["name"] and i["status"] == "assigned" for i in hist["items"]), str(hist["items"])[:200])
sid = assigned["id"]
s, _, _ = admin.req("DELETE", f"/api/cover/{sid}")
check("staff cancels a cover", s == 200, f"status={s}")
s, hist2, _ = admin.req("GET", f"/api/cover/history?from={monday}&to={plus_days(monday, 7)}")
check("cancelled covers show up in history", any(i["id"] == sid and i["status"] == "cancelled" for i in hist2["items"]), str(hist2["items"])[:200])

# cleanup: cancel pending requests so the audit trail stays predictable
s, pending, _ = admin.req("GET", "/api/leave?status=pending")
for item in pending["items"]:
    admin.req("POST", f"/api/leave/{item['id']}/cancel")

summary("suite_cover_leave")
