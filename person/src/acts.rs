//! What one person deliberately does to another.
//!
//! Everything a person did before this was addressed to the world: they ate, they worked,
//! they moved. Even the social ones were — `Deed::Socialize` relieved a need and named
//! nobody, and the mutual aid in `share_the_shortfall` picks whoever happens to be an ally
//! with something spare. Nothing in the model was a person choosing *a person* and doing
//! something to them on purpose.
//!
//! This is that. Five acts, each aimed at somebody, each scored from who the actor is and
//! what they hold about the target — so that a stranger gets helped because the person who
//! helped them is kind, and somebody gets killed because the person who killed them hated
//! them and had nothing left to lose.
//!
//! # Why this is not another `Deed`
//!
//! The obvious place is `Deed::ALL`, and it is the wrong place for a reason the project has
//! already paid for twice. Deeds are chosen by softmax over relative scores, so **a new deed
//! is a re-normalisation and not an addition** (§26.11): the one time an eighth was added it
//! left eating and sleeping alone in the code and moved migration by 64% in the world, and
//! it was reverted. Acts are scored independently — each one's appetite is a quantity in its
//! own right, and each is rolled against its own bar rather than against the others (see
//! [`choose`]) — so adding a sixth act changes the other five by *nothing*, not merely by
//! little. That property is worth more here than the elegance of one list, because this
//! vocabulary is going to grow.
//!
//! # Wrongs
//!
//! Two kinds, and they behave differently on purpose.
//!
//! **Harm is wrong everywhere.** Robbing somebody and killing them carry a weight that does
//! not depend on where you are standing or who raised you. That is not a claim about
//! metaethics; it is the minimum a model needs so that a murderer cannot emigrate into
//! innocence.
//!
//! **Obligation is local.** What you owe the person in front of you when they are going
//! short is a thing a people has, and different peoples have different amounts of it. It is
//! read off how they spend their days ([`what_is_expected`]) rather than stored, so it drifts
//! as a culture drifts and splits when a culture splits. And because a person carries their
//! own upbringing's version of it (§17.2.1's `norms`), somebody who moves can transgress
//! without knowing they have: they withhold exactly as they always did, in a place where
//! that is not done, and are judged by a standard they never learned.
//!
//! # Conscience
//!
//! There are no witnesses in this model and there is no need for any. A wrong is kept by
//! whoever did it, always, as [`What::DidWrong`] — and what that memory does is make the
//! next one harder. Guilt is felt in proportion to benevolence and to how anxious somebody
//! is, so the same act sits differently in two people, and it fades on `memory`'s hyperbolic
//! curve, so a wrong done at twenty still faintly restrains at sixty while a wrong done last
//! year restrains hard. Nobody has to see anything for that to work, which is what makes it
//! conscience rather than reputation. Reputation exists too, and is the tie graph's business.

use crate::dreams::Dream;
use crate::memory::{Held, What};
use crate::{Deed, Personality, PersonId, Values};
use sim_core::{Rng, Time};

/// The vocabulary of things one person can do to another.
///
/// Withholding is not in the list, and that is deliberate: it is not something anybody
/// chooses, it is the name for having chosen nothing in a moment that asked for something.
/// See [`withheld`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toward {
    /// Hand over some of what you have, to somebody who has less.
    Give,
    /// Pass on what you know to somebody young enough to use it.
    Teach,
    /// Refuse them, and say so to everybody you are close to.
    Shun,
    /// Take what they have.
    Rob,
    /// Kill them.
    Kill,
}

impl Toward {
    pub const ALL: [Toward; 5] = [
        Toward::Give,
        Toward::Teach,
        Toward::Shun,
        Toward::Rob,
        Toward::Kill,
    ];

    pub const COUNT: usize = Toward::ALL.len();

