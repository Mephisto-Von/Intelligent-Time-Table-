use super::*;

fn slots(days: &[i64], periods: usize) -> Vec<Slot> {
    let mut out = Vec::new();
    for &d in days {
        for p in 1..=periods {
            out.push(Slot { day: d, idx: p as i64 });
        }
    }
    out
}

fn slot_of(slots: &[Slot], day: i64, idx: i64) -> usize {
    slots
        .iter()
        .position(|s| s.day == day && s.idx == idx)
        .expect("slot exists")
}

fn base_input(slots: Vec<Slot>) -> Input {
    Input {
        slots,
        weeks: 1,
        lessons: vec![],
        teachers: vec![],
        classes: vec![],
        rooms: vec![],
        weights: Weights::default(),
        seed: 42,
        time_limit_ms: 0,
        iterations: Some(30_000),
    }
}

fn teacher(id: &str) -> TeacherIn {
    TeacherIn {
        id: id.to_string(),
        max_weekly: Some(40),
        max_daily: Some(8),
        unavailable: vec![],
    }
}

fn class(id: &str, size: i64) -> ClassIn {
    ClassIn { id: id.to_string(), size }
}

fn room(id: &str, capacity: i64, room_type: &str) -> RoomIn {
    RoomIn { id: id.to_string(), capacity, room_type: room_type.to_string() }
}

fn lesson(id: &str, class: usize, teacher: usize, subject: usize, periods: u32, max_per_day: u32) -> LessonIn {
    LessonIn {
        id: id.to_string(),
        class,
        teacher,
        subject,
        periods,
        max_per_day,
        room: None,
        required_room_type: None,
        fixed: vec![],
    }
}

fn assert_conflict_free(inp: &Input, out: &Output) {
    let ns = inp.slots.len();
    use std::collections::HashSet;
    let mut class_seen: HashSet<(usize, usize, usize)> = HashSet::new(); // (class, week, g)
    let mut teacher_seen: HashSet<(usize, usize, usize)> = HashSet::new();
    let mut room_seen: HashSet<(usize, usize, usize)> = HashSet::new();

    for p in &out.placed {
        let l = &inp.lessons[p.lesson];
        assert!(p.slot < ns, "slot in range");
        assert!(p.week < inp.weeks.max(1), "week in range");
        let g = p.week * ns + p.slot;
        assert!(class_seen.insert((l.class, p.week, g)), "class clash");
        assert!(teacher_seen.insert((l.teacher, p.week, g)), "teacher clash");
        if let Some(r) = p.room {
            assert!(room_seen.insert((r, p.week, g)), "room clash");
            let room_cfg = &inp.rooms[r];
            if l.room.is_none() {
                assert!(room_cfg.capacity >= inp.classes[l.class].size, "room too small");
                if let Some(t) = &l.required_room_type {
                    assert!(&room_cfg.room_type == t, "wrong room type");
                }
            }
        } else {
            assert!(l.room.is_none() && l.required_room_type.is_none(), "required room missing");
        }
        // teacher unavailability
        let s = &inp.slots[p.slot];
        for &(d, i) in &inp.teachers[l.teacher].unavailable {
            assert!(!(s.day == d && s.idx == i), "placed during unavailability");
        }
    }

    // max per day
    use std::collections::HashMap;
    let mut per_day: HashMap<(usize, usize, i64), u32> = HashMap::new();
    for p in &out.placed {
        let s = &inp.slots[p.slot];
        *per_day.entry((p.lesson, p.week, s.day)).or_default() += 1;
    }
    for ((l, _, _), n) in per_day {
        assert!(n <= inp.lessons[l].max_per_day.max(1), "max per day exceeded");
    }
}

fn total_unplaced(out: &Output) -> u32 {
    out.total_unplaced()
}

