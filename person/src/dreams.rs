//! What somebody is trying to get out of their life.
//!
//! Nobody in this world has ever wanted anything in particular. They have needs, which are
//! appetites that come back every day and are satisfied by eating; they have values, which
//! are fixed at birth and bend how they score everything; and since §35 they have acts, which
//! are things they do to whoever is in front of them tonight. **None of that is a want with a
//! shape.** A person who was robbed at twenty and spends the next forty years making sure it
//! cannot happen again is not expressible in any of it.
//!
//! # A dream is a reading, not a field
//!
//! There is no `Person::dream`. A longing is computed from what somebody carries (§34) and
//! where they have ended up, every time it is asked — the same discipline §26.1 applies to
//! social position, which is read out of the state and never stored so that it can be *lost*.
//!
//! That is not only tidiness. A stored dream would have to be given to somebody at some
//! moment by some rule, and every such rule is an author deciding what a person wants. A
//! reading cannot be authored: it says what a life so far adds up to, and it changes when the
//! life does. The man who wanted a house of his own stops wanting one the year he has it, and
//! nothing has to remember to clear a flag.
//!
//! # Where each one comes from
//!
//! Every longing below is grown from a **specific** thing that happened or a specific thing
//! that is true now, and not from a value. Values decide *which* of two available longings
//! takes hold, because a proud man and a frightened man draw different lessons from the same
//! bad year — but a value alone never produces a dream, or everybody with the same
//! temperament would want the same thing regardless of what their life had been, which is the
//! opposite of the claim.
//!
//! # What it is deliberately not allowed to do
//!
//! Not a `Deed`. Deeds are chosen by softmax over relative scores, so anything added to that
//! list re-prices eating and sleeping (§26.11). A dream weights the decisions that are already
//! scored *outside* that softmax — where to move, what trade to take up, what to do to the
//! person in front of you — and touches the daily rhythm nowhere.

use crate::memory::What;
use crate::{Person, PersonId, Values};
use sim_core::Time;

/// What somebody is after.
///
/// Seven, and the list is short on purpose: each has to be traceable to something that happened
/// and has to change a decision that already exists, or it is decoration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dream {
    /// A household of their own, and somebody in it.
    AHome,
    /// To be somebody here — to have what the people above them have.
    ToRise,
    /// To be anywhere but here.
    Away,
    /// To be the one who opens a door for somebody else.
    ToBeLookedTo,
    /// To work out the thing nobody has worked out.
    ToMakeSomething,
    /// That it never happens to them again.
    NeverAgain,
    /// To have what somebody they know has.
    ///
    /// The other six are grown from what happened to *you*. This one is grown from somebody
    /// else's life, which is where envy lives and is the half of wanting the rest of the list
    /// could not reach. It needs a **named person** rather than a rank — "I am in the bottom
    /// third" is a statistic and "Bould Maesk has what I do not" is a grudge — and the tie
    /// graph has held everything that takes since §17.
    WhatTheyHave,
}

impl Dream {
    pub const ALL: [Dream; 7] = [
        Dream::AHome,
        Dream::ToRise,
        Dream::Away,
        Dream::ToBeLookedTo,
        Dream::ToMakeSomething,
        Dream::NeverAgain,
        Dream::WhatTheyHave,
    ];

    pub const COUNT: usize = Dream::ALL.len();

    pub const fn label(self) -> &'static str {
        match self {
            Dream::AHome => "a home",
            Dream::ToRise => "to rise",
            Dream::Away => "away",
            Dream::ToBeLookedTo => "to be looked to",
            Dream::ToMakeSomething => "to make something",
            Dream::NeverAgain => "never again",
            Dream::WhatTheyHave => "what they have",
        }
    }
}

