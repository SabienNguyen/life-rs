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

/// The sounds a people make, drawn from what marks them out.
///
/// Not a language — see the module note — but not arbitrary either. Two peoples who
/// diverged in different directions get different sound sets, so the Unquietolu and the
/// Norhaven do not name their children out of the same bag. It is derived from the ways
/// vector, so it costs nothing to store and cannot fall out of step with the people it
/// belongs to.
fn voice_of(ways: &[f32; WAYS]) -> usize {
    let mut tone = 0.0f32;
    for (way, amount) in ways.iter().enumerate() {
        tone += (amount - 0.5) * (way as f32 * 1.7 + 1.0);
    }
    // A whole number that changes when the people do, and stays put when they do not.
    //
    // Signed, deliberately. Taking the absolute value here made a people who work all
    // hours and a people who barely work at all sound *identical*, because they sit the
    // same distance either side of the middle — and which side you are on is precisely
    // what distinguishes two peoples who diverged in opposite directions.
    ((tone * 97.0) as i64 + 100_000) as usize
}

const OPENERS: [&str; 14] = [
    "b", "br", "d", "g", "h", "k", "l", "m", "n", "r", "s", "t", "th", "v",
];
const VOWELS: [&str; 9] = ["a", "e", "i", "o", "u", "ae", "ei", "ou", "ia"];
const CLOSERS: [&str; 10] = ["l", "n", "r", "s", "th", "sk", "rn", "ld", "st", "m"];

/// A given name and a family name, in the sound of the people who gave them.
///
/// Names used to come from `faker_rand`'s English-US word lists, which produced "Ms. Rosa
/// Wiza MD" on a planet orbiting a nine-tenths-solar star, and gave "Conor Heller Jr." to a
/// woman. It was the same fault as the eight hardcoded countries in smaller print: a list
/// somebody else wrote, attached to nothing, in a project whose first principle is that
/// nothing is placed by fiat.
///
/// `family` is what a child inherits. Passing it through means kin share a surname, so the
/// links between them read as a family rather than as a list of strangers — which is what
/// they were, since a random draw per person shares nothing with anybody.
pub fn name_a_person(
    ways: &[f32; WAYS],
    female: bool,
    family: Option<&str>,
    rng: &mut Rng,
) -> (String, String) {
    let voice = voice_of(ways);
    let opener = |rng: &mut Rng, salt: usize| {
        OPENERS[(voice / (salt + 1) + (rng.next_u64() as usize % OPENERS.len())) % OPENERS.len()]
    };
    let vowel = |rng: &mut Rng| VOWELS[(voice / 7 + (rng.next_u64() as usize % VOWELS.len())) % VOWELS.len()];
    let closer = |rng: &mut Rng| CLOSERS[(voice / 3 + (rng.next_u64() as usize % CLOSERS.len())) % CLOSERS.len()];

    // Simple vowels for endings. Letting the full set finish a name stacked diphthong
    // on diphthong and produced "Neirneiei", which is not a name in any mouth.
    const ENDINGS: [&str; 5] = ["a", "e", "i", "o", "ia"];

    let mut given = String::new();
    given.push_str(opener(rng, 0));
    given.push_str(vowel(rng));
    let mut last = "";
    if rng.chance(0.55) {
        last = closer(rng);
        given.push_str(last);
        given.push_str(vowel(rng));
    }
    // Ending on a vowel or on a consonant is the commonest way real name systems mark
    // sex, and it is one rule rather than two lists.
    if female {
        // A consonant first if the name already ends in a vowel, or the ending lands
        // straight onto a diphthong and gives "Thouthiaa" — a spelling, not a name.
        if given.ends_with(|c| "aeiou".contains(c)) {
            given.push_str(closer(rng));
        }
        given.push_str(ENDINGS[(voice + rng.next_u64() as usize) % ENDINGS.len()]);
    } else {
        // Not the consonant we just used — "Theskaesk" is a stutter, not a name.
        let mut tail = closer(rng);
        if tail == last {
            tail = CLOSERS[(CLOSERS.iter().position(|c| *c == tail).unwrap_or(0) + 3) % CLOSERS.len()];
        }
        given.push_str(tail);
    }

    let surname = match family {
        Some(inherited) => inherited.to_string(),
        None => {
            let mut name = String::new();
            name.push_str(opener(rng, 2));
            name.push_str(vowel(rng));
            let first = closer(rng);
            name.push_str(first);
            if rng.chance(0.4) {
                name.push_str(vowel(rng));
                let mut tail = closer(rng);
                if tail == first {
                    tail = CLOSERS[(CLOSERS.iter().position(|c| *c == tail).unwrap_or(0) + 4) % CLOSERS.len()];
                }
                name.push_str(tail);
            }
            capitalised(&name)
        }
    };
    (capitalised(&given), surname)
}

