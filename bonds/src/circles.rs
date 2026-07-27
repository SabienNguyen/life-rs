//! Who stands with whom.
//!
//! A circle is not a thing anybody joins. It is a **reading**: the largest sets of people
//! who are warm to each other, walked out of the tie graph every time somebody asks. That
//! is the same rule `culture` applies to countries and §14 applies to a place's character,
//! and for the same reason — a faction that were stored could fall out of step with the
//! people in it, and would then need code to keep it honest.
//!
//! Circles are what make this politics rather than sociology. Once people stand together,
//! scarcity has to be settled between *groups* and not between individuals: when a
//! settlement is full, the household that gets in is the one with weight behind it, and
//! weight is your own standing plus what your allies will lend you. No violence is modelled
//! and none is needed — the whole of it is that there is not enough good land and some
//! people have friends.

use person::PersonId;

use crate::Bonds;

/// A set of people who stand together.
#[derive(Clone, Debug, PartialEq)]
pub struct Circle {
    /// Everybody in it, in a fixed order.
    pub members: Vec<PersonId>,
    /// The sum of what they think of each other — how tightly it holds.
    pub cohesion: f32,
}

/// How small a group can be and still be worth calling a circle.
///
/// Two people who like each other are a friendship. Three is the smallest number that can
/// take a side, exclude somebody, or outvote one of its own — which is what makes it a
/// political object rather than an affection.
pub const FEWEST_TO_STAND: usize = 3;

/// The circles among a set of people, largest first.
///
/// Mutual warmth, not one-sided: a hanger-on is not in the circle he would like to be in.
/// That asymmetry matters, because the whole reason ties are directed is that unrequited
/// regard is the ordinary case.
///
/// **Everyone with everyone, not everyone-reachable-from-everyone.** This began as a flood
/// fill — a circle was a connected component of mutual warmth, on the argument that a chain
/// of friendships is one faction even where the ends have never met. Measuring it settled
/// the question: at a mean of three or four allies apiece the ally graph is far above the
/// percolation threshold, so the flood fill returned one blob holding two thirds of the
/// town, every time, in every world. A faction with most of the population in it is not a
/// faction. So a circle is a set in which *every* pair stands together, which cannot
/// percolate, and which is what the description above actually said all along.
///
/// The finding that survives is worth keeping in view: mutual affection alone does not
/// divide a society into camps, because liking is not transitive but *reachability* is. Real
/// factions form around something to be against, and there is nothing here for anybody to
/// be against — see §25 for what that would take.
///
/// Circles overlap, and are meant to: people belong to several, and a model in which each
/// person has exactly one faction would be a caste system.
pub fn circles(bonds: &Bonds, among: &[PersonId]) -> Vec<Circle> {
    let present: std::collections::BTreeSet<PersonId> = among.iter().copied().collect();
    let stands_with =
        |a: PersonId, b: PersonId| bonds.tie(a, b).allied() && bonds.tie(b, a).allied();

    let mut found: Vec<Circle> = Vec::new();
    let mut seen: std::collections::BTreeSet<Vec<PersonId>> = std::collections::BTreeSet::new();

    for start in among {
        // Everybody this person stands with, warmest first, so a circle grows around the
        // strongest attachments rather than around whoever happens to sort first.
        let mut allies: Vec<(PersonId, f32)> = bonds
            .of(*start)
            .filter(|(other, _)| present.contains(other) && stands_with(*start, *other))
            .map(|(other, tie)| (other, tie.warmth))
            .collect();
        allies.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));

        let mut members = vec![*start];
        for (candidate, _) in allies {
            if members.iter().all(|held| stands_with(*held, candidate)) {
                members.push(candidate);
            }
        }
        if members.len() < FEWEST_TO_STAND {
            continue;
        }
        members.sort_unstable();
        // The same set reached from two of its own members is one circle, not two.
        if !seen.insert(members.clone()) {
            continue;
        }

        let mut cohesion = 0.0;
        for a in &members {
            for b in &members {
                if a != b {
                    cohesion += bonds.tie(*a, *b).warmth.max(0.0);
                }
            }
        }
        found.push(Circle {
            cohesion: cohesion / members.len() as f32,
            members,
        });
    }

    found.sort_by(|a, b| {
        b.members
            .len()
            .cmp(&a.members.len())
            .then(b.cohesion.total_cmp(&a.cohesion))
            .then(a.members.cmp(&b.members))
    });
    found
}

/// What somebody can bring to bear: their own standing, plus what their allies lend.
///
/// An ally lends in proportion to how warmly they hold you and how much they have — which
/// is why a poor man with rich friends outweighs a rich man with none, and why the way to
/// get on is to be liked by somebody who is already getting on.
///
/// Lent at a discount, because backing somebody is not the same as being them. Without the
/// discount a large enough circle makes every one of its members unbeatable, and the model
/// stops being about scarcity and starts being about headcount.
pub const LENT: f32 = 0.35;

pub fn standing_with_allies(
    bonds: &Bonds,
    who: PersonId,
    own: f32,
    standing_of: &dyn Fn(PersonId) -> Option<f32>,
) -> f32 {
    let mut weight = own;
    for (ally, tie) in bonds.of(who) {
        if !tie.allied() {
            continue;
        }
        if let Some(theirs) = standing_of(ally) {
            weight += LENT * tie.warmth * theirs;
        }
    }
    weight
}
