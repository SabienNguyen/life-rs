//! Saving a world, and getting it back.
//!
//! Not a serialised heap. A world here is a **pure function of a handful of numbers** —
//! that is the reproducibility guarantee the whole project is built on, tested in every
//! crate, and the thing that makes `--seed` mean anything. So the save file is those
//! numbers, and loading re-derives the world from them.
//!
//! ## Why this rather than a format
//!
//! Writing the state out would mean a schema covering arenas of people, a chronicle, a
//! genome per person, a lithosphere, a climate, a biosphere, an ecology and a phylogeny —
//! and then keeping every one of them in step with the code for as long as the project
//! lives. Save formats rot. Worse, a state file *can be wrong*: nothing stops a load
//! producing a world the simulation could not have produced, and every hour spent
//! debugging one of those is an hour spent on a bug that did not exist.
//!
//! A derivation cannot be wrong. If the numbers are right the world is bit-for-bit the
//! world that was saved, because it is the same computation. There is no schema, nothing to
//! migrate, and a save file from any version replays correctly on any version that still
//! produces the same world — and where it does not, that is a *real* change in the model
//! and something you would want to know about rather than paper over.
//!
//! ## What it costs, stated plainly
//!
//! Loading is O(the time being loaded). A world saved at three hundred years takes three
//! hundred years of simulation to open, which is seconds here and would not be at a
//! thousand times the population. That is the trade, it is the whole trade, and the moment
//! it stops being acceptable is the moment to write a real format — with the derivation
//! kept as the thing that checks it.

use std::fmt;
use std::str::FromStr;

use sim_core::{Duration, Salience, WorldSeed};

/// Everything needed to produce a world again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Provenance {
    pub seed: WorldSeed,
    /// How many people it was founded with.
    pub population: usize,
    /// How long it has been running.
    pub elapsed: Duration,
    /// How many people may be simulated finely. Part of the world, not of the view: the
    /// level-of-detail budget changes what happens, so a save that dropped it would
    /// reopen a different world.
    pub detail_budget: usize,
    /// The floor below which nothing was recorded. Also part of the world, for the same
    /// reason — an event not recorded is an event that cannot be remembered later.
    pub floor: Salience,
}

/// The tag that opens a save, so a file that is not one says so.
const MAGIC: &str = "life-rs/world";

impl fmt::Display for Provenance {
    /// One line, readable, and diffable.
    ///
    /// A save you can read is a save you can reason about — and, because the world is a
    /// function of these numbers, a save you can *edit* to ask a what-if. Changing the
    /// population and reopening is a legitimate experiment rather than a corrupted file.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{MAGIC} seed={} people={} seconds={} detail={} floor={}",
            self.seed,
            self.population,
            self.elapsed.as_secs(),
            self.detail_budget,
            floor_name(self.floor),
        )
    }
}

/// Why a save could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotASave {
    /// It does not begin with the tag.
    Unrecognised,
    /// A field is absent.
    Missing(&'static str),
    /// A field is present and not a number, or not a salience.
    Unreadable(&'static str),
}

impl fmt::Display for NotASave {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NotASave::Unrecognised => write!(f, "not a life-rs save"),
            NotASave::Missing(field) => write!(f, "a save needs {field}"),
            NotASave::Unreadable(field) => write!(f, "could not read {field}"),
        }
    }
}

impl std::error::Error for NotASave {}

impl FromStr for Provenance {
    type Err = NotASave;

    fn from_str(text: &str) -> Result<Provenance, NotASave> {
        let text = text.trim();
        let rest = text.strip_prefix(MAGIC).ok_or(NotASave::Unrecognised)?;

        let field = |name: &'static str| -> Option<&str> {
            rest.split_whitespace()
                .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
        };

        let seed = field("seed").ok_or(NotASave::Missing("seed"))?;
        let seed = WorldSeed::parse(seed).map_err(|_| NotASave::Unreadable("seed"))?;

        let population = field("people").ok_or(NotASave::Missing("people"))?;
        let population = population
            .parse()
            .map_err(|_| NotASave::Unreadable("people"))?;

        let seconds = field("seconds").ok_or(NotASave::Missing("seconds"))?;
        let seconds: u64 = seconds
            .parse()
            .map_err(|_| NotASave::Unreadable("seconds"))?;

        // These two arrived after the first saves were written, so a file without them is
        // an older save rather than a broken one and gets what those saves meant.
        let detail_budget = match field("detail") {
            Some(text) => text.parse().map_err(|_| NotASave::Unreadable("detail"))?,
            None => crate::FULL_DETAIL_BUDGET,
        };
        let floor = match field("floor") {
            Some(text) => floor_from(text).ok_or(NotASave::Unreadable("floor"))?,
            None => Salience::Routine,
        };