    pub const fn label(self) -> &'static str {
        match self {
            Toward::Give => "gave to",
            Toward::Teach => "taught",
            Toward::Shun => "shunned",
            Toward::Rob => "robbed",
            Toward::Kill => "killed",
        }
    }

    /// How much wanting it takes before somebody acts on it.
    ///
    /// Per act rather than one number for all five, and the reason is the same one that made
    /// the appetites sums instead of products: **these five quantities are not on a common
    /// scale and pretending they are is a bug.** Giving is a sum of four ordinary reasons and
    /// lands near one; killing is a product of five conditions each of which is already rare,
    /// and in a measured world the strongest anybody ever wanted it was 0.138. Against a
    /// shared bar of 0.25 that is not "nobody in this world would kill anybody" — it is a unit
    /// mismatch, reported as a finding about human nature.
    ///
    /// Because `choose` rolls each act separately, a per-act bar changes only that act. That
    /// is the whole reason it is safe to have one.
    pub const fn bar(self) -> f32 {
        match self {
            Toward::Kill => 0.08,
            Toward::Rob => 0.20,
            _ => 0.25,
        }
    }

    /// How readily a standing disposition finds an occasion, per evening.
    ///
    /// Also per act, and for a plainer reason: a killing needs a moment as well as a will.
    /// Somebody who would give if asked is asked most weeks; somebody who would kill a man
    /// they loathe needs to be alone with him and out of his sight afterwards, and the
    /// difference between wanting a thing and its coming about is far larger for the second.
    pub const fn rate(self) -> f32 {
        match self {
            Toward::Kill => 0.25,
            _ => 1.0,
        }
    }

    /// How much of a wrong this is, anywhere, to anyone.
    ///
    /// Shunning is on the list and is the interesting entry: it is a small wrong that is
    /// also how a people enforces everything else, so somebody who shuns a thief is doing a
    /// slightly bad thing for a good reason and carries a little of it either way. Giving
    /// and teaching are not wrongs and never become ones.
    pub const fn harm(self) -> f32 {
        match self {
            Toward::Kill => 1.0,
            Toward::Rob => 0.45,
            Toward::Shun => 0.12,
            Toward::Give | Toward::Teach => 0.0,
        }
    }
}

/// How much a people expects its members to see each other right, from how they live.
///
/// Not a stored virtue and not a doctrine — there is no doctrine in this world. A people
/// that spends a large share of its doing on each other is one in which turning away from
/// somebody in front of you is conspicuous; a people that spends its days apart has less of
/// a claim on anybody. So the obligation is the socialising share of the ways, against the
/// share of the ways that is anybody's to allocate at all.
///
/// Measured against `Deed::CHOSEN` rather than all seven, because eating and sleeping are
/// not allocations — including them would mean a people that slept more owed each other
/// less, which is arithmetic rather than a claim.
pub fn what_is_expected(ways: &[f32; Deed::COUNT]) -> f32 {
    let chosen: f32 = Deed::CHOSEN.iter().map(|d| ways[*d as usize]).sum();
    if chosen <= 0.0 {
        return 0.0;
    }
    // Doubled, so that the ordinary quarter-of-what-is-chosen reads as about a half rather
    // than as a quarter. This is a weight on a wrong, not a probability.
    (2.0 * ways[Deed::Socialize as usize] / chosen).clamp(0.0, 1.0)
}

/// Who is doing it — everything about the actor that bears on the choice.
///
/// A borrowed view rather than the `Person`, because the scoring must not be able to reach
/// anything it has not been handed. Half the defects this project has found were a decision
/// quietly reading a quantity nobody thought it could see.
pub struct Actor<'a> {
    pub values: &'a Values,
    pub personality: &'a Personality,
    pub held: &'a Held,
    /// What they have, in standing.
    pub means: f32,
    /// How short of feeding themselves they are, 0 upward.
    pub want: f32,
    /// How many people depend on them. The strongest thing anybody has to lose.
    pub dependents: usize,
    /// 0 at death's door, 1 hale.
    pub health: f32,
    /// How many years somebody their age can expect, against a whole life.
    pub life_ahead: f32,
    /// Whether they have a trade worth passing on.
    pub has_a_trade: bool,
    /// What their own upbringing says is owed to the person in front of them.
    pub own_ways: f32,
    /// What they are trying to get out of their life, if anything — see `dreams`.
    ///
    /// Three of the six bear on what somebody does to the person in front of them and three
    /// do not, and the three that do not are left doing nothing here rather than given a
    /// token weight. A dream that wants a house has no opinion about whether to rob a
    /// neighbour, and pretending otherwise would make this field a second personality.
    pub dream: Option<(Dream, f32)>,
}

