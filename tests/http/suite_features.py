"""Feature checks added on top of the core seed: notifications (webhook
channel), the audit log, public read-only links, and the CSV/ICS/PDF exports.
Must run after suite_core (relies on generated, fully-placed entries)."""

import sys

from common import ADMIN, SCHEDULER, STUDENT, TEACHER, Api, check, summary, webhook_events

admin = Api()
admin.login(*ADMIN)
sched = Api()
sched.login(*SCHEDULER)
teach = Api()
teach.login(*TEACHER)
stud = Api()
stud.login(*STUDENT)
tt = admin.req("GET", "/api/timetable")[1]
first_class = admin.req("GET", "/api/setup/all")[1]["classes"][0]
class_id = first_class["id"]

# ------------------------------------------------------------- notifications
s, status, _ = admin.req("GET", "/api/notifications/status")
check("notifications status reports the webhook channel", s == 200 and status["enabled"] and "webhook" in status["channels"], str(status))
check("status lists the three notification events", len(status["events"]) == 3, str(status["events"]))
check("scheduler (staff) can read notification status", sched.req("GET", "/api/notifications/status")[0] == 200)
check("teacher role cannot read notification status", teach.req("GET", "/api/notifications/status")[0] == 403)
check("student role cannot read notification status", stud.req("GET", "/api/notifications/status")[0] == 403)

events_before = len(webhook_events())
s, test, _ = admin.req("POST", "/api/notifications/test")
check("test notification is delivered over the webhook", s == 200 and "webhook" in test.get("delivered", []), f"{s} {test}")
check("notification test resolves staff recipients", test.get("staff_recipients") == 2, str(test))
check("staff roles test scheduled but not the notify test route", sched.req("POST", "/api/notifications/test")[0] == 403)
new_events = webhook_events()[events_before:]
check("the webhook receiver captured the test payload", any(e.get("event") == "test" and "configured correctly" in e.get("body", "") for e in new_events), str(new_events[:2]))

# ---------------------------------------------------------------------- audit
s, audit, _ = admin.req("GET", "/api/audit?limit=200")
actions = {item["action"] for item in audit["items"]}
check("audit log is readable by staff", s == 200 and audit["items"], f"{s} {len(audit.get('items', []))} items")
check("audit records subject edits", "subject.create" in actions, str(sorted(actions))[:200])
check("audit records generations", "timetable.generate" in actions, str(sorted(actions))[:200])
check("audit records user administration", "user.create" in actions and "user.delete" in actions, str(sorted(actions))[:200])
admin_rows = {i["actor_name"] for i in audit["items"]}
check("audit rows carry the actor name", "Sam Rivera" in admin_rows, str(admin_rows))
s, filtered, _ = admin.req("GET", "/api/audit?entity=subject&limit=50")
check("audit can filter by entity", s == 200 and all(i["entity"] == "subject" for i in filtered["items"]), str(filtered)[:160])
check("student cannot read the audit log", stud.req("GET", "/api/audit")[0] == 403)
check("teacher role cannot read the audit log", teach.req("GET", "/api/audit")[0] == 403)

# --------------------------------------------------------------- public links
s, link, _ = admin.req("POST", "/api/public-links", body={"entity": "class", "entity_id": class_id, "label": "E2E"})
check("staff creates a public link", s == 201 and link.get("path", "").startswith("/public/"), f"{s} {link}")
token = link["token"]
s, again, _ = admin.req("POST", "/api/public-links", body={"entity": "class", "entity_id": class_id})
check("re-creating a link for the same timetable is idempotent", s == 200 and again["id"] == link["id"], f"{s} {again.get('id')}")
s, links, _ = admin.req("GET", "/api/public-links")
check("staff lists public links", s == 200 and any(i["id"] == link["id"] for i in links["items"]), str(links)[:160])
check("bad entity kinds are rejected", admin.req("POST", "/api/public-links", body={"entity": "school", "entity_id": "x"})[0] == 400)
check("links for missing timetables are rejected", admin.req("POST", "/api/public-links", body={"entity": "class", "entity_id": "no-such"})[0] == 400)