        Ok(Provenance {
            seed,
            population,
            elapsed: Duration::from_secs(seconds),
            detail_budget,
            floor,
        })
    }
}

fn floor_name(floor: Salience) -> &'static str {
    match floor {
        Salience::Routine => "routine",
        Salience::Notable => "notable",
        Salience::Pivotal => "pivotal",
        Salience::Historic => "historic",
        Salience::Epochal => "epochal",
    }
}

fn floor_from(text: &str) -> Option<Salience> {
    Some(match text {
        "routine" => Salience::Routine,
        "notable" => Salience::Notable,
        "pivotal" => Salience::Pivotal,
        "historic" => Salience::Historic,
        "epochal" => Salience::Epochal,
        _ => return None,
    })
}

/// Everything needed to produce a deep-time run again.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeepProvenance {
    pub seed: WorldSeed,
    pub myr: f64,
    /// The step it was run in, which is part of the world and not of the view: the
    /// lithosphere subdivides its own step but the climate and the settlements do not, so
    /// the same span in different steps is a different history.
    pub step_myr: f32,
}

const DEEP_MAGIC: &str = "life-rs/ages";

impl fmt::Display for DeepProvenance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{DEEP_MAGIC} seed={} myr={} step={}",
            self.seed, self.myr, self.step_myr
        )
    }
}

impl FromStr for DeepProvenance {
    type Err = NotASave;

    fn from_str(text: &str) -> Result<DeepProvenance, NotASave> {
        let text = text.trim();
        let rest = text.strip_prefix(DEEP_MAGIC).ok_or(NotASave::Unrecognised)?;
        let field = |name: &'static str| -> Option<&str> {
            rest.split_whitespace()
                .find_map(|pair| pair.strip_prefix(name)?.strip_prefix('='))
        };