impl Actor<'_> {
    /// How strongly they want that particular thing, or nothing.
    fn dreaming_of(&self, dream: Dream) -> f32 {
        match self.dream {
            Some((held, how_much)) if held == dream => how_much,
            _ => 0.0,
        }
    }
}

/// And who it is done to.
pub struct Subject {
    pub who: PersonId,
    /// What the actor holds about them — the tie, unpacked.
    pub warmth: f32,
    pub regard: f32,
    /// Days of help owed. Positive means the subject owes the actor.
    pub debt: f32,
    pub known: f32,
    pub means: f32,
    pub want: f32,
    pub age_years: f64,
    pub matured: bool,
}

/// What somebody needs to keep a household going. Below it they are poor.
///
/// The scale everything here is measured against, because **the currency of this world is
/// means and not food.** That is a measurement rather than a preference: at the sizes this
/// project runs, the hungriest quarter's shortfall reads 0.00 and there is no famine to give
/// anybody relief from. Keying generosity on hunger would have produced a vocabulary that
/// never fired for want of anybody to feed — and keying it on the *hunger need*, which is
/// the daily rhythm between meals and is non-zero for everybody always, produced the
/// opposite and sillier thing: two hundred people wronged for not sharing lunch.
const SUBSISTENCE: f32 = 0.55;

/// How badly off somebody is, from comfortable at 0 to nothing at 1.
pub fn poverty(means: f32) -> f32 {
    ((SUBSISTENCE - means) / SUBSISTENCE).clamp(0.0, 1.0)
}

/// How much somebody has that they would lose by doing something unforgivable.
///
/// One anchor is enough. A man with a child to feed is held by that alone however sick and
/// poor and friendless he is, so this is one minus the *strongest* thing holding him rather
/// than a blend — a blend would have every poor old man a murderer, since the average of
/// three small numbers is a small number.
///
/// The three are: what people would take from you, who needs you, and how much life you have
/// left to forfeit. Condition is folded into the last rather than standing on its own,
/// because being ill is not a separate thing to lose — it is a shorter remainder — and as a
/// fourth anchor it required somebody to be at death's door before anything else counted,
/// which made the whole conjunction unreachable and killed nobody in any world.
///
/// Nothing here is a personality. A saint with everything to lose and a brute with everything
/// to lose are equally held; what differs between them is the appetite this gates.
pub fn nothing_to_lose(actor: &Actor) -> f32 {
    let anchors = [
        // Standing, saturating: the difference between nothing and a little is most of it.
        actor.means / (actor.means + 0.5),
        // Anybody at all. One dependent is nearly the whole of it, a second adds little.
        if actor.dependents > 0 { 0.85 } else { 0.0 },
        actor.life_ahead * actor.health,
    ];
    let held_by = anchors.iter().fold(0.0_f32, |most, a| most.max(*a));
    (1.0 - held_by).clamp(0.0, 1.0)
}

/// How heavily what somebody has already done sits on them.
///
/// Benevolence is most of it and anxiety is the rest, which is the ordinary finding: the
/// people who feel their own wrongs are the people who would not have done them lightly.
/// Somebody at the floor of both carries what they did as a fact rather than a weight, and
/// this returns nearly nothing for them — which is how a model gets somebody who can keep
/// doing it without anybody having to write down that they are a monster.
pub fn conscience(actor: &Actor, now: Time) -> f32 {
    let carried = actor.held.holds_of(What::DidWrong, now);
    let felt = 0.25 + 1.2 * actor.values.benevolence + 0.25 * actor.personality.neuroticism;
    carried * felt.max(0.0)
}

/// What the actor holds against this particular person.
fn grievance(actor: &Actor, at: &Subject, now: Time) -> f32 {
    actor.held.holds(What::Wronged, at.who, now)
}

/// How much of what they have somebody could give away without it mattering to them.
fn to_spare(actor: &Actor) -> f32 {
    // What they need themselves comes first, and a bad year raises what they need. Nobody
    // going short is in a position to be generous, whoever they are, and a model in which
    // they were would have a famine relieved by the starving.
    (actor.means - SUBSISTENCE - 2.0 * actor.want).max(0.0)
}

