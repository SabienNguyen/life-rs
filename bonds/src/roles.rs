//! What somebody *is* to the people around them.
//!
//! A society is not a heap of people with different numbers attached. It has positions in
//! it — the one everybody consults, the one everybody owes, the one nobody will stand with
//! — and those positions outlive whoever is holding them. That is the thing this world was
//! missing: it had inequality, and inequality is not the same as structure.
//!
//! ## A role is a reading
//!
//! Nothing here is stored, assigned, conferred or inherited. A role is walked out of what
//! can already be measured about a life, exactly as `Archetype` is walked out of a place's
//! vector, a `Country` out of who can reach whom, and a `Circle` out of who stands with
//! whom. Nobody is made an elder. Somebody is *read as* an elder because they are old, well
//! off, widely stood with, and owed by half the town — and on the day that stops being true
//! of them it is true of somebody else, which is what makes the position survive the person.
//! That is an institution, and it is the cheapest honest one: no office, no succession rule,
//! nothing to keep in step with reality because it *is* reality, re-read.
//!
//! ## Against whom
//!
//! Every quantity is a **rank within the people to hand**, not an absolute. A rich man in a
//! poor village is the patron; the same man among richer neighbours is nobody in particular,
//! and no threshold written here could say so. Rank also makes the reading scale-free, so a
//! hamlet of nine and a town of two hundred are both readable, and immune to the drift in
//! absolute standing that a long run produces.
//!
//! ## What is authored, and what is not
//!
//! The prototypes below are authored, and they are labels over a measured space — the same
//! arrangement §14 uses for neighbourhood archetypes and §13 for outlook. What is *not*
//! authored is which of them anybody is, how many of each a society has, whether it has any
//! at all, or what its people call them. A world with no famine has no creditors and reads
//! nobody as a patron. That is the model working, not a gap in it.

use person::{Deed, Person, PersonId};

use crate::Bonds;

/// Where somebody sits, measured against the people around them.
///
/// Every field is a rank in 0..1: 0 is the bottom of the local order, 1 the top, 0.5 the
/// middle of it. A society in which everybody is identical reads every field at 0.5 for
/// everybody, and every one of them as the ordinary case — which is correct.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Position {
    /// How long they have been at it.
    pub seniority: f32,
    /// What they have.
    pub means: f32,
    /// Net owed to them, in days of help. Low means they are the one in debt.
    pub credit: f32,
    /// How many people stand with them.
    pub allies: f32,
    /// What others hold about them — the sum of everybody's regard, which is the only
    /// quantity in this world that travels between people who have never met.
    pub repute: f32,
    /// The share of their life given to work.
    pub industry: f32,
    /// The share given to company.
    pub sociability: f32,
    /// The share given to wandering.
    pub roving: f32,
}

impl Position {
    /// The middle of every order: somebody exactly like everybody else.
    pub const ORDINARY: Position = Position {
        seniority: 0.5,
        means: 0.5,
        credit: 0.5,
        allies: 0.5,
        repute: 0.5,
        industry: 0.5,
        sociability: 0.5,
        roving: 0.5,
    };

    fn as_array(&self) -> [f32; 8] {
        [
            self.seniority,
            self.means,
            self.credit,
            self.allies,
            self.repute,
            self.industry,
            self.sociability,
            self.roving,
        ]
    }

    fn apart_from(&self, other: &Position) -> f32 {
        self.as_array()
            .iter()
            .zip(other.as_array())
            .map(|(a, b)| (a - b) * (a - b))
            .sum::<f32>()
            .sqrt()
    }
}

/// A position in a society, as the society would recognise it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Role {
    /// Old, comfortable, widely stood with, and owed by half the town.
    Elder,
    /// Not old, but the one who carries people and is owed for it.
    Patron,
    /// Carried, and has not made it good.
    Client,
    /// Always with somebody. Knows everyone, holds little.
    Broker,
    /// Spends the days working and not much else.
    Labourer,
    /// Away more than anybody. Knows the country better than the town.
    Rover,
    /// Widely thought poorly of, and nobody will stand with them.
    Outcast,
    /// The ordinary case, and by far the commonest. A household getting on with it.
    Householder,
}

impl Role {
    pub const ALL: [Role; 8] = [
        Role::Elder,
        Role::Patron,
        Role::Client,
        Role::Broker,
        Role::Labourer,
        Role::Rover,
        Role::Outcast,
        Role::Householder,
    ];

    pub const COUNT: usize = Role::ALL.len();

