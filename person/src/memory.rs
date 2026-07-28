//! What somebody carries of their own life.
//!
//! The chronicle (§16) is the world's record and it is complete, ordered and external.
//! This is not that. This is what one person has, which is partial, decaying, and about
//! *whom* — because a grudge is a remembered wrong against a name, and reputation earned
//! rather than assigned needs somebody to have seen something and kept it.
//!
//! # Why the strength curve is not an exponential
//!
//! The obvious model is a half-life, and it is wrong for a reason worth stating. Under
//! exponential decay every memory vanishes on the same timescale — changing the weight only
//! shifts *when* it disappears, never whether there is a long tail. You cannot get "she half
//! remembers her mother's death at seventy" and "he had forgotten that slight by spring" out
//! of one exponential rate; you need two, and then something has to decide which memories are
//! special, which is a stored flag and exactly what this project spends its time removing.
//!
//! `weight / (1 + age/SPAN)` gives both from one rule. It falls fast at first and then very
//! slowly, so a thing that landed hard is still faintly present decades later while a small
//! thing is gone within a year or two. **Permanence stops being a flag and becomes a
//! consequence of the curve** — §26.1's discipline about positions, applied to time. It is
//! also closer to what the forgetting literature actually measures than an exponential is.
//!
//! # Rehearsal, because proximity is what keeps a grudge sharp
//!
//! Meeting somebody again refreshes what you hold about them. So distance forgives and
//! nearness does not: the widow who moved away softens, the brother across the square does
//! not. That is one line, and it is the mechanism rather than a special case — retrieval
//! strengthening a trace is the same thing that makes a rehearsed fact stick.
//!
//! # Capacity, so that forgetting competes
//!
//! A bounded number of memories, evicting whichever is currently faintest. A crowded life
//! forgets more than a quiet one, which is true and comes for free.

use crate::PersonId;
use sim_core::Time;

/// How many things one person can hold at once.
///
/// Small on purpose. This is what somebody carries about their own life, not a log — and a
/// bound is what makes forgetting *competitive*, so that a life full of incident loses more
/// of it than a quiet one.
pub const HELD: usize = 24;

/// The age at which a memory has faded to half of what it was.
///
/// Not a half-life: the curve is hyperbolic, so this is the point where strength halves and
/// the decay from there is far slower than an exponential's. Eight years is roughly where a
/// slight stops stinging and a death does not.
const SPAN_YEARS: f32 = 8.0;

/// What a memory is about, and to whom.
///
/// Deliberately coarse. Storing the whole `Happening` would make a person's memory a second
/// chronicle, and what matters for how somebody treats another person is the *kind* of thing
/// and who did it — not its every particular.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum What {
    /// Somebody was born to them.
    Born,
    /// Somebody they knew died.
    Died,
    /// They set up house with somebody.
    Paired,
    /// Somebody opened a door for them, or they opened one.
    TakenUp,
    /// They were carried through a year they could not feed themselves.
    Carried,
    /// Somebody took what they had.
    Robbed,
    /// They worked something out.
    WorkedItOut,
    /// They moved.
    Moved,
}

impl What {
    /// How hard this lands when it happens, before any decay.
    ///
    /// The whole ordering matters more than the numbers: what a life is *about* should
    /// outlast what merely happened in it.
    pub fn weight(self) -> f32 {
        match self {
            What::Died => 1.0,
            What::TakenUp => 0.9,
            What::Robbed => 0.9,
            What::Paired => 0.8,
            What::Born => 0.7,
            What::Carried => 0.6,
            What::WorkedItOut => 0.5,
            What::Moved => 0.25,
        }
    }
}

/// One thing somebody carries.
#[derive(Clone, Copy, Debug)]
pub struct Memory {
    pub what: What,
    /// Who it was about, when it was about anybody. This is the field that makes a grudge
    /// possible: without a name, a wrong is only a mood.
    pub who: Option<PersonId>,
    /// When it happened — or, after a rehearsal, when it was last brought back up.
    pub since: Time,
    /// How hard it landed. Repetition adds to this rather than making a second memory,
    /// because being robbed twice by the same man is one fact about him, felt harder.
    pub weight: f32,
}

impl Memory {
    /// What is left of it now.
    pub fn strength(&self, now: Time) -> f32 {
        let age = now.since(self.since).as_years().max(0.0) as f32;
        self.weight / (1.0 + age / SPAN_YEARS)
    }
}

/// Everything one person still holds.
#[derive(Clone, Debug, Default)]
pub struct Held {
    kept: Vec<Memory>,
}

impl PartialEq for Held {
    /// Two people are the same person or not regardless of what either remembers, so this is
    /// deliberately trivial: `Person` derives `PartialEq` for identity and tests, and what
    /// somebody carries is state about their life rather than a fact about who they are.
    fn eq(&self, _: &Held) -> bool {
        true
    }
}

impl Held {
    pub fn is_empty(&self) -> bool {
        self.kept.is_empty()
    }

    pub fn len(&self) -> usize {
        self.kept.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Memory> {
        self.kept.iter()
    }

    /// Take something in.
    ///
    /// The same thing happening again with the same person deepens what is there rather than
    /// adding a second entry, and brings it back to the present — which is the rehearsal that
    /// keeps a live grievance live.
    pub fn keep(&mut self, what: What, who: Option<PersonId>, now: Time) {
        if let Some(already) = self
            .kept
            .iter_mut()
            .find(|m| m.what == what && m.who == who)
        {
            already.weight = (already.weight + what.weight() * 0.5).min(2.0);
            already.since = now;
            return;
        }
        self.kept.push(Memory {
            what,
            who,
            since: now,
            weight: what.weight(),
        });
        if self.kept.len() > HELD {
            // Whatever is faintest now goes. Not the oldest — a childhood that mattered
            // outlasts last year's move, which is the whole point of the curve.
            if let Some((at, _)) = self
                .kept
                .iter()
                .enumerate()
                .map(|(at, m)| (at, m.strength(now)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
            {
                self.kept.swap_remove(at);
            }
        }
    }

    /// Bring back up whatever is held about somebody, because they are here again.
    ///
    /// Distance forgives and nearness does not.
    pub fn rehearse(&mut self, about: PersonId, now: Time) {
        for memory in self.kept.iter_mut().filter(|m| m.who == Some(about)) {
            // Partial: seeing somebody does not make the memory new, it slows its going.
            let age = now.since(memory.since).as_years().max(0.0) as f32;
            if age > 0.0 {
                memory.since = memory.since + sim_core::Duration::from_years((age * 0.5) as u64);
            }
        }
    }

    /// How strongly this person holds anything at all about somebody.
    ///
    /// Signed by nothing — this is *how much they are carried*, not whether fondly. What it
    /// is carried as is the business of the tie.
    pub fn holds_about(&self, about: PersonId, now: Time) -> f32 {
        self.kept
            .iter()
            .filter(|m| m.who == Some(about))
            .map(|m| m.strength(now))
            .sum()
    }

    /// What is held about one kind of thing, whoever it was with.
    pub fn holds_of(&self, what: What, now: Time) -> f32 {
        self.kept
            .iter()
            .filter(|m| m.what == what)
            .map(|m| m.strength(now))
            .sum()
    }
}
