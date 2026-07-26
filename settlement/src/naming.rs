//! Names that say where they are.
//!
//! The five quarters this world used to start with were called Northside, The Wharf,
//! Elmhurst, Kingsfield and Lowgate, and they were called that in every world, on every
//! seed, whatever the ground under them turned out to be. Once places sit on real terrain
//! that is a lie the map tells about itself — a settlement called The Wharf eight hundred
//! kilometres inland on a cold plateau.
//!
//! So the second half of a name is chosen by the land and the first half is chosen by the
//! seed. This is how English place names actually work and it is why you can read a map of
//! England as a description of terrain: *-mouth* and *-haven* are on water, *-hurst* and
//! *-holt* are in woodland, *-fell* and *-tor* are high, *-wells* is where the water was
//! worth naming. A world that names its own settlements this way is one where the name
//! over the door tells you something true, which is the whole point of deriving rather
//! than authoring.

use sim_core::Rng;
use society::Terrain;

/// The first half. Sound rather than meaning — these are meant to read as the worn-down
/// remains of words nobody in the world remembers either.
const STEMS: [&str; 48] = [
    "Ald", "Ash", "Bram", "Cald", "Carn", "Dun", "Eld", "Fenn", "Gart", "Grim", "Hal", "Harl",
    "Hollin", "Ing", "Kell", "Kirk", "Lang", "Ling", "Marl", "Mel", "Nor", "Oak", "Pen", "Quill",
    "Raven", "Red", "Rill", "Sal", "Scar", "Sel", "Shaw", "Stan", "Stow", "Thorn", "Thwait",
    "Til", "Tor", "Twy", "Ux", "Vane", "Wal", "Wark", "Wen", "West", "Whit", "Wick", "Wyn", "Yarl",
];

/// On the water.
const COASTAL: [&str; 8] = [
    "mouth", "haven", "wick", "strand", "quay", "port", "ness", "hythe",
];
/// Under trees.
const WOODED: [&str; 6] = ["hurst", "holt", "wood", "shaw", "den", "leigh"];
/// Open ground.
const OPEN: [&str; 6] = ["field", "meadow", "ley", "croft", "garth", "ing"];
/// High ground.
const HIGH: [&str; 6] = ["fell", "tor", "crag", "scar", "edge", "combe"];
/// Dry ground, where the water is the thing worth naming.
const DRY: [&str; 5] = ["wells", "drift", "reach", "flats", "spring"];
/// Cold ground.
const COLD: [&str; 5] = ["rime", "frith", "hollow", "shieling", "moss"];

/// A name for somewhere, chosen by what it is.
pub fn name_for(terrain: &Terrain, coastal: bool, rng: &mut Rng) -> String {
    let stem = *rng.pick(&STEMS).unwrap_or(&"Stan");
    // In the order the land asserts itself. Being on the sea beats everything, because a
    // harbour is the first thing anyone says about a place that has one; after that the
    // most extreme fact about the ground wins.
    let endings: &[&str] = if coastal {
        &COASTAL
    } else if terrain.elevation_m > 1400.0 {
        &HIGH
    } else if terrain.harshness > 0.55 && terrain.fertility < 0.25 {
        &DRY
    } else if terrain.latitude.abs() > 55.0 {
        &COLD
    } else if terrain.fertility > 0.55 {
        &WOODED
    } else {
        &OPEN
    };
    let ending = *rng.pick(endings).unwrap_or(&"ton");
    format!("{stem}{ending}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::{Domain, WorldSeed};

    fn rng() -> Rng {
        WorldSeed::from_u128(0x5177).stream(Domain::Chance, 0, 0)
    }

    fn named(terrain: &Terrain, coastal: bool) -> Vec<String> {
        let mut rng = rng();
        (0..200)
            .map(|_| name_for(terrain, coastal, &mut rng))
            .collect()
    }

    #[test]
    fn a_port_is_named_for_its_water() {
        let names = named(&Terrain::middling(0), true);
        assert!(
            names.iter().all(|n| COASTAL.iter().any(|e| n.ends_with(e))),
            "somewhere on the sea should be named for it: {:?}",
            &names[..4]
        );
    }

    #[test]
    fn the_mountains_are_named_for_the_mountains() {
        let mut high = Terrain::middling(0);
        high.elevation_m = 2600.0;
        let names = named(&high, false);
        assert!(names.iter().all(|n| HIGH.iter().any(|e| n.ends_with(e))));
    }

    #[test]
    fn a_dry_place_is_named_for_its_water_too() {
        let mut desert = Terrain::middling(0);
        desert.harshness = 0.8;
        desert.fertility = 0.05;
        let names = named(&desert, false);
        assert!(names.iter().all(|n| DRY.iter().any(|e| n.ends_with(e))));
    }

    #[test]
    fn the_far_north_gets_its_own_endings() {
        let mut arctic = Terrain::middling(0);
        arctic.latitude = -63.0;
        let names = named(&arctic, false);
        assert!(names.iter().all(|n| COLD.iter().any(|e| n.ends_with(e))));
    }

    #[test]
    fn good_land_is_wooded_and_poor_land_is_open() {
        let mut rich = Terrain::middling(0);
        rich.fertility = 0.9;
        assert!(
            named(&rich, false)
                .iter()
                .all(|n| WOODED.iter().any(|e| n.ends_with(e)))
        );

        let mut thin = Terrain::middling(0);
        thin.fertility = 0.2;
        assert!(
            named(&thin, false)
                .iter()
                .all(|n| OPEN.iter().any(|e| n.ends_with(e)))
        );
    }

    #[test]
    fn a_world_does_not_name_every_town_the_same_thing() {
        let names = named(&Terrain::middling(0), false);
        let distinct: std::collections::BTreeSet<&String> = names.iter().collect();
        assert!(
            distinct.len() > 40,
            "only {} distinct names out of 200",
            distinct.len()
        );
    }

    #[test]
    fn the_same_seed_names_the_same_places() {
        assert_eq!(
            named(&Terrain::middling(0), false),
            named(&Terrain::middling(0), false)
        );
    }
}
