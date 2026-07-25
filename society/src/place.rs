//! Neighbourhoods, and what they do to the people in them.
//!
//! A place is not authored as "poor" or "leafy". It carries a vector of measurable
//! properties, most of which are **read off its residents** — affluence from what they
//! have, churn from how often they leave, norms from what they actually do. The
//! archetypes in the design's table are labels derived from that vector, exactly as
//! `Outlook` is a label derived from a personality.
//!
//! The consequence is a loop rather than a setting: who lives somewhere determines what
//! it is like, what it is like determines what its children become, and what they become
//! determines where they can afford to live.

use person::{Deed, Surroundings};
use sim_core::Id;

pub type PlaceId = Id<Place>;

/// The measurable properties of a neighbourhood, each 0 to 1.
#[derive(Clone, Debug, PartialEq)]
pub struct EnvironmentVector {
    pub affluence: f32,
    pub density: f32,
    pub safety: f32,
    /// Dense ties *within* the neighbourhood.
    pub bonding_capital: f32,
    /// Ties *out*, to opportunity elsewhere.
    pub bridging_capital: f32,
    pub education_access: f32,
    pub job_opportunity: f32,
    pub services: f32,
    pub pollution: f32,
    /// Residential turnover. Erodes bonding capital.
    pub churn: f32,
    /// How prevalent each deed is locally, 0.5 being unremarkable.
    pub norms: [f32; Deed::COUNT],
}

impl EnvironmentVector {
    /// A featureless place — every dial at the middle. The baseline a world starts from
    /// before its residents have made it into anywhere in particular.
    pub fn unremarkable() -> EnvironmentVector {
        EnvironmentVector {
            affluence: 0.5,
            density: 0.5,
            safety: 0.5,
            bonding_capital: 0.5,
            bridging_capital: 0.5,
            education_access: 0.5,
            job_opportunity: 0.5,
            services: 0.5,
            pollution: 0.5,
            churn: 0.2,
            norms: [0.5; Deed::COUNT],
        }
    }

    /// How well off this place is overall, as one number. Used for sorting and for the
    /// shared-environment term a child raised here receives.
    pub fn quality(&self) -> f32 {
        (self.affluence * 0.3
            + self.safety * 0.2
            + self.education_access * 0.2
            + self.job_opportunity * 0.2
            + self.services * 0.1)
            .clamp(0.0, 1.0)
    }

    /// The shared-environment contribution, as a z-score.
    ///
    /// Quality runs 0 to 1 and personality terms are z-scores, so the middle of the
    /// range has to map to zero — otherwise every child everywhere gets a positive
    /// shove and the population mean drifts.
    pub fn upbringing(&self) -> f32 {
        (self.quality() - 0.5) * 2.5
    }

    /// Accumulated pressure on someone living here — the third behaviour channel.
    pub fn stress(&self) -> f32 {
        ((1.0 - self.safety) * 0.4 + self.churn * 0.2 + (1.0 - self.affluence) * 0.4)
            .clamp(0.0, 1.0)
    }

    /// The four behaviour channels, as the scoring system sees them.
    ///
    /// This is where a neighbourhood stops being a description and starts changing what
    /// people do. Note what is *not* here: food, water and sleep keep a neutral payoff.
    /// Material scarcity is the economy's job, and gating survival on affluence before
    /// there is a food supply to model would just starve the poor by fiat.
    pub fn surroundings(&self, dependent: bool) -> Surroundings {
        let mut s = Surroundings::neutral();

        // Channel 3: the accumulated pressure of living here, which shortens the time
        // horizon and so suppresses anything that pays off slowly.
        s.stress = self.stress();

        // Channel 4: what people around here actually do.
        s.norms = self.norms;

        // Channel 1: whether the option exists at all. Work is the one that varies —
        // but it is floored, because subsistence work exists nearly everywhere. Letting
        // access fall with local prosperity all the way to nothing gave the model no
        // equilibrium at all: poor meant no work, no work meant poorer, and every
        // neighbourhood in every world slid to destitution within two generations.
        // What a poor place lacks is *good* work, which is channel two's business.
        s.availability[Deed::Work as usize] = if dependent {
            0.0
        } else {
            (0.35 + 0.65 * self.job_opportunity).clamp(0.0, 1.0)
        };

        // Channel 2: what the same effort returns here.
        s.payoff[Deed::Work as usize] = 0.30 + 1.40 * self.job_opportunity;
        s.payoff[Deed::Socialize as usize] = 0.50 + self.bonding_capital;
        s.payoff[Deed::Wash as usize] = 0.40 + 1.20 * self.services;
        s.payoff[Deed::Wander as usize] = 0.40 + 1.20 * self.safety;
        s
    }

