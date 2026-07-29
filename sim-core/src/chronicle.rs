//! The world's event log.
//!
//! One log for everything. A continental collision, a mass extinction, a marriage, and
//! an argument at dinner are all records differing in salience and scale — a biography
//! is the log filtered by participant, a geological history is the same log filtered by
//! salience.
//!
//! Two things make that affordable rather than merely tidy.
//!
//! **An index.** A biography is the log filtered by participant, and filtering a log of
//! ten million records to find the two hundred about one person is not something to do by
//! scanning. Records are filed under whoever they are about as they arrive, so a life is
//! a lookup.
//!
//! **Forgetting.** A few hundred people living a few decades is tens of millions of
//! records, and deep time is worse without limit. So the log forgets, and it forgets the
//! way memory does: the small things first, and the older the sooner. What it will not do
//! is forget silently — every record dropped is counted, so the chronicle can say how many
//! ordinary days it no longer holds rather than implying there were none.


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

/// Whatever a record is about — a person, a place, a plate, a species.
///
/// A bare integer rather than a typed handle, because the chronicle sits below everything
/// that has handles and must not know what any of them are. Callers hand over
/// `id.to_bits()` and get the same number back.
pub type Subject = u64;

/// How many salience levels there are, for the forgetting tally.
const LEVELS: usize = 5;

/// Hashing for subject identifiers, which are dense integers rather than arbitrary keys.
///
/// The index was a `BTreeMap` and it is on the hottest path there is: every recorded event
/// files itself under everyone it concerns, twenty-six million times in a sixty-year world,
/// and each one was a logarithmic descent through pointer-chased nodes. A hash map is the
/// right structure — nothing here ever iterates the index in order, only looks up by key,
/// mutates every entry, or counts them.
///
/// The hasher is written out rather than taken from `std` because the default is
/// SipHash-1-3, which is built to resist adversaries choosing keys to collide. Nobody is
/// attacking a chronicle. These keys are arena indices, so one multiply and a shift spreads
/// them perfectly well, and it is deterministic across runs and machines — which `std`'s
/// randomly-seeded default is not, and this simulation's whole contract is that the same
/// seed gives the same world.
#[derive(Default, Clone, Copy)]
pub struct SubjectHasher(u64);

impl std::hash::Hasher for SubjectHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 = (self.0 ^ *byte as u64).wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn write_u64(&mut self, value: u64) {
        // Fibonacci hashing: multiply by 2^64 / φ and let the high bits fall where they may.
        self.0 = value.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        self.0 ^= self.0 >> 32;
    }
}

#[derive(Default, Clone, Copy)]
pub struct BySubject;

impl std::hash::BuildHasher for BySubject {
    type Hasher = SubjectHasher;
    fn build_hasher(&self) -> SubjectHasher {
        SubjectHasher(0)
    }
}

/// A log of records, indexed by who they are about, which forgets the small and old.
pub struct Chronicle<K> {
    records: Vec<Record<K>>,
    /// Which records each subject appears in. Indices into `records`, so compaction has
    /// to rebuild it — which is why compaction is a deliberate act and not a side effect.
    index: std::collections::HashMap<Subject, Vec<u32>, BySubject>,
    floor: Salience,
    /// How many records have been dropped, by how much they mattered.
    forgotten: [u64; LEVELS],
}

