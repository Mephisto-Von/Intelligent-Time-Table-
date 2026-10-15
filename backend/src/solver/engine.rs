use std::time::Instant;

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use super::{Input, Output, Placed, ReasonCounts, SoftCosts, UnplacedInfo};

const T0: f64 = 6.0;
const T_END: f64 = 0.01;

#[derive(Clone, Copy, PartialEq)]
enum Row {
    Class { c: usize, week: usize, day: usize },
    Teacher { t: usize, week: usize, day: usize },
}

#[derive(Clone, Copy)]
enum Move {
    Relocate { e: usize, week: usize, slot: usize },
    Swap { a: usize, b: usize },
    Insert { lesson: usize },
}

struct Grid<'a> {
    inp: &'a Input,
    ns: usize,
    weeks: usize,
    ndays: usize,
    /// slot index -> day position
    day_pos: Vec<usize>,
    /// day position -> slot indices
    day_slots: Vec<Vec<usize>>,
    occ_class: Vec<i32>,
    occ_teacher: Vec<i32>,
    occ_room: Vec<i32>,
    entries: Vec<Placed>,
    lesson_day: Vec<u32>,
    teacher_day: Vec<u32>,
    teacher_week: Vec<u32>,
    class_week: Vec<u32>,
    unplaced: Vec<u32>,
    /// Per lesson: candidate room indices, sorted by capacity ascending.
    room_candidates: Vec<Vec<usize>>,
    /// Per (teacher, slot): unavailable flag.
    teacher_unavail: Vec<bool>,
    cost: f64,
}

struct Snapshot {
    entries: Vec<Placed>,
    unplaced: Vec<u32>,
    cost: f64,
}

fn gidx(week: usize, ns: usize, slot: usize) -> usize {
    week * ns + slot
}

impl<'a> Grid<'a> {
    fn new(inp: &'a Input) -> Self {
        let ns = inp.slots.len().max(1);
        let weeks = inp.weeks.max(1);
        let g = ns * weeks;

        let mut days: Vec<i64> = inp.slots.iter().map(|s| s.day).collect();
        days.sort_unstable();
        days.dedup();
        let ndays = days.len().max(1);
        let day_pos: Vec<usize> = inp
            .slots
            .iter()
            .map(|s| days.iter().position(|&d| d == s.day).unwrap_or(0))
            .collect();
        let mut day_slots = vec![Vec::new(); ndays];
        for (i, &dp) in day_pos.iter().enumerate() {
            day_slots[dp].push(i);
        }

        let nclasses = inp.classes.len();
        let nteachers = inp.teachers.len();
        let nrooms = inp.rooms.len();

        let room_candidates: Vec<Vec<usize>> = inp
            .lessons
            .iter()
            .map(|l| {
                if let Some(r) = l.room {
                    return vec![r];
                }
                let mut cands: Vec<usize> = (0..nrooms)
                    .filter(|&r| {
                        let room = &inp.rooms[r];
                        if let Some(t) = &l.required_room_type {
                            room.room_type == *t
                        } else {
                            true
                        }
                    })
                    .filter(|&r| {
                        inp.rooms[r].capacity >= inp.classes.get(l.class).map(|c| c.size).unwrap_or(0)
                    })
                    .collect();
                cands.sort_by_key(|&r| inp.rooms[r].capacity);
                cands
            })
            .collect();

        let mut teacher_unavail = vec![false; nteachers * ns];
        for (t, teacher) in inp.teachers.iter().enumerate() {
            for &(day, idx) in &teacher.unavailable {
                for (s, slot) in inp.slots.iter().enumerate() {
                    if slot.day == day && slot.idx == idx {
                        teacher_unavail[t * ns + s] = true;
                    }
                }
            }
        }

        Grid {
            inp,
            ns,
            weeks,
            ndays,
            day_pos,
            day_slots,
            occ_class: vec![-1; nclasses * g],
            occ_teacher: vec![-1; nteachers * g],
            occ_room: vec![-1; nrooms * g],
            entries: Vec::new(),
            lesson_day: vec![0; inp.lessons.len() * weeks * ndays],
            teacher_day: vec![0; nteachers * weeks * ndays],
            teacher_week: vec![0; nteachers * weeks],
            class_week: vec![0; nclasses * weeks],
            unplaced: vec![0; inp.lessons.len()],
            room_candidates,
            teacher_unavail,
            cost: 0.0,
        }
    }

