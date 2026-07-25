//! Age, health, and death.
//!
//! Age is derived from a birth date and the current clock rather than stored and
//! incremented — the same rule the planet's calendar follows, and for the same reason:
//! a counter can fall out of step with the world, a derivation cannot.

use sim_core::{Duration, Rng, Time};

/// How long something has been alive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Age(Duration);

impl Age {
    pub fn from_birth(born: Time, now: Time) -> Age {
        Age(now.since(born))
    }

    pub fn of_years(years: f64) -> Age {
        Age(Duration::from_secs(
            (years * sim_core::time::SECONDS_PER_YEAR as f64) as u64,
        ))
    }

    pub fn years(self) -> f64 {
        self.0.as_years()
    }

    pub fn elapsed(self) -> Duration {
        self.0
    }

    pub fn stage(self) -> LifeStage {
        LifeStage::at(self.years())
    }
}

/// Coarse bands of a life. Derived from age, never assigned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LifeStage {
    Infant,
    Child,
    Adolescent,
    Adult,
    Elder,
}

impl LifeStage {
    pub fn at(years: f64) -> LifeStage {
        match years {
            y if y < 3.0 => LifeStage::Infant,
            y if y < 13.0 => LifeStage::Child,
            y if y < 20.0 => LifeStage::Adolescent,
            y if y < 65.0 => LifeStage::Adult,
            _ => LifeStage::Elder,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            LifeStage::Infant => "infant",
            LifeStage::Child => "child",
            LifeStage::Adolescent => "adolescent",
            LifeStage::Adult => "adult",
            LifeStage::Elder => "elder",
        }
    }

    /// How fast this body runs its needs down, relative to an adult's.
    pub const fn metabolic_scale(self) -> f32 {
        match self {
            LifeStage::Infant => 1.30,
            LifeStage::Child => 1.15,
            LifeStage::Adolescent => 1.10,
            LifeStage::Adult => 1.00,
            LifeStage::Elder => 0.90,
        }
    }

    /// Whether this stage can look after itself. Gates actions later on.
    pub const fn is_dependent(self) -> bool {
        matches!(self, LifeStage::Infant | LifeStage::Child)
    }
}

/// Physical condition, 1.0 down to 0.0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Health {
    pub vitality: f32,
}

impl Health {
    pub fn hale() -> Health {
        Health { vitality: 1.0 }
    }

    /// Unmet vital needs wear a body down; met ones let it recover, slowly.
    ///
    /// Recovery is deliberately slower than decline. A body that bounced back as fast
    /// as it broke down would make deprivation consequence-free, and starvation would
    /// be an inconvenience rather than a cause of death.
    pub fn respond_to(&mut self, vital_pressure: f32, elapsed: Duration) {
        const DECLINE_PER_DAY: f32 = 0.30;
        const RECOVERY_PER_DAY: f32 = 0.05;

        let days = elapsed.as_days() as f32;
        if days <= 0.0 {
            return;
        }
        // Below this the body copes and mends; above it, it loses ground.
        //
        // Set above the range ordinary life oscillates through. Needs necessarily
        // climb between meals and sleeps — an option only wins once its need is
        // pressing — so a threshold below that peak would have healthy people
        // slowly wasting away from the normal rhythm of being alive. Pressure is
        // squared level, so 0.45 is roughly "chronically hungry, thirsty and tired
        // all at once", which is the right place for health to start going.
        const TOLERABLE: f32 = 0.45;
        let delta = if vital_pressure > TOLERABLE {
            -(vital_pressure - TOLERABLE) * DECLINE_PER_DAY * days
        } else {
            (TOLERABLE - vital_pressure) * RECOVERY_PER_DAY * days
        };
        self.vitality = (self.vitality + delta).clamp(0.0, 1.0);
    }

    /// How much this condition multiplies the risk of dying.
    ///
    /// Perfect health is neutral; a failing body raises the hazard steeply rather than
    /// linearly, which is what makes a bad winter kill the already-weak first.
    pub fn frailty(self) -> f64 {
        let deficit = (1.0 - self.vitality).clamp(0.0, 1.0) as f64;
        1.0 + 8.0 * deficit * deficit
    }

    pub fn is_dead(self) -> bool {
        self.vitality <= 0.0
    }
}

impl Default for Health {
    fn default() -> Self {
        Health::hale()
    }
}