/// Where somebody has ended up — the half of a dream that is not memory.
///
/// A borrowed view rather than the world, for the same reason `acts::Actor` is one: a reading
/// that can reach anything will eventually read something nobody meant it to.
pub struct Standing<'a> {
    pub values: &'a Values,
    /// Whether they have a household of their own.
    pub has_a_home: bool,
    /// Whether anybody shares it with them.
    pub has_somebody: bool,
    /// How they compare with the people around them, 0 at the bottom to 1 at the top.
    ///
    /// The world's regard rather than what they own, and that is a claim: what makes somebody
    /// want to get on is where they stand *in other people's eyes*. It is also the rank this
    /// world already keeps, walked once a year for everybody at once, so asking it costs a
    /// lookup rather than a sort of the neighbourhood.
    pub rank: f32,
    /// How short the place they live in is of feeding itself.
    pub want: f32,
    /// How many people stand with them.
    pub allies: usize,
    /// Whether anybody has ever opened a door for them.
    ///
    /// Whether they have opened one for somebody else is not here, and does not need to be:
    /// §34 has both parties keep `What::TakenUp`, so the memory below already carries it from
    /// either side. Two fields for one fact is two facts that can disagree.
    pub was_taken_up: bool,
    /// Their age against a whole life, 0 to 1.
    pub through_life: f32,
    /// Who they measure themselves against, and what there is to mind about it.
    ///
    /// **A person, not a rank**, which is the whole point. "I am in the bottom third" is a
    /// statistic; "Bould Maesk has what I do not" is a grudge, and only the second one makes
    /// anybody do anything. The comparison a life actually makes is local, and the tie graph
    /// has been able to say who is local to whom since §17.
    pub envied: Option<Envy>,
}

/// The person somebody measures themselves against, in pieces.
///
/// Three fields rather than one number, and the reason is the whole of §36.6. `sim` picks *who*
/// by combining them — which is the right way to rank candidates — but the **strength** of the
/// longing has to be built here, beside its six siblings, in the shape they all share. When this
/// arrived pre-multiplied, envy was a product of three sub-unit terms in a file where every
/// other longing is a fact times a set of weights, and it topped out at a third of what it
/// takes to want something. A number that has already been combined cannot be put back on its
/// siblings' scale.
#[derive(Clone, Copy, Debug)]
pub struct Envy {
    /// The one they mind.
    pub of: PersonId,
    /// How far above them that person is, 0 to 1 — saturating, so it does not depend on
    /// `means()` having a ceiling, which it does not.
    pub above: f32,
    /// How much of their life that person takes up.
    pub known: f32,
    /// And how little they like them.
    pub coolness: f32,
}

