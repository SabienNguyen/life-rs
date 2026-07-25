//! The world, and the systems that advance it.
//!
//! `World` owns every entity in arenas and hands out handles. Systems take `&mut World`
//! and resolve handles as they need them — nothing holds a reference to anything else,
//! which is what will let households, kinship, and food webs be cycles later on.
//!
//! Nothing here prints. Systems append to the chronicle; rendering is the frontend's
//! job. That separation is what makes a run comparable to another run, which is how the
//! reproducibility guarantee gets tested rather than merely asserted.
//!
//! Two rhythms run side by side, which is the scale ladder in miniature: people act at
//! the pace of what they are doing (minutes to hours), and age at the pace of a year.
//! Neither polls.

use person::{Cause, Deed, Person, PersonId, Situation};
use planet::{DayPhase, Planet, PlanetId};
use sim_core::{Arena, Chronicle, Domain, Duration, Rng, Salience, Scheduler, Time, WorldSeed};

/// Worlds start with a history behind them, so that a founding population can be
/// adults of varying ages rather than a single cohort of newborns.
///
/// A whole number of Earth days, so the clock starts at local midnight on a year
/// boundary rather than partway through a day the observer never saw.
pub const FOUNDING: Time = Time::from_secs(36_500 * 86_400);

/// Something that happened, as the simulation records it — structured, not prose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Happening {
    WorldBegins { planet: PlanetId },
    PhaseBegins { planet: PlanetId, phase: DayPhase },
    PersonArrives { person: PersonId },
    PersonDoes { person: PersonId, deed: Deed },
    PersonDies { person: PersonId, cause: Cause },
}

impl Happening {
    /// Who this concerns, if anyone. A biography is the log filtered by this.
    pub fn subject(&self) -> Option<PersonId> {
        match self {
            Happening::PersonArrives { person }
            | Happening::PersonDoes { person, .. }
            | Happening::PersonDies { person, .. } => Some(*person),
            _ => None,
        }
    }
}

/// Work the scheduler has queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Task {
    /// A planet announcing itself, once, at the start of the world.
    PlanetAwakens(PlanetId),
    /// A planet crossing into the next quarter of its day.
    PlanetPhase(PlanetId),
    /// A person finishing what they were doing and choosing the next thing.
    PersonActs(PersonId),
    /// A person getting a year older, and rolling against mortality.
    PersonAges(PersonId),
}

pub struct World {
    pub seed: WorldSeed,
    pub planets: Arena<Planet>,
    pub people: Arena<Person>,
    pub chronicle: Chronicle<Happening>,
    scheduler: Scheduler<Task>,
    next_stream: u64,
}

impl World {
    pub fn new(seed: WorldSeed) -> World {
        World {
            seed,
            planets: Arena::new(),
            people: Arena::new(),
            chronicle: Chronicle::new(),
            scheduler: Scheduler::starting_at(FOUNDING),
            next_stream: 0,
        }
    }

    pub fn now(&self) -> Time {
        self.scheduler.now()
    }

    /// Stop recording anything below this level. See `Chronicle::set_floor` — running
    /// for decades with every routine act retained is not affordable until compaction
    /// exists, so a long run has to say what it does not care about.
    pub fn record_only(&mut self, floor: Salience) {
        self.chronicle.set_floor(floor);
    }

    pub fn pending(&self) -> usize {
        self.scheduler.len()
    }

    /// How many people are still alive. The dead stay in the arena so their lives can
    /// still be read — death removes someone from the schedule, not from history.
    pub fn living(&self) -> usize {
        self.people.iter().filter(|(_, p)| p.is_alive()).count()
    }

    /// A stream for one purpose. Each call takes a fresh index, so adding a draw in one
    /// place cannot shift what any other place draws.
    pub fn stream(&mut self, domain: Domain) -> Rng {
        let index = self.next_stream;
        self.next_stream += 1;
        self.seed.stream(domain, index, 0)
    }

    /// A stream tied to one entity at one moment — reproducible without being shared,
    /// and independent of how the run happens to be chunked.
    fn moment_stream(&self, domain: Domain, entity: u64, at: Time) -> Rng {
        self.seed.stream(domain, entity, at.as_secs())
    }