/// A mortality schedule: the Siler competing-hazards model.
///
/// Three causes added together, which is what gives real life tables their bathtub
/// shape — high risk in infancy, a long flat plateau, then senescence rising
/// exponentially. A single exponential would make childhood safe and old age arrive
/// too gently.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mortality {
    /// Infant risk at birth, per year.
    pub juvenile: f64,
    /// How fast infant risk falls away.
    pub juvenile_decay: f64,
    /// Age-independent risk — accidents, violence, chance.
    pub baseline: f64,
    /// Senescent risk scale.
    pub senescent: f64,
    /// How fast senescent risk compounds with age.
    pub senescence_rate: f64,
}

impl Mortality {
    /// Roughly a pre-industrial-to-modern human blend. A placeholder until medicine,
    /// nutrition, and violence are simulated and can drive these directly.
    pub const HUMAN: Mortality = Mortality {
        juvenile: 0.060,
        juvenile_decay: 1.2,
        baseline: 0.0005,
        senescent: 0.00009,
        senescence_rate: 0.0855,
    };

    /// Instantaneous hazard, per year.
    pub fn hazard(&self, age: Age) -> f64 {
        let t = age.years().max(0.0);
        self.juvenile * (-self.juvenile_decay * t).exp()
            + self.baseline
            + self.senescent * (self.senescence_rate * t).exp()
    }

    /// Probability of dying within `over`, given a frailty multiplier.
    ///
    /// Converts a continuous hazard to a probability properly, so that the answer does
    /// not depend on how finely time is stepped — checking monthly twelve times must
    /// equal checking once a year.
    pub fn probability(&self, age: Age, over: Duration, frailty: f64) -> f64 {
        let integrated = self.hazard(age) * frailty * over.as_years();
        1.0 - (-integrated).exp()
    }

    pub fn rolls_death(&self, age: Age, over: Duration, frailty: f64, rng: &mut Rng) -> bool {
        rng.chance(self.probability(age, over, frailty))
    }

    /// Median age at death under this schedule, by numeric integration. For validating
    /// that a change to the parameters still produces a plausible population.
    pub fn median_lifespan(&self) -> f64 {
        let step = 0.05;
        let mut survival: f64 = 1.0;
        let mut t = 0.0;
        while t < 200.0 {
            survival *= (-self.hazard(Age::of_years(t)) * step).exp();
            if survival <= 0.5 {
                return t;
            }
            t += step;
        }
        t
    }
}