/// The appetite for each act, given who this is and who they are with.
///
/// Independent quantities, not a distribution — see the note at the top of this file. Each
/// is roughly 0 to 1, where 1 means somebody who would certainly do this and 0 means it does
/// not occur to them.
/// Note that what a people expects is *not* an input here. Somebody acts on their own
/// upbringing's idea of what is owed (`Actor::own_ways`) and is judged by the local one, and
/// keeping the local number out of this function is what makes that impossible to get wrong
/// — see [`withheld`], which is the only thing that takes it.
pub fn weigh(actor: &Actor, at: &Subject, now: Time) -> [f32; Toward::COUNT] {
    let mut appetite = [0.0_f32; Toward::COUNT];
    let warmth = at.warmth;
    let hate = (-warmth).max(0.0);
    let grudge = grievance(actor, at, now);
    let spare = to_spare(actor);
    let guilt = conscience(actor, now);
    // What conscience does: it does not forbid anything, it makes the next one dearer.
    let restraint = 1.0 / (1.0 + guilt);

    // Giving. Four separate reasons, **added** rather than multiplied, because they are four
    // different people doing the same thing: the kind one, the fond one, the one who knows
    // what is done here, and the one who owes. Only the second needs a tie — which is the
    // point, and is what lets a stranger be helped by somebody who has never met them.
    //
    // Adding is not a stylistic choice. Every appetite below was first written as a product
    // of five bounded factors, which put robbery three hundred times under the bar that
    // giving cleared comfortably, so nothing was ever robbed in any world. A score built as a
    // product of many sub-unit terms is not on the same scale as one built as a sum, and two
    // acts that are meant to compete have to be built the same way.
    let need = poverty(at.means) * (1.0 + at.want.min(1.0));
    if need > 0.0 && spare > 0.0 {
        let kindness = 0.9 * actor.values.benevolence;
        let fondness = 0.7 * warmth.max(0.0) * at.known;
        // And what their own upbringing says is owed — their *own*, not this place's. The
        // gap between the two is the whole of the migrant's problem.
        let duty = 0.6 * actor.own_ways;
        // Owing them makes it likelier; being owed makes it less so.
        let ledger = (-at.debt / 120.0).clamp(-0.4, 0.6);
        // And what they are after. Somebody who wants to be looked to gives in order to be
        // the person who gave; somebody who wants never to be caught short again keeps what
        // they have. This is the whole of §36's effect on kindness and it is one line, which
        // is the right size — a dream bends what somebody was going to do anyway.
        let after = 0.3 * actor.dreaming_of(Dream::ToBeLookedTo)
            - 0.4 * actor.dreaming_of(Dream::NeverAgain);
        appetite[Toward::Give as usize] =
            (need * (kindness + fondness + duty + ledger + after) * (spare / (spare + 0.4)))
                .max(0.0);
    }

    // Teaching. Somebody young enough for it to take, somebody you are warm to, and something
    // to pass on. The achievement term is why a proud craftsman teaches as readily as a kind
    // one does.
    //
    // Only the not-yet-grown, and that is a claim about what teaching *is* here rather than
    // about who can learn: what a lesson does in this model is go into an upbringing, and an
    // upbringing stops accumulating at maturity. Offering it to adults would have been an act
    // whose whole effect was zero.
    if actor.has_a_trade && !at.matured && at.known > 0.15 {
        let young = ((30.0 - at.age_years) / 18.0).clamp(0.0, 1.0) as f32;
        let willing = 0.6 * warmth.max(0.0) + 0.3 * at.regard.max(0.0);
        // Teaching is the plainest way anybody becomes a person other people look to, and the
        // one that costs least, so the dream weighs on it harder than it does on giving — but
        // only by half again, not by double. At 0.7 it took teaching from 161 acts in eight
        // worlds to 1,327, past giving and past everything else put together. A dream is
        // supposed to bend what somebody was going to do anyway; one wiring that multiplies an
        // act eightfold is not a bend, and an eightfold sensitivity is the kind that comes back
        // as a calibration band six months later.
        let after = 0.35 * actor.dreaming_of(Dream::ToBeLookedTo);
        appetite[Toward::Teach as usize] = young
            * (willing + after)
            * (0.4 + 0.6 * actor.values.achievement)
            * (0.5 + 0.5 * at.known);
    }

    // Shunning. What people do instead of violence, and the reason the vocabulary needs a
    // small wrong in it: without one, a society's only answer to a transgressor is to kill
    // him. Driven by a grudge and by contempt, and *raised* by tradition — a people's
    // enforcement runs through whoever cares most what is done.
    if at.known > 0.1 {
        let contempt = hate.max((-at.regard).max(0.0) * 0.7);
        // Holding people at arm's length is what "never again" looks like from the outside,
        // and it is the only way this world has of doing that.
        let after = 0.35 * actor.dreaming_of(Dream::NeverAgain);
        appetite[Toward::Shun as usize] = (grudge.min(1.5) * 0.55 + contempt * 0.5 + after)
            * (0.5 + 0.7 * actor.values.tradition)
            * (1.0 - 0.4 * actor.values.benevolence);
    }

    // Robbing. Three reasons again, added: need, greed, and spite. Times how much there is
    // to take, times how little the taker has to lose by it. A comfortable, kindly person
    // with no grudge scores a fiftieth of the bar and always will; the number only becomes an
    // act when several of the reasons are true of the same person at once.
    let worth_taking = (at.means - actor.means).max(0.0);
    if worth_taking > 0.0 {
        let coveting = worth_taking / (worth_taking + 0.4);
        let need_of_it = poverty(actor.means) + actor.want.min(1.0);
        let greed = 2.0 * actor.values.power * (1.0 - actor.values.benevolence);
        // Wanting to get on is a reason people take things, and it belongs beside greed
        // rather than instead of it: the difference between them is that greed is who
        // somebody is and this is what their life has made of them.
        let spite = grudge.min(1.0) + hate + 0.6 * actor.dreaming_of(Dream::ToRise);
        appetite[Toward::Rob as usize] = coveting
            * (need_of_it + greed + spite)
            * TAKING_BY_HAND
            * (0.25 + 0.75 * nothing_to_lose(actor))
            * restraint;
    }

    // And the last one. It needs both halves of the sentence: they hate them, *and* they have
    // nothing to lose. Either alone is common and produces nobody dead — plenty of people
    // loathe a neighbour their whole lives, and plenty have nothing, and it is the conjunction
    // that is rare. Both are gated *and* then weighted, so that clearing the gate by a hair
    // is not the same as being far past it and the act has a gradient rather than a cliff.
    //
    // The weights are on the raw quantities rather than on how far past the gate they are.
    // Renormalising the remainder was the first version and it made the act unreachable: the
    // most spent anybody in a measured world ever gets is 0.72, so `(spent - 0.5) / 0.5`
    // squashed the entire real range into 0 to 0.44 and the appetite never cleared its bar.
    // The gate says *whether*; the weight says *how much*, on the scale the quantity actually
    // occupies.
    let spent = nothing_to_lose(actor);
    if hate > HATRED && spent > DESPERATE && at.known > 0.2 {
        appetite[Toward::Kill as usize] = (0.35 + 0.65 * hate)
            * (0.35 + 0.65 * spent)
            * (0.4 + 0.6 * grudge.min(1.0))
            * (1.0 - actor.values.benevolence).max(0.0)
            * (0.5 + 0.5 * actor.values.power)
            * restraint;
    }

    // Nobody does anything to somebody they do not know at all — except be kind to them,
    // which is the one act above that survives a `known` of zero.
    if at.known <= 0.02 {
        for act in [Toward::Teach, Toward::Shun, Toward::Rob, Toward::Kill] {
            appetite[act as usize] = 0.0;
        }
    }
    appetite
}