    /// The nearest archetype. A reading, never a stored fact.
    pub fn archetype(&self) -> Archetype {
        Archetype::ALL
            .into_iter()
            .min_by(|a, b| {
                self.distance_to(a.prototype())
                    .total_cmp(&self.distance_to(b.prototype()))
            })
            .expect("Archetype::ALL is never empty")
    }

    fn distance_to(&self, other: EnvironmentVector) -> f32 {
        // Only the columns the archetype table distinguishes; the rest follow from them.
        [
            (self.affluence, other.affluence),
            (self.density, other.density),
            (self.safety, other.safety),
            (self.bonding_capital, other.bonding_capital),
            (self.bridging_capital, other.bridging_capital),
            (self.job_opportunity, other.job_opportunity),
            (self.churn, other.churn),
        ]
        .iter()
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f32>()
        .sqrt()
    }
}

impl Default for EnvironmentVector {
    fn default() -> Self {
        EnvironmentVector::unremarkable()
    }
}

/// Readable names for regions of the environment space. Presentation only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Archetype {
    DistressedUrban,
    WorkingClass,
    Suburb,
    MetropolitanCore,
    AffluentEnclave,
    Rural,
}

impl Archetype {
    pub const ALL: [Archetype; 6] = [
        Archetype::DistressedUrban,
        Archetype::WorkingClass,
        Archetype::Suburb,
        Archetype::MetropolitanCore,
        Archetype::AffluentEnclave,
        Archetype::Rural,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Archetype::DistressedUrban => "distressed urban",
            Archetype::WorkingClass => "working-class",
            Archetype::Suburb => "suburb",
            Archetype::MetropolitanCore => "metropolitan core",
            Archetype::AffluentEnclave => "affluent enclave",
            Archetype::Rural => "rural",
        }
    }

    /// The corner of the space this name describes.
    ///
    /// The bonding and bridging columns are the important ones. A distressed
    /// neighbourhood is typically not short on community — it is short on ties that
    /// reach opportunity, and collapsing the two into one "social capital" number would
    /// lose the mechanism that actually limits mobility.
    fn prototype(self) -> EnvironmentVector {
        let base = EnvironmentVector::unremarkable();
        match self {
            Archetype::DistressedUrban => EnvironmentVector {
                affluence: 0.15,
                density: 0.8,
                safety: 0.2,
                bonding_capital: 0.75,
                bridging_capital: 0.15,
                job_opportunity: 0.2,
                churn: 0.7,
                ..base
            },
            Archetype::WorkingClass => EnvironmentVector {
                affluence: 0.35,
                density: 0.5,
                safety: 0.5,
                bonding_capital: 0.75,
                bridging_capital: 0.4,
                job_opportunity: 0.5,
                churn: 0.2,
                ..base
            },
            Archetype::Suburb => EnvironmentVector {
                affluence: 0.65,
                density: 0.25,
                safety: 0.8,
                bonding_capital: 0.5,
                bridging_capital: 0.5,
                job_opportunity: 0.55,
                churn: 0.2,
                ..base
            },
            Archetype::MetropolitanCore => EnvironmentVector {
                affluence: 0.6,
                density: 0.95,
                safety: 0.45,
                bonding_capital: 0.25,
                bridging_capital: 0.85,
                job_opportunity: 0.85,
                churn: 0.7,
                ..base
            },
            Archetype::AffluentEnclave => EnvironmentVector {
                affluence: 0.9,
                density: 0.2,
                safety: 0.9,
                bonding_capital: 0.5,
                bridging_capital: 0.9,
                job_opportunity: 0.8,
                churn: 0.05,
                ..base
            },
            Archetype::Rural => EnvironmentVector {
                affluence: 0.35,
                density: 0.05,
                safety: 0.7,
                bonding_capital: 0.8,
                bridging_capital: 0.15,
                job_opportunity: 0.25,
                churn: 0.05,
                ..base
            },
        }
    }
}