    fn lesson_day_i(&self, lesson: usize, week: usize, day: usize) -> usize {
        (lesson * self.weeks + week) * self.ndays + day
    }
    fn teacher_day_i(&self, t: usize, week: usize, day: usize) -> usize {
        (t * self.weeks + week) * self.ndays + day
    }

    // ----- occupancy mutation -----

    fn place_entry(&mut self, e: usize, week: usize, slot: usize, room: Option<usize>) {
        let lesson = self.entries[e].lesson;
        self.entries[e] = Placed { lesson, week, slot, room };
        self.write_occ(e, lesson, week, slot, room, true);
    }

    fn detach_entry(&mut self, e: usize) {
        let p = self.entries[e];
        self.write_occ(e, p.lesson, p.week, p.slot, p.room, false);
    }

    fn push_entry(&mut self, p: Placed) {
        self.entries.push(p);
        let e = self.entries.len() - 1;
        self.write_occ(e, p.lesson, p.week, p.slot, p.room, true);
    }

    fn pop_entry(&mut self) {
        let e = self.entries.len() - 1;
        self.detach_entry(e);
        self.entries.pop();
    }

    fn write_occ(
        &mut self,
        eidx: usize,
        lesson: usize,
        week: usize,
        slot: usize,
        room: Option<usize>,
        on: bool,
    ) {
        let g = gidx(week, self.ns, slot);
        let stride = self.ns * self.weeks;
        let l = &self.inp.lessons[lesson];
        let cls = l.class;
        let t = l.teacher;
        let dp = self.day_pos[slot];
        let eidx = eidx as i32;
        let c_i = cls * stride + g;
        let t_i = t * stride + g;
        if on {
            self.occ_class[c_i] = eidx;
            self.occ_teacher[t_i] = eidx;
            if let Some(r) = room {
                self.occ_room[r * stride + g] = eidx;
            }
        } else {
            self.occ_class[c_i] = -1;
            self.occ_teacher[t_i] = -1;
            if let Some(r) = room {
                self.occ_room[r * stride + g] = -1;
            }
        }
        let ldi = self.lesson_day_i(lesson, week, dp);
        let tdi = self.teacher_day_i(t, week, dp);
        let twi = t * self.weeks + week;
        let cwi = cls * self.weeks + week;
        if on {
            self.lesson_day[ldi] += 1;
            self.teacher_day[tdi] += 1;
            self.teacher_week[twi] += 1;
            self.class_week[cwi] += 1;
        } else {
            self.lesson_day[ldi] = self.lesson_day[ldi].saturating_sub(1);
            self.teacher_day[tdi] = self.teacher_day[tdi].saturating_sub(1);
            self.teacher_week[twi] = self.teacher_week[twi].saturating_sub(1);
            self.class_week[cwi] = self.class_week[cwi].saturating_sub(1);
        }
    }

    // ----- feasibility -----

    fn try_hard(&self, lesson: usize, week: usize, slot: usize) -> bool {
        let l = &self.inp.lessons[lesson];
        let g = gidx(week, self.ns, slot);
        let stride = self.ns * self.weeks;
        if self.teacher_unavail[l.teacher * self.ns + slot] {
            return false;
        }
        if self.occ_class[l.class * stride + g] != -1 || self.occ_teacher[l.teacher * stride + g] != -1 {
            return false;
        }
        let dp = self.day_pos[slot];
        if self.lesson_day[self.lesson_day_i(lesson, week, dp)] + 1 > l.max_per_day.max(1) {
            return false;
        }
        let teacher = &self.inp.teachers[l.teacher];
        if let Some(md) = teacher.max_daily {
            if self.teacher_day[self.teacher_day_i(l.teacher, week, dp)] + 1 > md {
                return false;
            }
        }
        if let Some(mw) = teacher.max_weekly {
            if self.teacher_week[l.teacher * self.weeks + week] + 1 > mw {
                return false;
            }
        }
        true
    }