fn capitalised(word: &str) -> String {
    let mut letters = word.chars();
    match letters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + letters.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod people_names {
    use super::*;
    use sim_core::{Domain, WorldSeed};

    fn rng(seed: u128) -> Rng {
        WorldSeed::from_u128(seed).stream(Domain::Naming, 1, 0)
    }

    fn ways(work: f32) -> [f32; WAYS] {
        let mut w = [0.5; WAYS];
        w[Deed::Work as usize] = work;
        w
    }

    #[test]
    fn a_child_carries_its_parents_name() {
        let mut r = rng(1);
        let (_, family) = name_a_person(&ways(0.9), false, None, &mut r);
        let (given, inherited) = name_a_person(&ways(0.9), true, Some(&family), &mut r);
        assert_eq!(inherited, family, "a child was given a stranger's surname");
        assert_ne!(given, family);
    }

    #[test]
    fn different_peoples_sound_different() {
        // The whole reason names come from the culture rather than a list: two peoples
        // who went different ways should not name their children out of one bag.
        let draw = |w: f32| {
            let mut r = rng(2);
            (0..40)
                .map(|_| name_a_person(&ways(w), true, None, &mut r).0)
                .collect::<std::collections::BTreeSet<_>>()
        };
        let one = draw(0.95);
        let other = draw(0.05);
        let shared = one.intersection(&other).count();
        assert!(
            shared * 3 < one.len(),
            "two unlike peoples shared {shared} of {} names",
            one.len()
        );
    }

    #[test]
    fn names_are_names_and_not_titles() {
        // What this replaced produced "Ms. Rosa Wiza MD" and gave "Jr." to women.
        let mut r = rng(3);
        for _ in 0..60 {
            let (given, family) = name_a_person(&ways(0.7), true, None, &mut r);
            for part in [&given, &family] {
                assert!(!part.is_empty());
                assert!(
                    part.chars().all(|c| c.is_ascii_alphabetic()),
                    "{part} is not a plain name"
                );
                assert!(part.chars().next().unwrap().is_uppercase(), "{part}");
            }
        }
    }

    #[test]
    fn the_same_people_and_seed_name_the_same_child() {
        let go = || {
            let mut r = rng(4);
            name_a_person(&ways(0.8), false, None, &mut r)
        };
        assert_eq!(go(), go());
    }
}

/// A people's word for a position in their own society.
///
/// The meaning comes from `bonds::roles` — which social position this is — and the *sound*
/// comes from here, from the same voice that names their children. So the elders of two
/// peoples who diverged in opposite directions are called two different things, and the
/// elders of a people and its daughter are called nearly the same thing, without anybody
/// writing a word list.
///
/// Built by dressing the plain stem in the people's own consonants rather than by inventing
/// a word outright. That keeps it legible — a reader can see that *Bruldsk-elder* is an
/// elder — while still being theirs, and it is honest about what this is: not a language,
/// but a naming habit that differs between peoples and descends with them.
///
/// Deterministic in the people and the position, with no rng and nothing stored. Two calls
/// give the same word, and a people that drifts far enough to sound different has, by then,
/// actually become a different people.
pub fn name_a_role(ways: &[f32; WAYS], stem: &str) -> String {
    let voice = voice_of(ways);
    let opener = OPENERS[voice % OPENERS.len()];
    let vowel = VOWELS[(voice / 11) % VOWELS.len()];
    let closer = CLOSERS[(voice / 5 + stem.len()) % CLOSERS.len()];

    let mut prefix = String::with_capacity(8);
    prefix.push_str(opener);
    prefix.push_str(vowel);
    prefix.push_str(closer);
    let mut word = prefix.chars();
    let head: String = match word.next() {
        Some(first) => first.to_uppercase().collect::<String>() + word.as_str(),
        None => String::new(),
    };
    format!("{head}{}", stem.to_lowercase())
}