/// What a year of living here looked like, gathered from the residents themselves.
#[derive(Clone, Debug, Default)]
pub struct Census {
    pub households: u32,
    pub adults: u32,
    /// Mean standing of the adults living here.
    pub mean_standing: f32,
    /// Households that arrived this year.
    pub arrivals: u32,
    /// How often each deed was chosen here.
    pub deeds: [u32; Deed::COUNT],
}

/// A neighbourhood.
#[derive(Clone, Debug)]
pub struct Place {
    pub name: String,
    /// Households it holds before crowding. Authored — the one physical fact a place
    /// has until it sits on a grid cell.
    pub capacity: u32,
    pub env: EnvironmentVector,
}

/// How fast housing supply follows demand, per year.
///
/// Without this, capacity is fixed at whatever the founding population needed and every
/// quarter is permanently overcrowded once the world grows. Crowding then dominates
/// every choice of where to live, appeal equalises across the inhabited places, and no
/// neighbourhood is ever meaningfully better or worse than another — which quietly
/// switched off the entire environment mechanism.
const BUILD_RATE: f32 = 0.06;

/// How fast a place's character follows its residents.
///
/// Slow on purpose. A neighbourhood that re-derived itself instantly from this year's
/// arrivals would flicker between archetypes and no child would grow up anywhere in
/// particular; the lag is what makes somewhere have a *character* rather than a
/// current occupancy.
const ADJUSTMENT: f32 = 0.15;

impl Place {
    pub fn new(name: impl Into<String>, capacity: u32) -> Place {
        Place {
            name: name.into(),
            capacity: capacity.max(1),
            env: EnvironmentVector::unremarkable(),
        }
    }

    /// Bring the place's character into line with who is living in it.
    ///
    /// Almost everything here is derived rather than set. Affluence is what the
    /// residents have; safety and services follow affluence; bonding capital is what
    /// low turnover builds; norms are literally what people did.
    pub fn observe(&mut self, census: &Census) {
        self.build_for(census.households);
        let occupancy = (census.households as f32 / self.capacity as f32).clamp(0.0, 1.5);

        // An empty place is vacant, not destitute. Reading affluence off nobody gives
        // zero, which makes somewhere with no one in it look like the worst slum in the
        // world — so nobody moves there, so it stays empty. Emptying out has to leave a
        // neighbourhood's character intact and waiting.
        if census.adults == 0 {
            self.env.density += (0.0 - self.env.density) * ADJUSTMENT;
            self.env.churn += (0.0 - self.env.churn) * ADJUSTMENT;
            return;
        }
        let churn = if census.households == 0 {
            0.0
        } else {
            (census.arrivals as f32 / census.households as f32).clamp(0.0, 1.0)
        };

        let target = EnvironmentVector {
            affluence: census.mean_standing.clamp(0.0, 1.0),
            density: (occupancy / 1.5).clamp(0.0, 1.0),
            // Prosperity and stability buy safety; crowding costs a little of it.
            safety: (0.25 + 0.6 * census.mean_standing - 0.25 * churn - 0.1 * occupancy)
                .clamp(0.0, 1.0),
            // Built by staying put — *or* by needing each other. That second term is
            // why a poor neighbourhood has dense community despite heavy turnover:
            // where there is no money and no services, neighbours are the services.
            // Without it, hardship reads as social emptiness, which is both wrong and
            // the lazy version of this simulation.
            bonding_capital: ((0.5 - 0.45 * churn) + 0.45 * (1.0 - census.mean_standing))
                .clamp(0.0, 1.0),
            // Ties that reach *out* need means. Crowding alone buys none of them —
            // that is the whole difference between a metropolitan core and a slum,
            // which are equally dense and not at all equally connected.
            bridging_capital: (0.05
                + 0.60 * census.mean_standing
                + 0.30 * occupancy * census.mean_standing)
                .clamp(0.0, 1.0),
            education_access: (0.15 + 0.8 * census.mean_standing).clamp(0.0, 1.0),
            // Density multiplies opportunity, it does not create it — the same rule as
            // bridging capital, and for the same reason. A crowded poor neighbourhood
            // has no more work in it than an empty one; a crowded rich one has a great
            // deal. Letting occupancy add opportunity on its own would quietly make
            // slums the best places in the world to look for a job.
            job_opportunity: (0.30
                + 0.50 * census.mean_standing
                + 0.20 * occupancy * census.mean_standing)
                .clamp(0.0, 1.0),
            services: (0.1 + 0.8 * census.mean_standing).clamp(0.0, 1.0),
            pollution: (0.15 + 0.5 * occupancy - 0.2 * census.mean_standing).clamp(0.0, 1.0),
            churn,
            // A place nobody was watching reports no deeds. That is silence, not a
            // sudden absence of local custom, so its norms are left where they were.
            norms: if census.deeds.iter().all(|c| *c == 0) {
                self.env.norms
            } else {
                norms_from(&census.deeds)
            },
        };

        self.env = blend(&self.env, &target, ADJUSTMENT);
    }

