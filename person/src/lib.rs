//! People.
//!
//! A person is a handle-addressable bundle of who they are (fixed at birth), how they
//! are doing (needs, health), and what they are currently up to (intent). Deciding is
//! delegated to [`deeds`]; being alive is delegated to [`life`].
//!
//! Nothing here is updated on a tick. Needs and health are brought forward from the last
//! time anyone looked, which is what lets a large population sit dormant for free.

pub mod deeds;
pub mod psyche;

use faker_rand::en_us::names::FullName;
use life::{Age, Health, LifeStage, Mortality, Need, Needs};
use planet::PlanetId;
use sim_core::{Duration, Id, Rng, Time};
use std::fmt;

pub use deeds::{Choice, Deed, Mind, Situation, Surroundings};
pub use psyche::{Outlook, Personality, Values};

pub type PersonId = Id<Person>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Height {
    Short,
    Average,
    Tall,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ethnicity {
    Hispanic,
    African,
    Asian,
    White,
    PacificIslander,
    Indigenous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HairColor {
    Black,
    White,
    Brown,
    Blonde,
    Silver,
    Red,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    Underweight,
    Normal,
    Overweight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Country {
    Usa,
    Gbr,
    Deu,
    Can,
    Fra,
    Chn,
    Jpn,
    Vnm,
}

impl Country {
    pub const ALL: [Country; 8] = [
        Country::Can,
        Country::Chn,
        Country::Deu,
        Country::Fra,
        Country::Gbr,
        Country::Jpn,
        Country::Usa,
        Country::Vnm,
    ];
}

impl fmt::Display for Country {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Country::Can => "Canada",
            Country::Chn => "China",
            Country::Deu => "Germany",
            Country::Fra => "France",
            Country::Gbr => "United Kingdoms",
            Country::Jpn => "Japan",
            Country::Usa => "United States",
            Country::Vnm => "Vietnam",
        };
        f.write_str(name)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalAttrs {
    pub weight: Weight,
    pub height: Height,
}

impl PhysicalAttrs {
    pub fn new(weight: Weight, height: Height) -> PhysicalAttrs {
        PhysicalAttrs { weight, height }
    }
}

/// Something being done, and when it will be finished.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Intent {
    pub deed: Deed,
    pub started: Time,
    pub until: Time,
}

impl Intent {
    pub fn remaining(&self, now: Time) -> Duration {
        self.until.since(now)
    }
}

/// Why someone died. Derived from their state at the end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    Deprivation,
    Illness,
    OldAge,
    Misadventure,
}

impl Cause {
    pub const fn label(self) -> &'static str {
        match self {
            Cause::Deprivation => "deprivation",
            Cause::Illness => "illness",
            Cause::OldAge => "old age",
            Cause::Misadventure => "misadventure",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Person {
    pub name: String,
    pub country: Country,
    pub physical: PhysicalAttrs,
    pub personality: Personality,
    pub values: Values,
    /// Which world they live on — a handle, so no lifetime ties a person to a planet.
    pub home: PlanetId,
    pub born: Time,

    needs: Needs,
    health: Health,
    intent: Option<Intent>,
    died: Option<(Time, Cause)>,
    /// When needs and health were last brought forward.
    updated: Time,
    met: bool,
}

impl Person {
    pub fn new(
        name: impl Into<String>,
        country: Country,
        physical: PhysicalAttrs,
        personality: Personality,
        values: Values,
        home: PlanetId,
        born: Time,
    ) -> Person {
        Person {
            name: name.into(),
            country,
            physical,
            personality,
            values,
            home,
            born,
            needs: Needs::rested(),
            health: Health::hale(),
            intent: None,
            died: None,
            updated: born,
            met: false,
        }
    }

    pub fn age(&self, now: Time) -> Age {
        Age::from_birth(self.born, self.effective_now(now))
    }

    pub fn stage(&self, now: Time) -> LifeStage {
        self.age(now).stage()
    }

    pub fn needs(&self) -> &Needs {
        &self.needs
    }

    pub fn health(&self) -> Health {
        self.health
    }

    pub fn intent(&self) -> Option<Intent> {
        self.intent
    }

    pub fn is_alive(&self) -> bool {
        self.died.is_none()
    }

    pub fn death(&self) -> Option<(Time, Cause)> {
        self.died
    }

    /// True the first time anyone observes this person, false afterwards. Drives the
    /// one-off introduction.
    pub fn first_sighting(&mut self) -> bool {
        let first = !self.met;
        self.met = true;
        first
    }

    /// A dead person's clock stops, so their age and needs stay as they were.
    fn effective_now(&self, now: Time) -> Time {
        match self.died {
            Some((when, _)) => when.min(now),
            None => now,
        }
    }

    /// Treat this person as up to date and in ordinary condition as of `now`.
    ///
    /// A founding population has decades of life behind it that were never simulated.
    /// Without this, catching them up would charge them for every hour of that at once
    /// and kill the whole world on its first morning. Summarising an unsimulated past
    /// as "they are fine" is the crudest possible version of the backfill the design
    /// calls for — and the same seam a real reconstruction will slot into.
    pub fn assume_settled(&mut self, now: Time) {
        self.updated = now;
        self.needs = Needs::rested();
        self.health = Health::hale();
    }

    /// Bring needs and health forward to `now`. Idempotent, and cheap to call.
    pub fn catch_up(&mut self, now: Time) {
        if !self.is_alive() || now <= self.updated {
            return;
        }
        let elapsed = now.since(self.updated);
        let scale = self.stage(now).metabolic_scale();
        self.needs.accrue(elapsed, scale);
        self.health.respond_to(self.needs.vital_pressure(), elapsed);
        self.updated = now;

        if self.health.is_dead() {
            self.die(now, Cause::Deprivation);
        }
    }

    /// Apply a finished intent's effects, if it has finished.
    pub fn settle_intent(&mut self, now: Time) -> Option<Deed> {
        let intent = self.intent?;
        if intent.until > now {
            return None;
        }
        for (need, delta) in intent.deed.effects() {
            self.needs.adjust(*need, *delta);
        }
        self.intent = None;
        Some(intent.deed)
    }

    /// Decide what to do next and commit to it.
    pub fn decide(&mut self, now: Time, situation: &Situation, rng: &mut Rng) -> Choice {
        let choice = deeds::choose(
            &Mind {
                personality: &self.personality,
                values: &self.values,
                needs: &self.needs,
                age_years: self.age(now).years(),
            },
            situation,
            rng,
        );
        self.intent = Some(Intent {
            deed: choice.deed,
            started: now,
            until: now + choice.deed.duration(),
        });
        choice
    }

    /// Catch up, finish whatever was in progress, and pick the next thing. The whole
    /// per-person step, in the order it has to happen.
    pub fn step(&mut self, now: Time, situation: &Situation, rng: &mut Rng) -> Option<Choice> {
        self.catch_up(now);
        if !self.is_alive() {
            return None;
        }
        self.settle_intent(now);
        Some(self.decide(now, situation, rng))
    }

    /// Roll against the mortality schedule for a span of time.
    pub fn survive(&mut self, now: Time, over: Duration, rng: &mut Rng) -> Option<Cause> {
        if !self.is_alive() {
            return None;
        }
        let age = self.age(now);
        if Mortality::HUMAN.rolls_death(age, over, self.health.frailty(), rng) {
            let cause = self.likely_cause(age);
            self.die(now, cause);
            return Some(cause);
        }
        None
    }

    fn likely_cause(&self, age: Age) -> Cause {
        if self.needs.vital_pressure() > 0.5 {
            Cause::Deprivation
        } else if self.health.vitality < 0.6 {
            Cause::Illness
        } else if age.stage() == LifeStage::Elder {
            Cause::OldAge
        } else {
            Cause::Misadventure
        }
    }

    fn die(&mut self, when: Time, cause: Cause) {
        if self.is_alive() {
            self.died = Some((when, cause));
            self.intent = None;
        }
    }

    /// Force a need, for tests and for events that act on a person from outside.
    pub fn set_need(&mut self, need: Need, level: f32) {
        self.needs.set(need, level);
    }
}

/// A random person, drawn from a caller-supplied stream.
///
/// Personality is sampled directly here. Phase 2 replaces this with inheritance from a
/// genome, at which point siblings start resembling their parents and each other.
pub fn generate(rng: &mut Rng, home: PlanetId, born: Time) -> Person {
    // Names come from `faker_rand`, which draws from our stream but picks from its own
    // word lists — stable for a pinned version rather than forever. Cosmetic.
    use rand::Rng as _;
    let name: FullName = rng.r#gen();

    let personality = Personality::sample(rng);
    let values = Values::sample(rng, &personality);

    Person::new(
        name.to_string(),
        pick(rng, &Country::ALL),
        PhysicalAttrs::new(
            pick(
                rng,
                &[Weight::Underweight, Weight::Normal, Weight::Overweight],
            ),
            pick(rng, &[Height::Short, Height::Average, Height::Tall]),
        ),
        personality,
        values,
        home,
        born,
    )
}

fn pick<T: Copy>(rng: &mut Rng, options: &[T]) -> T {
    *rng.pick(options).expect("cannot choose from an empty set")
}

#[cfg(test)]
mod tests {
    use super::*;
    use planet::DayPhase;
    use sim_core::{Arena, Domain, WorldSeed};

    fn a_home() -> PlanetId {
        let mut arena: Arena<planet::Planet> = Arena::new();
        arena.insert(planet::Planet::earth())
    }

    fn rng(n: u64) -> Rng {
        WorldSeed::from_u128(0xb0_0c).stream(Domain::Behavior, n, 0)
    }

    fn somebody() -> Person {
        Person::new(
            "Ada",
            Country::Gbr,
            PhysicalAttrs::new(Weight::Normal, Height::Average),
            Personality::AVERAGE,
            Values::BALANCED,
            a_home(),
            Time::ORIGIN,
        )
    }

    fn adult() -> Person {
        let mut p = somebody();
        p.born = Time::ORIGIN;
        p.updated = Time::ORIGIN + Duration::from_years(30);
        p
    }

    #[test]
    fn needs_accrue_between_visits() {
        let mut p = somebody();
        assert_eq!(p.needs().total_pressure(), 0.0);
        p.catch_up(Time::ORIGIN + Duration::from_hours(12));
        assert!(p.needs().get(Need::Thirst) > 0.0);
    }

    #[test]
    fn catching_up_is_idempotent() {
        let mut p = somebody();
        let noon = Time::ORIGIN + Duration::from_hours(12);
        p.catch_up(noon);
        let once = *p.needs();
        p.catch_up(noon);
        assert_eq!(*p.needs(), once, "catching up twice must not double-charge");
    }

    #[test]
    fn one_long_gap_equals_many_short_ones() {
        let mut straight = somebody();
        straight.catch_up(Time::ORIGIN + Duration::from_hours(10));

        let mut piecemeal = somebody();
        for h in 1..=10 {
            piecemeal.catch_up(Time::ORIGIN + Duration::from_hours(h));
        }
        for need in Need::ALL {
            let (a, b) = (straight.needs().get(need), piecemeal.needs().get(need));
            assert!((a - b).abs() < 1e-4, "{need}: {a} vs {b}");
        }
    }

    #[test]
    fn a_step_settles_the_old_intent_and_starts_a_new_one() {
        let mut p = adult();
        p.set_need(Need::Thirst, 0.95);
        let now = p.updated;

        let first = p
            .step(now, &Situation::plain(DayPhase::Morning), &mut rng(1))
            .unwrap();
        assert_eq!(first.deed, Deed::Drink);
        assert!(p.intent().is_some());

        // Nothing settles before the intent is due.
        assert_eq!(p.settle_intent(now), None);

        let later = now + Deed::Drink.duration();
        p.catch_up(later);
        assert_eq!(p.settle_intent(later), Some(Deed::Drink));
        assert!(
            p.needs().get(Need::Thirst) < 0.5,
            "drinking should have helped"
        );
    }

    #[test]
    fn a_week_of_living_keeps_needs_under_control() {
        // The end-to-end check that the loop closes: someone left to run their own life
        // should feed, water, and rest themselves indefinitely. The planet's clock
        // drives their sense of time of day, exactly as the simulation drives it.
        let calendar = planet::Calendar::EARTH;
        let mut p = adult();
        let mut now = p.updated;
        let end = now + Duration::from_days(7);

        while now < end {
            let situation = Situation::plain(calendar.phase_at(now));
            p.step(now, &situation, &mut rng(now.as_secs()));
            now = p.intent().map(|i| i.until).unwrap_or(end);
        }
        p.catch_up(end);

        assert!(p.is_alive(), "should not have starved");
        assert!(
            p.needs().vital_pressure() < 0.5,
            "vital needs ran away: {:?}",
            p.needs()
        );
        assert!(
            p.health().vitality > 0.9,
            "health should hold up: {:?}",
            p.health()
        );
    }

    #[test]
    fn a_lived_week_shows_a_daily_rhythm() {
        // Sleep should land at night without anyone scheduling it. The circadian term
        // and the planet's derived day phase are the only things pushing it there.
        let calendar = planet::Calendar::EARTH;
        let mut p = adult();
        let mut now = p.updated;
        let end = now + Duration::from_days(7);
        let (mut at_night, mut by_day) = (0, 0);

        while now < end {
            let phase = calendar.phase_at(now);
            let choice = p
                .step(now, &Situation::plain(phase), &mut rng(now.as_secs()))
                .expect("still alive");
            if choice.deed == Deed::Sleep {
                match phase {
                    DayPhase::Night | DayPhase::Evening => at_night += 1,
                    _ => by_day += 1,
                }
            }
            now = p.intent().map(|i| i.until).unwrap_or(end);
        }

        assert!(at_night > 0, "should have slept at all");
        assert!(
            at_night > by_day,
            "sleep should prefer the dark: {at_night} night vs {by_day} day"
        );
    }

    #[test]
    fn neglect_kills() {
        let mut p = adult();
        p.catch_up(p.updated + Duration::from_days(60));
        assert!(!p.is_alive());
        assert_eq!(p.death().unwrap().1, Cause::Deprivation);
    }

    #[test]
    fn the_dead_stop_changing() {
        let mut p = adult();
        p.catch_up(p.updated + Duration::from_days(60));
        let at_death = *p.needs();
        let age_at_death = p.age(Time::ORIGIN + Duration::from_years(200)).years();

        p.catch_up(Time::ORIGIN + Duration::from_years(200));
        assert_eq!(*p.needs(), at_death, "a corpse does not get thirstier");
        assert!(
            (p.age(Time::ORIGIN + Duration::from_years(500)).years() - age_at_death).abs() < 0.01,
            "nor older"
        );
        assert!(
            p.step(
                Time::ORIGIN + Duration::from_years(200),
                &Situation::plain(DayPhase::Morning),
                &mut rng(1)
            )
            .is_none()
        );
    }

    #[test]
    fn age_and_stage_are_derived() {
        let p = somebody();
        assert_eq!(p.stage(Time::ORIGIN), LifeStage::Infant);
        assert_eq!(
            p.stage(Time::ORIGIN + Duration::from_years(30)),
            LifeStage::Adult
        );
        assert_eq!(
            p.stage(Time::ORIGIN + Duration::from_years(80)),
            LifeStage::Elder
        );
    }

    #[test]
    fn mortality_eventually_catches_everyone() {
        let mut deaths = 0;
        for i in 0..200u64 {
            let mut p = somebody();
            let mut year = 0u64;
            while p.is_alive() && year < 200 {
                let now = Time::ORIGIN + Duration::from_years(year);
                p.survive(now, Duration::from_years(1), &mut rng(i * 1000 + year));
                year += 1;
            }
            if !p.is_alive() {
                deaths += 1;
            }
        }
        assert_eq!(deaths, 200, "everyone dies within two centuries");
    }

    #[test]
    fn a_first_sighting_happens_once() {
        let mut p = somebody();
        assert!(p.first_sighting());
        assert!(!p.first_sighting());
    }

    #[test]
    fn the_same_seed_produces_the_same_person() {
        let home = a_home();
        let seed = WorldSeed::from_u128(0xfeed);
        let one = generate(&mut seed.stream(Domain::Naming, 0, 0), home, Time::ORIGIN);
        let two = generate(&mut seed.stream(Domain::Naming, 0, 0), home, Time::ORIGIN);
        assert_eq!(one, two);
    }

    #[test]
    fn a_different_world_produces_different_people() {
        let home = a_home();
        let a = generate(
            &mut WorldSeed::from_u128(1).stream(Domain::Naming, 0, 0),
            home,
            Time::ORIGIN,
        );
        let b = generate(
            &mut WorldSeed::from_u128(2).stream(Domain::Naming, 0, 0),
            home,
            Time::ORIGIN,
        );
        assert_ne!(a, b);
    }

    #[test]
    fn generated_people_are_well_formed_and_varied() {
        let home = a_home();
        let seed = WorldSeed::from_u128(77);
        let people: Vec<Person> = (0..200)
            .map(|i| generate(&mut seed.stream(Domain::Naming, i, 0), home, Time::ORIGIN))
            .collect();

        for p in &people {
            assert!(!p.name.trim().is_empty());
            assert!(p.is_alive());
            assert_eq!(p.home, home);
        }

        let outlooks: std::collections::HashSet<Outlook> =
            people.iter().map(|p| p.personality.outlook()).collect();
        assert!(
            outlooks.len() >= 2,
            "a population should not share one outlook"
        );
    }
}
