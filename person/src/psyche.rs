//! Personality and values.
//!
//! Five continuous traits instead of `Outlook` plus a bool. The old enums bought six
//! distinguishable people; a vector buys a population. `Outlook` survives as a *label
//! read off* the vector, so prose stays readable while the mechanics stay continuous.
//!
//! Everything here is a **phenotype** — an output. Phase 1 samples it directly, which is
//! a placeholder: Phase 2 computes it from a genome and Phase 3 adds the environment
//! term, at which point nothing outside this module has to change.

use sim_core::Rng;

/// The five factors, as z-scores. 0 is unremarkable, ±1 is a standard deviation out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Personality {
    pub openness: f32,
    pub conscientiousness: f32,
    pub extraversion: f32,
    pub agreeableness: f32,
    pub neuroticism: f32,
}

impl Personality {
    pub const AVERAGE: Personality = Personality {
        openness: 0.0,
        conscientiousness: 0.0,
        extraversion: 0.0,
        agreeableness: 0.0,
        neuroticism: 0.0,
    };

    /// Draw a personality. Placeholder for the genome-plus-upbringing computation.
    pub fn sample(rng: &mut Rng) -> Personality {
        let mut z = || rng.normal().clamp(-3.0, 3.0) as f32;
        Personality {
            openness: z(),
            conscientiousness: z(),
            extraversion: z(),
            agreeableness: z(),
            neuroticism: z(),
        }
    }

    /// The label a reader wants, derived rather than stored.
    ///
    /// Pessimism tracks high neuroticism; optimism needs low neuroticism and some
    /// openness to go with it; everyone else reads as a realist.
    pub fn outlook(&self) -> Outlook {
        if self.neuroticism > 0.5 {
            Outlook::Pessimistic
        } else if self.neuroticism < -0.3 && self.openness > -0.3 {
            Outlook::Optimistic
        } else {
            Outlook::Realist
        }
    }

    /// Also derived — the old `confident` flag, now a reading.
    pub fn is_confident(&self) -> bool {
        self.extraversion - self.neuroticism > 0.4
    }

    /// How much weight this person gives to what those around them are doing.
    ///
    /// Agreeableness raises it, and it peaks in adolescence — the channel by which
    /// neighbourhoods reproduce themselves culturally.
    pub fn conformity(&self, age_years: f64) -> f32 {
        let by_trait = 0.5 + 0.2 * self.agreeableness;
        let by_age = match age_years {
            y if y < 13.0 => 1.1,
            y if y < 20.0 => 1.4, // the peak
            y if y < 35.0 => 1.0,
            y if y < 65.0 => 0.9,
            _ => 0.8,
        };
        (by_trait * by_age).clamp(0.0, 2.0)
    }

    /// Willingness to try something other than the obvious best option.
    ///
    /// This is a softmax temperature applied to *relative* scores (see `deeds`), so it
    /// reads as a fraction: at 0.15, an option worth 85% of the best is roughly as
    /// likely to be picked as the best one. Warmer than about 0.4 and people stop
    /// pursuing their own urgent needs, which reads as scatterbrained rather than free.
    pub fn exploration(&self) -> f32 {
        (0.15 + 0.05 * self.openness).clamp(0.04, 0.40)
    }
}

/// What a person is trying to get out of life. Weights, 0..1.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Values {
    pub security: f32,
    pub achievement: f32,
    pub benevolence: f32,
    pub hedonism: f32,
    pub tradition: f32,
    pub power: f32,
}

impl Values {
    pub const BALANCED: Values = Values {
        security: 0.5,
        achievement: 0.5,
        benevolence: 0.5,
        hedonism: 0.5,
        tradition: 0.5,
        power: 0.5,
    };

    /// Values correlate with traits rather than being independent — an open person
    /// leans hedonic and away from tradition, a conscientious one toward achievement.
    /// Drawing them independently produces incoherent people.
    pub fn sample(rng: &mut Rng, personality: &Personality) -> Values {
        let mut around = |centre: f32| (centre + rng.normal() as f32 * 0.15).clamp(0.0, 1.0);
        Values {
            security: around(0.5 + 0.10 * personality.neuroticism),
            achievement: around(0.5 + 0.12 * personality.conscientiousness),
            benevolence: around(0.5 + 0.12 * personality.agreeableness),
            hedonism: around(0.5 + 0.08 * personality.openness),
            tradition: around(0.5 - 0.10 * personality.openness),
            power: around(0.5 + 0.08 * personality.extraversion),
        }
    }
}

/// A readable summary of a personality. Presentation only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Outlook {
    Optimistic,
    Pessimistic,
    Realist,
}

