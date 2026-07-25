//! The world, and the systems that advance it.
//!
//! `World` owns every entity in arenas and hands out handles. Systems take `&mut World`
//! and resolve handles as they need them — nothing holds a reference to anything else,
//! which is what will let households, kinship, and food webs be cycles later on.
//!
//! Nothing here prints. Systems append to the chronicle; rendering is the frontend's
//! job. That separation is what makes a run comparable to another run, which is how the
//! reproducibility guarantee gets tested rather than merely asserted.

use person::{Action, Person, PersonId, Remark};
use planet::{DayPhase, Planet, PlanetId};
use sim_core::{Arena, Chronicle, Domain, Duration, Rng, Salience, Scheduler, Time, WorldSeed};

/// Something that happened, as the simulation records it — structured, not prose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Happening {
    WorldBegins { planet: PlanetId },
    PhaseBegins { planet: PlanetId, phase: DayPhase },
    PersonArrives { person: PersonId },
    PersonRemarks { person: PersonId, remark: Remark },
}

/// Work the scheduler has queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Task {
    /// A planet announcing itself, once, at the start of the world.
    PlanetAwakens(PlanetId),
    /// A planet crossing into the next quarter of its day.
    PlanetPhase(PlanetId),
    /// A person deciding what to do.
    PersonActs(PersonId),
}

pub struct World {
    pub seed: WorldSeed,
    pub planets: Arena<Planet>,
    pub people: Arena<Person>,
    pub chronicle: Chronicle<Happening>,
    scheduler: Scheduler<Task>,
    /// Counts entities ever created, so each gets its own RNG stream.
    next_stream: u64,
}

impl World {
    pub fn new(seed: WorldSeed) -> World {
        World {
            seed,
            planets: Arena::new(),
            people: Arena::new(),
            chronicle: Chronicle::new(),
            scheduler: Scheduler::new(),
            next_stream: 0,
        }
    }

    pub fn now(&self) -> Time {
        self.scheduler.now()
    }

    pub fn pending(&self) -> usize {
        self.scheduler.len()
    }

    /// A stream for one purpose. Each call gets a fresh entity index, so adding a draw
    /// in one place cannot shift what any other place draws.
    pub fn stream(&mut self, domain: Domain) -> Rng {
        let index = self.next_stream;
        self.next_stream += 1;
        self.seed.stream(domain, index, 0)
    }

    pub fn add_planet(&mut self, planet: Planet) -> PlanetId {
        self.planets.insert(planet)
    }

    pub fn add_person(&mut self, person: Person) -> PersonId {
        self.people.insert(person)
    }

    /// Populate a world with an Earth-like planet and some people on it.
    pub fn genesis(seed: WorldSeed, population: usize) -> World {
        let mut world = World::new(seed);
        let earth = world.add_planet(Planet::earth());
        for _ in 0..population {
            let mut rng = world.stream(Domain::Naming);
            let inhabitant = person::generate(&mut rng, earth);
            world.add_person(inhabitant);
        }
        world.start();
        world
    }

    /// Queue the opening events.
    ///
    /// People are queued before planets so that, at instants where both are due, the
    /// inhabitants act on the world as it stands and the planet then announces the
    /// change — the order the original loop produced. Ties break by insertion, so this
    /// one decision fixes the ordering for the whole run.
    pub fn start(&mut self) {
        let people: Vec<PersonId> = self.people.ids().collect();
        for id in people {
            self.scheduler
                .schedule_at(Time::ORIGIN, Task::PersonActs(id));
        }
        let planets: Vec<PlanetId> = self.planets.ids().collect();
        for id in planets {
            self.scheduler
                .schedule_at(Time::ORIGIN, Task::PlanetAwakens(id));
        }
    }

