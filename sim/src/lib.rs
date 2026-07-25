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

use genetics::{Architecture, FounderPool};
use person::{Cause, Deed, FERTILE_FROM, Person, PersonId, Situation};
use planet::{DayPhase, Planet, PlanetId};
use sim_core::{Arena, Chronicle, Domain, Duration, Rng, Salience, Scheduler, Time, WorldSeed};
use society::{Census, Place, PlaceId, Society};

/// Worlds start with a history behind them, so that a founding population can be
/// adults of varying ages rather than a single cohort of newborns.
///
/// A whole number of Earth days, so the clock starts at local midnight on a year
/// boundary rather than partway through a day the observer never saw.
pub const FOUNDING: Time = Time::from_secs(36_500 * 86_400);

/// How long a pregnancy runs.
pub const GESTATION: Duration = Duration::from_days(273);

/// Annual chance that a fertile, partnered woman in good health conceives.
///
/// Set deliberately above replacement rather than tuned to sit on it. With no feedback,
/// fertility near replacement is a knife edge: the population is a branching process, so
/// slightly below it dies out and slightly above it grows exponentially, with drift
/// deciding which. Measured here, 0.16 grows about fivefold per two centuries while 0.13
/// went extinct — there is no stable value in between to find.
///
/// Real populations are steady because fertility *responds* to conditions — density,
/// food, child mortality — and that negative feedback is what a constant cannot supply.
/// It arrives with resources and an economy. Until then, unchecked slow growth is the
/// honest failure mode; a knife-edge constant would only look stable until it wasn't.
const CONCEPTION_PER_YEAR: f32 = 0.16;

/// How much better a neighbourhood has to be before a household will move to it.
///
/// Without a threshold, households shuffle endlessly between places that differ in the
/// third decimal, and churn — which erodes community — becomes an artefact of the
/// sorting loop rather than a fact about the world.
const MOVE_THRESHOLD: f32 = 0.05;

/// What a spell of work adds to standing, where there is work worth having.
///
/// Derived rather than guessed. Gain saturates and decay is proportional, so standing
/// settles where `W·g·q = d·s` — with W the roughly four hundred spells of work in a
/// year, q the local opportunity and d the yearly decay. These values put a typical
/// worker in a middling neighbourhood near 0.5. Two earlier attempts were wrong in
/// opposite directions: one implied an equilibrium of 0.99 and turned every world into
/// an affluent enclave inside a decade, the other left no equilibrium at all.
const WORK_GAIN: f32 = 0.0017;

/// What a year takes back. Standing is a position needing upkeep, not a hoard.
const STANDING_DECAY: f32 = 0.15;

/// The share of their parents' standing a child starts from.
///
/// The most direct of the three routes by which advantage passes down — the other two
/// being the genes they inherit and the neighbourhood they grow up in.
///
/// Measured, it turns out to be the *weakest* of the three: moving it between 0.20 and
/// 0.55 shifts intergenerational elasticity only from 0.55 to 0.62. Advantage here
/// travels mostly through the neighbourhood a child is raised in, not through what is
/// handed to them at birth — which is worth knowing before reaching for this dial to
/// change how mobile a world is.
const INHERITED_STANDING: f32 = 0.35;

// ---- escape routes (§14.4) ---------------------------------------------------------
//
// Without these, a world is deterministic doom: advantage travels down through the
// neighbourhood a child is raised in, nothing travels the other way, and measured
// mobility never recovers. Three routes, chosen because none of them reward the people
// already ahead.
//
// These values sit on a frontier rather than at an optimum, and the trade is not a
// tuning artefact — it is arithmetic. An escape route works precisely by decoupling
// where someone ends up from where they began, so anything that lowers
// intergenerational elasticity also lowers the share of outcome that upbringing can
// explain, and raises the share left to chance. Measured across four settings:
//
//   routes off      elasticity 0.62   genes 0.39 / circumstance 0.39 / luck 0.46
//   these values    elasticity 0.55   genes 0.42 / circumstance 0.37 / luck 0.46
//   stronger        elasticity 0.40   genes 0.41 / circumstance 0.15 / luck 0.55
//   stronger still  elasticity 0.33   genes 0.39 / circumstance 0.07 / luck 0.59
//
// §15 asks for elasticity 0.20–0.50 *and* circumstance near 0.40 *and* luck near 0.30.
// This model cannot currently deliver all three at once. These values buy the closest
// thing to the design's central claim — that neither genes nor circumstance decides
// a life — and leave elasticity and luck each a little outside their bands, which the
// harness reports rather than hides.

/// Yearly chance of an unearned gain — a windfall, a good turn, being in the right place.
const WINDFALL_CHANCE: f64 = 0.015;
const WINDFALL: f32 = 0.12;

/// And of the reverse. Slightly likelier than the windfall, because ruin is.
const SETBACK_CHANCE: f64 = 0.020;
const SETBACK: f32 = 0.12;

/// Yearly chance, per unit of local bonding capital, that a young adult is taken up by
/// someone who can open doors.
///
/// Scaled by *bonding* capital rather than bridging, which is the whole point. Bridging
/// ties belong to the already-comfortable, so routing patronage through them would only
/// have widened the gap. Dense mutual-dependence community is what poor neighbourhoods
/// actually have, and turning it into a way out is what makes them produce escapees
/// rather than only outcomes.
const MENTOR_CHANCE: f64 = 0.055;
const PATRONAGE: f32 = 2.1;