impl Outlook {
    pub const fn label(self) -> &'static str {
        match self {
            Outlook::Optimistic => "optimistic",
            Outlook::Pessimistic => "pessimistic",
            Outlook::Realist => "level-headed",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::{Domain, WorldSeed};

    fn rng(n: u64) -> Rng {
        WorldSeed::from_u128(0x9001).stream(Domain::Behavior, n, 0)
    }

    #[test]
    fn a_population_is_roughly_standard_normal() {
        let people: Vec<Personality> = (0..4_000)
            .map(|i| Personality::sample(&mut rng(i)))
            .collect();
        let n = people.len() as f32;

        let mean = people.iter().map(|p| p.openness).sum::<f32>() / n;
        let var = people
            .iter()
            .map(|p| (p.openness - mean).powi(2))
            .sum::<f32>()
            / n;
        assert!(mean.abs() < 0.1, "mean {mean}");
        assert!((var - 1.0).abs() < 0.15, "variance {var}");
    }

    #[test]
    fn traits_are_independent_before_genetics_arrives() {
        let people: Vec<Personality> = (0..3_000)
            .map(|i| Personality::sample(&mut rng(i)))
            .collect();
        let n = people.len() as f32;
        let corr = people
            .iter()
            .map(|p| p.openness * p.neuroticism)
            .sum::<f32>()
            / n;
        // Pleiotropy in Phase 2 is what will make these correlate. Not yet.
        assert!(corr.abs() < 0.08, "unexpected correlation {corr}");
    }

    #[test]
    fn outlook_is_read_off_the_vector() {
        let anxious = Personality {
            neuroticism: 1.2,
            ..Personality::AVERAGE
        };
        assert_eq!(anxious.outlook(), Outlook::Pessimistic);

        let sunny = Personality {
            neuroticism: -1.0,
            openness: 0.5,
            ..Personality::AVERAGE
        };
        assert_eq!(sunny.outlook(), Outlook::Optimistic);

        assert_eq!(Personality::AVERAGE.outlook(), Outlook::Realist);
    }

    #[test]
    fn all_three_outlooks_occur_in_a_population() {
        let mut counts = [0; 3];
        for i in 0..2_000 {
            match Personality::sample(&mut rng(i)).outlook() {
                Outlook::Optimistic => counts[0] += 1,
                Outlook::Pessimistic => counts[1] += 1,
                Outlook::Realist => counts[2] += 1,
            }
        }
        assert!(counts.iter().all(|c| *c > 100), "lopsided: {counts:?}");
    }

    #[test]
    fn confidence_needs_more_than_calm() {
        let bold = Personality {
            extraversion: 1.0,
            neuroticism: -0.5,
            ..Personality::AVERAGE
        };
        let timid = Personality {
            extraversion: -1.0,
            neuroticism: 1.0,
            ..Personality::AVERAGE
        };
        assert!(bold.is_confident());
        assert!(!timid.is_confident());
    }

    #[test]
    fn conformity_peaks_in_adolescence() {
        let p = Personality::AVERAGE;
        let child = p.conformity(8.0);
        let teenager = p.conformity(16.0);
        let adult = p.conformity(40.0);
        let elder = p.conformity(70.0);

        assert!(teenager > child);
        assert!(teenager > adult);
        assert!(adult > elder);
    }

    #[test]
    fn agreeable_people_conform_more() {
        let yielding = Personality {
            agreeableness: 2.0,
            ..Personality::AVERAGE
        };
        let contrary = Personality {
            agreeableness: -2.0,
            ..Personality::AVERAGE
        };
        assert!(yielding.conformity(30.0) > contrary.conformity(30.0));
    }

    #[test]
    fn openness_drives_exploration() {
        let curious = Personality {
            openness: 2.0,
            ..Personality::AVERAGE
        };
        let settled = Personality {
            openness: -2.0,
            ..Personality::AVERAGE
        };
        assert!(curious.exploration() > settled.exploration());
        assert!(settled.exploration() > 0.0, "nobody is perfectly rigid");
    }

    #[test]
    fn values_track_traits_rather_than_being_independent() {
        let diligent = Personality {
            conscientiousness: 2.5,
            ..Personality::AVERAGE
        };
        let idle = Personality {
            conscientiousness: -2.5,
            ..Personality::AVERAGE
        };

        let mean_achievement = |p: &Personality| {
            let total: f32 = (0..400)
                .map(|i| Values::sample(&mut rng(i), p).achievement)
                .sum();
            total / 400.0
        };
        assert!(mean_achievement(&diligent) > mean_achievement(&idle));
    }

    #[test]
    fn values_stay_in_range() {
        for i in 0..500 {
            let p = Personality::sample(&mut rng(i));
            let v = Values::sample(&mut rng(i + 9_000), &p);
            for weight in [
                v.security,
                v.achievement,
                v.benevolence,
                v.hedonism,
                v.tradition,
                v.power,
            ] {
                assert!((0.0..=1.0).contains(&weight), "{weight}");
            }
        }
    }
}
