//! What people are to each other.
//!
//! Before this, a person could be paired with somebody, born to somebody, and live in a
//! household with somebody — and that was the whole of it. Everything else was aggregate:
//! you did not interact with *people*, you interacted with a statistic of your neighbours.
//! Two things make that plain. `Deed::Socialize` relieved the social need and named nobody,
//! so people in this world socialised alone. And `seek_patron` — the single largest fact
//! about a life here, worth more of the variance in attainment than every other input
//! combined — was a coin flip against local bonding capital with **no patron in it**. There
//! was no mentor. There was a multiplier.
//!
//! ## Four numbers, one direction
//!
//! A tie runs *from* somebody *to* somebody and carries what one of them holds about the
//! other. It has to be directed: unrequited regard is the ordinary case, and a model where
//! liking is always mutual cannot express a hanger-on, a patron, or a grudge somebody else
//! has forgotten.
//!
//! - **warmth** — do I like you. Moves with how well we suit each other, and sours when
//!   what I am owed goes unpaid.
//! - **regard** — do I rate you. This is the one that travels: opinions move between people
//!   who talk, which is gossip with no words in it.
//! - **debt** — signed, in days of help. Positive means you owe me.
//! - **known** — how familiar we are. Decays without contact, and gates everything else,
//!   because you cannot fall out with somebody you have never met.
//!
//! ## What is deliberately not here
//!
//! No language, no lies, no promises, and no violence. Everything below is reciprocity and
//! third parties, which is enough for friendship, exploitation, grudges, reputation and
//! faction — and none of it needs a word to be spoken. A faction here is what a country is
//! in `culture`: a **reading**, computed from who holds what about whom, and never stored.

use person::{Personality, PersonId};
use sim_core::Rng;

pub mod circles;
pub use circles::{Circle, standing_with_allies};

/// How many ties one person keeps.
///
/// The sympathy group rather than Dunbar's hundred and fifty. The larger number is the
/// people you can recognise and place; this is the smaller layer that actually does the
/// work of a social life — the ones you would notice missing. Keeping the acquaintance
/// layer would multiply the cost tenfold to model people who barely affect each other.
pub const CLOSE_TIES: usize = 20;

/// How fast familiarity grows with meeting, and fades without it.
///
/// A tie that nobody tends is gone in a handful of years, which is what makes moving away
/// cost something and what makes a tight place tight.
const MEETING: f32 = 0.16;
const FORGETTING: f32 = 0.22;

/// How fast liking follows from suiting each other.
const WARMING: f32 = 0.14;

/// How fast an obligation fades on its own.
///
/// Slower than familiarity. Being owed outlasts being close, which is why old debts between
/// people who have drifted apart are the ones that turn sour.
const FORGIVING: f32 = 0.12;

/// How much unpaid debt costs the debtor's standing in the creditor's eyes, per day owed.
///
/// This is the whole of reciprocity: help given and not returned turns warmth negative, and
/// that single rule is what makes cooperation with strangers rare and cooperation with
/// neighbours ordinary, without either being written down.
const RESENTMENT: f32 = 0.30;

/// How much of somebody's opinion rubs off when you spend time with them.
///
/// Weighted by how much you like them, so an opinion travels along warm ties and stalls at
/// cold ones. Small, because a reputation that could be rewritten in one conversation would
/// be noise rather than a reputation.
const HEARSAY: f32 = 0.06;

/// How well you can come to know somebody you have never met.
///
/// Hearing about a person makes you aware of them and gives you an opinion; it cannot make
/// you close. Without this ceiling a decade of ordinary gossip left everybody in a town as
/// familiar to each other as lifelong friends, which both flattens the society and costs
/// the square of its population to carry.
const HEARD_OF: f32 = 0.25;

/// What one person holds about another.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tie {
    /// Do I like you, from loathing at −1 to devotion at 1.
    pub warmth: f32,
    /// Do I rate you. Travels between people who meet — see `hearsay`.
    pub regard: f32,
    /// Days of help owed. Positive means you owe me.
    pub debt: f32,
    /// How well I know you, 0 to 1. Everything else is gated on this.
    pub known: f32,
}

