//! What a people call themselves.
//!
//! Not from a list of countries. The name of a people here is built from **what they
//! actually do** — the way of spending a day that most distinguishes them from everybody
//! else — plus a sound, so that two peoples who diverge in different directions get names
//! that differ in the way they did.
//!
//! This is closer to the truth than a table would be. A great many real ethnonyms are
//! descriptions that stuck: the people of the marsh, the horse people, the ones who eat
//! fish. Almost none of them were chosen; they accreted, and the ones that were chosen were
//! usually chosen by somebody else.
//!
//! What it deliberately is not is a language. There is no phonology, no sound change, no
//! borrowing, and two names that resemble each other do not indicate a relationship. §23
//! puts languages out of scope and they would need their own evolutionary dynamics; a name
//! that describes a practice is the cheapest thing that carries real information.

use person::Deed;
use sim_core::Rng;

use crate::WAYS;

/// What each way of spending a day makes a people, when they do more of it than anybody.
///
/// Adjectival rather than descriptive, so the names read as names. The mapping is fixed
/// because the deeds are fixed; what varies is which one a people ends up known for, and
/// that is drift and circumstance rather than anything written here.
fn known_for(way: usize) -> &'static str {
    match Deed::ALL[way] {
        Deed::Eat => "Feast",
        Deed::Drink => "Spring",
        Deed::Sleep => "Quiet",
        Deed::Wash => "Clear",
        Deed::Socialize => "Gather",
        Deed::Work => "Toil",
        Deed::Wander => "Roam",
    }
}

/// The endings a people's name can take.
///
/// Sound only. They carry no meaning and are not a language — see the module note.
const ENDINGS: [&str; 12] = [
    "ir", "eth", "ai", "ung", "ora", "esh", "ka", "im", "ard", "olu", "yr", "ane",
];

/// Name a people after the thing they do more of than anybody else.
///
/// The way furthest from the unremarkable middle is the one that names them, which is the
/// right rule: what marks a people out is not what they do most of — everybody sleeps —
/// but what they do *unusually* much or little of.
pub fn name_a_people(ways: &[f32; WAYS], rng: &mut Rng) -> String {
    let mut striking = 0;
    let mut furthest = 0.0;
    for (way, amount) in ways.iter().enumerate() {
        let unusual = (amount - 0.5).abs();
        if unusual > furthest {
            furthest = unusual;
            striking = way;
        }
    }

    let stem = known_for(striking);
    let ending = *rng.pick(&ENDINGS).unwrap_or(&"ir");
    // A people who do notably *little* of something are named for the lack of it, which is
    // the other half of being distinctive and costs one word.
    if ways[striking] < 0.5 {
        format!("Un{}{ending}", stem.to_lowercase())
    } else {
        format!("{stem}{ending}")
    }
}

/// Name a country after its largest place.
///
/// Which is how the overwhelming majority of real countries got their names, and it means
/// the name is not a new invention: it is already in the world, derived from the terrain
/// the place stands on, so a country called Wickstrand is called that because somebody
/// settled a strand and it grew.
pub fn name_a_country(largest_place: &str) -> String {
    largest_place.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::{Domain, WorldSeed};

    fn rng() -> Rng {
        WorldSeed::from_u128(0xc017).stream(Domain::Naming, 0, 0)
    }

    #[test]
    fn a_people_is_named_for_what_marks_them_out() {
        let mut ways = [0.5; WAYS];
        ways[Deed::Work as usize] = 0.95;
        let mut rng = rng();
        for _ in 0..40 {
            let name = name_a_people(&ways, &mut rng);
            assert!(name.starts_with("Toil"), "{name}");
        }
    }

    #[test]
    fn doing_notably_little_of_something_names_a_people_too() {
        let mut ways = [0.5; WAYS];
        ways[Deed::Work as usize] = 0.02;
        let mut rng = rng();
        let name = name_a_people(&ways, &mut rng);
        assert!(name.starts_with("Untoil"), "{name}");
    }

    #[test]
    fn the_unusual_way_names_them_not_the_commonest() {
        // Everybody sleeps. What marks a people out is what they do unusually much of.
        let mut ways = [0.5; WAYS];
        ways[Deed::Sleep as usize] = 0.62;
        ways[Deed::Wander as usize] = 0.93;
        let mut rng = rng();
        assert!(name_a_people(&ways, &mut rng).starts_with("Roam"));
    }

    #[test]
    fn peoples_who_diverge_differently_get_different_names() {
        let mut rng = rng();
        let mut wanderers = [0.5; WAYS];
        wanderers[Deed::Wander as usize] = 0.9;
        let mut gatherers = [0.5; WAYS];
        gatherers[Deed::Socialize as usize] = 0.9;
        assert_ne!(
            name_a_people(&wanderers, &mut rng),
            name_a_people(&gatherers, &mut rng)
        );
    }

    #[test]
    fn a_featureless_people_still_gets_a_name() {
        let mut rng = rng();
        let name = name_a_people(&[0.5; WAYS], &mut rng);
        assert!(!name.is_empty());
    }

    #[test]
    fn a_country_is_named_after_its_largest_place() {
        assert_eq!(name_a_country("Wickstrand"), "Wickstrand");
    }

    #[test]
    fn a_world_does_not_call_every_people_the_same_thing() {
        let mut rng = rng();
        let mut ways = [0.5; WAYS];
        ways[Deed::Work as usize] = 0.9;
        let names: std::collections::BTreeSet<String> =
            (0..60).map(|_| name_a_people(&ways, &mut rng)).collect();
        assert!(names.len() > 6, "only {} distinct names", names.len());
    }
}
