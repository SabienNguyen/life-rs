//! What people do, and how they choose between options.
//!
//! This replaces `match self.state`. A state machine cannot say *why* it did something,
//! and it cannot let personality tilt a decision without a new arm for every
//! combination. Scoring can do both: every option is priced, the prices are kept, and
//! the observer can show its working.
//!
//! The score is the product of the four channels from the design's environment model.
//! They are wired in now and mostly neutral until places exist — but the shape has to
//! be right from the start, because retrofitting a parameter into every action later is
//! the expensive kind of change.

use crate::psyche::{Personality, Values};
use life::{Need, Needs};
use planet::DayPhase;
use sim_core::{Duration, Rng};

/// Something a person can spend time doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(usize)]
pub enum Deed {
    Eat,
    Drink,
    Sleep,
    Wash,
    Socialize,
    Work,
    Wander,
}

impl Deed {
    pub const ALL: [Deed; 7] = [
        Deed::Eat,
        Deed::Drink,
        Deed::Sleep,
        Deed::Wash,
        Deed::Socialize,
        Deed::Work,
        Deed::Wander,
    ];

    pub const COUNT: usize = Deed::ALL.len();

    /// The deeds somebody has a real choice about.
    ///
    /// Eating, drinking and sleeping are not choices — everybody does them, and doing more
    /// of them distinguishes nobody from anybody. These four are where a temperament shows
    /// up as a life, and so they are what a social position can be read off.
    pub const CHOSEN: [Deed; 4] = [Deed::Wash, Deed::Socialize, Deed::Work, Deed::Wander];

    pub const fn label(self) -> &'static str {
        match self {
            Deed::Eat => "eating",
            Deed::Drink => "drinking",
            Deed::Sleep => "sleeping",
            Deed::Wash => "washing",
            Deed::Socialize => "socialising",
            Deed::Work => "working",
            Deed::Wander => "wandering",
        }
    }

    pub fn duration(self) -> Duration {
        match self {
            Deed::Eat => Duration::from_minutes(45),
            Deed::Drink => Duration::from_minutes(5),
            Deed::Sleep => Duration::from_hours(8),
            Deed::Wash => Duration::from_minutes(30),
            Deed::Socialize => Duration::from_hours(2),
            Deed::Work => Duration::from_hours(4),
            Deed::Wander => Duration::from_hours(1),
        }
    }

    /// What doing this changes. Negative relieves a need, positive costs one — work
    /// pays in purpose and charges in tiredness.
    pub fn effects(self) -> &'static [(Need, f32)] {
        match self {
            Deed::Eat => &[(Need::Hunger, -0.9), (Need::Thirst, -0.2)],
            Deed::Drink => &[(Need::Thirst, -1.0)],
            Deed::Sleep => &[(Need::Energy, -1.0), (Need::Hunger, 0.1)],
            Deed::Wash => &[(Need::Hygiene, -1.0)],
            Deed::Socialize => &[(Need::Social, -0.8), (Need::Purpose, -0.1)],
            Deed::Work => &[
                (Need::Purpose, -0.5),
                (Need::Safety, -0.25),
                (Need::Energy, 0.15),
                (Need::Hygiene, 0.1),
            ],
            Deed::Wander => &[
                (Need::Purpose, -0.15),
                (Need::Social, -0.1),
                (Need::Energy, 0.05),
            ],
        }
    }

    /// How long before the benefit actually lands, in days.
    ///
    /// Eating pays now; work pays later. This is the hook that lets stress matter:
    /// a short time horizon discounts delayed payoffs, so scarcity suppresses
    /// investment without anyone's personality changing.
    pub const fn payoff_delay_days(self) -> f32 {
        match self {
            Deed::Work => 7.0,
            Deed::Wash | Deed::Socialize => 0.5,
            _ => 0.0,
        }
    }

    /// How much this appeals to a given temperament, independent of need.
    ///
    /// Public so that a tier which does not deliberate can still ask how much somebody
    /// wants a thing — see `sim::World::live_coarsely`, which needs to know how sociable an
    /// unwatched person is without scoring four thousand decisions to find out. Writing a
    /// second expression for the same question is how the two tiers drift apart.
    pub fn appeal(self, personality: &Personality, values: &Values) -> f32 {
        let raw = match self {
            Deed::Socialize => 1.0 + 0.30 * personality.extraversion + 0.20 * values.benevolence,
            Deed::Work => {
                1.0 + 0.30 * personality.conscientiousness + 0.35 * values.achievement
                    - 0.20 * values.hedonism
            }
            Deed::Wander => 1.0 + 0.35 * personality.openness - 0.20 * values.security,
            Deed::Wash => 1.0 + 0.25 * personality.conscientiousness + 0.15 * values.tradition,
            Deed::Eat => 1.0 + 0.15 * values.hedonism,
            Deed::Sleep => 1.0 + 0.10 * personality.neuroticism.max(0.0),
            Deed::Drink => 1.0,
        };
        raw.max(0.05)
    }

    /// The body's clock. Derived from the planet's rotation, so it is a real
    /// consequence of where someone lives rather than a hardcoded schedule.
    fn circadian(self, phase: DayPhase) -> f32 {
        match (self, phase) {
            (Deed::Sleep, DayPhase::Night) => 3.0,
            // Below 1.0: an evening has to be genuinely exhausting before it beats
            // staying up. Any higher and people turn in at six and sleep twice a day.
            (Deed::Sleep, DayPhase::Evening) => 0.7,
            (Deed::Sleep, _) => 0.25,

            (Deed::Work, DayPhase::Morning | DayPhase::Afternoon) => 1.5,
            (Deed::Work, DayPhase::Evening) => 0.6,
            (Deed::Work, DayPhase::Night) => 0.1,

            (Deed::Socialize, DayPhase::Evening) => 1.6,
            (Deed::Socialize, DayPhase::Night) => 0.3,

            (Deed::Eat, DayPhase::Afternoon | DayPhase::Evening) => 1.3,
            (Deed::Eat, DayPhase::Night) => 0.5,

            (Deed::Wander, DayPhase::Night) => 0.4,
            _ => 1.0,
        }
    }
}