impl Default for Mortality {
    fn default() -> Self {
        Mortality::HUMAN
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::{Domain, WorldSeed};

    fn rng() -> Rng {
        WorldSeed::from_u128(11).stream(Domain::Demography, 0, 0)
    }

    #[test]
    fn age_is_derived_from_birth_and_now() {
        let born = Time::ORIGIN + Duration::from_days(10);
        let now = born + Duration::from_years(30);
        let age = Age::from_birth(born, now);
        assert!((age.years() - 30.0).abs() < 0.01);
        assert_eq!(age.stage(), LifeStage::Adult);
    }

    #[test]
    fn a_person_not_yet_born_has_no_negative_age() {
        let born = Time::ORIGIN + Duration::from_years(5);
        let age = Age::from_birth(born, Time::ORIGIN);
        assert_eq!(age.years(), 0.0);
    }

    #[test]
    fn stages_partition_a_lifetime() {
        assert_eq!(LifeStage::at(0.0), LifeStage::Infant);
        assert_eq!(LifeStage::at(2.9), LifeStage::Infant);
        assert_eq!(LifeStage::at(3.0), LifeStage::Child);
        assert_eq!(LifeStage::at(12.9), LifeStage::Child);
        assert_eq!(LifeStage::at(13.0), LifeStage::Adolescent);
        assert_eq!(LifeStage::at(20.0), LifeStage::Adult);
        assert_eq!(LifeStage::at(64.9), LifeStage::Adult);
        assert_eq!(LifeStage::at(65.0), LifeStage::Elder);
        assert_eq!(LifeStage::at(120.0), LifeStage::Elder);
    }

    #[test]
    fn only_the_young_are_dependent() {
        assert!(LifeStage::Infant.is_dependent());
        assert!(LifeStage::Child.is_dependent());
        assert!(!LifeStage::Adolescent.is_dependent());
        assert!(!LifeStage::Elder.is_dependent());
    }

    #[test]
    fn mortality_has_the_bathtub_shape() {
        let m = Mortality::HUMAN;
        let (infant, child, young_adult, midlife, old) = (
            m.hazard(Age::of_years(0.0)),
            m.hazard(Age::of_years(8.0)),
            m.hazard(Age::of_years(25.0)),
            m.hazard(Age::of_years(50.0)),
            m.hazard(Age::of_years(85.0)),
        );

        assert!(infant > child, "infancy must be riskier than childhood");
        assert!(child < young_adult, "then a long safe plateau");
        assert!(young_adult < midlife);
        assert!(midlife < old, "and senescence at the end");
        assert!(old > infant, "old age must eventually exceed infancy");
    }

    #[test]
    fn median_lifespan_is_plausible() {
        let median = Mortality::HUMAN.median_lifespan();
        assert!(
            (60.0..95.0).contains(&median),
            "median lifespan was {median}, which is not a human population"
        );
    }

    #[test]
    fn death_probability_is_step_size_independent() {
        // Checking monthly must agree with checking annually, or demography would
        // depend on how often the scheduler happens to run.
        let m = Mortality::HUMAN;
        let age = Age::of_years(70.0);

        let yearly = m.probability(age, Duration::from_years(1), 1.0);
        let monthly = m.probability(age, Duration::from_secs(31_557_600 / 12), 1.0);
        let compounded = 1.0 - (1.0 - monthly).powi(12);

        assert!(
            (yearly - compounded).abs() < 1e-6,
            "{yearly} vs {compounded}"
        );
    }

    #[test]
    fn probabilities_stay_in_range_even_at_absurd_ages() {
        let m = Mortality::HUMAN;
        let p = m.probability(Age::of_years(150.0), Duration::from_years(1), 20.0);
        assert!((0.0..=1.0).contains(&p), "got {p}");
        assert!(p > 0.99, "nobody survives that");
    }

    #[test]
    fn frailty_raises_risk_without_being_linear() {
        let hale = Health::hale();
        let ailing = Health { vitality: 0.5 };
        let dying = Health { vitality: 0.1 };

        assert_eq!(hale.frailty(), 1.0);
        assert!(ailing.frailty() > hale.frailty());
        // Steep, not proportional: the last of the decline costs the most.
        assert!(dying.frailty() > ailing.frailty() * 2.0);
    }

    #[test]
    fn deprivation_wears_a_body_down_and_relief_mends_it() {
        let mut health = Health::hale();
        health.respond_to(0.9, Duration::from_days(2));
        let worn = health.vitality;
        assert!(worn < 1.0, "starvation should cost something");

        health.respond_to(0.0, Duration::from_days(2));
        assert!(health.vitality > worn, "relief should help");
        assert!(
            health.vitality < 1.0,
            "but not undo two days of it in two days"
        );
    }

    #[test]
    fn sustained_deprivation_is_fatal() {
        let mut health = Health::hale();
        for _ in 0..30 {
            health.respond_to(1.0, Duration::from_days(1));
        }
        assert!(health.is_dead());
    }

    #[test]
    fn health_never_leaves_its_range() {
        let mut health = Health::hale();
        health.respond_to(0.0, Duration::from_years(10));
        assert_eq!(health.vitality, 1.0);
        health.respond_to(1.0, Duration::from_years(10));
        assert_eq!(health.vitality, 0.0);
    }

    #[test]
    fn a_simulated_cohort_dies_off_plausibly() {
        // The demographic check: run 2000 lives to their end and look at the shape.
        let m = Mortality::HUMAN;
        let mut rng = rng();
        let mut ages = Vec::new();

        for _ in 0..2_000 {
            let mut years = 0.0;
            while years < 200.0 {
                if m.rolls_death(Age::of_years(years), Duration::from_years(1), 1.0, &mut rng) {
                    break;
                }
                years += 1.0;
            }
            ages.push(years);
        }
        ages.sort_by(f64::total_cmp);

        let median = ages[ages.len() / 2];
        let infant_deaths = ages.iter().filter(|a| **a < 1.0).count() as f64 / ages.len() as f64;
        let centenarians = ages.iter().filter(|a| **a >= 100.0).count() as f64 / ages.len() as f64;

        assert!((60.0..95.0).contains(&median), "median {median}");
        assert!(
            (0.02..0.12).contains(&infant_deaths),
            "infant mortality {infant_deaths}"
        );
        assert!(centenarians < 0.05, "too many centenarians: {centenarians}");
        assert!(
            *ages.last().unwrap() < 130.0,
            "nobody should reach {}",
            ages.last().unwrap()
        );
    }
}