        let seed = field("seed").ok_or(NotASave::Missing("seed"))?;
        let seed = WorldSeed::parse(seed).map_err(|_| NotASave::Unreadable("seed"))?;
        let myr: f64 = field("myr")
            .ok_or(NotASave::Missing("myr"))?
            .parse()
            .map_err(|_| NotASave::Unreadable("myr"))?;
        let step_myr: f32 = field("step")
            .ok_or(NotASave::Missing("step"))?
            .parse()
            .map_err(|_| NotASave::Unreadable("step"))?;
        if !myr.is_finite() || myr < 0.0 || !step_myr.is_finite() || step_myr <= 0.0 {
            return Err(NotASave::Unreadable("myr"));
        }
        Ok(DeepProvenance {
            seed,
            myr,
            step_myr,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::World;

    fn saved(seed: u128, people: usize, years: u64) -> (World, Provenance) {
        let mut world = World::genesis(WorldSeed::from_u128(seed), people);
        world.record_only(Salience::Pivotal);
        if years > 0 {
            world.run_for(Duration::from_years(years));
        }
        let save = world.provenance();
        (world, save)
    }

    #[test]
    fn a_reopened_world_is_the_world_that_was_saved() {
        // The only claim that matters. Not "close enough" — the same computation, so the
        // same world, and the test is written to catch anything less.
        let (before, save) = saved(0x5a7e, 30, 40);
        let after = World::reopen(&save);

        assert_eq!(before.people.len(), after.people.len());
        assert_eq!(before.living(), after.living());
        assert_eq!(before.now(), after.now());
        assert_eq!(before.chronicle.len(), after.chronicle.len());
        assert_eq!(before.places.len(), after.places.len());

        // Every person, to the last decimal.
        let read = |w: &World| -> Vec<(String, f32, f32, bool)> {
            w.people
                .iter()
                .map(|(_, p)| {
                    (
                        p.name.clone(),
                        p.peak_standing(),
                        p.personality.conscientiousness,
                        p.is_alive(),
                    )
                })
                .collect()
        };
        assert_eq!(read(&before), read(&after));

        // And every place, including the ground under it.
        let places = |w: &World| -> Vec<(String, f32, u32, Option<u32>)> {
            w.places
                .iter()
                .map(|(_, p)| {
                    (
                        p.name.clone(),
                        p.env.affluence,
                        p.capacity,
                        p.terrain.as_ref().map(|t| t.cell),
                    )
                })
                .collect()
        };
        assert_eq!(places(&before), places(&after));
    }

    #[test]
    fn the_planet_comes_back_too() {
        let (before, save) = saved(0x5a7f, 20, 20);
        let after = World::reopen(&save);
        let (a, b) = (before.surface().unwrap(), after.surface().unwrap());
        assert_eq!(a.planet.land_fraction(), b.planet.land_fraction());
        assert_eq!(a.climate.co2_ppm(), b.climate.co2_ppm());
        assert_eq!(a.star(), b.star());
        assert_eq!(a.orbit(), b.orbit());
    }

    #[test]
    fn a_save_survives_being_written_and_read() {
        let (_, save) = saved(0xbeef, 25, 15);
        let text = save.to_string();
        assert!(text.starts_with(MAGIC));
        assert_eq!(text.parse::<Provenance>().unwrap(), save);
        // Readable, which is the other half of why it is text.
        assert!(text.contains("people=25"));
        assert!(text.contains("floor=pivotal"));
    }

    #[test]
    fn an_edited_save_is_a_legitimate_experiment() {
        // Because a world is a function of these numbers, changing one and reopening asks
        // a what-if rather than corrupting a file. This is a property worth keeping.
        let (_, save) = saved(0xed17, 20, 10);
        let asked = save.to_string().replace("people=20", "people=60");
        let other: Provenance = asked.parse().unwrap();
        assert_eq!(other.population, 60);
        assert_eq!(other.seed, save.seed);
        let world = World::reopen(&other);
        assert!(world.people.len() > World::reopen(&save).people.len());
    }

    #[test]
    fn a_fresh_world_saves_at_zero_and_reopens_instantly() {
        let (before, save) = saved(0x0000, 12, 0);
        assert_eq!(save.elapsed, Duration::ZERO);
        let after = World::reopen(&save);
        assert_eq!(before.people.len(), after.people.len());
        assert_eq!(before.now(), after.now());
    }

    #[test]
    fn something_that_is_not_a_save_says_so() {
        assert_eq!(
            "hello".parse::<Provenance>().unwrap_err(),
            NotASave::Unrecognised
        );
        assert_eq!(
            format!("{MAGIC} people=3 seconds=0")
                .parse::<Provenance>()
                .unwrap_err(),
            NotASave::Missing("seed")
        );
        assert_eq!(
            format!("{MAGIC} seed=zzz people=3 seconds=0")
                .parse::<Provenance>()
                .unwrap_err(),
            NotASave::Unreadable("seed")
        );
        assert_eq!(
            format!("{MAGIC} seed=0x1 people=3 seconds=0 floor=loud")
                .parse::<Provenance>()
                .unwrap_err(),
            NotASave::Unreadable("floor")
        );
    }

    #[test]
    fn an_older_save_without_the_later_fields_still_opens() {
        // There is no schema to migrate, but there are still saves written before a field
        // existed, and they should mean what they meant.
        let old = format!("{MAGIC} seed=0x2b people=8 seconds=0");
        let save: Provenance = old.parse().unwrap();
        assert_eq!(save.population, 8);
        assert_eq!(save.floor, Salience::Routine);
        assert_eq!(save.detail_budget, crate::FULL_DETAIL_BUDGET);
    }

    #[test]
    fn every_field_that_changes_the_world_is_in_the_save() {
        // The failure this guards against is silent: a save that omits something the world
        // depends on reopens a *different* world and nothing complains. Both of these are
        // in the file because both change what happens.
        // Everything recorded, not only what is pivotal. Filtered to the pivotal the two
        // runs came to the same *number* of births and deaths on one seed — which is a
        // coincidence and was read as the budget not mattering. What the budget changes is
        // whether anybody deliberates, and that is a claim about deeds.
        let mut coarse = World::genesis(WorldSeed::from_u128(0xd1), 40);
        coarse.set_detail_budget(0);
        coarse.run_for(Duration::from_years(30));

        let mut fine = World::genesis(WorldSeed::from_u128(0xd1), 40);
        fine.run_for(Duration::from_years(30));

        assert_ne!(
            coarse.chronicle.len(),
            fine.chronicle.len(),
            "the detail budget has to matter, or its being in the save is cargo cult"
        );
        assert_eq!(coarse.provenance().detail_budget, 0);
        assert_eq!(World::reopen(&coarse.provenance()).chronicle.len(), coarse.chronicle.len());
    }

    #[test]
    fn a_deep_run_saves_too() {
        let save = DeepProvenance {
            seed: WorldSeed::from_u128(0xa9e5),
            myr: 250.0,
            step_myr: 4.0,
        };
        let text = save.to_string();
        assert_eq!(text.parse::<DeepProvenance>().unwrap(), save);
        assert_eq!(
            "life-rs/world seed=0x1 people=1 seconds=0"
                .parse::<DeepProvenance>()
                .unwrap_err(),
            NotASave::Unrecognised,
            "a fine save is not a deep save"
        );
        assert!("life-rs/ages seed=0x1 myr=-4 step=4".parse::<DeepProvenance>().is_err());
        assert!("life-rs/ages seed=0x1 myr=4 step=0".parse::<DeepProvenance>().is_err());
    }
}