/// The four environment channels, per deed where they vary.
///
/// Neutral in Phase 1. Phase 3 fills these from a place's environment vector, at which
/// point neighbourhoods start changing behaviour without any scoring code changing.
#[derive(Clone, Debug, PartialEq)]
pub struct Surroundings {
    /// Channel 1 — whether the option exists here at all. A hard gate; 0 removes it.
    pub availability: [f32; Deed::COUNT],
    /// Channel 2 — what it returns here, relative to typical.
    pub payoff: [f32; Deed::COUNT],
    /// Channel 3 — accumulated pressure, which shortens the time horizon.
    pub stress: f32,
    /// Channel 4 — how prevalent this is locally, 0.5 being unremarkable.
    pub norms: [f32; Deed::COUNT],
}

impl Surroundings {
    pub fn neutral() -> Surroundings {
        Surroundings {
            availability: [1.0; Deed::COUNT],
            payoff: [1.0; Deed::COUNT],
            stress: 0.0,
            norms: [0.5; Deed::COUNT],
        }
    }

    /// The stress this place imposes. Named for symmetry with `discount_rate`.
    pub fn env_stress(&self) -> f32 {
        self.stress
    }

    /// How steeply this person discounts a delayed reward.
    ///
    /// Rises with stress: under scarcity, a payoff a week away is worth much less, so
    /// investment falls. Looks like low motivation, is actually correct reasoning about
    /// a worse deal.
    pub fn discount_rate(&self) -> f32 {
        0.02 + 0.35 * self.stress.clamp(0.0, 1.0)
    }
}

impl Default for Surroundings {
    fn default() -> Self {
        Surroundings::neutral()
    }
}

/// Everything about a person that bears on a decision.
pub struct Mind<'a> {
    pub personality: &'a Personality,
    pub values: &'a Values,
    pub needs: &'a Needs,
    pub age_years: f64,
    /// What *this person* takes to be usual here, which is not the same as what is usual
    /// here — see `Person::learn_norms`.
    pub norms: &'a [f32; Deed::COUNT],
}

/// Where a decision is being made.
#[derive(Clone, Debug)]
pub struct Situation {
    pub phase: DayPhase,
    pub env: Surroundings,
}

impl Situation {
    pub fn plain(phase: DayPhase) -> Situation {
        Situation {
            phase,
            env: Surroundings::neutral(),
        }
    }
}

/// A decision, with the reasoning kept.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub deed: Deed,
    /// Every option's score, in `Deed::ALL` order — this is what `why()` displays.
    pub scores: [f32; Deed::COUNT],
}

impl Choice {
    pub fn score_of(&self, deed: Deed) -> f32 {
        self.scores[deed as usize]
    }