impl<K> Chronicle<K> {
    pub fn new() -> Self {
        Chronicle {
            records: Vec::new(),
            index: std::collections::HashMap::default(),
            floor: Salience::Routine,
            forgotten: [0; LEVELS],
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
        self.record_about(at, salience, kind, &[]);
    }

    /// Record something, filed under everyone it concerns.
    ///
    /// A record may name several subjects — a marriage is about two people, a war about
    /// two nations — and appears in each of their histories.
    pub fn record_about(&mut self, at: Time, salience: Salience, kind: K, about: &[Subject]) {
        if salience < self.floor {
            self.forgotten[salience as usize] += 1;
            return;
        }
        debug_assert!(
            self.records.last().is_none_or(|last| last.at <= at),
            "the chronicle must stay ordered in time"
        );
        let slot = self.records.len() as u32;
        self.records.push(Record { at, salience, kind });
        for subject in about {
            self.index.entry(*subject).or_default().push(slot);
        }
    }

    /// Everything on record about one subject, oldest first — a biography.
    pub fn about(&self, subject: Subject) -> impl Iterator<Item = &Record<K>> {
        self.index
            .get(&subject)
            .map(|slots| slots.as_slice())
            .unwrap_or(&[])
            .iter()
            .map(|slot| &self.records[*slot as usize])
    }

    /// How many records mention this subject.
    pub fn mentions(&self, subject: Subject) -> usize {
        self.index.get(&subject).map_or(0, |slots| slots.len())
    }

    pub fn subjects(&self) -> usize {
        self.index.len()
    }

    /// How many records have been dropped, at each level of salience.
    pub fn forgotten(&self) -> [u64; LEVELS] {
        self.forgotten
    }

    pub fn forgotten_total(&self) -> u64 {
        self.forgotten.iter().sum()
    }

    /// Forget enough of the small and old to come back under a budget.
    ///
    /// The rule is the one memory uses: the least important goes first, and among equally
    /// unimportant things the oldest goes first. Salience is walked upwards from the
    /// bottom, dropping everything at each level that falls in the oldest part of the log,
    /// until the log fits. Nothing above `keep_above` is ever dropped however old, because
    /// a mass extinction has to still be there in a billion years.
    ///
    /// Returns how many records were let go.
    pub fn compact(&mut self, budget: usize, keep_above: Salience) -> usize {
        if self.records.len() <= budget {
            return 0;
        }
        let mut doomed = vec![false; self.records.len()];
        let mut over = self.records.len() - budget;

        for level in 0..keep_above as usize {
            if over == 0 {
                break;
            }
            for (slot, record) in self.records.iter().enumerate() {
                if over == 0 {
                    break;
                }
                if record.salience as usize == level && !doomed[slot] {
                    doomed[slot] = true;
                    self.forgotten[level] += 1;
                    over -= 1;
                }
            }
        }

        let dropped = doomed.iter().filter(|d| **d).count();
        if dropped == 0 {
            return 0;
        }

        // Rebuild, and rebuild the index with it: every surviving record has moved.
        let mut moved_to = vec![u32::MAX; self.records.len()];
        let mut kept = Vec::with_capacity(self.records.len() - dropped);
        for (slot, record) in std::mem::take(&mut self.records).into_iter().enumerate() {
            if doomed[slot] {
                continue;
            }
            moved_to[slot] = kept.len() as u32;
            kept.push(record);
        }
        self.records = kept;

        for slots in self.index.values_mut() {
            slots.retain(|slot| moved_to[*slot as usize] != u32::MAX);
            for slot in slots.iter_mut() {
                *slot = moved_to[*slot as usize];
            }
        }
        self.index.retain(|_, slots| !slots.is_empty());
        dropped
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

    // ---- the index -----------------------------------------------------------

    #[test]
    fn a_biography_is_a_lookup_rather_than_a_search() {
        let mut c = Chronicle::new();
        for i in 0..1000u64 {
            c.record_about(
                Time::from_secs(i),
                Salience::Routine,
                Happening::Born(i as u32),
                &[i % 7],
            );
        }
        // Seven subjects, and each should own its own share.
        assert_eq!(c.subjects(), 7);
        assert_eq!(c.mentions(3), 143);
        let theirs: Vec<u32> = c
            .about(3)
            .map(|r| match r.kind {
                Happening::Born(n) => n,
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(theirs.len(), 143);
        assert!(theirs.iter().all(|n| *n % 7 == 3), "somebody else's life");
        // And in order, oldest first, which is what a life is.
        assert!(theirs.windows(2).all(|w| w[0] < w[1]));
    }

    #[test]
    fn one_record_can_be_about_several_people() {
        let mut c = Chronicle::new();
        c.record_about(
            Time::ORIGIN,
            Salience::Pivotal,
            Happening::Born(1),
            &[10, 20],
        );
        assert_eq!(c.mentions(10), 1);
        assert_eq!(c.mentions(20), 1);
        assert_eq!(c.len(), 1, "it is one event, not two");
    }

    #[test]
    fn a_stranger_has_no_history_rather_than_an_error() {
        let c: Chronicle<Happening> = Chronicle::new();
        assert_eq!(c.about(99).count(), 0);
        assert_eq!(c.mentions(99), 0);
    }

    // ---- forgetting -----------------------------------------------------------

    #[test]
    fn the_floor_counts_what_it_refuses() {
        // Refusing to store something is still a decision to forget it, and the count has
        // to say so — otherwise a run reports a tidy hundred events and implies that is
        // all that happened.
        let mut c: Chronicle<Happening> = Chronicle::new();
        c.set_floor(Salience::Pivotal);
        for i in 0..50u64 {
            c.record(Time::from_secs(i), Salience::Routine, Happening::Sunrise);
        }
        assert_eq!(c.len(), 0);
        assert_eq!(c.forgotten_total(), 50);
        assert_eq!(c.forgotten()[Salience::Routine as usize], 50);
    }

    #[test]
    fn compaction_drops_the_small_before_the_large() {
        let mut c = Chronicle::new();
        for i in 0..300u64 {
            let salience = match i % 3 {
                0 => Salience::Routine,
                1 => Salience::Notable,
                _ => Salience::Historic,
            };
            c.record_about(
                Time::from_secs(i),
                salience,
                Happening::Born(i as u32),
                &[i % 5],
            );
        }

        let dropped = c.compact(150, Salience::Pivotal);
        assert_eq!(dropped, 150);
        assert_eq!(c.len(), 150);
        // Every routine thing went, then half the notable ones, and nothing historic was
        // touched — which is the order it is supposed to work in.
        assert_eq!(
            c.iter().filter(|r| r.salience == Salience::Routine).count(),
            0
        );
        assert_eq!(
            c.iter().filter(|r| r.salience == Salience::Notable).count(),
            50
        );
        assert_eq!(c.at_least(Salience::Historic).count(), 100);
        assert_eq!(c.forgotten_total(), 150);
    }

    #[test]
    fn compaction_keeps_the_oldest_of_what_it_keeps_and_the_newest_of_what_it_drops() {
        // Among equally unimportant things the oldest goes first, which is the other half
        // of how memory works.
        let mut c = Chronicle::new();
        for i in 0..100u64 {
            c.record(
                Time::from_secs(i),
                Salience::Routine,
                Happening::Born(i as u32),
            );
        }
        c.compact(40, Salience::Pivotal);
        let left: Vec<u32> = c
            .iter()
            .map(|r| match r.kind {
                Happening::Born(n) => n,
                _ => unreachable!(),
            })
            .collect();
        assert_eq!(left.len(), 40);
        assert_eq!(left[0], 60, "it kept the wrong end");
        assert_eq!(left[39], 99);
    }

    #[test]
    fn compaction_never_touches_what_matters() {
        // A mass extinction has to still be there in a billion years, however tight the
        // budget gets.
        let mut c = Chronicle::new();
        for i in 0..100u64 {
            c.record(Time::from_secs(i), Salience::Epochal, Happening::Extinction);
        }
        assert_eq!(c.compact(10, Salience::Pivotal), 0);
        assert_eq!(c.len(), 100, "it forgot an epoch to meet a budget");
    }

    #[test]
    fn the_index_survives_compaction() {
        // The failure this guards against is silent and total: indices are positions in
        // the log, compaction moves every record, and an index left pointing at the old
        // positions returns somebody else's life.
        let mut c = Chronicle::new();
        for i in 0..200u64 {
            let salience = if i % 2 == 0 {
                Salience::Routine
            } else {
                Salience::Historic
            };
            c.record_about(
                Time::from_secs(i),
                salience,
                Happening::Born(i as u32),
                &[i % 4],
            );
        }
        c.compact(100, Salience::Pivotal);

        for subject in 0..4u64 {
            for record in c.about(subject) {
                let Happening::Born(n) = record.kind else {
                    unreachable!()
                };
                assert_eq!(
                    n as u64 % 4,
                    subject,
                    "after compaction, subject {subject}'s life contains event {n}"
                );
            }
        }
        // And nobody keeps a pointer to a record that is gone.
        assert_eq!(c.about(0).count(), c.mentions(0));
    }

    #[test]
    fn compacting_a_log_that_fits_does_nothing() {
        let mut c = sample();
        assert_eq!(c.compact(100, Salience::Pivotal), 0);
        assert_eq!(c.len(), 3);
    }
}