/// Ages at which someone will still uproot themselves for work.
const RESTLESS_UNTIL: f64 = 32.0;

/// How much more readily somewhere takes in the young.
///
/// They are renting a room, not buying a house. Without this the spatial trap is
/// absolute: you cannot move to where the work is until you have the standing that
/// moving there would earn you.
const YOUNG_MOVER_SLACK: f32 = 0.30;

/// Something that happened, as the simulation records it — structured, not prose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Happening {
    WorldBegins {
        planet: PlanetId,
    },
    PhaseBegins {
        planet: PlanetId,
        phase: DayPhase,
    },
    PersonArrives {
        person: PersonId,
    },
    PersonDoes {
        person: PersonId,
        deed: Deed,
    },
    PersonDies {
        person: PersonId,
        cause: Cause,
    },
    PersonPairs {
        person: PersonId,
        with: PersonId,
    },
    PersonBorn {
        child: PersonId,
        mother: PersonId,
        father: PersonId,
    },
    PersonMoves {
        person: PersonId,
        to: PlaceId,
    },
    PersonMentored {
        person: PersonId,
    },
    PlaceChanges {
        place: PlaceId,
        into: society::Archetype,
    },
}

impl Happening {
    /// Who this concerns, if anyone. A biography is the log filtered by this.
    pub fn subject(&self) -> Option<PersonId> {
        match self {
            Happening::PersonArrives { person }
            | Happening::PersonDoes { person, .. }
            | Happening::PersonDies { person, .. }
            | Happening::PersonPairs { person, .. }
            | Happening::PersonMoves { person, .. }
            | Happening::PersonMentored { person } => Some(*person),
            Happening::PersonBorn { child, .. } => Some(*child),
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
    /// A pregnancy coming to term.
    Birth { mother: PersonId, father: PersonId },
    /// A yearly census: every place reads itself off its residents, and households
    /// consider moving.
    Reckoning,
}

pub struct World {
    pub seed: WorldSeed,
    pub planets: Arena<Planet>,
    pub people: Arena<Person>,
    pub society: Society,
    pub places: Arena<Place>,
    pub chronicle: Chronicle<Happening>,
    architecture: &'static Architecture,
    pool: FounderPool,
    scheduler: Scheduler<Task>,
    next_stream: u64,
    /// Mothers with a birth already queued. Ordered, so iteration cannot vary.
    expecting: std::collections::BTreeSet<PersonId>,
    /// What each place read as at the last reckoning, so a change of character is
    /// noticed rather than merely happening.
    was: std::collections::BTreeMap<PlaceId, society::Archetype>,
    /// Households that arrived somewhere since the last reckoning.
    arrivals: std::collections::BTreeMap<PlaceId, u32>,
    /// What was done where, since the last reckoning. Norms are read off this.
    deeds_done: std::collections::BTreeMap<PlaceId, [u32; Deed::COUNT]>,
}

impl World {
    pub fn new(seed: WorldSeed) -> World {
        World {
            seed,
            planets: Arena::new(),
            people: Arena::new(),
            society: Society::new(),
            places: Arena::new(),
            chronicle: Chronicle::new(),
            architecture: genetics::standard_architecture(),
            pool: FounderPool::uniform(),
            scheduler: Scheduler::starting_at(FOUNDING),
            next_stream: 0,
            expecting: std::collections::BTreeSet::new(),
            was: std::collections::BTreeMap::new(),
            arrivals: std::collections::BTreeMap::new(),
            deeds_done: std::collections::BTreeMap::new(),
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

    /// Populate a world with an Earth-like planet, some neighbourhoods, and people.
    ///
    /// The neighbourhoods start identical and unremarkable. Everything that
    /// distinguishes them afterwards — which becomes the enclave, which the slum — comes
    /// out of who ends up living in them, not out of anything written here.
    pub fn genesis(seed: WorldSeed, population: usize) -> World {
        let mut world = World::new(seed);
        let earth = world.add_planet(Planet::earth());
        world
            .scheduler
            .schedule_at(FOUNDING, Task::PlanetAwakens(earth));

        let quarters = [
            "Northside",
            "The Wharf",
            "Elmhurst",
            "Kingsfield",
            "Lowgate",
        ];
        let capacity = ((population / 3).max(4)) as u32;
        for name in quarters {
            world.places.insert(Place::new(name, capacity));
        }
        let place_ids: Vec<PlaceId> = world.places.ids().collect();

        for i in 0..population {
            let mut rng = world.stream(Domain::Genetics);
            // Founders span the adult range, so the population has a shape from the
            // start rather than being one cohort that ages and dies together.
            let age_years = rng.range_f64(18.0, 70.0);
            let born = FOUNDING - Duration::from_secs((age_years * 31_557_600.0) as u64);
            let standing = rng.unit_f32();

            let architecture = world.architecture;
            let pool = world.pool.clone();
            let mut inhabitant = person::found(architecture, &pool, &mut rng, earth, born, 0.0);
            // Their life before the world started was never simulated; do not bill
            // them for it, and treat their upbringing as already behind them.
            inhabitant.assume_settled(FOUNDING);
            inhabitant.set_standing(standing);
            inhabitant.mature();

            let id = world.add_person(inhabitant);
            let home = world.society.found_household(FOUNDING, 0.0);
            world.society.move_in(id, home);
            // Spread the founders around; sorting takes it from here.
            let quarter = place_ids[i % place_ids.len()];
            world.society.settle(home, quarter);
        }

        // The first reckoning is a year in, once there is a year to read.
        world
            .scheduler
            .schedule_at(FOUNDING + Duration::from_years(1), Task::Reckoning);
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
            Task::Birth { mother, father } => self.birth(at, mother, father),
            Task::Reckoning => self.reckoning(at),
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

        let where_they_live = self.society.place_of(id);
        let dependent = self
            .people
            .get(id)
            .is_some_and(|p| p.stage(at).is_dependent());

        // The four channels, filled from the neighbourhood they actually live in.
        let mut env = where_they_live
            .and_then(|p| self.places.get(p))
            .map(|place| place.env.surroundings(dependent))
            .unwrap_or_else(person::Surroundings::neutral);

        let (job_opportunity, schooling) = where_they_live
            .and_then(|p| self.places.get(p))
            .map(|place| (place.env.job_opportunity, place.env.education_access))
            .unwrap_or((0.5, 0.5));

        let mut rng = self.moment_stream(Domain::Behavior, id.to_bits(), at);
        let Some(subject) = self.people.get_mut(id) else {
            return;
        };
        if !subject.is_alive() {
            return;
        }

        // Their own unmet need adds to whatever the neighbourhood already imposes.
        env.stress = (env.stress + subject.needs().total_pressure()).clamp(0.0, 1.0);
        let situation = Situation { phase, env };

        let first = subject.first_sighting();
        let finished = subject.settle_intent_only(at);
        let outcome = subject.step(at, &situation, &mut rng);
        let death = subject.death();

        // Work pays where there is work worth having — the same channel that decided
        // whether it was on offer decides what it returns.
        if finished == Some(Deed::Work) {
            // How much a spell of work is worth depends on the person as well as the
            // place. Without that, everyone with the same neighbourhood converges on
            // the same standing and there is no spread for sorting to act on — the
            // whole world settles into one indistinguishable suburb.
            //
            // The two terms are the two inheritances: conscientiousness comes down
            // through the genome, schooling through the neighbourhood a child grew up
            // in. Advantage passes along both, and along the transfer at birth.
            let diligence = (0.6 + 0.5 * subject.personality.conscientiousness).clamp(0.2, 2.0);
            let taught = 0.5 + schooling;
            subject.earn(WORK_GAIN * job_opportunity * diligence * taught * subject.patronage());
        }

        if first {
            self.chronicle.record(
                at,
                Salience::Pivotal,
                Happening::PersonArrives { person: id },
            );
        }

        match outcome {
            Some(choice) => {
                if let Some(place) = where_they_live {
                    let counts = self.deeds_done.entry(place).or_insert([0; Deed::COUNT]);
                    counts[choice.deed as usize] += 1;
                }
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
            Some(cause) => {
                self.society.separate(id);
                self.society.move_out(id);
                self.chronicle.record(
                    at,
                    Salience::Pivotal,
                    Happening::PersonDies { person: id, cause },
                );
            }
            None => {
                if let Some(person) = self.people.get_mut(id)
                    && !person.stage(at).is_dependent()
                {
                    // Adults only. A child cannot work — the option is gated off — so
                    // decaying their standing too meant every generation arrived at
                    // adulthood with nothing, and the whole society ratcheted to zero
                    // within a few generations. What a child has is their family's
                    // backing, and that does not erode because they are young.
                    person.slip(STANDING_DECAY);
                }
                self.roll_fortune(at, id);
                self.seek_patron(at, id);
                self.seek_partner(at, id);
                self.try_conceive(at, id);
                self.scheduler
                    .schedule_at(at + Duration::from_years(1), Task::PersonAges(id));
            }
        }
    }

    /// Plain luck: the windfalls and ruins that no one earns.
    ///
    /// Uncorrelated with everything, which is the point — it is the only share of an
    /// outcome that cannot be inherited, and a world without it is a morality tale.
    fn roll_fortune(&mut self, at: Time, id: PersonId) {
        let mut rng = self.moment_stream(Domain::Chance, id.to_bits(), at);
        let Some(person) = self.people.get_mut(id) else {
            return;
        };
        if person.stage(at).is_dependent() {
            return;
        }
        if rng.chance(WINDFALL_CHANCE) {
            person.earn(WINDFALL);
        }
        if rng.chance(SETBACK_CHANCE) {
            person.slip(SETBACK);
        }
    }

    /// A young adult may be taken up by someone who can open doors.
    fn seek_patron(&mut self, at: Time, id: PersonId) {
        let Some(person) = self.people.get(id) else {
            return;
        };
        let age = person.age(at).years();
        if person.is_mentored() || !(FERTILE_FROM..RESTLESS_UNTIL).contains(&age) {
            return;
        }
        let bonding = self
            .society
            .place_of(id)
            .and_then(|p| self.places.get(p))
            .map(|place| place.env.bonding_capital)
            .unwrap_or(0.0);

        let mut rng = self.moment_stream(Domain::Chance, id.to_bits() ^ 0x_1e17, at);
        if !rng.chance(MENTOR_CHANCE * f64::from(bonding)) {
            return;
        }
        if self
            .people
            .get_mut(id)
            .is_some_and(|p| p.take_patron(PATRONAGE))
        {
            self.chronicle.record(
                at,
                Salience::Pivotal,
                Happening::PersonMentored { person: id },
            );
        }
    }

    /// Look for someone to pair off with.
    ///
    /// Choice is assortative: among a handful of candidates, the most compatible wins.
    /// That is worth the extra work rather than pairing at random, because partners who
    /// resemble each other produce a wider spread of children than random pairing does,
    /// and the spread of a population is much of what the genetics is for.
    fn seek_partner(&mut self, at: Time, id: PersonId) {
        let Some(seeker) = self.people.get(id) else {
            return;
        };
        if self.society.is_partnered(id) || !seeker.is_marriageable(at) {
            return;
        }
        let (sex, age) = (seeker.sex, seeker.age(at).years());

        let eligible: Vec<PersonId> = self
            .people
            .iter()
            .filter(|(other_id, other)| {
                *other_id != id
                    && other.sex != sex
                    && other.is_marriageable(at)
                    && !self.society.is_partnered(*other_id)
                    && (other.age(at).years() - age).abs() <= 15.0
                    && !self.society.is_close_kin(id, *other_id)
            })
            .map(|(other_id, _)| other_id)
            .collect();

        if eligible.is_empty() {
            return;
        }

        // Consider a few, not everyone: nobody surveys the whole world before choosing.
        let mut rng = self.moment_stream(Domain::Behavior, id.to_bits() ^ 0x9a1d, at);
        let mut shortlist = eligible;
        rng.shuffle(&mut shortlist);
        shortlist.truncate(8);

        let Some(&chosen) = shortlist.iter().max_by(|a, b| {
            let score = |c: &PersonId| {
                self.people
                    .get(*c)
                    .map(|other| seeker.compatibility(other))
                    .unwrap_or(0.0)
            };
            score(a).total_cmp(&score(b))
        }) else {
            return;
        };

        self.society.pair(id, chosen);

        // A new household, with its own character, for the two of them and any
        // children they raise.
        let inherited_place = self
            .society
            .place_of(id)
            .or_else(|| self.society.place_of(chosen));
        let home = self.society.found_household(at, 0.0);
        self.society.move_in(id, home);
        self.society.move_in(chosen, home);
        if let Some(place) = inherited_place {
            self.society.settle(home, place);
        }
        self.society.dissolve_empty();

        self.chronicle.record(
            at,
            Salience::Pivotal,
            Happening::PersonPairs {
                person: id,
                with: chosen,
            },
        );
    }

    /// Roll for a pregnancy, and queue the birth if one takes.
    fn try_conceive(&mut self, at: Time, id: PersonId) {
        if self.expecting.contains(&id) {
            return;
        }
        let Some(mother) = self.people.get(id) else {
            return;
        };
        if !mother.is_fertile(at) {
            return;
        }
        let Some(father_id) = self.society.partner_of(id) else {
            return;
        };
        let Some(father) = self.people.get(father_id) else {
            return;
        };
        if !father.is_alive() {
            return;
        }

        let mut rng = self.moment_stream(Domain::Demography, id.to_bits() ^ 0xbabe, at);
        if !rng.chance(f64::from(CONCEPTION_PER_YEAR) * f64::from(mother.health().vitality)) {
            return;
        }

        self.expecting.insert(id);
        self.scheduler.schedule_at(
            at + GESTATION,
            Task::Birth {
                mother: id,
                father: father_id,
            },
        );
    }

    /// The yearly loop that closes people and places together.
    ///
    /// Three steps in order, and the order matters: every place first reads itself off
    /// the people currently in it, then children absorb the place they are growing up
    /// in, then households reconsider where they live. Sorting last means a household
    /// moves on this year's reading, not on a stale one.
    fn reckoning(&mut self, at: Time) {
        self.take_census(at);
        self.absorb_upbringings(at);
        self.sort_households(at);
        self.scheduler
            .schedule_at(at + Duration::from_years(1), Task::Reckoning);
    }

    fn take_census(&mut self, at: Time) {
        let place_ids: Vec<PlaceId> = self.places.ids().collect();
        for id in place_ids {
            let mut census = Census::default();

            for (home, household) in self.society.households_in(id) {
                census.households += 1;
                if household.founded.since(at).as_years() < 1.0
                    && household.founded >= at - Duration::from_years(1)
                {
                    census.arrivals += 1;
                }
                let _ = home;
                for member in &household.members {
                    if let Some(person) = self.people.get(*member)
                        && person.is_alive()
                        && !person.stage(at).is_dependent()
                    {
                        census.adults += 1;
                        census.mean_standing += person.standing();
                    }
                }
            }
            census.arrivals += self.arrivals.get(&id).copied().unwrap_or(0);
            if census.adults > 0 {
                census.mean_standing /= census.adults as f32;
            }

            // Norms are literally what people did here this year.
            if let Some(counts) = self.deeds_done.get(&id) {
                census.deeds = *counts;
            }

            if let Some(place) = self.places.get_mut(id) {
                let before = place.archetype();
                place.observe(&census);
                let after = place.archetype();
                if self.was.insert(id, after) == Some(before) && before != after {
                    self.chronicle.record(
                        at,
                        Salience::Historic,
                        Happening::PlaceChanges {
                            place: id,
                            into: after,
                        },
                    );
                }
            }
        }
        self.arrivals.clear();
        self.deeds_done.clear();
    }

    /// Children take on the character of wherever they are living.
    fn absorb_upbringings(&mut self, at: Time) {
        let ids: Vec<PersonId> = self.people.ids().collect();
        for id in ids {
            let quality = self
                .society
                .place_of(id)
                .and_then(|p| self.places.get(p))
                .map(|p| p.env.upbringing())
                .unwrap_or(0.0);

            let opportunity = self
                .society
                .place_of(id)
                .and_then(|p| self.places.get(p))
                .map(|p| p.env.job_opportunity)
                .unwrap_or(0.0);

            let Some(person) = self.people.get_mut(id) else {
                continue;
            };
            if !person.is_alive() {
                continue;
            }
            let age = person.age(at).years();

            if !person.stage(at).is_dependent() {
                person.work_amid(opportunity, 1.0);
            }
            if person.has_matured() {
                continue;
            }
            person.absorb(quality, age, 1.0);
            if age >= 20.0 {
                // The window closes: what was absorbed becomes who they are.
                person.mature();
            }
        }
    }

    /// Households consider moving somewhere that suits them better.
    ///
    /// This is what produces sorting, and sorting is what makes neighbourhoods diverge
    /// rather than all drifting to the same middling average.
    fn sort_households(&mut self, at: Time) {
        let homes: Vec<society::HouseholdId> =
            self.society.households().map(|(id, _)| id).collect();

        for home in homes {
            let Some(household) = self.society.household(home) else {
                continue;
            };
            if household.members.is_empty() {
                continue;
            }
            let members = household.members.clone();
            let current = household.place;

            let standing = {
                let (sum, count) = members
                    .iter()
                    .filter_map(|m| self.people.get(*m))
                    .filter(|p| p.is_alive() && !p.stage(at).is_dependent())
                    .fold((0.0, 0), |(s, c), p| (s + p.standing(), c + 1));
                if count == 0 { 0.0 } else { sum / count as f32 }
            };

            // The young will uproot for work, and are taken in more readily — they are
            // renting a room, not buying a house. Everyone else is choosing a place to
            // live, and is ranked out of the good ones by what they have.
            let restless = members
                .iter()
                .filter_map(|m| self.people.get(*m))
                .filter(|p| p.is_alive() && !p.stage(at).is_dependent())
                .all(|p| p.age(at).years() < RESTLESS_UNTIL);

            let best = self
                .places
                .iter()
                .filter(|(id, place)| {
                    let occupancy =
                        self.society.households_in(*id).count() as f32 / place.capacity as f32;
                    let means = if restless {
                        standing + YOUNG_MOVER_SLACK
                    } else {
                        standing
                    };
                    place.admits(means, occupancy)
                })
                .max_by(|(_, a), (_, b)| {
                    let worth = |e: &society::EnvironmentVector| {
                        if restless {
                            e.job_opportunity
                        } else {
                            e.quality()
                        }
                    };
                    worth(&a.env).total_cmp(&worth(&b.env))
                })
                .map(|(id, _)| id);

            let Some(best) = best else { continue };
            if current == Some(best) {
                continue;
            }

            // Moving costs something, so only a real improvement is worth it —
            // otherwise households churn between near-identical places forever.
            let gain = self
                .places
                .get(best)
                .map(|p| p.env.quality())
                .unwrap_or(0.0)
                - current
                    .and_then(|c| self.places.get(c))
                    .map(|p| p.env.quality())
                    .unwrap_or(0.0);
            if current.is_some() && gain < MOVE_THRESHOLD {
                continue;
            }

            self.society.settle(home, best);
            *self.arrivals.entry(best).or_insert(0) += 1;
            for member in members {
                if self.people.get(member).is_some_and(|p| p.is_alive()) {
                    // Pivotal, not routine. §14 makes the neighbourhood a child grows
                    // up in the largest single influence on how they turn out, so
                    // changing it is one of the more consequential things that can
                    // happen to a family.
                    self.chronicle.record(
                        at,
                        Salience::Pivotal,
                        Happening::PersonMoves {
                            person: member,
                            to: best,
                        },
                    );
                }
            }
        }
    }

    fn birth(&mut self, at: Time, mother_id: PersonId, father_id: PersonId) {
        self.expecting.remove(&mother_id);

        // Either parent may have died while the pregnancy ran.
        let Some(mother) = self.people.get(mother_id) else {
            return;
        };
        let Some(father) = self.people.get(father_id) else {
            return;
        };
        if !mother.is_alive() {
            return;
        }

        let home = self.society.home_of(mother_id);
        let upbringing = self
            .society
            .place_of(mother_id)
            .and_then(|p| self.places.get(p))
            .map(|place| place.env.upbringing())
            .unwrap_or(0.0);
        let inherited = INHERITED_STANDING
            * (mother.standing()
                + self
                    .people
                    .get(father_id)
                    .map(|f| f.standing())
                    .unwrap_or(0.0))
            / 2.0;
        let mut rng = self.moment_stream(Domain::Genetics, mother_id.to_bits() ^ 0xb0_11, at);

        let child = person::born_to(
            self.architecture,
            (mother_id, mother),
            (father_id, father),
            &mut rng,
            at,
            upbringing,
        );

        let mut child = child;
        child.set_standing(inherited);
        let child_id = self.add_person(child);
        self.society.record_birth(child_id, mother_id, father_id);
        if let Some(home) = home {
            self.society.move_in(child_id, home);
        }

        self.chronicle.record(
            at,
            Salience::Pivotal,
            Happening::PersonBorn {
                child: child_id,
                mother: mother_id,
                father: father_id,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use life::{LifeStage, Need};

    /// One world, lived for three generations, shared by every test needing lineages.
    ///
    /// Built once: the population grows as it runs, so paying for it per test is what
    /// made the suite unbearable.
    ///
    /// Serves both the family tests and the neighbourhood ones: seventy years is long
    /// enough for three generations *and* for identical quarters to pull apart, and one
    /// world is half the cost of two.
    ///
    /// Sixty founders, not thirty. Below about fifty the world reliably dwindles, and
    /// that is the simulation being right rather than wrong: the pairing market thins,
    /// and after a few generations close-kin exclusion rules out most of the remaining
    /// candidates, so a small isolated population struggles to reproduce itself. Real
    /// enough to keep — but it makes a small fixture a study of near-extinction rather
    /// than of families.
    fn lineages() -> &'static World {
        static WORLD: std::sync::LazyLock<World> = std::sync::LazyLock::new(|| {
            let mut world = World::genesis(WorldSeed::from_u128(0x11), 60);
            world.record_only(Salience::Pivotal);
            world.run_for(Duration::from_years(70));
            world
        });
        &WORLD
    }

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
    fn a_population_sustains_itself() {
        // Phase 2's headline: births now offset deaths, where Phase 1 could only decline.
        let world = lineages();
        let born = world
            .chronicle
            .iter()
            .filter(|r| matches!(r.kind, Happening::PersonBorn { .. }))
            .count();

        assert!(born > 20, "only {born} births in sixty-five years");
        assert!(
            world.people.len() > 60,
            "no one new ever existed: {}",
            world.people.len()
        );
        // Not strict growth: the founding cohort was eighteen to seventy at the start,
        // so most of it dies inside the first fifty years while its children are still
        // reaching childbearing age. The population dips through that transient before
        // it climbs. Holding most of its size across it is the real property.
        assert!(
            world.living() >= 40,
            "population collapsed to {} from 60",
            world.living()
        );
    }

    #[test]
    fn families_reach_a_third_generation() {
        let world = lineages();
        let grandparents = world
            .people
            .ids()
            .filter(|id| {
                world
                    .society
                    .children_of(*id)
                    .iter()
                    .any(|child| !world.society.children_of(*child).is_empty())
            })
            .count();
        assert!(grandparents > 0, "no lineage reached a third generation");

        let deep = world
            .people
            .ids()
            .filter(|id| world.society.ancestors_of(*id, 3).len() >= 4)
            .count();
        assert!(deep > 0, "nobody has four known grandparents");
    }

    #[test]
    fn nobody_pairs_with_close_kin() {
        let world = lineages();
        let mut pairings = 0;
        for record in world.chronicle.iter() {
            if let Happening::PersonPairs { person, with } = record.kind {
                pairings += 1;
                assert!(
                    !world.society.is_close_kin(person, with),
                    "{person:?} paired with close kin {with:?}"
                );
            }
        }
        assert!(pairings > 3, "only {pairings} pairings to check");
    }

    #[test]
    fn children_resemble_their_parents() {
        // Whether inheritance survives being wired through households and birth — the
        // coefficient itself is measured in the genetics crate, over thousands of
        // samples. A handful of families is far too few for a correlation, so this
        // compares distances instead: a child should sit closer to its own parents than
        // to a stranger, which is a much lower-variance thing to ask.
        let world = lineages();
        let architecture = genetics::standard_architecture();
        let value = |p: &Person| architecture.genetic_value(&p.genome, genetics::Trait::Openness);

        let others: Vec<f32> = world.people.iter().map(|(_, p)| value(p)).collect();
        let mut to_parents = Vec::new();
        let mut to_strangers = Vec::new();

        for (child_id, child) in world.people.iter() {
            let Some((mother, father)) = world.society.parents_of(child_id) else {
                continue;
            };
            let (Some(m), Some(f)) = (world.people.get(mother), world.people.get(father)) else {
                continue;
            };
            let midparent = (value(m) + value(f)) / 2.0;
            to_parents.push((value(child) - midparent).abs());

            // Everyone else, averaged — no single stranger, so no lucky draw.
            let mean_other = others.iter().sum::<f32>() / others.len() as f32;
            to_strangers.push((value(child) - mean_other).abs());
        }

        assert!(
            to_parents.len() > 8,
            "only {} families to measure",
            to_parents.len()
        );
        let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
        assert!(
            mean(&to_parents) < mean(&to_strangers),
            "children should sit closer to their parents ({:.3}) than to the population ({:.3})",
            mean(&to_parents),
            mean(&to_strangers)
        );
    }

    #[test]
    fn siblings_share_an_upbringing_without_it_being_identical() {
        // In Phase 2 full siblings had exactly the same shared term, because it was
        // fixed at birth from the household. It is now accumulated across a childhood,
        // so siblings born a decade apart experience the same neighbourhood in
        // different states — and one may be carried elsewhere partway through. Close,
        // not equal, is the right claim, and it is the developmental window working.
        let world = lineages();
        let mut siblings = Vec::new();
        let mut unrelated = Vec::new();

        let matured: Vec<(PersonId, f32)> = world
            .people
            .iter()
            .filter(|(_, p)| p.has_matured())
            .map(|(id, p)| (id, p.absorbed_upbringing()))
            .collect();

        for (id, mine) in &matured {
            let Some(parents) = world.society.parents_of(*id) else {
                continue;
            };
            for sibling in world.society.siblings_of(*id) {
                if world.society.parents_of(sibling) != Some(parents) {
                    continue;
                }
                if let Some((_, theirs)) = matured.iter().find(|(o, _)| *o == sibling) {
                    siblings.push((mine - theirs).abs());
                }
            }
            for (other, theirs) in &matured {
                if *other != *id && world.society.parents_of(*other) != Some(parents) {
                    unrelated.push((mine - theirs).abs());
                }
            }
        }

        assert!(siblings.len() > 2, "only {} sibling pairs", siblings.len());
        let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
        assert!(
            mean(&siblings) < mean(&unrelated),
            "siblings should be raised more alike ({:.3}) than strangers ({:.3})",
            mean(&siblings),
            mean(&unrelated)
        );
    }

    #[test]
    fn a_child_is_born_into_its_mothers_household() {
        let world = lineages();
        let mut checked = 0;
        for record in world.chronicle.iter() {
            let Happening::PersonBorn { child, mother, .. } = record.kind else {
                continue;
            };
            // Only recent births, and only children who have not yet left. The dead
            // move out, anyone who pairs off founds a household of their own, and a
            // widowed mother who pairs again moves to a new one — all correct, and all
            // reasons a present-day comparison says nothing about a birth long ago.
            if record.at < world.now() - Duration::from_years(6) {
                continue;
            }
            let Some(hers) = world.society.home_of(mother) else {
                continue;
            };
            let Some(offspring) = world.people.get(child) else {
                continue;
            };
            if !offspring.is_alive() || world.society.is_partnered(child) {
                continue;
            }
            assert_eq!(
                world.society.home_of(child),
                Some(hers),
                "a child should live with its mother"
            );
            checked += 1;
        }
        assert!(checked > 2, "only {checked} living mothers to check");
    }

    #[test]
    fn the_dead_leave_their_partner_and_their_house() {
        let world = lineages();
        let mut dead = 0;
        for (id, person) in world.people.iter() {
            if person.is_alive() {
                continue;
            }
            dead += 1;
            assert!(
                world.society.partner_of(id).is_none(),
                "{} is dead and still partnered",
                person.name
            );
            assert!(
                world.society.home_of(id).is_none(),
                "{} is dead and still housed",
                person.name
            );
        }
        assert!(dead > 5, "only {dead} deaths to check");
    }

    #[test]
    fn children_cannot_take_work() {
        // Channel one doing structural work: the option is absent, not unattractive.
        // Needs routine recording, so it runs its own small, short world.
        let mut world = World::genesis(WorldSeed::from_u128(0x55), 8);
        world.run_for(Duration::from_years(12));

        let now = world.now();
        let mut dependants = 0;
        for (id, person) in world.people.iter() {
            if !person.is_alive() || !person.stage(now).is_dependent() {
                continue;
            }
            dependants += 1;
            let worked = world.chronicle.iter().any(|r| {
                matches!(r.kind, Happening::PersonDoes { person: p, deed } if p == id && deed == Deed::Work)
            });
            assert!(!worked, "{} worked while still a child", person.name);
        }
        assert!(dependants > 0, "no children were born to check");
    }

    #[test]
    fn neighbourhoods_diverge_from_identical_beginnings() {
        // Every quarter starts unremarkable and identical. Nothing here writes "slum"
        // or "enclave" anywhere: the spread is what sorting and accumulation produce.
        let fresh = World::genesis(WorldSeed::from_u128(0x11), 4);
        let at_founding: Vec<f32> = fresh.places.iter().map(|(_, p)| p.env.affluence).collect();
        assert!(
            at_founding.windows(2).all(|w| w[0] == w[1]),
            "they should begin the same"
        );

        let world = lineages();
        let affluence: Vec<f32> = world.places.iter().map(|(_, p)| p.env.affluence).collect();
        let lowest = affluence.iter().cloned().fold(f32::MAX, f32::min);
        let highest = affluence.iter().cloned().fold(f32::MIN, f32::max);
        assert!(
            highest - lowest > 0.2,
            "neighbourhoods should pull apart: {affluence:?}"
        );

        let kinds: std::collections::HashSet<society::Archetype> =
            world.places.iter().map(|(_, p)| p.archetype()).collect();
        assert!(
            kinds.len() > 1,
            "they should not all read the same: {kinds:?}"
        );
    }

    #[test]
    fn where_a_child_grows_up_shapes_them() {
        // The developmental window, end to end. Same machinery, two upbringings.
        let world = lineages();
        let raised: Vec<(f32, f32)> = world
            .people
            .iter()
            .filter(|(_, p)| p.has_matured() && p.parents.is_some())
            .map(|(_, p)| (p.absorbed_upbringing(), p.origins.conscientiousness.shared))
            .collect();

        assert!(raised.len() > 5, "only {} raised here", raised.len());
        assert!(
            raised.iter().any(|(a, _)| *a != raised[0].0),
            "children should not all have absorbed the same place"
        );
        // The shared term is that absorption, not a birth-time snapshot.
        for (absorbed, shared) in &raised {
            assert!(
                (shared - absorbed * (0.20f32).sqrt()).abs() < 1e-4,
                "shared term should be the absorbed upbringing: {shared} vs {absorbed}"
            );
        }
    }

    #[test]
    fn a_hard_neighbourhood_suppresses_work() {
        // Channels one to three, together: the same person works less where there is
        // less to be had, and the gap is structural rather than a matter of character.
        let world = lineages();
        let poorest = world
            .places
            .iter()
            .min_by(|(_, a), (_, b)| a.env.affluence.total_cmp(&b.env.affluence))
            .map(|(_, p)| p.env.clone())
            .unwrap();
        let richest = world
            .places
            .iter()
            .max_by(|(_, a), (_, b)| a.env.affluence.total_cmp(&b.env.affluence))
            .map(|(_, p)| p.env.clone())
            .unwrap();

        let hard = poorest.surroundings(false);
        let easy = richest.surroundings(false);
        assert!(hard.availability[Deed::Work as usize] < easy.availability[Deed::Work as usize]);
        assert!(hard.payoff[Deed::Work as usize] < easy.payoff[Deed::Work as usize]);
        assert!(hard.discount_rate() > easy.discount_rate());
    }

    #[test]
    fn standing_settles_instead_of_saturating_or_collapsing() {
        // Two failure modes this went through on the way here: an equilibrium of 0.99
        // that made every quarter an enclave, and no equilibrium at all, which slid
        // every world to destitution within two generations.
        let world = lineages();
        let standings: Vec<f32> = world
            .people
            .iter()
            .filter(|(_, p)| p.is_alive() && !p.stage(world.now()).is_dependent())
            .map(|(_, p)| p.standing())
            .collect();
        assert!(standings.len() > 20, "not enough adults to judge");

        let mean = standings.iter().sum::<f32>() / standings.len() as f32;
        assert!(
            (0.10..0.85).contains(&mean),
            "mean standing {mean:.3} is a collapse or a saturation"
        );

        let spread = standings.iter().cloned().fold(f32::MIN, f32::max)
            - standings.iter().cloned().fold(f32::MAX, f32::min);
        assert!(
            spread > 0.2,
            "everyone ended up the same: spread {spread:.3}"
        );
    }

    #[test]
    fn some_people_are_taken_up_by_someone() {
        let world = lineages();
        let mentored = world
            .chronicle
            .iter()
            .filter(|r| matches!(r.kind, Happening::PersonMentored { .. }))
            .count();
        assert!(mentored > 0, "nobody ever found a patron");

        // And it is a way out, not a reward for already being ahead: everyone who found
        // one keeps it for life, and it multiplies what their work returns.
        let lucky = world.people.iter().filter(|(_, p)| p.is_mentored()).count();
        assert!(lucky > 0);
        assert!(
            lucky < world.people.len() / 2,
            "patronage should be uncommon, not the norm"
        );
    }

    #[test]
    fn patronage_favours_tight_communities_over_comfortable_ones() {
        // The inversion that makes the mechanism worth having. Bridging ties belong to
        // people who are already comfortable; routing a way out through them would only
        // widen the gap. Bonding capital is what a poor neighbourhood actually has.
        let world = lineages();
        let mut with_patron = Vec::new();
        let mut without = Vec::new();

        for (id, person) in world.people.iter() {
            if !person.has_matured() {
                continue;
            }
            let Some(bonding) = world
                .society
                .place_of(id)
                .and_then(|p| world.places.get(p))
                .map(|p| p.env.bonding_capital)
            else {
                continue;
            };
            if person.is_mentored() {
                with_patron.push(bonding);
            } else {
                without.push(bonding);
            }
        }

        if with_patron.len() >= 3 && !without.is_empty() {
            let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
            assert!(
                mean(&with_patron) > mean(&without) - 0.15,
                "patrons should not be concentrated in the comfortable places: {:.2} vs {:.2}",
                mean(&with_patron),
                mean(&without)
            );
        }
    }

    #[test]
    fn the_young_will_move_for_work() {
        // The spatial trap without this is absolute: you cannot move to where the work
        // is until you have the standing that moving there would have earned you.
        let world = lineages();
        let movers = world
            .chronicle
            .iter()
            .filter(|r| matches!(r.kind, Happening::PersonMoves { .. }))
            .count();
        assert!(movers > 5, "only {movers} moves in seventy years");
    }

    #[test]
    fn fortune_is_uncorrelated_with_deserving() {
        // Luck has to reach people regardless of who they are, or it is not luck.
        let world = lineages();
        let now = world.now();
        let adults: Vec<&Person> = world
            .people
            .iter()
            .map(|(_, p)| p)
            .filter(|p| p.has_matured() && !p.stage(now).is_dependent())
            .collect();
        assert!(adults.len() > 20);

        // Standing should not have collapsed onto one value, which is what would happen
        // if the shocks were the only thing moving it.
        let peaks: Vec<f32> = adults.iter().map(|p| p.peak_standing()).collect();
        let spread = peaks.iter().cloned().fold(f32::MIN, f32::max)
            - peaks.iter().cloned().fold(f32::MAX, f32::min);
        assert!(
            spread > 0.2,
            "outcomes collapsed together: spread {spread:.2}"
        );
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
