//! People.
//!
//! Phase 0 keeps the attributes the project already had and changes only how a person
//! is *reached* and how they *decide*:
//!
//! - A person holds `home: PlanetId`, not `&Planet`. Borrowing the world was what made
//!   families impossible to represent — two people who point at each other are a cycle,
//!   and a cycle of borrows does not compile.
//! - Acting returns an [`Action`] instead of printing. Deciding and narrating are
//!   different jobs, and only the first belongs in the simulation.
//!
//! Personality is still a pair of enums here. It becomes a heritable vector computed
//! from a genome and an upbringing in a later phase; the enums stay as the labels that
//! get read off it.

use faker_rand::en_us::names::FullName;
use planet::{DayPhase, PlanetId};
use sim_core::{Id, Rng};
use std::fmt;

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
pub enum Outlook {
    Optimistic,
    Pessimistic,
    Realist,
}

impl Outlook {
    pub const ALL: [Outlook; 3] = [Outlook::Optimistic, Outlook::Pessimistic, Outlook::Realist];
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Personality {
    pub outlook: Outlook,
    pub confident: bool,
}

impl Personality {
    pub fn new(outlook: Outlook, confident: bool) -> Personality {
        Personality { outlook, confident }
    }
}

/// What a person is currently doing. Survives from the original state machine, and
/// becomes the in-progress `Intent` once behaviour is utility-scored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Start,
    Idle,
    Eat,
    Sleep,
    DrinkWater,
}

/// The outcome of one decision. The simulation produces these; the frontend renders
/// them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Introduce,
    Remark(Remark),
    /// Chosen nothing this step — no output, no state change.
    Idle,
}

/// The things a person currently has to say. A stand-in for utility-scored actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Remark {
    Bored,
    Lunch,
    Dinner,
    GoodNight,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Person {
    state: State,
    pub name: String,
    pub country: Country,
    pub physical: PhysicalAttrs,
    pub personality: Personality,
    /// Which world they live on — a handle, so no lifetime ties a person to a planet.
    pub home: PlanetId,
}

impl Person {
    pub fn new(
        name: impl Into<String>,
        country: Country,
        physical: PhysicalAttrs,
        personality: Personality,
        home: PlanetId,
    ) -> Person {
        Person {
            state: State::Start,
            name: name.into(),
            country,
            physical,
            personality,
            home,
        }
    }

    pub fn state(&self) -> State {
        self.state
    }

    /// Decide what to do, given the time of day where they are.
    pub fn act(&mut self, phase: DayPhase) -> Action {
        match self.state {
            State::Start => {
                self.state = State::Idle;
                Action::Introduce
            }
            State::Idle => Action::Remark(match phase {
                DayPhase::Morning => Remark::Bored,
                DayPhase::Afternoon => Remark::Lunch,
                DayPhase::Evening => Remark::Dinner,
                DayPhase::Night => Remark::GoodNight,
            }),
            // Reached once needs drive behaviour; nothing schedules them yet.
            State::Eat | State::Sleep | State::DrinkWater => Action::Idle,
        }
    }
}

/// A random person, drawn from a caller-supplied stream.
///
/// Takes the stream rather than reaching for `thread_rng()`: the same world seed must
/// produce the same people, or none of the save/replay/backfill machinery works.
pub fn generate(rng: &mut Rng, home: PlanetId) -> Person {
    // Name generation goes through `faker_rand`, which draws from our stream but picks
    // from its own word lists — so names are stable for a pinned version of that crate
    // rather than forever. Cosmetic, and worth revisiting when naming becomes cultural.
    use rand::Rng as _;
    let name: FullName = rng.r#gen();

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
        Personality::new(pick(rng, &Outlook::ALL), rng.coin()),
        home,
    )
}

fn pick<T: Copy>(rng: &mut Rng, options: &[T]) -> T {
    *rng.pick(options).expect("cannot choose from an empty set")
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::{Arena, Domain, WorldSeed};

    fn a_home() -> PlanetId {
        let mut arena: Arena<planet::Planet> = Arena::new();
        arena.insert(planet::Planet::earth())
    }

    fn somebody() -> Person {
        Person::new(
            "Ada",
            Country::Gbr,
            PhysicalAttrs::new(Weight::Normal, Height::Average),
            Personality::new(Outlook::Realist, true),
            a_home(),
        )
    }

    #[test]
    fn a_new_person_introduces_themselves_once() {
        let mut p = somebody();
        assert_eq!(p.state(), State::Start);
        assert_eq!(p.act(DayPhase::Morning), Action::Introduce);
        assert_eq!(p.state(), State::Idle);
        assert_eq!(p.act(DayPhase::Morning), Action::Remark(Remark::Bored));
    }

    #[test]
    fn remarks_follow_the_time_of_day() {
        let mut p = somebody();
        p.act(DayPhase::Morning); // consume the introduction
        assert_eq!(p.act(DayPhase::Morning), Action::Remark(Remark::Bored));
        assert_eq!(p.act(DayPhase::Afternoon), Action::Remark(Remark::Lunch));
        assert_eq!(p.act(DayPhase::Evening), Action::Remark(Remark::Dinner));
        assert_eq!(p.act(DayPhase::Night), Action::Remark(Remark::GoodNight));
    }

    #[test]
    fn the_same_seed_produces_the_same_person() {
        let home = a_home();
        let seed = WorldSeed::from_u128(0xfeed);
        let one = generate(&mut seed.stream(Domain::Naming, 0, 0), home);
        let two = generate(&mut seed.stream(Domain::Naming, 0, 0), home);
        assert_eq!(one, two);
    }

    #[test]
    fn different_streams_produce_different_people() {
        let home = a_home();
        let seed = WorldSeed::from_u128(0xfeed);
        let people: Vec<Person> = (0..12)
            .map(|i| generate(&mut seed.stream(Domain::Naming, i, 0), home))
            .collect();
        let names: std::collections::HashSet<_> = people.iter().map(|p| &p.name).collect();
        assert!(names.len() > 8, "expected variety, got {names:?}");
    }

    #[test]
    fn a_different_world_produces_different_people() {
        let home = a_home();
        let a = generate(
            &mut WorldSeed::from_u128(1).stream(Domain::Naming, 0, 0),
            home,
        );
        let b = generate(
            &mut WorldSeed::from_u128(2).stream(Domain::Naming, 0, 0),
            home,
        );
        assert_ne!(a, b);
    }

    #[test]
    fn generated_people_are_well_formed() {
        let home = a_home();
        let seed = WorldSeed::from_entropy();
        for i in 0..50 {
            let p = generate(&mut seed.stream(Domain::Naming, i, 0), home);
            assert!(!p.name.trim().is_empty());
            assert!(Country::ALL.contains(&p.country));
            assert_eq!(p.home, home);
            assert_eq!(p.state(), State::Start);
        }
    }
}