    /// First free room from the lesson's candidate list. Err(()) when a room is
    /// required (fixed room or required type) but none is free; Ok(None) when the
    /// lesson accepts "no room" as a fallback.
    fn best_room(&self, lesson: usize, week: usize, slot: usize) -> Result<Option<usize>, ()> {
        let l = &self.inp.lessons[lesson];
        let g = gidx(week, self.ns, slot);
        let stride = self.ns * self.weeks;
        for &r in &self.room_candidates[lesson] {
            if self.occ_room[r * stride + g] == -1 {
                return Ok(Some(r));
            }
        }
        if l.room.is_some() || l.required_room_type.is_some() {
            Err(())
        } else {
            Ok(None)
        }
    }

    // ----- costs -----

    fn class_row_parts(&self, c: usize, week: usize, day: usize) -> (f64, f64) {
        let stride = self.ns * self.weeks;
        let slots = &self.day_slots[day];
        let mut occupied = 0usize;
        let mut first: Option<usize> = None;
        let mut last = 0usize;
        let mut subjects: Vec<(usize, u32)> = Vec::new();
        for &s in slots {
            let g = gidx(week, self.ns, s);
            let e = self.occ_class[c * stride + g];
            if e >= 0 {
                let p = self.entries[e as usize];
                occupied += 1;
                first.get_or_insert(s);
                last = s;
                let subj = self.inp.lessons[p.lesson].subject;
                match subjects.iter_mut().find(|(sj, _)| *sj == subj) {
                    Some((_, n)) => *n += 1,
                    None => subjects.push((subj, 1)),
                }
            }
        }
        let gaps = match first {
            Some(f) => (last - f + 1 - occupied) as f64,
            None => 0.0,
        };
        let same: f64 = subjects
            .iter()
            .filter(|(_, n)| *n > 1)
            .map(|(_, n)| (*n - 1) as f64)
            .sum();
        (gaps, same)
    }

    fn teacher_row_parts(&self, t: usize, week: usize, day: usize) -> (f64, f64) {
        let stride = self.ns * self.weeks;
        let slots = &self.day_slots[day];
        let mut occupied = 0usize;
        let mut first: Option<usize> = None;
        let mut last = 0usize;
        for &s in slots {
            let g = gidx(week, self.ns, s);
            if self.occ_teacher[t * stride + g] >= 0 {
                occupied += 1;
                first.get_or_insert(s);
                last = s;
            }
        }
        let gaps = match first {
            Some(f) => (last - f + 1 - occupied) as f64,
            None => 0.0,
        };
        let balance = (occupied * occupied) as f64;
        (gaps, balance)
    }

    fn rows_cost(&self, rows: &[Row]) -> f64 {
        let w = &self.inp.weights;
        let mut total = 0.0;
        for row in rows {
            match *row {
                Row::Class { c, week, day } => {
                    let (gaps, same) = self.class_row_parts(c, week, day);
                    total += gaps * w.class_gap + same * w.same_subject_per_day;
                }
                Row::Teacher { t, week, day } => {
                    let (gaps, balance) = self.teacher_row_parts(t, week, day);
                    total += gaps * w.teacher_gap + balance * w.teacher_balance;
                }
            }
        }
        total
    }

    fn rows_of(&self, lesson: usize, week: usize, slot: usize) -> [Row; 2] {
        let dp = self.day_pos[slot];
        let l = &self.inp.lessons[lesson];
        [
            Row::Class { c: l.class, week, day: dp },
            Row::Teacher { t: l.teacher, week, day: dp },
        ]
    }

    fn week_term(&self) -> f64 {
        let w = self.inp.weights.week_balance;
        if w == 0.0 {
            return 0.0;
        }
        let t: f64 = self.teacher_week.iter().map(|&c| (c * c) as f64).sum();
        let c: f64 = self.class_week.iter().map(|&c| (c * c) as f64).sum();
        (t + c) * w
    }

    fn recompute_cost(&mut self) {
        let mut total = 0.0;
        for c in 0..self.inp.classes.len() {
            for week in 0..self.weeks {
                for day in 0..self.ndays {
                    total += self.rows_cost(&[Row::Class { c, week, day }]);
                }
            }
        }
        for t in 0..self.inp.teachers.len() {
            for week in 0..self.weeks {
                for day in 0..self.ndays {
                    total += self.rows_cost(&[Row::Teacher { t, week, day }]);
                }
            }
        }
        let unplaced: u32 = self.unplaced.iter().sum();
        total += unplaced as f64 * self.inp.weights.unplaced;
        total += self.week_term();
        self.cost = total;
    }

