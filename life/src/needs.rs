//! Needs — the pressures that make anything do anything.
//!
//! Booleans ("is hungry") cannot rank two options; scalars can. Needs run 0 (satisfied)
//! to 1 (desperate), rise on their own, and are what actions compete to reduce.
//!
//! Nothing decays on a tick. A need's level is a function of how long it has been since
//! it was last satisfied, so a person asleep for eight hours costs one calculation on
//! waking rather than 32 updates. That is the same argument the scheduler makes, applied
//! to state instead of events, and it is what keeps a large dormant population free.

use sim_core::Duration;
use std::fmt;

/// The needs every person carries. Animals use a subset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(usize)]
pub enum Need {
    Hunger,
    Thirst,
    Energy,
    Hygiene,
    Social,
    Safety,
    Purpose,
}

impl Need {
    pub const ALL: [Need; 7] = [
        Need::Hunger,
        Need::Thirst,
        Need::Energy,
        Need::Hygiene,
        Need::Social,
        Need::Safety,
        Need::Purpose,
    ];

    pub const COUNT: usize = Need::ALL.len();

    pub const fn label(self) -> &'static str {
        match self {
            Need::Hunger => "hunger",
            Need::Thirst => "thirst",
            Need::Energy => "tiredness",
            Need::Hygiene => "grubbiness",
            Need::Social => "loneliness",
            Need::Safety => "unease",
            Need::Purpose => "aimlessness",
        }
    }

    /// How much of this need accrues per day when nothing is done about it.
    ///
    /// Read these as the reciprocal of how long a body tolerates the deficit before it
    /// is desperate: tiredness and thirst reach that point in about a day, hunger in
    /// under two, aimlessness over a season. Calibrating against tolerance rather than
    /// by feel matters more than it sounds — set thirst fast enough and nobody can
    /// sleep through a night without waking parched, which quietly wrecks the daily
    /// rhythm and then wears health down through it.
    pub const fn daily_rate(self) -> f32 {
        match self {
            Need::Energy => 1.00,
            Need::Thirst => 0.90,
            Need::Hunger => 0.60,
            Need::Hygiene => 0.30,
            Need::Social => 0.25,
            Need::Safety => 0.15,
            Need::Purpose => 0.07,
        }
    }

    /// Needs that kill if left unmet. Loneliness is miserable; thirst is fatal.
    pub const fn is_vital(self) -> bool {
        matches!(self, Need::Hunger | Need::Thirst | Need::Energy)
    }
}

impl fmt::Display for Need {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// One creature's current pressures.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Needs {
    levels: [f32; Need::COUNT],
}

impl Needs {
    /// Everything satisfied — the state a newborn starts in.
    pub fn rested() -> Needs {
        Needs {
            levels: [0.0; Need::COUNT],
        }
    }

    pub fn get(&self, need: Need) -> f32 {
        self.levels[need as usize]
    }

    pub fn set(&mut self, need: Need, level: f32) {
        self.levels[need as usize] = level.clamp(0.0, 1.0);
    }

    pub fn adjust(&mut self, need: Need, delta: f32) {
        self.set(need, self.get(need) + delta);
    }

    /// Bring needs up to date after `elapsed` of neglect.
    ///
    /// `rate_scale` lets age and health change how fast a body runs down — an infant
    /// and an elder do not get hungry at the same rate.
    pub fn accrue(&mut self, elapsed: Duration, rate_scale: f32) {
        let days = elapsed.as_days() as f32;
        if days <= 0.0 {
            return;
        }
        for need in Need::ALL {
            let growth = need.daily_rate() * days * rate_scale;
            self.adjust(need, growth);
        }
    }

    /// How loudly a need is asking to be dealt with.
    ///
    /// Squared, so urgency accelerates: a need at 0.9 does not merely outrank one at
    /// 0.45, it dominates. Without the curve, a person tends to a little of everything
    /// and never prioritises, which reads as indecisive rather than alive.
    pub fn pressure(&self, need: Need) -> f32 {
        let level = self.get(need);
        level * level
    }

    /// The need most in want of attention, with its pressure.
    pub fn most_pressing(&self) -> (Need, f32) {
        Need::ALL
            .into_iter()
            .map(|need| (need, self.pressure(need)))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .expect("Need::ALL is never empty")
    }

    /// Total unmet need, normalised to 0..1. Feeds stress, and through it health.
    pub fn total_pressure(&self) -> f32 {
        let sum: f32 = Need::ALL.into_iter().map(|n| self.pressure(n)).sum();
        sum / Need::COUNT as f32
    }