impl Tie {
    pub const STRANGERS: Tie = Tie {
        warmth: 0.0,
        regard: 0.0,
        debt: 0.0,
        known: 0.0,
    };

    /// Whether this is a tie at all, or the memory of one.
    pub fn holds(&self) -> bool {
        self.known > 0.04
    }

    /// Somebody you would call an ally: known, liked, and not in your debt.
    pub fn allied(&self) -> bool {
        self.known > 0.3 && self.warmth > 0.25
    }
}

/// Everybody's ties, in one place.
///
/// Directed and sparse: what each holder holds, about whom. Nested rather than keyed by a
/// pair, so "everything this person holds" is a lookup rather than a range scan — which it
/// has to be, because that is the question asked on every decision. Ordered rather than
/// hashed, because circles are read off this by walking it and the walk must not depend on
/// where a hash happened to put somebody.
#[derive(Default)]
pub struct Bonds {
    ties: std::collections::BTreeMap<PersonId, std::collections::BTreeMap<PersonId, Tie>>,
}

impl Bonds {
    pub fn new() -> Bonds {
        Bonds::default()
    }

    /// How many ties are held in all.
    pub fn len(&self) -> usize {
        self.ties.values().map(|held| held.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// What `holder` holds about `subject`.
    pub fn tie(&self, holder: PersonId, subject: PersonId) -> Tie {
        self.ties
            .get(&holder)
            .and_then(|held| held.get(&subject))
            .copied()
            .unwrap_or(Tie::STRANGERS)
    }

    /// Everybody `holder` has a tie to, and what they hold.
    pub fn of(&self, holder: PersonId) -> impl Iterator<Item = (PersonId, Tie)> + '_ {
        self.ties
            .get(&holder)
            .into_iter()
            .flat_map(|held| held.iter().map(|(subject, tie)| (*subject, *tie)))
    }

    /// How many ties somebody is carrying.
    pub fn count(&self, holder: PersonId) -> usize {
        self.ties.get(&holder).map_or(0, |held| held.len())
    }

    fn edit(&mut self, holder: PersonId, subject: PersonId) -> &mut Tie {
        self.ties
            .entry(holder)
            .or_default()
            .entry(subject)
            .or_insert(Tie::STRANGERS)
    }

    /// Two people spend time together.
    ///
    /// `suits` is how well their temperaments go together, which `person` already computes
    /// for pairing — the same number, because liking somebody and being able to live with
    /// them are not different faculties.
    pub fn meet(&mut self, a: PersonId, b: PersonId, suits: f32) {
        self.meet_repeatedly(a, b, suits, 1);
    }

    /// A season's worth of evenings, in one call.
    ///
    /// The same arithmetic as `meet` applied `times` over, and *exactly* the same — the
    /// loop is over two floats rather than over the map, so a hundred meetings cost one
    /// lookup and a hundred multiplies. This is what lets the coarse tier hold a society
    /// at all: unwatched people cannot spend a deed each on an evening, but their ties
    /// have to advance as if they had, or looking away would quietly dissolve everybody's
    /// friendships and looking back would rebuild them from nothing.
    pub fn meet_repeatedly(&mut self, a: PersonId, b: PersonId, suits: f32, times: u32) {
        if a == b || times == 0 {
            return;
        }
        // Warmth follows how well they suit each other, but only as fast as they actually
        // know each other: first impressions are weak on purpose.
        let target = (suits * 2.0 - 1.0).clamp(-1.0, 1.0);
        for (holder, subject) in [(a, b), (b, a)] {
            let tie = self.edit(holder, subject);
            let (mut known, mut warmth) = (tie.known, tie.warmth);
            for _ in 0..times {
                known = (known + MEETING * (1.0 - known)).clamp(0.0, 1.0);
                warmth = (warmth + WARMING * known * (target - warmth)).clamp(-1.0, 1.0);
            }
            tie.known = known;
            tie.warmth = warmth;
        }
    }

    /// One person does something for another, at a cost to themselves.
    ///
    /// `days` is what it cost. The receiver owes it; the giver is owed it. Nothing here
    /// decides whether it will ever be repaid — that is what `year` is for, and what makes
    /// this reciprocity rather than charity.
    pub fn helped(&mut self, giver: PersonId, taker: PersonId, days: f32) {
        if giver == taker || days <= 0.0 {
            return;
        }
        self.edit(giver, taker).debt += days;
        let owed = self.edit(taker, giver);
        owed.debt -= days;
        // Being helped is warming on its own, whatever happens to the debt afterwards.
        owed.warmth = (owed.warmth + 0.05 * days).clamp(-1.0, 1.0);
    }

    /// Somebody settles what they owe.
    pub fn repaid(&mut self, debtor: PersonId, creditor: PersonId, days: f32) {
        if debtor == creditor || days <= 0.0 {
            return;
        }
        let theirs = self.edit(creditor, debtor);
        theirs.debt = (theirs.debt - days).max(0.0);
        // Paying up raises what the creditor thinks of you, which is the point of paying.
        theirs.regard = (theirs.regard + 0.08 * days).clamp(-1.0, 1.0);
        let mine = self.edit(debtor, creditor);
        mine.debt = (mine.debt + days).min(0.0);
    }

    /// What `listener` comes to think of everybody, after time spent with `speaker`.
    ///
    /// Gossip, with no words in it. The listener's regard for each third party drifts
    /// towards the speaker's, in proportion to how warmly the listener regards the speaker
    /// — so opinions travel along friendships and stop dead at strangers. This is the rule
    /// that turns a heap of pairwise ties into a society with a shared view of who is who.
    pub fn hearsay(&mut self, listener: PersonId, speaker: PersonId) {
        self.hearsay_repeatedly(listener, speaker, 1);
    }

    /// A season of talk, in one call — the coarse tier's version, as `meet_repeatedly` is
    /// of `meet`. Opinions converge rather than compound, so repeating is a step towards
    /// the speaker's view and not a multiple of it.
    pub fn hearsay_repeatedly(&mut self, listener: PersonId, speaker: PersonId, times: u32) {
        let trust = self.tie(listener, speaker);
        if times == 0 || !trust.holds() || trust.warmth <= 0.0 {
            return;
        }
        let weight = HEARSAY * trust.warmth * trust.known;
        // Only what the speaker knows first hand. People pass on what they think of the
        // people they actually know, not the whole of what they have ever heard — and
        // without that restriction a rumour crosses a town in a season and the cost of an
        // evening's talk grows with the population rather than with Dunbar.
        let theirs: Vec<(PersonId, f32)> = self
            .of(speaker)
            .filter(|(about, tie)| *about != listener && tie.known > HEARD_OF)
            .map(|(about, tie)| (about, tie.regard))
            .collect();
        for (about, said) in theirs {
            let mine = self.edit(listener, about);
            let (mut known, mut regard) = (mine.known, mine.regard);
            for _ in 0..times {
                // You cannot have an opinion of somebody you have never heard of, so
                // hearing about them is itself a little bit of knowing them — up to the
                // point where knowing *of* somebody would become knowing them.
                known += weight * 0.5 * (HEARD_OF - known).max(0.0);
                regard = (regard + weight * (said - regard)).clamp(-1.0, 1.0);
            }
            mine.known = known;
            mine.regard = regard;
        }
    }

    /// A year of ties fading, debts ageing, and patience running out.
    ///
    /// `alive` says who is still here; ties to the dead are dropped, and so are ties too
    /// faint to matter, which is what keeps this sparse without a cap that would have to
    /// choose whom to forget.
    pub fn year(&mut self, alive: &dyn Fn(PersonId) -> bool) {
        self.ties.retain(|holder, _| alive(*holder));
        for (_, held) in self.ties.iter_mut() {
            held.retain(|subject, _| alive(*subject));
            for (_, tie) in held.iter_mut() {
                tie.known *= 1.0 - FORGETTING;
                // Warmth drifts back towards indifference as people fall out of touch. Not
                // to zero — old friends who have not met in years are not strangers — but
                // towards it, in proportion to how far the familiarity has gone.
                tie.warmth *= 0.97;
                tie.regard *= 0.98;

                // What is owed fades, and what is owed *and not paid* sours. The creditor
                // is the one who resents: a debt is only a grievance from the side that is
                // out of pocket.
                if tie.debt > 0.0 {
                    let unpaid = tie.debt;
                    tie.debt = (tie.debt - FORGIVING * tie.debt).max(0.0);
                    tie.warmth = (tie.warmth - RESENTMENT * unpaid * 0.01).clamp(-1.0, 1.0);
                    tie.regard = (tie.regard - RESENTMENT * unpaid * 0.005).clamp(-1.0, 1.0);
                } else if tie.debt < 0.0 {
                    tie.debt = (tie.debt + FORGIVING * -tie.debt).min(0.0);
                }
            }
            // A tie nobody has tended is not a faint tie, it is not a tie. Dropping them
            // is what keeps this sparse without a cap that would have to choose whom to
            // forget on somebody's behalf.
            held.retain(|_, tie| tie.holds());
        }
        self.ties.retain(|_, held| !held.is_empty());
    }

    /// Drop everything anybody held about somebody who is gone.
    pub fn forget(&mut self, who: PersonId) {
        self.ties.remove(&who);
        for held in self.ties.values_mut() {
            held.remove(&who);
        }
        self.ties.retain(|_, held| !held.is_empty());
    }

    /// Whom to spend an evening with, out of those to hand.
    ///
    /// Weighted by how well you know them and how much you like them, with a floor so that
    /// strangers are met at all — a society where nobody ever talks to somebody new has no
    /// way to grow a tie in the first place. Friends of friends are favoured, which is what
    /// makes cliques form rather than everybody knowing everybody equally.
    ///
    /// `to_hand` is meant to be a handful, not a census. The friend-of-a-friend term costs
    /// a walk of the chooser's own ties per candidate, which Dunbar bounds — but nothing
    /// bounds how many people live in a town, and nobody surveys the whole town before
    /// deciding who to spend an evening with. The caller samples; this picks.
    pub fn choose_company(
        &self,
        who: PersonId,
        to_hand: &[PersonId],
        rng: &mut Rng,
    ) -> Option<PersonId> {
        let mut best: Option<(PersonId, f32)> = None;
        let mut total = 0.0;
        for other in to_hand {
            if *other == who {
                continue;
            }
            let tie = self.tie(who, *other);
            // A stranger is worth meeting; a friend is worth more; a friend of a friend
            // sits between, which is triadic closure and is why groups close into circles.
            let mutual = self
                .of(who)
                .filter(|(_, mine)| mine.allied())
                .filter(|(friend, _)| self.tie(*friend, *other).allied())
                .count() as f32;
            let want = 0.12
                + tie.known * (1.0 + tie.warmth).max(0.0)
                + 0.25 * mutual.min(4.0);
            total += want;
            let roll = rng.unit_f32() * total;
            if best.is_none() || roll >= total - want {
                best = Some((*other, want));
            }
        }
        best.map(|(who, _)| who)
    }
}

/// How well two temperaments suit each other, 0 to 1.
///
/// The same measure `person` uses to pair people off, because getting on with somebody and
/// being able to live with them are not separate faculties. Similarity, mostly — which is
/// what the assortment literature finds and what makes friendship groups resemble each
/// other rather than complement each other.
pub fn suits(a: &Personality, b: &Personality) -> f32 {
    let gap = (a.openness - b.openness).abs()
        + (a.conscientiousness - b.conscientiousness).abs()
        + (a.extraversion - b.extraversion).abs()
        + (a.agreeableness - b.agreeableness).abs()
        + (a.neuroticism - b.neuroticism).abs();
    // Five factors, each a z-score; a gap of ten is about as unlike as two people get.
    (1.0 - gap / 10.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests;