    /// What the position is, in a word, before any people has its own word for it.
    ///
    /// This is the *meaning*; `culture::naming::name_a_role` turns it into a word in a
    /// particular people's mouth. Kept apart so that `culture` never has to know what a
    /// social position is, and this module never has to know what a language sounds like.
    pub const fn stem(self) -> &'static str {
        match self {
            Role::Elder => "Elder",
            Role::Patron => "Keeper",
            Role::Client => "Bound",
            Role::Broker => "Speaker",
            Role::Labourer => "Hand",
            Role::Rover => "Farer",
            Role::Outcast => "Shunned",
            Role::Householder => "Steader",
        }
    }

    /// The plain English label, for a reader with no people of their own.
    pub const fn label(self) -> &'static str {
        match self {
            Role::Elder => "elder",
            Role::Patron => "patron",
            Role::Client => "client",
            Role::Broker => "go-between",
            Role::Labourer => "labourer",
            Role::Rover => "rover",
            Role::Outcast => "outcast",
            Role::Householder => "householder",
        }
    }

    /// Whether somebody could be this at all, whatever else is true of them.
    ///
    /// Nearest-prototype on its own gave a patron with nobody owing her anything: she had
    /// the most of everybody, and that was enough to put her in the corner of the space
    /// where patrons live even though the relation that makes somebody a patron did not
    /// exist. A position is not a location in a space of measurements — it is a *relation*,
    /// and the relation either holds or it does not. So the sign of the thing is checked
    /// first, and only then is proximity used to choose among the positions somebody could
    /// actually occupy.
    ///
    /// `Householder` qualifies always, which is what makes it the fallback rather than a
    /// prototype that has to win on distance.
    pub fn open_to(self, position: &Position, credit_days: f32, repute: f32, allies: usize) -> bool {
        match self {
            // Old, and owed. An elder who is owed nothing by anybody is a pensioner.
            Role::Elder => {
                position.seniority >= 0.75 && credit_days > 0.0 && position.credit >= 0.7 && allies >= 1
            }
            // Owed enough to be marked out by it. In a society where a bad year is settled
            // between neighbours nearly everybody is owed *something*, so being a creditor
            // is not a position — being one of the town's creditors is.
            Role::Patron => credit_days > 0.0 && position.credit >= 0.75 && allies >= 1,
            // Somebody has to be on the other end of it — and being in debt is not a
            // position if you are comfortable, it is just a debt. A rank threshold was tried
            // here and it was wrong for a reason worth keeping: in a town where five people
            // owe one, the five *share* the bottom ranks and none of them is at the bottom,
            // so who counted as a client depended on how many others happened to be in debt
            // rather than on anything about them.
            Role::Client => credit_days < 0.0 && position.means <= 0.5,
            Role::Broker => allies >= 2 && position.allies >= 0.5 && position.sociability >= 0.5,
            Role::Labourer => position.industry >= 0.5,
            Role::Rover => position.roving >= 0.5,
            // Not merely poor: actually thought poorly of, which only happens to somebody
            // who was carried and did not make it good.
            // Not merely below zero — in a society where nearly everybody owes somebody
            // something they cannot repay, nearly everybody's regard is below zero and the
            // sign says nothing. Being shunned is being thought worse of *than the rest*.
            Role::Outcast => repute < 0.0 && position.repute <= 0.15,
            Role::Householder => true,
        }
    }

    /// The corner of the measured space this position sits in.
    ///
    /// Authored, and a label over a measurement rather than a fact about anybody — the same
    /// arrangement as `Archetype::prototype`. Read the fields as "what would have to be true
    /// of somebody for the town to call them this".
    pub fn prototype(self) -> Position {
        let p = Position::ORDINARY;
        match self {
            // Old, comfortable, stood with, and owed by half the town.
            // No claim about what an elder does with their days. There was one — that they
            // work less than others — and it was doing more work in the arithmetic than
            // every claim about age and credit put together, because it was the only field
            // on which the two nearby prototypes disagreed. A prototype should assert what
            // it means and stay silent about everything else, or the silence gets a vote.
            Role::Elder => Position {
                seniority: 1.0,
                means: 0.85,
                credit: 0.85,
                allies: 0.8,
                repute: 0.8,
                ..p
            },
            // The same weight without the years. A patron is somebody who can carry others,
            // and the mark of it is that others owe them.
            Role::Patron => Position {
                means: 0.95,
                credit: 0.95,
                allies: 0.7,
                repute: 0.85,
                ..p
            },
            // The other end of the same relation. Somebody has to be on it.
            Role::Client => Position {
                seniority: 0.4,
                means: 0.15,
                credit: 0.05,
                allies: 0.45,
                repute: 0.4,
                industry: 0.6,
                ..p
            },
            // Knows everybody, holds nothing. The one who carries word between circles.
            Role::Broker => Position {
                means: 0.4,
                allies: 0.95,
                repute: 0.7,
                sociability: 0.95,
                industry: 0.25,
                ..p
            },
            Role::Labourer => Position {
                means: 0.35,
                credit: 0.4,
                allies: 0.3,
                industry: 0.95,
                sociability: 0.15,
                ..p
            },
            Role::Rover => Position {
                means: 0.35,
                allies: 0.2,
                roving: 0.95,
                sociability: 0.3,
                ..p
            },
            // Thought poorly of, stood with by nobody, and owing what they were given.
            Role::Outcast => Position {
                means: 0.1,
                credit: 0.1,
                allies: 0.03,
                repute: 0.03,
                ..p
            },
            Role::Householder => p,
        }
    }
}