    // ----- greedy -----

    fn greedy(&mut self, rng: &mut StdRng) {
        let _ = rng;
        // Phase 0: fixed placements.
        for l in 0..self.inp.lessons.len() {
            let fixed = self.inp.lessons[l].fixed.clone();
            for f in fixed {
                if f.slot >= self.ns || f.week >= self.weeks {
                    self.unplaced[l] += 1;
                    continue;
                }
                if !self.try_hard(l, f.week, f.slot) {
                    self.unplaced[l] += 1;
                    continue;
                }
                let room = match f.room {
                    Some(r) => {
                        let g = gidx(f.week, self.ns, f.slot);
                        if self.occ_room[r * (self.ns * self.weeks) + g] == -1 {
                            Some(r)
                        } else {
                            match self.best_room(l, f.week, f.slot) {
                                Ok(r) => r,
                                Err(_) => {
                                    self.unplaced[l] += 1;
                                    continue;
                                }
                            }
                        }
                    }
                    None => match self.best_room(l, f.week, f.slot) {
                        Ok(r) => r,
                        Err(_) => {
                            self.unplaced[l] += 1;
                            continue;
                        }
                    },
                };
                self.push_entry(Placed { lesson: l, week: f.week, slot: f.slot, room });
            }
        }

        // Phase 1: most-constrained-first greedy over remaining periods.
        let mut order: Vec<usize> = (0..self.inp.lessons.len()).collect();
        let freedom = |g: &Grid, l: usize| -> usize {
            let t = g.inp.lessons[l].teacher;
            let mut f = 0;
            for s in 0..g.ns {
                if !g.teacher_unavail[t * g.ns + s] {
                    f += 1;
                }
            }
            f * g.weeks
        };
        let mut freedom_cache: Vec<usize> = (0..self.inp.lessons.len()).map(|l| freedom(self, l)).collect();
        let _ = &mut freedom_cache;
        order.sort_by(|&a, &b| {
            let la = &self.inp.lessons[a];
            let lb = &self.inp.lessons[b];
            let ra = la.required_room_type.is_some() || la.room.is_some();
            let rb = lb.required_room_type.is_some() || lb.room.is_some();
            rb.cmp(&ra)
                .then(lb.periods.cmp(&la.periods))
                .then(freedom_cache[a].cmp(&freedom_cache[b]))
                .then(a.cmp(&b))
        });

        for &l in &order {
            let already = self.entries.iter().filter(|p| p.lesson == l).count() as u32;
            let target = self.inp.lessons[l].periods;
            for _ in already..target {
                match self.best_slot_for(l) {
                    Some((week, slot, room)) => {
                        self.push_entry(Placed { lesson: l, week, slot, room });
                    }
                    None => {
                        self.unplaced[l] += 1;
                    }
                }
            }
        }
        self.recompute_cost();
    }

    fn best_slot_for(&mut self, lesson: usize) -> Option<(usize, usize, Option<usize>)> {
        let mut best: Option<(f64, usize, usize, Option<usize>)> = None;
        let wt_state = self.week_term();
        for week in 0..self.weeks {
            for slot in 0..self.ns {
                if !self.try_hard(lesson, week, slot) {
                    continue;
                }
                let room = match self.best_room(lesson, week, slot) {
                    Ok(r) => r,
                    Err(_) => continue,
                };
                let rows = self.rows_of(lesson, week, slot);
                let before = self.rows_cost(&rows);
                self.push_entry(Placed { lesson, week, slot, room });
                let after = self.rows_cost(&rows);
                let wt_after = self.week_term();
                self.pop_entry();
                let delta = (after - before) + (wt_after - wt_state);
                let cand = (delta, week, slot, room);
                if best.as_ref().map(|b| delta < b.0).unwrap_or(true) {
                    best = Some(cand);
                }
            }
        }
        best.map(|(_, w, s, r)| (w, s, r))
    }