#[test]
fn places_everything_when_feasible() {
    let sl = slots(&[1, 2, 3, 4, 5], 6);
    let mut inp = base_input(sl);
    inp.teachers = vec![teacher("t0"), teacher("t1"), teacher("t2")];
    inp.classes = vec![class("c0", 30), class("c1", 30), class("c2", 30)];
    inp.rooms = vec![room("r0", 35, "general"), room("r1", 35, "general")];
    for c in 0..3 {
        for t in 0..3 {
            inp.lessons.push(lesson(
                &format!("l{c}{t}"),
                c,
                t,
                t,
                8,
                2,
            ));
        }
    }
    let out = run(&inp);
    assert_eq!(total_unplaced(&out), 0, "all periods should be placed: {:?}", out.unplaced);
    assert_eq!(out.placed.len(), 72);
    assert_conflict_free(&inp, &out);
}

#[test]
fn respects_teacher_unavailability() {
    let sl = slots(&[1, 2, 3, 4, 5], 5);
    let mut inp = base_input(sl);
    inp.teachers = vec![TeacherIn {
        id: "t0".into(),
        max_weekly: Some(40),
        max_daily: Some(8),
        unavailable: vec![(1, 1), (1, 2), (1, 3), (1, 4), (1, 5), (2, 1)],
    }];
    inp.classes = vec![class("c0", 30)];
    inp.rooms = vec![room("r0", 35, "general")];
    inp.lessons = vec![lesson("l0", 0, 0, 0, 9, 3)];
    let out = run(&inp);
    assert_eq!(total_unplaced(&out), 0, "should fit in remaining slots: {:?}", out.unplaced);
    assert_conflict_free(&inp, &out);
    let s = &inp.slots;
    let i = slot_of(s, 1, 1);
    assert!(!out.placed.iter().any(|p| p.slot == i), "must not use unavailable slot");
}

#[test]
fn respects_max_per_day() {
    let sl = slots(&[1, 2, 3, 4, 5], 6);
    let mut inp = base_input(sl);
    inp.teachers = vec![teacher("t0")];
    inp.classes = vec![class("c0", 30)];
    inp.rooms = vec![room("r0", 35, "general")];
    inp.lessons = vec![lesson("l0", 0, 0, 0, 10, 2)];
    let out = run(&inp);
    assert_eq!(total_unplaced(&out), 0);
    assert_conflict_free(&inp, &out);
}

#[test]
fn honours_fixed_placements() {
    let sl = slots(&[1, 2, 3, 4, 5], 6);
    let mut inp = base_input(sl.clone());
    inp.teachers = vec![teacher("t0"), teacher("t1")];
    inp.classes = vec![class("c0", 30)];
    inp.rooms = vec![room("r0", 30, "general"), room("r1", 30, "general")];
    let mut l0 = lesson("l0", 0, 0, 0, 4, 2);
    let mut l1 = lesson("l1", 0, 1, 1, 4, 2);
    l0.fixed = vec![FixedPlacement { week: 0, slot: slot_of(&sl, 3, 2), room: None }];
    l1.fixed = vec![FixedPlacement { week: 0, slot: slot_of(&sl, 1, 5), room: None }];
    inp.lessons = vec![l0, l1];
    let out = run(&inp);
    assert_eq!(total_unplaced(&out), 0);
    assert!(out
        .placed
        .iter()
        .any(|p| p.lesson == 0 && p.week == 0 && p.slot == slot_of(&inp.slots, 3, 2)));
    assert!(out
        .placed
        .iter()
        .any(|p| p.lesson == 1 && p.week == 0 && p.slot == slot_of(&inp.slots, 1, 5)));
    assert_conflict_free(&inp, &out);
}

#[test]
fn reports_unplaced_when_infeasible() {
    let sl = slots(&[1], 2);
    let mut inp = base_input(sl);
    inp.teachers = vec![teacher("t0")];
    inp.classes = vec![class("c0", 30)];
    inp.rooms = vec![room("r0", 30, "general")];
    inp.lessons = vec![lesson("l0", 0, 0, 0, 5, 5)];
    let out = run(&inp);
    assert_eq!(total_unplaced(&out), 3, "only 2 slots available");
    assert_eq!(out.unplaced.len(), 1);
    assert!(!out.unplaced[0].dominant.is_empty());
}