    /// Options ranked best-first.
    pub fn ranked(&self) -> Vec<(Deed, f32)> {
        let mut ranked: Vec<(Deed, f32)> = Deed::ALL
            .into_iter()
            .map(|d| (d, self.score_of(d)))
            .collect();
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1));
        ranked
    }

    /// Options that were not merely outscored but ruled out — the structural channel,
    /// which is the one worth telling apart from preference.
    pub fn unavailable(&self) -> Vec<Deed> {
        Deed::ALL
            .into_iter()
            .filter(|d| self.score_of(*d) <= 0.0)
            .collect()
    }
}

/// Price every option for this person, here, now.
pub fn score_all(mind: &Mind<'_>, situation: &Situation) -> [f32; Deed::COUNT] {
    let conformity = mind.personality.conformity(mind.age_years);
    let mut scores = [0.0; Deed::COUNT];

    // What a payoff is worth at each remove. Four of the seven deeds pay off the moment
    // they are done, so their discount is exactly one and calling `exp` on nought to find
    // that out is pure waste — and Wash and Socialize pay off at the same remove as each
    // other, so between them there are two distinct answers rather than seven. `expf` was
    // the most expensive function in the whole simulation at a tenth of all instructions;
    // this is five sevenths of one of its two callers, for no change in any result.
    let rate = situation.env.discount_rate();
    let (mut known_delay, mut known_discount) = (f32::NAN, 1.0f32);

    for deed in Deed::ALL {
        let i = deed as usize;

        // Channel 1: a gate, not a weight. Zero means the option does not exist.
        let availability = situation.env.availability[i];
        if availability <= 0.0 {
            continue;
        }

        // What this would actually relieve, weighted by how loudly each need is asking.
        let relief: f32 = deed
            .effects()
            .iter()
            .filter(|(_, delta)| *delta < 0.0)
            .map(|(need, delta)| mind.needs.pressure(*need) * -delta)
            .sum();

        // A floor, so a contented person still has reasons to do things.
        let need_term = relief + 0.02;

        // Channels 2 and 3: what it returns, discounted by how far off the return is.
        let delay = deed.payoff_delay_days();
        let discount = if delay == 0.0 {
            1.0
        } else if delay == known_delay {
            known_discount
        } else {
            known_delay = delay;
            known_discount = (-rate * delay).exp();
            known_discount
        };
        let payoff_term = situation.env.payoff[i] * discount;

        // Channel 4: practice pulls, in proportion to how much this person yields — and it
        // is *their* picture of local practice, not the place's own record of it. Somebody
        // who moved last year is pulled by where they came from.
        let norm_term = (1.0 + conformity * (mind.norms[i] - 0.5)).max(0.05);

        scores[i] = need_term
            * deed.appeal(mind.personality, mind.values)
            * deed.circadian(situation.phase)
            * availability
            * payoff_term
            * norm_term;
    }

    scores
}