    pub fn add_planet(&mut self, planet: Planet) -> PlanetId {
        self.planets.insert(planet)
    }

    pub fn add_person(&mut self, person: Person) -> PersonId {
        let id = self.people.insert(person);
        self.enlist(id);
        id
    }

    /// Put a person on the schedule: acting now, and aging on their own anniversary.
    fn enlist(&mut self, id: PersonId) {
        let now = self.scheduler.now();
        self.scheduler.schedule_at(now, Task::PersonActs(id));

        // Stagger anniversaries across the year so a large population does not roll
        // mortality all on one day.
        let offset = Duration::from_secs(
            (id.to_bits().wrapping_mul(2_654_435_761)) % sim_core::time::SECONDS_PER_YEAR,
        );
        self.scheduler
            .schedule_at(now + offset, Task::PersonAges(id));
    }

    /// Populate a world with an Earth-like planet and a founding population.
    pub fn genesis(seed: WorldSeed, population: usize) -> World {
        let mut world = World::new(seed);
        let earth = world.add_planet(Planet::earth());
        world
            .scheduler
            .schedule_at(FOUNDING, Task::PlanetAwakens(earth));

        for _ in 0..population {
            let mut rng = world.stream(Domain::Naming);
            // Founders span the adult range, so the population has a shape from the
            // start rather than being one cohort that all ages and dies together.
            let age_years = rng.range_f64(18.0, 70.0);
            let born = FOUNDING - Duration::from_secs((age_years * 31_557_600.0) as u64);
            let mut inhabitant = person::generate(&mut rng, earth, born);
            // Their life before the world started was never simulated; do not bill
            // them for it.
            inhabitant.assume_settled(FOUNDING);
            world.add_person(inhabitant);
        }
        world
    }

    /// Advance to `horizon`, running everything due at or before it.
    pub fn run_until(&mut self, horizon: Time) {
        while let Some((at, task)) = self.scheduler.next_event_until(horizon) {
            self.run_task(at, task);
        }
        self.scheduler.advance_to(horizon);
    }

    pub fn run_for(&mut self, span: Duration) {
        self.run_until(self.now() + span);
    }

    fn run_task(&mut self, at: Time, task: Task) {
        match task {
            Task::PlanetAwakens(id) => {
                let Some(planet) = self.planets.get(id) else {
                    return;
                };
                let next = planet.next_phase_change(at);
                self.chronicle
                    .record(at, Salience::Epochal, Happening::WorldBegins { planet: id });
                self.scheduler.schedule_at(next, Task::PlanetPhase(id));
            }

            Task::PlanetPhase(id) => {
                let Some(planet) = self.planets.get(id) else {
                    return;
                };
                let phase = planet.phase_at(at);
                let next = planet.next_phase_change(at);
                self.chronicle.record(
                    at,
                    Salience::Routine,
                    Happening::PhaseBegins { planet: id, phase },
                );
                self.scheduler.schedule_at(next, Task::PlanetPhase(id));
            }

            Task::PersonActs(id) => self.person_acts(at, id),
            Task::PersonAges(id) => self.person_ages(at, id),
        }
    }