/// How strongly somebody wants each of the seven.
///
/// Independent quantities, not a distribution — the same shape as `acts::weigh`, and for the
/// same reason: a longing that had to be traded off against the others would change when a
/// seventh was added. A seventh was added, and none of the six moved.
pub fn longings(who: &Person, at: &Standing, now: Time) -> [f32; Dream::COUNT] {
    let held = who.held();
    let mut want = [0.0_f32; Dream::COUNT];
    let values = at.values;

    // All seven have the same shape: **a sum of reasons, times a weight from values.** That
    // is not a style. The first version scored each as a product of three or four sub-unit
    // terms and the result was a world in which four of the then-six longings never occurred
    // to anybody at all and a fifth accounted for ninety-eight percent of the rest — a
    // constant with a name, which is exactly what §36.1's instrument was written to catch before any of this
    // was wired to a decision. It is the same error §35.2 records three times over, and
    // knowing about it did not stop me making it a fourth.

    // What being taken from adds up to. Being robbed is the fact; being slighted is the
    // ordinary friction of living among people who owe each other things, and there is a great
    // deal more of it — some four and a half thousand withholdings in eight worlds against
    // ninety robberies — so it is weighted down. Otherwise every longing keyed on a wrong
    // becomes a longing keyed on having neighbours.
    let taken = held.holds_of(What::Robbed, now) + 0.4 * held.holds_of(What::Wronged, now);

    // A home. Not having somebody, and not having a household that is yours rather than the
    // one you were raised in — multiplied by how overdue it is, because a man of forty with
    // nobody is not in the position of a man of twenty with nobody. It goes to nothing the
    // year both are true, and nothing anywhere clears a flag.
    if !at.has_somebody || !at.has_a_home {
        let overdue = ((at.through_life - 0.25) / 0.35).clamp(0.0, 1.0);
        let lacking = if at.has_somebody { 0.0 } else { 0.7 }
            + if at.has_a_home { 0.0 } else { 0.5 };
        want[Dream::AHome as usize] =
            lacking * (0.5 + 0.5 * overdue) * (0.5 + 0.5 * values.security);
    }

    // To rise. Being near the bottom of somewhere, which is a comparison and not a quantity —
    // a poor man among poor men is not the one who burns to get on. Sharpened by having been
    // taken from: this and `NeverAgain` are the two lessons a robbery can teach, and which one
    // it teaches is the one place values are allowed to decide.
    let below = (1.0 - at.rank).clamp(0.0, 1.0);
    want[Dream::ToRise as usize] = below
        * (0.2 + 0.4 * values.achievement + 0.4 * values.power)
        * (1.0 + 0.4 * taken.min(1.5));

    // Away. Hunger, having nobody, having been wronged here, and being at the bottom of the
    // heap — the four reasons anybody has ever left anywhere. `friendless` is measured against
    // eight rather than against one: people in this world carry tens of ties, so "two over the
    // number you have" read as nought-point-nought-five for everybody and the term did nothing.
    let friendless = (1.0 - at.allies as f32 / 8.0).clamp(0.0, 1.0);
    let hurt_here = held.holds_of(What::Wronged, now).min(1.5);
    want[Dream::Away as usize] = (at.want.min(1.0) * 0.9
        + friendless * 0.7
        + hurt_here * 0.5
        + below * 0.5)
        * (0.4 + 0.6 * (1.0 - values.tradition))
        // Nobody dreams of leaving at seventy.
        * (1.0 - at.through_life).clamp(0.0, 1.0);

    // To be looked to. Grown from having been taken up — the largest single fact about a life
    // here (§25), and the one that most obviously reproduces itself — and from having
    // somewhere to stand, because a man at the bottom does not dream of being a patron. He
    // dreams of not being at the bottom, which is the longing above.
    let carried = held.holds_of(What::TakenUp, now).min(1.5);
    if at.was_taken_up || carried > 0.0 {
        let door = if at.was_taken_up { 0.4 } else { 0.0 };
        want[Dream::ToBeLookedTo as usize] =
            (0.5 * carried + 0.8 * at.rank + door) * (0.4 + 0.6 * values.power);
    }

    // To make something. The only longing that comes from having already done the thing once:
    // §29's advances are rare enough that having had one is a fact about a life, and somebody
    // who has worked one thing out is who goes looking for the next.
    let worked_out = held.holds_of(What::WorkedItOut, now).min(1.5);
    if worked_out > 0.0 {
        want[Dream::ToMakeSomething as usize] = (0.9 * worked_out
            + 0.5 * who.personality.openness.max(0.0))
            * (0.5 + 0.5 * values.achievement);
    }

    // Never again. The other lesson of a wrong, and the one a frightened person draws where a
    // proud one draws `ToRise`. Straight off the memory, so it fades exactly as the memory
    // does — which is the point of §34's curve: a robbery at twenty still faintly shapes a
    // life at sixty, and a slight last spring shapes it hard.
    if taken > 0.0 {
        want[Dream::NeverAgain as usize] = taken.min(2.0)
            * 0.9
            * (0.4 + 0.6 * values.security)
            * (0.5 + 0.5 * (1.0 - values.power));
    }

    // What somebody else has. The same shape as `to rise` above and for the same reason: one
    // fact, times what makes it sting, times who they are. The fact is the gap — envy without
    // one is not envy — so it multiplies rather than joining a sum, and a person with no gap
    // scores nothing however much they dislike whoever they are looking at.
    //
    // What makes it sting is being *near* them and not being fond of them. Both are reasons
    // rather than requirements, which is why they sit in a sum with a floor under it: a rich
    // stranger you have no feeling about is still minded, just less than the cousin next door
    // who has what you have not.
    //
    // The 0.15 floor on the values term is there so that envy is not the exclusive property of
    // the ambitious. Anybody can mind. The ambitious mind more.
    if let Some(envy) = at.envied {
        want[Dream::WhatTheyHave as usize] = envy.above.clamp(0.0, 1.0)
            * (0.6 + 0.7 * envy.known.clamp(0.0, 1.0) + 0.5 * envy.coolness.clamp(0.0, 1.0))
            * (0.15 + 0.5 * values.power + 0.45 * values.achievement)
            * (1.0 - 0.5 * values.benevolence);
    }

    want
}

/// What somebody is actually after, if anything.
///
/// The strongest longing, if it is strong enough to be worth calling one. Most people want
/// nothing in particular and that has to be the ordinary case — a world in which everybody is
/// driven is a world in which being driven means nothing.
pub fn of(who: &Person, at: &Standing, now: Time) -> Option<(Dream, f32)> {
    let want = longings(who, at, now);
    let (mut best, mut most) = (None, WORTH_WANTING);
    for dream in Dream::ALL {
        if want[dream as usize] > most {
            most = want[dream as usize];
            best = Some(dream);
        }
    }
    best.map(|dream| (dream, most))
}

