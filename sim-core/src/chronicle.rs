//! The world's event log.
//!
//! One log for everything. A continental collision, a mass extinction, a marriage, and
//! an argument at dinner are all records differing in salience and scale — a biography
//! is the log filtered by participant, a geological history is the same log filtered by
//! salience.
//!
//! Phase 0 keeps this deliberately plain: append and scan. Per-participant indices and
//! the compaction that makes megayears affordable arrive with the chronicle phase
//! proper; the shape is fixed now so that systems can start emitting into it.

use crate::time::Time;

/// How much this event matters — the dial the observer turns when zooming out.
///
/// At a megayear view only `Epochal` survives; sitting with one person, everything does.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
#[repr(u8)]
pub enum Salience {
    /// The texture of a day. Forgotten almost immediately.
    Routine = 0,
    /// Worth remembering: a meeting, a quarrel, a good harvest.
    Notable = 1,
    /// Turns a life: a birth, a death, a migration.
    Pivotal = 2,
    /// Turns a region: a war, a famine, a city founded.
    Historic = 3,
    /// Turns a world: a mass extinction, an ice age, an ocean closing.
    Epochal = 4,
}

/// One thing that happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record<K> {
    pub at: Time,
    pub salience: Salience,
    pub kind: K,
}

/// An append-only log of records.
pub struct Chronicle<K> {
    records: Vec<Record<K>>,
    floor: Salience,
}

impl<K> Chronicle<K> {
    pub fn new() -> Self {
        Chronicle {
            records: Vec::new(),
            floor: Salience::Routine,
        }
    }

    /// Refuse to record anything below this level.
    ///
    /// The zoom control applied at *write* time rather than read time. Keeping every
    /// routine act of every person forever is precisely the cost compaction is meant to
    /// solve: a few hundred people living a few decades is tens of millions of records,
    /// unaffordable long before deep time. Until compaction exists, raising the floor is
    /// the honest way to run long — choosing not to know the small things, rather than
    /// pretending they were free.
    pub fn set_floor(&mut self, floor: Salience) {
        self.floor = floor;
    }

    pub fn floor(&self) -> Salience {
        self.floor
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn record(&mut self, at: Time, salience: Salience, kind: K) {
        if salience < self.floor {
            return;
        }
        debug_assert!(
            self.records.last().is_none_or(|last| last.at <= at),
            "the chronicle must stay ordered in time"
        );
        self.records.push(Record { at, salience, kind });
    }

    pub fn iter(&self) -> impl Iterator<Item = &Record<K>> {
        self.records.iter()
    }

    /// Everything at least this important — the zoom control.
    pub fn at_least(&self, salience: Salience) -> impl Iterator<Item = &Record<K>> {
        self.records.iter().filter(move |r| r.salience >= salience)
    }

    /// Everything within a span, inclusive of both ends.
    pub fn between(&self, from: Time, to: Time) -> impl Iterator<Item = &Record<K>> {
        self.records
            .iter()
            .filter(move |r| r.at >= from && r.at <= to)
    }

    /// Records matching a predicate — a biography, once the predicate is "involves
    /// this person".
    pub fn matching<'a, F>(&'a self, predicate: F) -> impl Iterator<Item = &'a Record<K>>
    where
        F: Fn(&K) -> bool + 'a,
    {
        self.records.iter().filter(move |r| predicate(&r.kind))
    }

    pub fn last(&self) -> Option<&Record<K>> {
        self.records.last()
    }
}

impl<K> Default for Chronicle<K> {
    fn default() -> Self {
        Chronicle::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::Duration;

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Happening {
        Sunrise,
        Born(u32),
        Extinction,
    }

    fn sample() -> Chronicle<Happening> {
        let mut c = Chronicle::new();
        c.record(Time::ORIGIN, Salience::Routine, Happening::Sunrise);
        c.record(Time::from_secs(500), Salience::Pivotal, Happening::Born(7));
        c.record(
            Time::ORIGIN + Duration::from_myr(1),
            Salience::Epochal,
            Happening::Extinction,
        );
        c
    }

    #[test]
    fn records_accumulate_in_order() {
        let c = sample();
        assert_eq!(c.len(), 3);
        assert_eq!(c.last().unwrap().kind, Happening::Extinction);
    }

    #[test]
    fn salience_filters_the_zoom_level() {
        let c = sample();
        let epochal: Vec<_> = c.at_least(Salience::Epochal).map(|r| &r.kind).collect();
        assert_eq!(epochal, vec![&Happening::Extinction]);

        // Zoomed in, everything is visible.
        assert_eq!(c.at_least(Salience::Routine).count(), 3);
    }

    #[test]
    fn spans_are_inclusive() {
        let c = sample();
        let early: Vec<_> = c
            .between(Time::ORIGIN, Time::from_secs(500))
            .map(|r| &r.kind)
            .collect();
        assert_eq!(early, vec![&Happening::Sunrise, &Happening::Born(7)]);
    }

    #[test]
    fn a_biography_is_a_filtered_log() {
        let c = sample();
        let about_seven: Vec<_> = c
            .matching(|k| matches!(k, Happening::Born(7)))
            .map(|r| &r.kind)
            .collect();
        assert_eq!(about_seven, vec![&Happening::Born(7)]);
    }

    #[test]
    fn the_floor_refuses_small_things() {
        let mut c: Chronicle<Happening> = Chronicle::new();
        c.set_floor(Salience::Pivotal);
        c.record(Time::ORIGIN, Salience::Routine, Happening::Sunrise);
        c.record(Time::from_secs(1), Salience::Pivotal, Happening::Born(1));
        c.record(Time::from_secs(2), Salience::Epochal, Happening::Extinction);

        assert_eq!(
            c.len(),
            2,
            "the routine event should never have been stored"
        );
        assert_eq!(c.floor(), Salience::Pivotal);
    }

    #[test]
    fn the_default_floor_keeps_everything() {
        let c = sample();
        assert_eq!(c.floor(), Salience::Routine);
        assert_eq!(c.len(), 3);
    }

    #[test]
    fn salience_is_ordered_from_routine_to_epochal() {
        assert!(Salience::Routine < Salience::Notable);
        assert!(Salience::Pivotal < Salience::Historic);
        assert!(Salience::Historic < Salience::Epochal);
    }
}