    fn person_acts(&mut self, at: Time, id: PersonId) {
        // A person whose handle no longer resolves, or whose world is gone, simply
        // stops being scheduled. The lookup failing is the mechanism.
        let Some(home) = self.people.get(id).map(|p| p.home) else {
            return;
        };
        let Some(planet) = self.planets.get(home) else {
            return;
        };
        let phase = planet.phase_at(at);

        let mut rng = self.moment_stream(Domain::Behavior, id.to_bits(), at);
        let Some(subject) = self.people.get_mut(id) else {
            return;
        };
        if !subject.is_alive() {
            return;
        }

        // Stress is the person's own unmet need for now. Phase 3 adds the
        // neighbourhood's contribution; the channel is already load-bearing, because it
        // is what shortens the time horizon and suppresses work under deprivation.
        let mut situation = Situation::plain(phase);
        situation.env.stress = subject.needs().total_pressure();

        let first = subject.first_sighting();
        let outcome = subject.step(at, &situation, &mut rng);
        let death = subject.death();

        if first {
            self.chronicle.record(
                at,
                Salience::Pivotal,
                Happening::PersonArrives { person: id },
            );
        }

        match outcome {
            Some(choice) => {
                self.chronicle.record(
                    at,
                    Salience::Routine,
                    Happening::PersonDoes {
                        person: id,
                        deed: choice.deed,
                    },
                );
                self.scheduler
                    .schedule_at(at + choice.deed.duration(), Task::PersonActs(id));
            }
            None => {
                // Died catching up — record it and stop scheduling them.
                if let Some((_, cause)) = death {
                    self.chronicle.record(
                        at,
                        Salience::Pivotal,
                        Happening::PersonDies { person: id, cause },
                    );
                }
            }
        }
    }