/// How much longing it takes before it is a dream rather than a preference.
pub const WORTH_WANTING: f32 = 0.5;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Deed;
    use sim_core::{Domain, Duration, WorldSeed};

    fn somebody() -> Person {
        let mut home: sim_core::Arena<planet::Planet> = sim_core::Arena::new();
        let planet = home.insert(planet::Planet::earth());
        let mut rng = WorldSeed::from_u128(0xd2_ea).stream(Domain::Behavior, 1, 0);
        crate::found(
            genetics::standard_architecture(),
            &genetics::FounderPool::uniform(),
            &mut rng,
            planet,
            Time::ORIGIN,
            0.0,
        )
    }

    fn settled<'a>(values: &'a Values) -> Standing<'a> {
        Standing {
            values,
            has_a_home: true,
            has_somebody: true,
            rank: 0.5,
            want: 0.0,
            allies: 6,
            was_taken_up: false,
            through_life: 0.4,
            envied: None,
        }
    }

    #[test]
    fn a_dream_ends_when_the_thing_is_had() {
        let (values, who) = (Values::BALANCED, somebody());
        let mut at = settled(&values);
        at.has_a_home = false;
        at.has_somebody = false;
        let wanting = longings(&who, &at, Time::ORIGIN)[Dream::AHome as usize];
        assert!(wanting > WORTH_WANTING, "{wanting}");

        // And nothing clears a flag: the same person, the year they have one.
        at.has_a_home = true;
        at.has_somebody = true;
        assert_eq!(longings(&who, &at, Time::ORIGIN)[Dream::AHome as usize], 0.0);
    }

    #[test]
    fn the_same_injury_teaches_two_people_different_things() {
        // The one place values are allowed to decide: a robbery makes a proud man want to
        // rise and a frightened one want it never to happen again. Same memory, same
        // situation, two lives.
        let now = Time::ORIGIN + Duration::from_years(30);
        let mut who = somebody();
        who.keep(What::Robbed, None, now);

        let proud = Values {
            power: 1.0,
            achievement: 1.0,
            security: 0.0,
            ..Values::BALANCED
        };
        let frightened = Values {
            power: 0.0,
            achievement: 0.0,
            security: 1.0,
            ..Values::BALANCED
        };
        let mut at = settled(&proud);
        at.rank = 0.2;
        let his = longings(&who, &at, now);
        let at = Standing {
            values: &frightened,
            ..settled(&frightened)
        };
        let mut at = at;
        at.rank = 0.2;
        let hers = longings(&who, &at, now);

        assert!(
            his[Dream::ToRise as usize] > his[Dream::NeverAgain as usize],
            "the proud one: rise {:.2} against never again {:.2}",
            his[Dream::ToRise as usize],
            his[Dream::NeverAgain as usize]
        );
        assert!(
            hers[Dream::NeverAgain as usize] > hers[Dream::ToRise as usize],
            "the frightened one: never again {:.2} against rise {:.2}",
            hers[Dream::NeverAgain as usize],
            hers[Dream::ToRise as usize]
        );
    }

    #[test]
    fn a_wrong_long_ago_shapes_a_life_less_than_one_last_year() {
        // §34's curve, doing something. The same injury, read at two ages.
        let values = Values {
            security: 1.0,
            power: 0.0,
            ..Values::BALANCED
        };
        let mut who = somebody();
        let hurt = Time::ORIGIN + Duration::from_years(20);
        who.keep(What::Robbed, None, hurt);
        let at = settled(&values);

        let fresh = longings(&who, &at, hurt + Duration::from_years(1))[Dream::NeverAgain as usize];
        let old = longings(&who, &at, hurt + Duration::from_years(40))[Dream::NeverAgain as usize];
        assert!(fresh > old * 2.0, "fresh {fresh:.2}, forty years on {old:.2}");
        assert!(old > 0.0, "and it never quite goes");
    }

    #[test]
    fn most_people_want_nothing_in_particular() {
        // A world where everybody is driven is a world where being driven means nothing.
        let (values, who) = (Values::BALANCED, somebody());
        let at = settled(&values);
        assert_eq!(of(&who, &at, Time::ORIGIN), None);
        // And the deed list is untouched by any of this, which is the architectural claim.
        assert_eq!(Deed::COUNT, 7);
    }
}