/// How much warmth has to have soured before killing is even on the table.
const HATRED: f32 = 0.45;
/// And how far gone somebody has to be.
const DESPERATE: f32 = 0.5;

/// What the three reasons for a robbery are worth once added up.
///
/// The one number that sets how much of this there is. Kept apart from the reasons rather
/// than folded into them so that ablating the rate is one edit and does not change *who*
/// robs — only how often anybody does.
const TAKING_BY_HAND: f32 = 0.35;

/// Which act, if any, this evening turns out to be.
///
/// Deliberately not a softmax and — after measurement — deliberately not a maximum either.
///
/// A maximum was the first version, and it made murder impossible for a reason worth keeping.
/// Shunning and killing are driven by the same hatred, and shunning is far cheaper, so the
/// shunning appetite is above the killing one at every level of loathing anybody ever
/// reaches. Under a maximum that means **the cheap sanction masks the grave one, always**,
/// and no amount of tuning the killing appetite fixes it: raise it enough to win and every
/// falling-out is a murder. There is something true in there — a society reaches for the
/// cheap sanction first — but "and therefore nobody is ever killed" is not it.
///
/// So each act is rolled *independently* against its own appetite, and if more than one comes
/// up, the gravest is what happened. A person who has decided to kill somebody does not
/// settle for cutting them. This also finishes the property the whole module is built for:
/// an act cannot suppress another act, so adding a sixth changes the other five by nothing at
/// all, not merely by little.
pub fn choose(appetite: &[f32; Toward::COUNT], occasion: f32, rng: &mut Rng) -> Option<Toward> {
    let mut done: Option<Toward> = None;
    for act in Toward::ALL {
        let want = appetite[act as usize];
        if want <= act.bar() {
            continue;
        }
        // An appetite is not an occasion: somebody who would give if asked does not give on
        // every one of the sixteen evenings a year they spend in company.
        if !rng.chance((want * occasion * act.rate()).min(0.5) as f64) {
            continue;
        }
        if done.is_none_or(|already| act.harm() > already.harm()) {
            done = Some(act);
        }
    }
    done
}