    fn person_ages(&mut self, at: Time, id: PersonId) {
        let mut rng = self.moment_stream(Domain::Demography, id.to_bits(), at);
        let Some(subject) = self.people.get_mut(id) else {
            return;
        };
        if !subject.is_alive() {
            return;
        }

        subject.catch_up(at);
        let cause = subject
            .survive(at, Duration::from_years(1), &mut rng)
            .or_else(|| {
                subject
                    .death()
                    .map(|(_, c)| c)
                    .filter(|_| !subject.is_alive())
            });

        match cause {
            Some(cause) => self.chronicle.record(
                at,
                Salience::Pivotal,
                Happening::PersonDies { person: id, cause },
            ),
            None => self
                .scheduler
                .schedule_at(at + Duration::from_years(1), Task::PersonAges(id)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use life::{LifeStage, Need};

    fn happenings(world: &World) -> Vec<Happening> {
        world.chronicle.iter().map(|r| r.kind).collect()
    }

    fn deeds_of(world: &World, id: PersonId) -> Vec<Deed> {
        world
            .chronicle
            .iter()
            .filter_map(|r| match r.kind {
                Happening::PersonDoes { person, deed } if person == id => Some(deed),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_world_starts_and_keeps_time() {
        let mut world = World::genesis(WorldSeed::from_u128(1), 1);
        world.run_for(Duration::from_hours(30));

        let planet = world.planets.ids().next().unwrap();
        use DayPhase::*;

        let phases: Vec<DayPhase> = world
            .chronicle
            .iter()
            .filter_map(|r| match r.kind {
                Happening::PhaseBegins { phase, .. } => Some(phase),
                _ => None,
            })
            .collect();
        assert_eq!(phases, vec![Morning, Afternoon, Evening, Night, Morning]);

        assert!(matches!(
            happenings(&world)[0],
            Happening::WorldBegins { planet: p } if p == planet
        ));
    }

    #[test]
    fn a_person_arrives_once_and_then_gets_on_with_it() {
        let mut world = World::genesis(WorldSeed::from_u128(2), 1);
        world.run_for(Duration::from_days(2));
        let id = world.people.ids().next().unwrap();

        let arrivals = world
            .chronicle
            .iter()
            .filter(|r| matches!(r.kind, Happening::PersonArrives { .. }))
            .count();
        assert_eq!(arrivals, 1);
        assert!(deeds_of(&world, id).len() > 5, "should have done things");
    }

    #[test]
    fn people_look_after_themselves() {
        // The behaviour loop, end to end: a month of unattended life.
        let mut world = World::genesis(WorldSeed::from_u128(3), 40);
        world.run_for(Duration::from_days(30));

        let starved = world
            .people
            .iter()
            .filter(|(_, p)| p.death().map(|(_, c)| c) == Some(Cause::Deprivation))
            .count();
        assert_eq!(
            starved, 0,
            "nobody should starve with food freely available"
        );

        for (_, p) in world.people.iter().filter(|(_, p)| p.is_alive()) {
            assert!(
                p.needs().vital_pressure() < 0.6,
                "{} is in trouble: {:?}",
                p.name,
                p.needs()
            );
        }
    }

    #[test]
    fn everyone_sleeps_and_eats_and_drinks() {
        let mut world = World::genesis(WorldSeed::from_u128(4), 10);
        world.run_for(Duration::from_days(5));

        for id in world.people.ids() {
            let deeds: std::collections::HashSet<Deed> = deeds_of(&world, id).into_iter().collect();
            for essential in [Deed::Sleep, Deed::Eat, Deed::Drink] {
                assert!(
                    deeds.contains(&essential),
                    "{:?} never got round to {essential:?}",
                    world.people.get(id).map(|p| &p.name)
                );
            }
        }
    }

    #[test]
    fn a_founding_population_has_a_shape() {
        let world = World::genesis(WorldSeed::from_u128(5), 300);
        let ages: Vec<f64> = world
            .people
            .iter()
            .map(|(_, p)| p.age(FOUNDING).years())
            .collect();

        let youngest = ages.iter().cloned().fold(f64::MAX, f64::min);
        let oldest = ages.iter().cloned().fold(f64::MIN, f64::max);
        assert!(youngest >= 18.0 && oldest <= 70.0, "{youngest}..{oldest}");
        assert!(
            oldest - youngest > 30.0,
            "founders should not be one cohort"
        );

        // And they are adults, not a nursery.
        assert!(
            world
                .people
                .iter()
                .all(|(_, p)| !p.stage(FOUNDING).is_dependent())
        );
    }

    #[test]
    fn people_age_and_eventually_die() {
        // Populations in the decade-scale tests are deliberately small. Everyone acts
        // every few hours whether or not it is recorded, so cost scales with
        // people x years; not simulating the unobserved is what level-of-detail buys,
        // and it does not exist yet.
        let mut world = World::genesis(WorldSeed::from_u128(6), 15);
        world.record_only(Salience::Pivotal);
        world.run_for(Duration::from_years(40));

        let deaths = world.people.iter().filter(|(_, p)| !p.is_alive()).count();
        assert!(
            deaths > 3,
            "40 years should thin a founding population: {deaths}"
        );
        assert!(deaths < 15, "but not extinguish it outright");

        // Old age should be the usual reason, not starvation.
        let by_cause = |want: Cause| {
            world
                .people
                .iter()
                .filter(|(_, p)| p.death().map(|(_, c)| c) == Some(want))
                .count()
        };
        assert!(by_cause(Cause::OldAge) > by_cause(Cause::Deprivation));
    }

    #[test]
    fn the_dead_stop_acting_but_stay_readable() {
        // Routine recording is on here, because the property under test is precisely
        // that no routine act appears after a death. Small population, because that
        // makes every deed of every person for fifty years affordable to check.
        let mut world = World::genesis(WorldSeed::from_u128(7), 6);
        world.run_for(Duration::from_years(50));

        let dead: Vec<PersonId> = world
            .people
            .iter()
            .filter(|(_, p)| !p.is_alive())
            .map(|(id, _)| id)
            .collect();
        assert!(!dead.is_empty(), "fifty years should claim someone");

        for id in dead {
            let person = world.people.get(id).expect("the dead stay readable");
            let (died_at, _) = person.death().unwrap();
            assert!(!person.name.is_empty(), "still has a name to show");

            let acted_after = world.chronicle.iter().any(|r| {
                r.at > died_at
                    && matches!(r.kind, Happening::PersonDoes { person, .. } if person == id)
            });
            assert!(!acted_after, "{} kept going after dying", person.name);
        }
    }

    #[test]
    fn a_death_is_recorded_exactly_once() {
        let mut world = World::genesis(WorldSeed::from_u128(8), 12);
        world.record_only(Salience::Pivotal);
        world.run_for(Duration::from_years(40));

        let mut seen = std::collections::HashMap::new();
        for record in world.chronicle.iter() {
            if let Happening::PersonDies { person, .. } = record.kind {
                *seen.entry(person).or_insert(0) += 1;
            }
        }
        assert!(!seen.is_empty(), "somebody should have died in 40 years");
        for (person, count) in seen {
            assert_eq!(count, 1, "{person:?} died {count} times");
        }
    }

    #[test]
    fn behaviour_answers_to_circumstance() {
        // Same world, same person, different need: the choice must follow.
        let mut world = World::genesis(WorldSeed::from_u128(9), 1);
        let id = world.people.ids().next().unwrap();
        world
            .people
            .get_mut(id)
            .unwrap()
            .set_need(Need::Thirst, 0.99);
        world.run_for(Duration::from_minutes(10));

        assert!(
            deeds_of(&world, id).contains(&Deed::Drink),
            "a parched person should drink: {:?}",
            deeds_of(&world, id)
        );
    }

    #[test]
    fn the_same_seed_replays_exactly() {
        let seed = WorldSeed::from_u128(0xabc_def);
        let run = || {
            let mut w = World::genesis(seed, 12);
            w.run_for(Duration::from_days(40));
            (
                happenings(&w),
                w.people.iter().map(|(_, p)| p.clone()).collect::<Vec<_>>(),
            )
        };
        assert_eq!(run(), run(), "a world must be reproducible from its seed");
    }

    #[test]
    fn different_seeds_give_different_worlds() {
        let lives = |seed| {
            let mut w = World::genesis(WorldSeed::from_u128(seed), 10);
            w.run_for(Duration::from_days(10));
            happenings(&w)
        };
        assert_ne!(lives(1), lives(2));
    }

    #[test]
    fn fresh_worlds_are_not_variations_on_one_theme() {
        let names_of = |seed| {
            let w = World::genesis(seed, 6);
            w.people
                .iter()
                .map(|(_, p)| p.name.clone())
                .collect::<Vec<_>>()
        };
        assert_ne!(
            names_of(WorldSeed::from_entropy()),
            names_of(WorldSeed::from_entropy())
        );
    }

    #[test]
    fn running_in_pieces_matches_running_straight_through() {
        let seed = WorldSeed::from_u128(555);

        let mut whole = World::genesis(seed, 5);
        whole.run_for(Duration::from_days(20));

        let mut pieces = World::genesis(seed, 5);
        for _ in 0..20 {
            pieces.run_for(Duration::from_days(1));
        }

        assert_eq!(happenings(&whole), happenings(&pieces));
        assert_eq!(whole.now(), pieces.now());
        assert_eq!(whole.living(), pieces.living());
    }

    #[test]
    fn the_chronicle_zooms() {
        let mut world = World::genesis(WorldSeed::from_u128(10), 1);
        world.run_for(Duration::from_days(3));

        let epochal: Vec<_> = world.chronicle.at_least(Salience::Epochal).collect();
        assert_eq!(epochal.len(), 1);
        assert!(matches!(epochal[0].kind, Happening::WorldBegins { .. }));
        assert!(world.chronicle.at_least(Salience::Routine).count() > 20);
    }

    #[test]
    fn a_biography_is_the_log_filtered_by_subject() {
        let mut world = World::genesis(WorldSeed::from_u128(11), 5);
        world.run_for(Duration::from_days(3));
        let id = world.people.ids().next().unwrap();

        let mine = world
            .chronicle
            .iter()
            .filter(|r| r.kind.subject() == Some(id))
            .count();
        let all = world
            .chronicle
            .iter()
            .filter(|r| r.kind.subject().is_some())
            .count();
        assert!(mine > 0 && mine < all, "{mine} of {all}");
    }

    #[test]
    fn time_advances_to_the_horizon_even_when_quiet() {
        let mut world = World::new(WorldSeed::from_u128(1));
        world.run_for(Duration::from_myr(1));
        assert_eq!(world.now(), FOUNDING + Duration::from_myr(1));
        assert!(world.chronicle.is_empty());
    }

    #[test]
    fn elders_exist_after_enough_time() {
        let mut world = World::genesis(WorldSeed::from_u128(12), 20);
        world.record_only(Salience::Pivotal);
        world.run_for(Duration::from_years(10));
        let now = world.now();
        assert!(
            world
                .people
                .iter()
                .any(|(_, p)| p.is_alive() && p.stage(now) == LifeStage::Elder),
            "people should have aged into their sixties"
        );
    }
}