    pub fn archetype(&self) -> Archetype {
        self.env.archetype()
    }

    /// Housing follows demand, slowly and in one direction.
    ///
    /// Somewhere crowded gets built out; somewhere emptied keeps its buildings, because
    /// houses do not disappear when the people do. The lag is what lets a place be
    /// genuinely overcrowded for a while rather than instantly accommodating everyone.
    fn build_for(&mut self, households: u32) {
        if households > self.capacity {
            let wanted = households as f32;
            let grown = self.capacity as f32 + (wanted - self.capacity as f32) * BUILD_RATE;
            self.capacity = grown.ceil() as u32;
        }
    }

    /// Whether a household of this standing could get in, given how full it is.
    ///
    /// Scarcity is what actually sorts people. Without it every household simply moves
    /// to the best neighbourhood, the best neighbourhood absorbs everyone, and the world
    /// converges on one uniform place — which is what happened before occupancy entered
    /// this calculation. Somewhere with room to spare will take you slightly above your
    /// means; somewhere full wants better than its own average.
    pub fn admits(&self, standing: f32, occupancy: f32) -> bool {
        let slack = if occupancy >= 1.0 {
            // Past capacity, getting in costs steeply more the fuller it is. A flat
            // penalty is not enough: somewhere desirable simply absorbed the whole
            // world and ran at three times its capacity while other quarters emptied.
            -0.05 - 0.45 * (occupancy - 1.0)
        } else {
            0.15 * (1.0 - occupancy)
        };
        standing + slack >= self.env.affluence
    }
}

/// What people actually did here, as a distribution around the unremarkable 0.5.
fn norms_from(counts: &[u32; Deed::COUNT]) -> [f32; Deed::COUNT] {
    let total: u32 = counts.iter().sum();
    if total == 0 {
        return [0.5; Deed::COUNT];
    }
    // Relative to an even split, so a deed done more often than average reads above 0.5.
    let even = 1.0 / Deed::COUNT as f32;
    let mut norms = [0.5; Deed::COUNT];
    for (norm, count) in norms.iter_mut().zip(counts) {
        let share = *count as f32 / total as f32;
        *norm = (0.5 + (share - even) / (2.0 * even)).clamp(0.0, 1.0);
    }
    norms
}