/// What a role is read from, for one person — the facts a caller has and this crate does not.
pub struct Facts<'a> {
    pub who: PersonId,
    pub person: &'a Person,
    /// Their age in years. Passed rather than computed, because only the caller knows when
    /// "now" is.
    pub age: f64,
}

/// Read everybody's position in the society they are actually in.
///
/// One call for the whole group rather than one per person, because a rank is not a property
/// of a person: it only exists against everybody else, and computing it one at a time would
/// mean either recomputing the group each time or quietly comparing people against a group
/// that has changed underneath them.
pub fn among(bonds: &Bonds, people: &[Facts]) -> Vec<(PersonId, Position, Role)> {
    if people.is_empty() {
        return Vec::new();
    }

    let raw: Vec<[f32; 8]> = people
        .iter()
        .map(|f| {
            let ties: Vec<(PersonId, crate::Tie)> = bonds.of(f.who).collect();
            let allies = ties.iter().filter(|(_, t)| t.allied()).count() as f32;
            let credit: f32 = ties.iter().map(|(_, t)| t.debt).sum();
            // What everybody else holds about them, which needs the other direction of the
            // graph — the one nobody stores. Walked here because a reputation is precisely
            // the thing you cannot read off your own ties. The same measure the world uses
            // to decide whether a place will have somebody, so that the position read and
            // the door closed are two views of one fact rather than two facts.
            let repute = bonds.repute_of(f.who);
            [
                f.age as f32,
                // What they *attained*, not what they hold today. Standing decays in old
                // age, so reading position off the current figure made every elder poor by
                // construction and a society with elders in it impossible.
                f.person.peak_standing(),
                credit,
                allies,
                repute,
                f.person.share_of_life(Deed::Work),
                f.person.share_of_life(Deed::Socialize),
                f.person.share_of_life(Deed::Wander),
            ]
        })
        .collect();

    let ranked = rank_columns(&raw);
    ranked
        .into_iter()
        .enumerate()
        .map(|(at, row)| {
            let position = Position {
                seniority: row[0],
                means: row[1],
                credit: row[2],
                allies: row[3],
                repute: row[4],
                industry: row[5],
                sociability: row[6],
                roving: row[7],
            };
            let (credit, repute, allies) = (raw[at][2], raw[at][4], raw[at][3] as usize);
            let role = Role::ALL
                .into_iter()
                .filter(|role| role.open_to(&position, credit, repute, allies))
                .min_by(|a, b| {
                    position
                        .apart_from(&a.prototype())
                        .total_cmp(&position.apart_from(&b.prototype()))
                })
                .unwrap_or(Role::Householder);
            (people[at].who, position, role)
        })
        .collect()
}

/// Turn each column into a rank in 0..1, ties sharing the average rank.
///
/// Ranks rather than z-scores, because these distributions are not normal and some of them
/// are barely continuous — most people are owed nothing at all, so a z-score on credit is
/// dominated by whether the denominator happened to be small. A rank says what it means: how
/// far up the local order somebody is.
fn rank_columns(raw: &[[f32; 8]]) -> Vec<[f32; 8]> {
    let n = raw.len();
    let mut out = vec![[0.5; 8]; n];
    if n < 2 {
        return out;
    }
    let mut order: Vec<usize> = (0..n).collect();
    for column in 0..8 {
        order.sort_by(|a, b| raw[*a][column].total_cmp(&raw[*b][column]).then(a.cmp(b)));
        let mut at = 0;
        while at < n {
            // Everybody on the same value shares the middle of the places they take up, so
            // that a town where nobody is owed anything does not read one of them as the
            // richest creditor alive.
            let mut end = at + 1;
            while end < n && raw[order[end]][column] == raw[order[at]][column] {
                end += 1;
            }
            let share = ((at + end - 1) as f32 / 2.0) / (n - 1) as f32;
            for slot in &order[at..end] {
                out[*slot][column] = share;
            }
            at = end;
        }
    }
    out
}
