//! Everything there is to know about one person, gathered in one place.
//!
//! This is what the whole project was for: pick anybody at random out of a world of
//! thousands and have the world answer, completely, why they are the way they are and how
//! they came to be where they are. Not a name and a job title — the actual causal chain.
//!
//! Four questions, and the second and fourth are the ones a simulation can answer and a
//! written character cannot:
//!
//! - **Who are they.** Name, age, family, where they live, what kind of place that is.
//! - **Why are they like that.** Every personality factor split into what came down the
//!   genome, what the household did, what the neighbourhood did, and what was nobody's
//!   doing at all. Not an estimate — the decomposition was recorded as the person was
//!   made.
//! - **What has happened to them.** Their life, read out of the chronicle by the index
//!   rather than searched for.
//! - **Why are they doing that.** The scoring table behind their current intent,
//!   including — and this is the part that matters — the options that were *gated off*
//!   rather than merely outscored. "She is not working" and "she cannot work" are
//!   different facts about a life, and only one of them is visible in a list of scores.

use person::{Choice, Deed, Person, PersonId};
use sim::{Happening, World};
use sim_core::{Record, Salience};

/// A complete reading of one person.
pub struct Dossier<'a> {
    pub id: PersonId,
    pub person: &'a Person,
    /// Where they live, and what sort of place it is.
    pub place: Option<Whereabouts>,
    pub kin: Kin,
    /// What made them who they are, factor by factor.
    pub origins: [Attribution; 5],
    /// What they are doing and why, if they are alive and anyone has asked.
    pub intent: Option<Reasoning>,
}

/// Where somebody is, and what that place is like.
pub struct Whereabouts {
    pub name: String,
    pub archetype: &'static str,
    pub affluence: f32,
    pub safety: f32,
    pub opportunity: f32,
    pub schooling: f32,
    /// How many people live there.
    pub residents: usize,
}

/// Who somebody's people are.
pub struct Kin {
    pub parents: Option<(PersonId, PersonId)>,
    pub partner: Option<PersonId>,
    pub children: Vec<PersonId>,
    pub siblings: Vec<PersonId>,
}

/// One personality factor, and where it came from.
///
/// Three contributions, and they add up to the trait. They were recorded as the person
/// was made rather than inferred afterwards, which is the only reason this can be exact:
/// for a real person the same decomposition is a statistical estimate over a population,
/// and here it is a fact about an individual.
pub struct Attribution {
    pub factor: &'static str,
    pub value: f32,
    /// What came down the genome.
    pub genetic: f32,
    /// What the household and the neighbourhood did between them — they are one term
    /// because the model cannot tell them apart, and pretending otherwise would be
    /// inventing a precision it has not got.
    pub upbringing: f32,
    /// What was nobody's doing.
    pub luck: f32,
}

impl Attribution {
    /// Which of the three did the most, in words.
    pub fn chiefly(&self) -> &'static str {
        let mut best = ("what they were born with", self.genetic.abs());
        for (label, share) in [
            ("where they grew up", self.upbringing.abs()),
            ("nothing anyone can name", self.luck.abs()),
        ] {
            if share > best.1 {
                best = (label, share);
            }
        }
        best.0
    }

    /// The same trait, had this person grown up somewhere of a different quality.
    ///
    /// Nearly free and exactly right, because the contributions were never merged: swap
    /// the upbringing term and add it back up. Genes and luck are untouched, which is
    /// precisely why place matters here without being destiny.
    pub fn if_raised(&self, elsewhere: f32, index: usize) -> f32 {
        person::psyche::FACTORS[index]
            .variance()
            .1
            .sqrt()
            .mul_add(elsewhere, self.genetic + self.luck)
    }
}

/// Why somebody is doing what they are doing.
pub struct Reasoning {
    pub doing: Deed,
    /// Every option and its score, best first.
    pub ranked: Vec<(Deed, f32)>,
    /// The options that were never on the table at all.
    ///
    /// Kept apart from the ranking on purpose. "She is not working" and "she cannot work"
    /// are different facts about a life, and a list of scores conflates them — a gated
    /// option scores nothing, and so does an option nobody wants.
    pub gated: Vec<Deed>,
}