    // ----- annealing moves -----

    fn snapshot(&self) -> Snapshot {
        Snapshot { entries: self.entries.clone(), unplaced: self.unplaced.clone(), cost: self.cost }
    }

    fn apply_snapshot(&mut self, snap: &Snapshot) {
        // Rebuild occupancy from a snapshot.
        self.occ_class.iter_mut().for_each(|v| *v = -1);
        self.occ_teacher.iter_mut().for_each(|v| *v = -1);
        self.occ_room.iter_mut().for_each(|v| *v = -1);
        self.lesson_day.iter_mut().for_each(|v| *v = 0);
        self.teacher_day.iter_mut().for_each(|v| *v = 0);
        self.teacher_week.iter_mut().for_each(|v| *v = 0);
        self.entries.clear();
        for p in &snap.entries {
            self.entries.push(*p);
            let e = self.entries.len() - 1;
            self.write_occ(e, p.lesson, p.week, p.slot, p.room, true);
        }
        self.unplaced = snap.unplaced.clone();
        self.cost = snap.cost;
    }

    fn pick_move(&mut self, rng: &mut StdRng) -> Option<Move> {
        let unplaced: u32 = self.unplaced.iter().sum();
        let r: f64 = rng.gen_range(0.0..1.0);
        if unplaced > 0 && r < 0.3 {
            let idxs: Vec<usize> = (0..self.unplaced.len())
                .filter(|&l| self.unplaced[l] > 0)
                .collect();
            let lesson = idxs[rng.gen_range(0..idxs.len())];
            return Some(Move::Insert { lesson });
        }
        if self.entries.len() < 2 {
            return None;
        }
        if rng.gen_range(0.0..1.0) < 0.45 {
            let a = rng.gen_range(0..self.entries.len());
            let mut b = rng.gen_range(0..self.entries.len());
            if b == a {
                b = (b + 1) % self.entries.len();
            }
            return Some(Move::Swap { a, b });
        }
        let e = rng.gen_range(0..self.entries.len());
        let cur = self.entries[e];
        let mut week;
        let mut slot;
        let mut tries = 0;
        loop {
            week = rng.gen_range(0..self.weeks);
            slot = rng.gen_range(0..self.ns);
            if week != cur.week || slot != cur.slot {
                break;
            }
            tries += 1;
            if tries > 12 {
                return None;
            }
        }
        Some(Move::Relocate { e, week, slot })
    }

    fn accept(delta: f64, temp: f64, rng: &mut StdRng) -> bool {
        delta <= 0.0 || rng.gen_range(0.0..1.0) < (-delta / temp.max(1e-9)).exp()
    }