anon = Api()
s, pub, _ = anon.req("GET", f"/api/public/timetable/{token}")
check("anonymous reader gets the shared timetable", s == 200 and pub.get("school_name") == "Demo High School", f"{s} {str(pub)[:160]}")
check("the link is scoped to one class", pub.get("entity") == "class" and pub.get("entity_id") == class_id, str(pub)[:120])
own_entries = [e for e in tt["entries"] if e["class_id"] == class_id]
check("the public timetable shows only that class", len(pub["entries"]) == len(own_entries), f"{len(pub['entries'])} vs {len(own_entries)}")
check("a bad token is a 404", anon.req("GET", "/api/public/timetable/deadbeef")[0] == 404)
check("teacher role cannot manage public links", teach.req("GET", "/api/public-links")[0] == 403)
check("student cannot manage public links", stud.req("GET", "/api/public-links")[0] == 403)
s, _, _ = admin.req("DELETE", f"/api/public-links/{link['id']}")
check("staff can revoke a link", s == 200, f"status={s}")
check("revoked links stop working immediately", anon.req("GET", f"/api/public/timetable/{token}")[0] == 404)

# --------------------------------------------------------------------- exports
entries0 = [e for e in tt["entries"] if e["week"] == 0 and e["class_id"] == class_id and (e["day"], e["idx"])]

# CSV
s, csv_text, ctype = admin.req("GET", f"/api/export/timetable.csv?entity=class&id={class_id}&week=0")
lines = csv_text.strip().split("\n")
check("CSV export returns the right content type", "text/csv" in ctype, ctype)
check("CSV has a header and a title line", lines and lines[0].startswith("Period,Time,") and "Grade 7A" in lines[1], lines[:2])
subject_codes = {e["subject_code"] for e in entries0}
check("CSV lists the class's subject codes", any(code in csv_text for code in subject_codes), f"codes={sorted(subject_codes)}")
check("CSV renders the lunch break row", any("BREAK" in l for l in lines), f"rows={len(lines)}")

# ICS
s, ics, ctype = admin.req("GET", f"/api/export/timetable.ics?entity=class&id={class_id}&week=0")
check("ICS export returns the calendar content type", "text/calendar" in ctype, ctype)
check("ICS starts and ends as a calendar", ics.startswith("BEGIN:VCALENDAR\r\n") and ics.endswith("END:VCALENDAR\r\n"), ics[:60])
check("ICS uses local floating date-times with a T separator", "DTSTART:20" in ics and "T" in ics.split("DTSTART:")[1][:16], ics.split("DTSTART:")[1][:20])
check("ICS stamps DTSTAMP in UTC", "DTSTAMP:" in ics and ics.split("DTSTAMP:")[1][15] == "Z", ics.split("DTSTAMP:")[1][:20])
check("ICS repeats every week for a 1-week cycle", "RRULE:FREQ=WEEKLY;INTERVAL=1;COUNT=" in ics, [l for l in ics.splitlines() if "RRULE" in l][:1])
vevents = ics.count("BEGIN:VEVENT")
check("ICS emits one event per placed period", vevents == len(entries0), f"{vevents} vs {len(entries0)}")
check("ICS folds every line under 76 octets", all(len(l.encode()) <= 76 for l in ics.split("\r\n")), max(ics.split("\r\n"), key=len)[:80])

# PDF
s, raw, ctype = admin.req("GET", f"/api/export/timetable.pdf?entity=class&id={class_id}&week=0", raw=True)
check("PDF export returns application/pdf", "application/pdf" in ctype, ctype)
check("PDF has a header and EOF marker", raw.startswith(b"%PDF-1.4") and b"%%EOF" in raw[-32:], raw[:16])
check("PDF embeds Helvetica with WinAnsi encoding", b"/BaseFont /Helvetica" in raw and b"/WinAnsiEncoding" in raw, "")

summary("suite_features")