    /// Advance to `horizon`, running everything due at or before it.
    pub fn run_until(&mut self, horizon: Time) {
        while let Some((at, task)) = self.scheduler.next_event_until(horizon) {
            self.run_task(at, task);
        }
        // Land exactly on the horizon even if the last event fell short of it.
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

            Task::PersonActs(id) => {
                // A dead person, or one whose world is gone, simply stops being
                // scheduled — the handle failing to resolve is the mechanism.
                let Some(home) = self.people.get(id).map(|p| p.home) else {
                    return;
                };
                let Some(planet) = self.planets.get(home) else {
                    return;
                };
                let phase = planet.phase_at(at);
                let next = planet.next_phase_change(at);

                let action = self.people.get_mut(id).expect("resolved above").act(phase);
                match action {
                    Action::Introduce => self.chronicle.record(
                        at,
                        Salience::Pivotal,
                        Happening::PersonArrives { person: id },
                    ),
                    Action::Remark(remark) => self.chronicle.record(
                        at,
                        Salience::Routine,
                        Happening::PersonRemarks { person: id, remark },
                    ),
                    Action::Idle => {}
                }
                self.scheduler.schedule_at(next, Task::PersonActs(id));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn happenings(world: &World) -> Vec<Happening> {
        world.chronicle.iter().map(|r| r.kind).collect()
    }

    #[test]
    fn a_world_runs_the_original_sequence() {
        let mut world = World::genesis(WorldSeed::from_u128(1), 1);
        world.run_for(Duration::from_hours(30));

        let planet = world.planets.ids().next().unwrap();
        let person = world.people.ids().next().unwrap();
        use DayPhase::*;
        use Remark::*;

        // The transcript the pre-Phase-0 loop produced, now driven by derived time of
        // day rather than by two state machines nudging each other.
        assert_eq!(
            happenings(&world),
            vec![
                Happening::PersonArrives { person },
                Happening::WorldBegins { planet },
                Happening::PersonRemarks {
                    person,
                    remark: Bored
                },
                Happening::PhaseBegins {
                    planet,
                    phase: Morning
                },
                Happening::PersonRemarks {
                    person,
                    remark: Lunch
                },
                Happening::PhaseBegins {
                    planet,
                    phase: Afternoon
                },
                Happening::PersonRemarks {
                    person,
                    remark: Dinner
                },
                Happening::PhaseBegins {
                    planet,
                    phase: Evening
                },
                Happening::PersonRemarks {
                    person,
                    remark: GoodNight
                },
                Happening::PhaseBegins {
                    planet,
                    phase: Night
                },
                Happening::PersonRemarks {
                    person,
                    remark: Bored
                },
                Happening::PhaseBegins {
                    planet,
                    phase: Morning
                },
            ]
        );
    }

    #[test]
    fn the_same_seed_replays_exactly() {
        let seed = WorldSeed::from_u128(0xabc_def);
        let run = || {
            let mut w = World::genesis(seed, 8);
            w.run_for(Duration::from_days(30));
            (
                happenings(&w),
                w.people.iter().map(|(_, p)| p.clone()).collect::<Vec<_>>(),
            )
        };
        assert_eq!(run(), run(), "a world must be reproducible from its seed");
    }

    #[test]
    fn different_seeds_give_different_worlds() {
        let people_of = |seed| {
            let w = World::genesis(WorldSeed::from_u128(seed), 8);
            w.people
                .iter()
                .map(|(_, p)| p.name.clone())
                .collect::<Vec<_>>()
        };
        assert_ne!(people_of(1), people_of(2));
    }

    #[test]
    fn fresh_worlds_are_not_variations_on_one_theme() {
        // The user-facing promise: start a new world, get a different one.
        let names_of = |seed| {
            let w = World::genesis(seed, 6);
            w.people
                .iter()
                .map(|(_, p)| p.name.clone())
                .collect::<Vec<_>>()
        };
        let a = names_of(WorldSeed::from_entropy());
        let b = names_of(WorldSeed::from_entropy());
        assert_ne!(a, b);
    }

    #[test]
    fn every_person_is_reachable_by_handle() {
        let world = World::genesis(WorldSeed::from_u128(7), 25);
        assert_eq!(world.people.len(), 25);
        for id in world.people.ids() {
            let p = world.people.get(id).expect("handle must resolve");
            assert!(world.planets.contains(p.home), "home must resolve too");
        }
    }

    #[test]
    fn a_removed_person_stops_being_simulated() {
        let mut world = World::genesis(WorldSeed::from_u128(3), 2);
        world.run_for(Duration::from_hours(7));

        let victim = world.people.ids().next().unwrap();
        world.people.remove(victim);
        let before = world.chronicle.len();
        world.run_for(Duration::from_days(2));

        assert!(world.chronicle.len() > before, "the survivor keeps going");
        assert!(
            !world.chronicle.iter().skip(before).any(
                |r| matches!(r.kind, Happening::PersonRemarks { person, .. } if person == victim)
            ),
            "a dead person must not keep talking"
        );
    }

    #[test]
    fn time_advances_to_the_horizon_even_when_quiet() {
        let mut world = World::new(WorldSeed::from_u128(1)); // nothing scheduled
        world.run_for(Duration::from_myr(1));
        assert_eq!(world.now(), Time::ORIGIN + Duration::from_myr(1));
        assert!(world.chronicle.is_empty());
    }

    #[test]
    fn running_in_pieces_matches_running_straight_through() {
        let seed = WorldSeed::from_u128(555);

        let mut whole = World::genesis(seed, 3);
        whole.run_for(Duration::from_days(10));

        let mut pieces = World::genesis(seed, 3);
        for _ in 0..10 {
            pieces.run_for(Duration::from_days(1));
        }

        assert_eq!(happenings(&whole), happenings(&pieces));
        assert_eq!(whole.now(), pieces.now());
    }

    #[test]
    fn the_chronicle_zooms() {
        let mut world = World::genesis(WorldSeed::from_u128(9), 1);
        world.run_for(Duration::from_days(3));

        // Zoomed out, only the world's beginning survives the salience filter.
        let epochal: Vec<_> = world.chronicle.at_least(Salience::Epochal).collect();
        assert_eq!(epochal.len(), 1);
        assert!(matches!(epochal[0].kind, Happening::WorldBegins { .. }));

        // Zoomed in, the texture of the days is all there.
        assert!(world.chronicle.at_least(Salience::Routine).count() > 20);
    }

    #[test]
    fn a_crowd_scales_without_disturbing_the_clock() {
        let mut world = World::genesis(WorldSeed::from_u128(4242), 500);
        world.run_for(Duration::from_days(2));
        assert_eq!(world.people.len(), 500);
        // 500 people x (1 arrival + 8 remarks) + 1 world beginning + 8 phases.
        assert_eq!(world.chronicle.len(), 500 * 9 + 9);
    }
}