#[test]
fn required_room_type_and_capacity_respected() {
    let sl = slots(&[1, 2, 3, 4, 5], 6);
    let mut inp = base_input(sl);
    inp.teachers = vec![teacher("t0"), teacher("t1")];
    inp.classes = vec![class("c0", 28), class("c1", 20)];
    inp.rooms = vec![
        room("small", 20, "general"),
        room("big", 45, "general"),
        room("lab", 30, "lab"),
    ];
    let mut l0 = lesson("l0", 0, 0, 0, 5, 1);
    l0.required_room_type = Some("lab".to_string());
    let mut l1 = lesson("l1", 0, 1, 1, 5, 1);
    l1.room = Some(1); // fixed big room
    let l2 = lesson("l2", 1, 1, 1, 5, 1);
    inp.lessons = vec![l0, l1, l2];
    let out = run(&inp);
    assert_conflict_free(&inp, &out);
    assert_eq!(total_unplaced(&out), 0);
    for p in &out.placed {
        if p.lesson == 0 {
            assert_eq!(p.room, Some(2), "must use the lab");
        }
        if p.lesson == 1 {
            assert_eq!(p.room, Some(1), "fixed room");
        }
        if p.lesson == 2 {
            assert_eq!(p.room, Some(0), "20-size class fits small room");
        }
    }
}

#[test]
fn two_week_cycle_is_conflict_free_and_balanced() {
    let sl = slots(&[1, 2, 3, 4, 5], 7);
    let mut inp = base_input(sl);
    inp.weeks = 2;
    inp.teachers = (0..6).map(|i| teacher(&format!("t{i}"))).collect();
    inp.classes = (0..4).map(|i| class(&format!("c{i}"), 28)).collect();
    inp.rooms = vec![
        room("g1", 35, "general"),
        room("g2", 35, "general"),
        room("lab", 30, "lab"),
        room("gym", 60, "gym"),
    ];
    for c in 0..4 {
        let mut l = lesson(&format!("math{c}"), c, 0, 0, 5, 1);
        inp.lessons.push(l.clone());
        l = lesson(&format!("eng{c}"), c, 1, 1, 4, 1);
        inp.lessons.push(l.clone());
        let mut phy = lesson(&format!("phy{c}"), c, 2, 2, 3, 1);
        phy.required_room_type = Some("lab".into());
        inp.lessons.push(phy);
        let mut pe = lesson(&format!("pe{c}"), c, 3, 3, 2, 1);
        pe.required_room_type = Some("gym".into());
        inp.lessons.push(pe);
        inp.lessons.push(lesson(&format!("sci{c}"), c, 4, 4, 4, 1));
        inp.lessons.push(lesson(&format!("art{c}"), c, 5, 5, 2, 1));
    }
    let out = run(&inp);
    assert_eq!(total_unplaced(&out), 0, "everything should fit: {:?}", out.unplaced);
    assert_conflict_free(&inp, &out);
    // weeks should be roughly balanced per class (soft goal)
    for c in 0..4 {
        for w in 0..2 {
            let n = out
                .placed
                .iter()
                .filter(|p| inp.lessons[p.lesson].class == c && p.week == w)
                .count();
            assert!(n >= 5, "class {c} week {w} has only {n} lessons");
        }
    }
}

#[test]
fn deterministic_for_fixed_seed_and_iterations() {
    let sl = slots(&[1, 2, 3], 5);
    let mut inp = base_input(sl);
    inp.teachers = vec![teacher("t0"), teacher("t1")];
    inp.classes = vec![class("c0", 30), class("c1", 30)];
    inp.rooms = vec![room("r0", 35, "general")];
    inp.lessons = vec![
        lesson("a", 0, 0, 0, 6, 2),
        lesson("b", 0, 1, 1, 6, 2),
        lesson("c", 1, 0, 0, 5, 2),
    ];
    let out1 = run(&inp);
    let out2 = run(&inp);
    assert_eq!(
        serde_json::to_value(&out1.placed).unwrap(),
        serde_json::to_value(&out2.placed).unwrap()
    );
    assert!((out1.cost - out2.cost).abs() < 1e-9);
}