    fn execute(&mut self, mv: Move, temp: f64, rng: &mut StdRng) -> bool {
        match mv {
            Move::Relocate { e, week, slot } => {
                let old = self.entries[e];
                if !self.try_hard(old.lesson, week, slot) {
                    return false;
                }
                let room = match self.best_room(old.lesson, week, slot) {
                    Ok(r) => r,
                    Err(_) => return false,
                };
                let rows = dedup_rows([
                    self.rows_of(old.lesson, old.week, old.slot),
                    self.rows_of(old.lesson, week, slot),
                ]);
                let before = self.rows_cost(&rows);
                let wt_before = self.week_term();
                self.detach_entry(e);
                self.place_entry(e, week, slot, room);
                let after = self.rows_cost(&rows);
                let delta = (after - before) + (self.week_term() - wt_before);
                if Self::accept(delta, temp, rng) {
                    self.cost += delta;
                    true
                } else {
                    self.detach_entry(e);
                    self.place_entry(e, old.week, old.slot, old.room);
                    false
                }
            }
            Move::Swap { a, b } => {
                let pa = self.entries[a];
                let pb = self.entries[b];
                if pa.lesson == pb.lesson && pa.week == pb.week && pa.slot == pb.slot {
                    return false;
                }
                let wt_orig = self.week_term();
                // Feasibility after detaching both.
                self.detach_entry(a);
                self.detach_entry(b);
                let ok_a = self.try_hard(pa.lesson, pb.week, pb.slot)
                    .then(|| self.best_room(pa.lesson, pb.week, pb.slot));
                let room_a = match ok_a {
                    Some(Ok(r)) => r,
                    _ => {
                        self.place_entry(a, pa.week, pa.slot, pa.room);
                        self.place_entry(b, pb.week, pb.slot, pb.room);
                        return false;
                    }
                };
                self.place_entry(a, pb.week, pb.slot, room_a);
                let ok_b = self.try_hard(pb.lesson, pa.week, pa.slot)
                    .then(|| self.best_room(pb.lesson, pa.week, pa.slot));
                let room_b = match ok_b {
                    Some(Ok(r)) => r,
                    _ => {
                        self.detach_entry(a);
                        self.place_entry(a, pa.week, pa.slot, pa.room);
                        self.place_entry(b, pb.week, pb.slot, pb.room);
                        return false;
                    }
                };
                self.place_entry(b, pa.week, pa.slot, room_b);

                let rows = dedup_rows([
                    self.rows_of(pa.lesson, pa.week, pa.slot),
                    self.rows_of(pa.lesson, pb.week, pb.slot),
                    self.rows_of(pb.lesson, pa.week, pa.slot),
                    self.rows_of(pb.lesson, pb.week, pb.slot),
                ]);
                // rows_cost now includes the swapped state; estimate "before" by
                // computing the same rows with entries back at origin.
                let after = self.rows_cost(&rows);
                let wt_swapped = self.week_term();
                self.detach_entry(a);
                self.detach_entry(b);
                self.place_entry(a, pa.week, pa.slot, pa.room);
                self.place_entry(b, pb.week, pb.slot, pb.room);
                let before = self.rows_cost(&rows);
                let delta = (after - before) + (wt_swapped - wt_orig);
                if Self::accept(delta, temp, rng) {
                    self.detach_entry(a);
                    self.detach_entry(b);
                    self.place_entry(a, pb.week, pb.slot, room_a);
                    self.place_entry(b, pa.week, pa.slot, room_b);
                    self.cost += delta;
                    true
                } else {
                    false
                }
            }
            Move::Insert { lesson } => {
                if self.unplaced[lesson] == 0 {
                    return false;
                }
                let week = rng.gen_range(0..self.weeks);
                let slot = rng.gen_range(0..self.ns);
                if !self.try_hard(lesson, week, slot) {
                    return false;
                }
                let room = match self.best_room(lesson, week, slot) {
                    Ok(r) => r,
                    Err(_) => return false,
                };
                let rows = self.rows_of(lesson, week, slot);
                let before = self.rows_cost(&rows);
                let wt_before = self.week_term();
                self.push_entry(Placed { lesson, week, slot, room });
                self.unplaced[lesson] -= 1;
                let after = self.rows_cost(&rows);
                let delta = (after - before) - self.inp.weights.unplaced + (self.week_term() - wt_before);
                if Self::accept(delta, temp, rng) {
                    self.cost += delta;
                    true
                } else {
                    self.pop_entry();
                    self.unplaced[lesson] += 1;
                    false
                }
            }
        }
    }

    // ----- reporting -----

    fn soft_breakdown(&self) -> SoftCosts {
        let w = &self.inp.weights;
        let mut out = SoftCosts::default();
        for c in 0..self.inp.classes.len() {
            for week in 0..self.weeks {
                for day in 0..self.ndays {
                    let (gaps, same) = self.class_row_parts(c, week, day);
                    out.class_gaps += gaps * w.class_gap;
                    out.same_subject_per_day += same * w.same_subject_per_day;
                }
            }
        }
        for t in 0..self.inp.teachers.len() {
            for week in 0..self.weeks {
                for day in 0..self.ndays {
                    let (gaps, balance) = self.teacher_row_parts(t, week, day);
                    out.teacher_gaps += gaps * w.teacher_gap;
                    out.teacher_balance += balance * w.teacher_balance;
                }
            }
        }
        out
    }