/// The wrong in having done nothing.
///
/// Not an act and not chosen — this is what is *left over* when somebody stood next to a
/// person going short, could have helped, and did not. It is a wrong in proportion to what
/// this place expects of people, which is why the same shrug is nothing in one valley and a
/// disgrace in the next.
///
/// Returns what it weighs, or zero if there was no obligation to begin with.
pub fn withheld(actor: &Actor, at: &Subject, expected_here: f32) -> f32 {
    if to_spare(actor) <= 0.0 {
        return 0.0;
    }
    // Only somebody you are actually among. An obligation to everybody alive is an obligation
    // to nobody.
    if at.known <= 0.05 {
        return 0.0;
    }
    // And only somebody visibly worse off than you. Without this the first version counted
    // every evening spent beside anybody poor as a wrong and produced **twenty-four thousand
    // of them** in three worlds — which savaged every tie in every settlement and drove the
    // largest quarter's share of households from 0.47 to 0.65. An obligation that everybody
    // is failing all the time is not an obligation; it is a tax on being sociable.
    if at.means + NOTICEABLY_WORSE > actor.means {
        return 0.0;
    }
    (expected_here * poverty(at.means) * (1.0 + at.want.min(1.0))).clamp(0.0, 1.0)
}

/// How much better off you have to be before turning away is anybody's business.
const NOTICEABLY_WORSE: f32 = 0.2;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::Held;
    use sim_core::{Duration, Time};

    /// Two handles that are not each other. Ids have no public constructor on purpose —
    /// a handle is a thing an arena hands out — so two people get made to get two of them.
    fn somebody_and_somebody_else() -> (PersonId, PersonId) {
        let mut home: sim_core::Arena<planet::Planet> = sim_core::Arena::new();
        let planet = home.insert(planet::Planet::earth());
        let mut rng =
            sim_core::WorldSeed::from_u128(0xac_75).stream(sim_core::Domain::Behavior, 1, 0);
        let arch = genetics::standard_architecture();
        let pool = genetics::FounderPool::uniform();
        let mut who: sim_core::Arena<crate::Person> = sim_core::Arena::new();
        let a = who.insert(crate::found(arch, &pool, &mut rng, planet, Time::ORIGIN, 0.0));
        let b = who.insert(crate::found(arch, &pool, &mut rng, planet, Time::ORIGIN, 0.0));
        (a, b)
    }

    fn actor<'a>(values: &'a Values, personality: &'a Personality, held: &'a Held) -> Actor<'a> {
        Actor {
            values,
            personality,
            held,
            means: 1.0,
            want: 0.0,
            dependents: 0,
            health: 1.0,
            life_ahead: 0.5,
            has_a_trade: true,
            own_ways: 0.5,
            dream: None,
        }
    }

    fn subject(who: PersonId) -> Subject {
        Subject {
            who,
            warmth: 0.0,
            regard: 0.0,
            debt: 0.0,
            known: 0.5,
            means: 1.0,
            want: 0.0,
            age_years: 40.0,
            matured: true,
        }
    }

    #[test]
    fn one_thing_to_lose_is_enough() {
        let (values, personality, held) = (Values::BALANCED, Personality::AVERAGE, Held::default());
        let mut who = actor(&values, &personality, &held);
        who.means = 0.0;
        who.health = 0.1;
        who.life_ahead = 0.05;
        assert!(nothing_to_lose(&who) > 0.85, "a man with nothing is spent");
        who.dependents = 1;
        assert!(
            nothing_to_lose(&who) < 0.2,
            "and a child to feed holds him anyway"
        );
    }

    #[test]
    fn kindness_reaches_a_stranger_and_nothing_else_does() {
        let (mut values, personality, held) =
            (Values::BALANCED, Personality::AVERAGE, Held::default());
        values.benevolence = 1.0;
        let who = actor(&values, &personality, &held);
        let (_, other) = somebody_and_somebody_else();
        let mut them = subject(other);
        them.known = 0.0;
        them.want = 1.0;
        them.means = 0.0;
        let appetite = weigh(&who, &them, Time::ORIGIN);
        assert!(
            appetite[Toward::Give as usize] > Toward::Give.bar(),
            "{}",
            appetite[Toward::Give as usize]
        );
        for act in [Toward::Teach, Toward::Shun, Toward::Rob, Toward::Kill] {
            assert_eq!(appetite[act as usize], 0.0, "{}", act.label());
        }
    }

    #[test]
    fn hatred_alone_kills_nobody() {
        let (mut values, personality, held) =
            (Values::BALANCED, Personality::AVERAGE, Held::default());
        values.benevolence = 0.0;
        values.power = 1.0;
        let mut who = actor(&values, &personality, &held);
        let (_, other) = somebody_and_somebody_else();
        let mut them = subject(other);
        them.warmth = -1.0;
        // Everything to lose: a man with people who need him does not do this however much
        // he loathes somebody.
        who.dependents = 2;
        assert_eq!(weigh(&who, &them, Time::ORIGIN)[Toward::Kill as usize], 0.0);
        // Nothing to lose and no hatred does not either.
        who.dependents = 0;
        who.means = 0.0;
        who.health = 0.05;
        who.life_ahead = 0.0;
        them.warmth = 0.0;
        assert_eq!(weigh(&who, &them, Time::ORIGIN)[Toward::Kill as usize], 0.0);
        // Both halves of the sentence, and it is on the table.
        them.warmth = -1.0;
        assert!(weigh(&who, &them, Time::ORIGIN)[Toward::Kill as usize] > 0.0);
    }

    #[test]
    fn a_wrong_already_done_makes_the_next_one_dearer() {
        let (values, personality) = (Values::BALANCED, Personality::AVERAGE);
        let (already, other) = somebody_and_somebody_else();
        let (clean, mut carrying) = (Held::default(), Held::default());
        let now = Time::ORIGIN + Duration::from_years(1);
        carrying.keep(What::DidWrong, Some(already), now);

        let mut them = subject(other);
        them.means = 3.0;
        them.warmth = -0.3;
        let mut without = actor(&values, &personality, &clean);
        without.means = 0.0;
        without.want = 0.5;
        let mut with = actor(&values, &personality, &carrying);
        with.means = 0.0;
        with.want = 0.5;

        let alone = weigh(&without, &them, now)[Toward::Rob as usize];
        let after = weigh(&with, &them, now)[Toward::Rob as usize];
        assert!(alone > 0.0 && after < alone, "{alone} then {after}");
    }

    #[test]
    fn what_a_people_expects_is_read_off_how_they_live() {
        let mut apart = [0.1_f32; Deed::COUNT];
        apart[Deed::Socialize as usize] = 0.02;
        let mut together = [0.1_f32; Deed::COUNT];
        together[Deed::Socialize as usize] = 0.30;
        assert!(what_is_expected(&together) > what_is_expected(&apart) * 2.0);
    }
}