/// Choose what to do.
///
/// Softmax rather than argmax: people are varied without being random, and the same
/// person in the same state does not do exactly the same thing forever. Temperature
/// comes from openness, so incurious people are more predictable — which is the sort of
/// thing that should fall out of a trait rather than be asserted.
pub fn choose(mind: &Mind<'_>, situation: &Situation, rng: &mut Rng) -> Choice {
    let scores = score_all(mind, situation);
    let temperature = mind.personality.exploration();

    let best = scores.iter().copied().fold(f32::MIN, f32::max);
    if best <= 0.0 {
        // Everything is gated off. Wandering is always possible.
        return Choice {
            deed: Deed::Wander,
            scores,
        };
    }

    // Scores are unnormalised products, so their absolute scale drifts with how
    // desperate a person is. Comparing each against the best makes the temperature mean
    // the same thing in every situation — and dividing by the best also keeps the
    // exponent negative, which is the overflow guard.
    // A fixed array, not a `Vec`. There are seven deeds and there always will be, so
    // collecting them was a heap allocation and a free on every decision anybody made —
    // twenty-six million of each in a sixty-year world.
    let mut weights = [0.0f32; Deed::COUNT];
    for (i, s) in scores.iter().enumerate() {
        weights[i] = if *s <= 0.0 {
            0.0
        } else if *s >= best {
            // The best option's exponent is exactly nought, so its weight is exactly one.
            1.0
        } else {
            ((s / best - 1.0) / temperature).exp()
        };
    }

    let total: f32 = weights.iter().sum();
    let mut target = rng.unit_f32() * total;
    for (i, weight) in weights.iter().enumerate() {
        target -= weight;
        if target <= 0.0 && *weight > 0.0 {
            return Choice {
                deed: Deed::ALL[i],
                scores,
            };
        }
    }

    // Floating-point residue only; fall back to the best option.
    let deed = Deed::ALL
        .into_iter()
        .max_by(|a, b| scores[*a as usize].total_cmp(&scores[*b as usize]))
        .expect("Deed::ALL is never empty");
    Choice { deed, scores }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::{Domain, WorldSeed};

    fn rng(n: u64) -> Rng {
        WorldSeed::from_u128(0x5eed).stream(Domain::Behavior, n, 0)
    }

    struct Fixture {
        personality: Personality,
        values: Values,
        needs: Needs,
    }

    impl Fixture {
        fn average() -> Fixture {
            Fixture {
                personality: Personality::AVERAGE,
                values: Values::BALANCED,
                needs: Needs::rested(),
            }
        }
        fn mind(&self) -> Mind<'_> {
            Mind {
                personality: &self.personality,
                values: &self.values,
                needs: &self.needs,
                age_years: 30.0,
                // Somebody with no opinion about what is usual, so these tests keep
                // measuring what they were written to measure. The ones about the norm
                // channel set it explicitly.
                norms: &[0.5; Deed::COUNT],
            }
        }
    }

    #[test]
    fn need_drives_the_choice() {
        let mut f = Fixture::average();
        f.needs.set(Need::Thirst, 0.95);
        let choice = choose(&f.mind(), &Situation::plain(DayPhase::Morning), &mut rng(1));
        assert_eq!(choice.deed, Deed::Drink);
    }

    #[test]
    fn the_most_urgent_need_wins_between_two() {
        let mut f = Fixture::average();
        f.needs.set(Need::Hunger, 0.4);
        f.needs.set(Need::Social, 0.95);
        let scores = score_all(&f.mind(), &Situation::plain(DayPhase::Evening));
        assert!(scores[Deed::Socialize as usize] > scores[Deed::Eat as usize]);
    }

    #[test]
    fn time_of_day_shapes_what_makes_sense() {
        let mut f = Fixture::average();
        f.needs.set(Need::Energy, 0.6);

        let night = score_all(&f.mind(), &Situation::plain(DayPhase::Night));
        let morning = score_all(&f.mind(), &Situation::plain(DayPhase::Morning));
        assert!(night[Deed::Sleep as usize] > morning[Deed::Sleep as usize]);
        assert!(morning[Deed::Work as usize] > night[Deed::Work as usize]);
    }

    #[test]
    fn personality_tilts_the_same_situation() {
        let lonely = |extraversion: f32| {
            let mut f = Fixture::average();
            f.personality.extraversion = extraversion;
            f.needs.set(Need::Social, 0.7);
            score_all(&f.mind(), &Situation::plain(DayPhase::Evening))[Deed::Socialize as usize]
        };
        assert!(lonely(2.0) > lonely(-2.0));
    }

    #[test]
    fn a_gated_option_is_not_merely_unattractive() {
        let mut f = Fixture::average();
        f.needs.set(Need::Purpose, 0.9);

        let mut situation = Situation::plain(DayPhase::Morning);
        situation.env.availability[Deed::Work as usize] = 0.0;

        let choice = choose(&f.mind(), &situation, &mut rng(2));
        assert_ne!(choice.deed, Deed::Work);
        assert_eq!(choice.score_of(Deed::Work), 0.0);
        assert!(choice.unavailable().contains(&Deed::Work));
    }

    #[test]
    fn everything_gated_still_leaves_something_to_do() {
        let f = Fixture::average();
        let mut situation = Situation::plain(DayPhase::Morning);
        situation.env.availability = [0.0; Deed::COUNT];

        let choice = choose(&f.mind(), &situation, &mut rng(3));
        assert_eq!(choice.deed, Deed::Wander);
    }

    #[test]
    fn stress_shortens_the_time_horizon() {
        // The scarcity mechanism: same person, same needs, worse circumstances.
        let mut f = Fixture::average();
        f.needs.set(Need::Purpose, 0.8);

        let mut calm = Situation::plain(DayPhase::Morning);
        calm.env.stress = 0.0;
        let mut strained = Situation::plain(DayPhase::Morning);
        strained.env.stress = 1.0;

        let work_calm = score_all(&f.mind(), &calm)[Deed::Work as usize];
        let work_strained = score_all(&f.mind(), &strained)[Deed::Work as usize];
        assert!(
            work_strained < work_calm * 0.5,
            "delayed payoff should collapse under stress: {work_strained} vs {work_calm}"
        );

        // An immediate payoff is untouched — that is what makes it a horizon effect
        // rather than a general penalty.
        f.needs.set(Need::Thirst, 0.8);
        let drink_calm = score_all(&f.mind(), &calm)[Deed::Drink as usize];
        let drink_strained = score_all(&f.mind(), &strained)[Deed::Drink as usize];
        assert!((drink_calm - drink_strained).abs() < 1e-6);
    }

    #[test]
    fn what_somebody_takes_to_be_usual_pulls_on_the_conforming() {
        // The pull comes from the *person's* picture of local practice, not the place's own
        // record of it. Two people standing in the same room, one who has watched these
        // neighbours all their life and one who arrived from somewhere that did the
        // opposite, do not face the same decision — which is the whole of §17.2's second
        // gap, and is why the two norm vectors here belong to the minds rather than to the
        // situation they share.
        let mut f = Fixture::average();
        f.needs.set(Need::Purpose, 0.5);
        let here = Situation::plain(DayPhase::Morning);

        let mut steeped = f.mind();
        let common = [1.0; Deed::COUNT];
        steeped.norms = &common;

        let mut newcomer = f.mind();
        let unheard_of = [0.0; Deed::COUNT];
        newcomer.norms = &unheard_of;

        assert!(
            score_all(&steeped, &here)[Deed::Work as usize]
                > score_all(&newcomer, &here)[Deed::Work as usize],
            "somebody who has seen everyone here work should be readier to"
        );
    }

    #[test]
    fn the_same_seed_makes_the_same_decision() {
        let f = Fixture::average();
        let situation = Situation::plain(DayPhase::Morning);
        let a = choose(&f.mind(), &situation, &mut rng(7));
        let b = choose(&f.mind(), &situation, &mut rng(7));
        assert_eq!(a, b);
    }

    #[test]
    fn choices_vary_without_being_arbitrary() {
        let mut f = Fixture::average();
        f.needs.set(Need::Hunger, 0.9);
        let situation = Situation::plain(DayPhase::Afternoon);

        let mut counts = std::collections::HashMap::new();
        for i in 0..400 {
            let choice = choose(&f.mind(), &situation, &mut rng(i));
            *counts.entry(choice.deed).or_insert(0) += 1;
        }
        assert!(counts.len() > 1, "softmax should explore");
        assert!(
            counts[&Deed::Eat] * 2 > 400,
            "but the obvious answer should dominate: {counts:?}"
        );
    }

    #[test]
    fn incurious_people_are_more_predictable() {
        let spread = |openness: f32| {
            let mut f = Fixture::average();
            f.personality.openness = openness;
            f.needs.set(Need::Hunger, 0.6);
            let situation = Situation::plain(DayPhase::Afternoon);
            let distinct: std::collections::HashSet<Deed> = (0..300)
                .map(|i| choose(&f.mind(), &situation, &mut rng(i)).deed)
                .collect();
            distinct.len()
        };
        assert!(spread(2.5) >= spread(-2.5));
    }

    #[test]
    fn the_reasoning_is_kept_and_ranked() {
        let mut f = Fixture::average();
        f.needs.set(Need::Thirst, 0.9);
        let choice = choose(&f.mind(), &Situation::plain(DayPhase::Morning), &mut rng(4));

        let ranked = choice.ranked();
        assert_eq!(ranked.len(), Deed::COUNT);
        assert_eq!(ranked[0].0, Deed::Drink, "why: thirst outscored everything");
        assert!(ranked[0].1 >= ranked[1].1, "must be sorted");
    }

    #[test]
    fn effects_relieve_what_they_claim_to() {
        for deed in Deed::ALL {
            let mut needs = Needs::rested();
            for (need, _) in deed.effects() {
                needs.set(*need, 0.8);
            }
            let before = needs.total_pressure();
            for (need, delta) in deed.effects() {
                needs.adjust(*need, *delta);
            }
            assert!(
                needs.total_pressure() < before,
                "{deed:?} should be worth doing"
            );
        }
    }

    #[test]
    fn work_costs_energy_while_paying_purpose() {
        let effects = Deed::Work.effects();
        let purpose = effects.iter().find(|(n, _)| *n == Need::Purpose).unwrap().1;
        let energy = effects.iter().find(|(n, _)| *n == Need::Energy).unwrap().1;
        assert!(purpose < 0.0, "work should relieve aimlessness");
        assert!(energy > 0.0, "and should be tiring");
    }
}

