//! A future-event queue.
//!
//! Nothing is polled. A sleeping person, a dormant seed bank, and a plate that will not
//! collide for eight million years all cost exactly nothing until they are due. Most of
//! the world, most of the time, is free — which is what makes the population ceiling a
//! question of *active* entities rather than existing ones.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::time::{Duration, Time};

struct Entry<E> {
    at: Time,
    // Insertion order, so that events due at the same instant fire in a defined
    // sequence. Without this the heap's internal ordering leaks into the simulation
    // and two runs of the same seed diverge.
    seq: u64,
    event: E,
}

impl<E> PartialEq for Entry<E> {
    fn eq(&self, other: &Self) -> bool {
        self.at == other.at && self.seq == other.seq
    }
}
impl<E> Eq for Entry<E> {}
impl<E> Ord for Entry<E> {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reversed: BinaryHeap is a max-heap and we want the earliest event.
        other
            .at
            .cmp(&self.at)
            .then_with(|| other.seq.cmp(&self.seq))
    }
}
impl<E> PartialOrd for Entry<E> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Holds pending events and the clock they advance.
pub struct Scheduler<E> {
    queue: BinaryHeap<Entry<E>>,
    now: Time,
    next_seq: u64,
}

impl<E> Scheduler<E> {
    pub fn new() -> Self {
        Scheduler {
            queue: BinaryHeap::new(),
            now: Time::ORIGIN,
            next_seq: 0,
        }
    }

    pub fn starting_at(now: Time) -> Self {
        Scheduler {
            queue: BinaryHeap::new(),
            now,
            next_seq: 0,
        }
    }

    pub fn now(&self) -> Time {
        self.now
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Queue an event. Events at the same instant fire in the order they were queued.
    ///
    /// Scheduling into the past clamps to now, so the event runs late rather than being
    /// dropped or bringing down a run that is already a million years in. Deliberately
    /// not an assertion: a stale deadline is a recoverable condition, and a system that
    /// cares can compare against [`Scheduler::now`] itself.
    pub fn schedule_at(&mut self, at: Time, event: E) {
        let at = at.max(self.now);
        self.queue.push(Entry {
            at,
            seq: self.next_seq,
            event,
        });
        self.next_seq += 1;
    }

    pub fn schedule_in(&mut self, delay: Duration, event: E) {
        self.schedule_at(self.now + delay, event);
    }

    /// When the next event is due, without consuming it.
    pub fn peek_time(&self) -> Option<Time> {
        self.queue.peek().map(|entry| entry.at)
    }

    /// Take the next event, advancing the clock to it.
    pub fn next_event(&mut self) -> Option<(Time, E)> {
        let entry = self.queue.pop()?;
        debug_assert!(entry.at >= self.now, "clock went backwards");
        self.now = entry.at;
        Some((entry.at, entry.event))
    }

    /// Take the next event only if it falls at or before `limit`, advancing the clock
    /// to it. Returns `None` — leaving the queue untouched — once the horizon is
    /// reached, so a run can stop at an exact time.
    pub fn next_event_until(&mut self, limit: Time) -> Option<(Time, E)> {
        if self.peek_time()? > limit {
            return None;
        }
        self.next_event()
    }

    /// Jump the clock forward with nothing to run. Used when promoting a region that
    /// has been dormant.
    pub fn advance_to(&mut self, when: Time) {
        debug_assert!(when >= self.now, "cannot rewind the clock");
        self.now = self.now.max(when);
    }

    pub fn clear(&mut self) {
        self.queue.clear();
    }
}

impl<E> Default for Scheduler<E> {
    fn default() -> Self {
        Scheduler::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_come_out_in_time_order() {
        let mut s = Scheduler::new();
        s.schedule_in(Duration::from_hours(3), "third");
        s.schedule_in(Duration::from_hours(1), "first");
        s.schedule_in(Duration::from_hours(2), "second");

        let order: Vec<_> = std::iter::from_fn(|| s.next_event())
            .map(|(_, e)| e)
            .collect();
        assert_eq!(order, ["first", "second", "third"]);
    }

    #[test]
    fn ties_break_by_insertion_order() {
        // The determinism guarantee rests on this: same seed, same order, always.
        let mut s = Scheduler::new();
        for i in 0..64 {
            s.schedule_at(Time::from_secs(500), i);
        }
        let order: Vec<_> = std::iter::from_fn(|| s.next_event())
            .map(|(_, e)| e)
            .collect();
        assert_eq!(order, (0..64).collect::<Vec<_>>());
    }

    #[test]
    fn the_clock_follows_the_events() {
        let mut s = Scheduler::new();
        assert_eq!(s.now(), Time::ORIGIN);
        s.schedule_in(Duration::from_days(2), ());
        s.next_event();
        assert_eq!(s.now(), Time::ORIGIN + Duration::from_days(2));
    }

    #[test]
    fn rescheduling_from_within_an_event_works() {
        // The usual pattern: an event queues its own next occurrence.
        let mut s = Scheduler::new();
        s.schedule_in(Duration::from_hours(6), 0u32);

        let mut fired = Vec::new();
        while let Some((at, n)) = s.next_event_until(Time::from_secs(86_400)) {
            fired.push((at.as_secs(), n));
            s.schedule_in(Duration::from_hours(6), n + 1);
        }

        assert_eq!(
            fired,
            vec![(21_600, 0), (43_200, 1), (64_800, 2), (86_400, 3)]
        );
        assert!(
            !s.is_empty(),
            "the event past the horizon should still be queued"
        );
    }

    #[test]
    fn the_horizon_is_inclusive_and_stops_cleanly() {
        let mut s = Scheduler::new();
        s.schedule_at(Time::from_secs(100), "at");
        s.schedule_at(Time::from_secs(101), "past");

        assert!(s.next_event_until(Time::from_secs(100)).is_some());
        assert!(s.next_event_until(Time::from_secs(100)).is_none());
        assert_eq!(s.len(), 1, "the later event must survive the refusal");
        assert_eq!(s.now(), Time::from_secs(100), "clock must not overshoot");
    }

    #[test]
    fn scheduling_into_the_past_runs_late_rather_than_vanishing() {
        let mut s = Scheduler::starting_at(Time::from_secs(1_000));
        s.schedule_at(Time::from_secs(10), "stale");
        let (at, event) = s.next_event().unwrap();
        assert_eq!(event, "stale");
        assert_eq!(at, Time::from_secs(1_000));
    }

    #[test]
    fn identical_programs_produce_identical_traces() {
        fn run() -> Vec<(u64, u32)> {
            let mut s = Scheduler::new();
            for i in 0..16 {
                s.schedule_at(Time::from_secs(u64::from(i % 4) * 60), i);
            }
            std::iter::from_fn(|| s.next_event())
                .map(|(at, e)| (at.as_secs(), e))
                .collect()
        }
        assert_eq!(run(), run());
    }
}