fn blend(from: &EnvironmentVector, to: &EnvironmentVector, rate: f32) -> EnvironmentVector {
    let lerp = |a: f32, b: f32| a + (b - a) * rate;
    let mut norms = [0.0; Deed::COUNT];
    for (i, norm) in norms.iter_mut().enumerate() {
        *norm = lerp(from.norms[i], to.norms[i]);
    }
    EnvironmentVector {
        affluence: lerp(from.affluence, to.affluence),
        density: lerp(from.density, to.density),
        safety: lerp(from.safety, to.safety),
        bonding_capital: lerp(from.bonding_capital, to.bonding_capital),
        bridging_capital: lerp(from.bridging_capital, to.bridging_capital),
        education_access: lerp(from.education_access, to.education_access),
        job_opportunity: lerp(from.job_opportunity, to.job_opportunity),
        services: lerp(from.services, to.services),
        pollution: lerp(from.pollution, to.pollution),
        churn: lerp(from.churn, to.churn),
        norms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn census(households: u32, standing: f32, arrivals: u32) -> Census {
        Census {
            households,
            adults: households * 2,
            mean_standing: standing,
            arrivals,
            deeds: [0; Deed::COUNT],
        }
    }

    /// Let a place settle into the character its residents imply.
    fn settled(capacity: u32, census: &Census) -> Place {
        let mut place = Place::new("Somewhere", capacity);
        for _ in 0..80 {
            place.observe(census);
        }
        place
    }

    #[test]
    fn a_hard_place_shuts_doors_rather_than_discouraging() {
        let slum = settled(20, &census(30, 0.05, 18));
        let enclave = settled(40, &census(12, 0.95, 0));

        let poor = slum.env.surroundings(false);
        let rich = enclave.env.surroundings(false);

        // Channel one: work is scarcer, but not absent. Subsistence work exists nearly
        // everywhere — a place with *no* work at all left the model no equilibrium and
        // slid every world to destitution.
        assert!(
            poor.availability[Deed::Work as usize] < rich.availability[Deed::Work as usize] - 0.2,
            "work should be markedly scarcer: {} vs {}",
            poor.availability[Deed::Work as usize],
            rich.availability[Deed::Work as usize]
        );
        assert!(
            poor.availability[Deed::Work as usize] > 0.3,
            "but there should still be something to do"
        );
        assert!(rich.availability[Deed::Work as usize] > 0.8);

        // Channel two: and what work there is returns less.
        assert!(poor.payoff[Deed::Work as usize] < rich.payoff[Deed::Work as usize]);

        // Channel three: which compounds, because stress shortens the horizon that a
        // slow payoff has to survive.
        assert!(poor.env_stress() > rich.env_stress() + 0.3);
        assert!(poor.discount_rate() > rich.discount_rate());

        // Survival is not gated on being rich.
        assert_eq!(poor.availability[Deed::Eat as usize], 1.0);
        assert_eq!(poor.payoff[Deed::Drink as usize], 1.0);
    }

    #[test]
    fn children_have_no_work_to_take_anywhere() {
        let enclave = settled(40, &census(12, 0.95, 0));
        let child = enclave.env.surroundings(true);
        assert_eq!(child.availability[Deed::Work as usize], 0.0);
    }

    #[test]
    fn community_survives_hardship_but_reach_does_not() {
        // The distinction the design leans on hardest. A poor crowded place and a rich
        // crowded place are equally dense and not remotely equally connected — and the
        // poor one is not socially empty, it is socially *enclosed*.
        let slum = settled(20, &census(28, 0.10, 18));
        let core = settled(20, &census(28, 0.75, 18));

        assert!(slum.env.bonding_capital > slum.env.bridging_capital);
        assert!(core.env.bridging_capital > core.env.bonding_capital);
        assert!(
            slum.env.bonding_capital > core.env.bonding_capital,
            "needing each other builds ties: {} vs {}",
            slum.env.bonding_capital,
            core.env.bonding_capital
        );
        assert!(core.env.bridging_capital > slum.env.bridging_capital + 0.3);
    }

    #[test]
    fn every_archetype_recognises_its_own_prototype() {
        for archetype in Archetype::ALL {
            assert_eq!(
                archetype.prototype().archetype(),
                archetype,
                "{archetype:?} should read as itself"
            );
        }
    }

    #[test]
    fn poverty_and_crowding_read_as_a_distressed_neighbourhood() {
        // Nobody wrote "slum" anywhere: this is what low standing, high occupancy and
        // heavy turnover add up to.
        let place = settled(20, &census(28, 0.1, 18));
        assert_eq!(place.archetype(), Archetype::DistressedUrban);

        // And the mechanism that matters is present: community without reach.
        assert!(place.env.bonding_capital > place.env.bridging_capital);
        assert!(place.env.job_opportunity < 0.5);
    }

    #[test]
    fn wealth_and_space_read_as_an_enclave() {
        let place = settled(40, &census(12, 0.95, 0));
        assert_eq!(place.archetype(), Archetype::AffluentEnclave);
        assert!(place.env.bridging_capital > 0.6);
        assert!(place.env.safety > 0.7);
    }

    #[test]
    fn modest_means_and_stability_read_as_working_class() {
        let place = settled(30, &census(18, 0.35, 1));
        assert!(
            matches!(
                place.archetype(),
                Archetype::WorkingClass | Archetype::Rural | Archetype::Suburb
            ),
            "got {:?}",
            place.archetype()
        );
        assert!(
            place.env.bonding_capital > 0.6,
            "stability should build ties"
        );
    }

    #[test]
    fn emptiness_reads_as_rural() {
        let place = settled(200, &census(6, 0.35, 0));
        assert_eq!(place.archetype(), Archetype::Rural);
        assert!(place.env.density < 0.2);
    }

    #[test]
    fn turnover_erodes_community() {
        let stable = settled(20, &census(20, 0.5, 0));
        let transient = settled(20, &census(20, 0.5, 16));
        assert!(
            stable.env.bonding_capital > transient.env.bonding_capital + 0.2,
            "{} vs {}",
            stable.env.bonding_capital,
            transient.env.bonding_capital
        );
        // But churn does not touch the ties that reach out.
        assert!((stable.env.bridging_capital - transient.env.bridging_capital).abs() < 0.05);
    }

    #[test]
    fn a_place_changes_slowly() {
        // A neighbourhood that re-derived itself each year would flicker, and nobody
        // would grow up anywhere in particular.
        let mut place = Place::new("Somewhere", 20);
        let before = place.env.affluence;
        place.observe(&census(20, 1.0, 0));
        let after_one_year = place.env.affluence;

        assert!(after_one_year > before, "it should move");
        assert!(
            after_one_year < 0.65,
            "but not arrive in a single year: {after_one_year}"
        );
    }

    #[test]
    fn a_place_can_change_character_entirely() {
        // Gentrification, as a consequence of who moved in rather than an event.
        let mut place = settled(30, &census(35, 0.1, 20));
        assert_eq!(place.archetype(), Archetype::DistressedUrban);

        let incomers = census(18, 0.95, 2);
        for _ in 0..120 {
            place.observe(&incomers);
        }
        assert_eq!(place.archetype(), Archetype::AffluentEnclave);
    }

    #[test]
    fn quality_and_upbringing_track_each_other_and_centre_on_zero() {
        let poor = settled(20, &census(30, 0.05, 15));
        let rich = settled(40, &census(12, 0.95, 0));
        let middling = EnvironmentVector::unremarkable();

        assert!(poor.env.quality() < rich.env.quality());
        assert!(poor.env.upbringing() < 0.0, "a hard place should cost");
        assert!(rich.env.upbringing() > 0.0, "a good one should help");
        assert_eq!(
            middling.upbringing(),
            0.0,
            "an unremarkable place must not shove the population mean"
        );
    }

    #[test]
    fn hardship_registers_as_stress() {
        let poor = settled(20, &census(30, 0.05, 15));
        let rich = settled(40, &census(12, 0.95, 0));
        assert!(poor.env.stress() > rich.env.stress() + 0.3);
        assert!((0.0..=1.0).contains(&poor.env.stress()));
    }

    #[test]
    fn silence_leaves_local_custom_alone() {
        // A coarsely simulated place emits no deeds. Reading that as "nobody here does
        // anything in particular" would wipe out its character the moment it stopped
        // being watched.
        let mut place = Place::new("Somewhere", 20);
        let mut busy = census(20, 0.5, 0);
        busy.deeds[Deed::Work as usize] = 500;
        for _ in 0..40 {
            place.observe(&busy);
        }
        let working = place.env.norms[Deed::Work as usize];
        assert!(working > 0.7);

        for _ in 0..40 {
            place.observe(&census(20, 0.5, 0));
        }
        assert_eq!(
            place.env.norms[Deed::Work as usize],
            working,
            "silence should not erase custom"
        );
    }

    #[test]
    fn norms_are_what_people_actually_did() {
        let mut deeds = [0u32; Deed::COUNT];
        deeds[Deed::Work as usize] = 700;
        deeds[Deed::Eat as usize] = 100;

        let norms = norms_from(&deeds);
        assert!(
            norms[Deed::Work as usize] > 0.8,
            "working should read as usual"
        );
        assert!(
            norms[Deed::Socialize as usize] < 0.5,
            "what nobody does should read as unusual"
        );

        // With nothing observed, nothing is prevailing.
        assert_eq!(norms_from(&[0; Deed::COUNT]), [0.5; Deed::COUNT]);
    }

    #[test]
    fn admission_depends_on_means_and_on_room() {
        let enclave = settled(40, &census(12, 0.95, 0));
        assert!(!enclave.admits(0.2, 0.3), "means matter");
        assert!(enclave.admits(0.95, 0.3));

        // With room to spare, just below the average is still within reach.
        assert!(enclave.admits(enclave.env.affluence - 0.1, 0.3));
        // Once full, it wants better than its own average — which is what excludes.
        assert!(!enclave.admits(enclave.env.affluence - 0.1, 1.0));
        assert!(enclave.admits(enclave.env.affluence + 0.1, 1.0));
    }

    #[test]
    fn housing_follows_demand_but_never_vanishes() {
        let mut place = Place::new("Somewhere", 20);
        for _ in 0..60 {
            place.observe(&census(80, 0.5, 0));
        }
        assert!(
            place.capacity >= 70,
            "a crowded quarter should get built out: capacity {}",
            place.capacity
        );

        // And then everyone leaves. The buildings stay.
        let grown = place.capacity;
        for _ in 0..40 {
            place.observe(&census(0, 0.0, 0));
        }
        assert_eq!(
            place.capacity, grown,
            "houses do not disappear with the people"
        );
    }

    #[test]
    fn building_lags_behind_the_crowd() {
        // Instant accommodation would mean nowhere is ever crowded, and crowding is
        // half of what makes one quarter different from another.
        let mut place = Place::new("Somewhere", 10);
        place.observe(&census(100, 0.5, 0));
        assert!(
            place.capacity < 30,
            "one year should not house everyone: capacity {}",
            place.capacity
        );
    }

    #[test]
    fn an_empty_place_is_vacant_rather_than_destitute() {
        let mut place = settled(20, &census(18, 0.8, 0));
        let was_affluent = place.env.affluence;

        for _ in 0..20 {
            place.observe(&census(0, 0.0, 0));
        }
        assert!(
            place.env.affluence > was_affluent - 0.05,
            "an emptied neighbourhood should keep its character, not read as a slum"
        );
        assert!(place.env.density < 0.2, "but it should read as empty");
        assert!(place.env.churn.is_finite(), "no division by zero");
    }

    #[test]
    fn crowding_shuts_the_door_progressively() {
        let place = settled(20, &census(20, 0.5, 0));
        let a = place.env.affluence;
        // Plenty of room: a little below the average still gets in.
        assert!(place.admits(a - 0.1, 0.2));
        // Filling up: the same household no longer qualifies.
        assert!(!place.admits(a - 0.1, 0.6));
        // At capacity: you need to beat the average outright.
        assert!(!place.admits(a - 0.01, 1.0));
        // Well over: only much better than average, so somewhere desirable cannot
        // simply swallow the whole world.
        assert!(!place.admits(a + 0.2, 2.0));
        assert!(place.admits(a + 0.6, 2.0));
    }
}