/// The five factors, in the order [`person::Origins`] holds them.
const FACTORS: [&str; 5] = [
    "openness",
    "conscientiousness",
    "extraversion",
    "agreeableness",
    "neuroticism",
];

/// Gather everything about one person.
pub fn dossier<'a>(world: &'a World, id: PersonId) -> Option<Dossier<'a>> {
    let person = world.people.get(id)?;

    let place = world.society.place_of(id).and_then(|place_id| {
        let place = world.places.get(place_id)?;
        Some(Whereabouts {
            name: place.name.clone(),
            archetype: place.archetype().label(),
            affluence: place.env.affluence,
            safety: place.env.safety,
            opportunity: place.env.job_opportunity,
            schooling: place.env.education_access,
            residents: world
                .society
                .households_in(place_id)
                .flat_map(|(_, h)| h.members.iter())
                .filter(|m| world.people.get(**m).is_some_and(|p| p.is_alive()))
                .count(),
        })
    });

    // Siblings are the other children of either parent, which is a question about the
    // kinship graph rather than a thing anybody stores.
    let siblings = person
        .parents
        .map(|(mother, _)| {
            world
                .society
                .children_of(mother)
                .iter()
                .copied()
                .filter(|c| *c != id)
                .collect()
        })
        .unwrap_or_default();

    let each = person.origins.each();
    let origins = std::array::from_fn(|i| Attribution {
        factor: FACTORS[i],
        value: each[i].total(),
        genetic: each[i].genetic,
        upbringing: each[i].shared,
        luck: each[i].unique,
    });

    Some(Dossier {
        id,
        person,
        place,
        kin: Kin {
            parents: person.parents,
            partner: world.society.partner_of(id),
            children: world.society.children_of(id).to_vec(),
            siblings,
        },
        origins,
        intent: None,
    })
}

/// Somebody's life, as the chronicle holds it.
pub fn life(
    world: &World,
    id: PersonId,
    at_least: Salience,
) -> impl Iterator<Item = &Record<Happening>> {
    world.life_of(id).filter(move |r| r.salience >= at_least)
}

/// Work out why somebody is about to do what they are about to do.
///
/// Re-runs the scoring rather than remembering it, which keeps the observer read-only:
/// asking a person why they are doing something must not change what they do.
pub fn why(world: &World, id: PersonId) -> Option<Reasoning> {
    let person = world.people.get(id)?;
    if !person.is_alive() {
        return None;
    }
    // The situation they are actually in, not a neutral one. It is the difference
    // between "she ranked work poorly" and "work was never on offer", and the second is
    // the interesting answer.
    let situation = world.situation_for(id)?;
    let scores = person.weigh(world.now(), &situation);
    let choice = Choice {
        deed: person.intent().map(|i| i.deed).unwrap_or(Deed::Wander),
        scores,
    };
    Some(Reasoning {
        doing: choice.deed,
        ranked: choice.ranked(),
        gated: choice.unavailable(),
    })
}

/// Somebody's ancestry, up to a given depth.
///
/// Breadth-first, so it comes out generation by generation, and it stops at whoever the
/// world was founded with.
pub fn ancestry(world: &World, id: PersonId, depth: u8) -> Vec<Vec<PersonId>> {
    let mut generations = Vec::new();
    let mut front = vec![id];
    for _ in 0..depth {
        let mut next = Vec::new();
        for person in &front {
            if let Some(p) = world.people.get(*person)
                && let Some((mother, father)) = p.parents
            {
                next.push(mother);
                next.push(father);
            }
        }
        if next.is_empty() {
            break;
        }
        generations.push(next.clone());
        front = next;
    }
    generations
}

/// Everyone descended from somebody, to a given depth.
pub fn descendants(world: &World, id: PersonId, depth: u8) -> Vec<Vec<PersonId>> {
    let mut generations = Vec::new();
    let mut front = vec![id];
    for _ in 0..depth {
        let mut next = Vec::new();
        for person in &front {
            next.extend(world.society.children_of(*person).iter().copied());
        }
        if next.is_empty() {
            break;
        }
        generations.push(next.clone());
        front = next;
    }
    generations
}