    fn unplaced_info(&self) -> Vec<UnplacedInfo> {
        let mut out = Vec::new();
        for l in 0..self.inp.lessons.len() {
            let n = self.unplaced[l];
            if n == 0 {
                continue;
            }
            let mut rc = ReasonCounts::default();
            for week in 0..self.weeks {
                for slot in 0..self.ns {
                    let g = gidx(week, self.ns, slot);
                    let stride = self.ns * self.weeks;
                    let les = &self.inp.lessons[l];
                    if self.teacher_unavail[les.teacher * self.ns + slot] {
                        rc.teacher_unavailable += 1;
                        continue;
                    }
                    let dp = self.day_pos[slot];
                    let mut blocked = false;
                    if self.occ_class[les.class * stride + g] != -1 {
                        rc.class_busy += 1;
                        blocked = true;
                    }
                    if self.occ_teacher[les.teacher * stride + g] != -1 {
                        rc.teacher_busy += 1;
                        blocked = true;
                    }
                    if self.lesson_day[self.lesson_day_i(l, week, dp)] + 1 > les.max_per_day.max(1) {
                        rc.limits += 1;
                        blocked = true;
                    }
                    if self.best_room(l, week, slot).is_err() {
                        rc.room_busy += 1;
                        blocked = true;
                    }
                    let _ = blocked;
                }
            }
            let counts = [
                (rc.teacher_unavailable, "teacher unavailable at those times"),
                (rc.class_busy, "class already has a lesson in every otherwise-free slot"),
                (rc.teacher_busy, "teacher already teaching in every otherwise-free slot"),
                (rc.room_busy, "no suitable free room in otherwise-free slots"),
                (rc.limits, "daily/weekly workload limits reached"),
            ];
            let dominant = counts
                .iter()
                .max_by_key(|(c, _)| *c)
                .map(|(_, s)| s.to_string())
                .unwrap_or_else(|| "no feasible slot found".to_string());
            out.push(UnplacedInfo { lesson: l, count: n, reasons: rc, dominant });
        }
        out
    }
}

fn dedup_rows<const N: usize>(rows: [[Row; 2]; N]) -> Vec<Row> {
    let mut out: Vec<Row> = Vec::with_capacity(N * 2);
    for pair in rows {
        for r in pair {
            if !out.contains(&r) {
                out.push(r);
            }
        }
    }
    out
}

pub fn run(input: &Input) -> Output {
    let start = Instant::now();
    let mut grid = Grid::new(input);
    if grid.ns == 0 || grid.inp.slots.is_empty() {
        // Nothing can be placed.
        let mut unplaced = Vec::new();
        for l in 0..input.lessons.len() {
            grid.unplaced[l] = input.lessons[l].periods;
            unplaced.push(UnplacedInfo {
                lesson: l,
                count: input.lessons[l].periods,
                reasons: ReasonCounts::default(),
                dominant: "no schedulable time slots defined".to_string(),
            });
        }
        return Output {
            placed: vec![],
            unplaced,
            cost: 0.0,
            soft: SoftCosts::default(),
            iterations: 0,
            elapsed_ms: start.elapsed().as_millis() as u64,
        };
    }

    let mut rng = StdRng::seed_from_u64(input.seed.wrapping_add(0x9E3779B97F4A7C15));
    grid.greedy(&mut rng);

    let mut best = grid.snapshot();
    let mut iterations = 0u64;
    let total_iters = input.iterations;
    let limit = input.time_limit_ms.max(1);

    loop {
        if let Some(ti) = total_iters {
            if iterations >= ti {
                break;
            }
        }
        if iterations % 512 == 0 {
            if total_iters.is_none() && start.elapsed().as_millis() as u64 >= limit {
                break;
            }
        }
        let progress = match total_iters {
            Some(ti) => iterations as f64 / ti.max(1) as f64,
            None => (start.elapsed().as_millis() as f64 / limit as f64).clamp(0.0, 1.0),
        };
        let temp = T0 * (T_END / T0).powf(progress);
        if let Some(mv) = grid.pick_move(&mut rng) {
            grid.execute(mv, temp, &mut rng);
            if grid.cost + 1e-9 < best.cost {
                best = grid.snapshot();
            }
        }
        iterations += 1;
    }

    grid.apply_snapshot(&best);
    let soft = grid.soft_breakdown();
    let unplaced = grid.unplaced_info();
    Output {
        placed: grid.entries.clone(),
        unplaced,
        cost: best.cost,
        soft,
        iterations,
        elapsed_ms: start.elapsed().as_millis() as u64,
    }
}