    /// Pressure from the needs that can actually kill.
    pub fn vital_pressure(&self) -> f32 {
        let vital: Vec<Need> = Need::ALL.into_iter().filter(|n| n.is_vital()).collect();
        let sum: f32 = vital.iter().map(|n| self.pressure(*n)).sum();
        sum / vital.len() as f32
    }

    pub fn iter(&self) -> impl Iterator<Item = (Need, f32)> + '_ {
        Need::ALL.into_iter().map(|need| (need, self.get(need)))
    }
}

impl Default for Needs {
    fn default() -> Self {
        Needs::rested()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rested_creature_wants_nothing() {
        let needs = Needs::rested();
        assert_eq!(needs.total_pressure(), 0.0);
        for (_, level) in needs.iter() {
            assert_eq!(level, 0.0);
        }
    }

    #[test]
    fn needs_accrue_with_time_and_saturate() {
        let mut needs = Needs::rested();
        needs.accrue(Duration::from_hours(6), 1.0);
        assert!(needs.get(Need::Thirst) > needs.get(Need::Hunger));
        assert!(needs.get(Need::Hunger) > needs.get(Need::Purpose));

        // Neglected for a year, everything is pinned at desperate — never beyond.
        needs.accrue(Duration::from_years(1), 1.0);
        for (need, level) in needs.iter() {
            assert_eq!(level, 1.0, "{need} should be saturated");
        }
    }

    #[test]
    fn accrual_is_a_function_of_elapsed_time_not_step_count() {
        // The whole reason needs are lazy: one long step must equal many short ones.
        let mut in_one_go = Needs::rested();
        in_one_go.accrue(Duration::from_hours(12), 1.0);

        let mut in_pieces = Needs::rested();
        for _ in 0..12 {
            in_pieces.accrue(Duration::from_hours(1), 1.0);
        }

        for need in Need::ALL {
            let (a, b) = (in_one_go.get(need), in_pieces.get(need));
            assert!((a - b).abs() < 1e-5, "{need}: {a} vs {b}");
        }
    }

    #[test]
    fn zero_elapsed_time_changes_nothing() {
        let mut needs = Needs::rested();
        needs.accrue(Duration::ZERO, 1.0);
        assert_eq!(needs.total_pressure(), 0.0);
    }

    #[test]
    fn a_slower_metabolism_accrues_more_slowly() {
        let mut brisk = Needs::rested();
        let mut slow = Needs::rested();
        brisk.accrue(Duration::from_hours(8), 1.0);
        slow.accrue(Duration::from_hours(8), 0.5);
        assert!(slow.get(Need::Hunger) < brisk.get(Need::Hunger));
    }

    #[test]
    fn pressure_accelerates_rather_than_scaling() {
        let mut mild = Needs::rested();
        mild.set(Need::Hunger, 0.45);
        let mut severe = Needs::rested();
        severe.set(Need::Hunger, 0.9);

        // Twice the level, four times the pull — that is what makes people prioritise.
        let ratio = severe.pressure(Need::Hunger) / mild.pressure(Need::Hunger);
        assert!((ratio - 4.0).abs() < 0.01, "ratio was {ratio}");
    }

    #[test]
    fn the_most_pressing_need_wins() {
        let mut needs = Needs::rested();
        needs.set(Need::Hunger, 0.4);
        needs.set(Need::Social, 0.8);
        assert_eq!(needs.most_pressing().0, Need::Social);

        needs.set(Need::Hunger, 0.95);
        assert_eq!(needs.most_pressing().0, Need::Hunger);
    }

    #[test]
    fn satisfying_a_need_relieves_it() {
        let mut needs = Needs::rested();
        needs.accrue(Duration::from_days(1), 1.0);
        let before = needs.get(Need::Hunger);
        needs.adjust(Need::Hunger, -0.5);
        assert!(needs.get(Need::Hunger) < before);
        // And relief cannot go below satisfied.
        needs.adjust(Need::Hunger, -10.0);
        assert_eq!(needs.get(Need::Hunger), 0.0);
    }

    #[test]
    fn only_some_needs_are_fatal() {
        let mut lonely = Needs::rested();
        lonely.set(Need::Social, 1.0);
        assert_eq!(lonely.vital_pressure(), 0.0, "loneliness is not fatal");

        let mut parched = Needs::rested();
        parched.set(Need::Thirst, 1.0);
        assert!(parched.vital_pressure() > 0.0);
    }
}
