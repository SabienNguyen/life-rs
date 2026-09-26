//! What a town makes, in four trades.
//!
//! The people-level world has five trades and five goods, and they are exactly right for a
//! village: stock, tools, food, meals and upkeep, each made out of the one before. They are
//! also, by construction, a village's economy and nothing more. There is no good in that chain
//! whose demand does not saturate — a place can only use so many tools and eat so many meals —
//! so once farming gets good enough there is nothing for the freed hands to do, and an economy
//! built from it can get richer only by getting smaller.
//!
//! A town at the scale of nations needs the one thing a village does not: something to spend a
//! rising income *on*. So four sectors, each the people-level trades seen from further away:
//!
//! | here | in a village | what it makes |
//! |---|---|---|
//! | **farming** | farmer | food, off land that crowds |
//! | **making** | hewer and smith | wares — tools, and everything else made of stuff — off ground that is good or bad for timber and rock |
//! | **serving** | cook and keeper | services, which cannot be carried anywhere |
//! | **reckoning** | *nobody yet* | the capacity to keep count: what lets exchange happen between people who cannot see each other's goods |
//!
//! Reckoning is the new one, and the reason this crate exists. It makes nothing anybody eats or
//! wears; what it makes is exchange *possible* at a distance, and everything from coinage to a
//! ledger nobody keeps is a way of doing more of it with less.
//!
//! ## The ground is the ground the people-level world stands on
//!
//! What a hand gets off a town's land is `economy::ground_of` — the same function, the same
//! terrain, the same biome arithmetic — scaled by how many villages' worth of land the town
//! has. Output is homogeneous of degree one in land and hands, so a town of a hundred thousand
//! villages is a hundred thousand villages, and a town that never specialises produces exactly
//! what that many people-level villages would.

use society::Terrain;

/// What people spend their working year on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(usize)]
pub enum Sector {
    Farming,
    Making,
    Serving,
    Reckoning,
}

impl Sector {
    pub const ALL: [Sector; 4] = [
        Sector::Farming,
        Sector::Making,
        Sector::Serving,
        Sector::Reckoning,
    ];
    pub const COUNT: usize = Sector::ALL.len();

    pub const fn label(self) -> &'static str {
        match self {
            Sector::Farming => "farming",
            Sector::Making => "making",
            Sector::Serving => "serving",
            Sector::Reckoning => "reckoning",
        }
    }

    /// The people who do it.
    pub const fn workers(self) -> &'static str {
        match self {
            Sector::Farming => "farmers",
            Sector::Making => "makers",
            Sector::Serving => "servers",
            Sector::Reckoning => "reckoners",
        }
    }
}

/// How many square kilometres one people-level village farms.
///
/// The unit of land in `economy` is "one village's ground": a place's fertility is its land,
/// and a village of a few dozen lives on it. Five square kilometres is about what a medieval
/// village of that size worked — open fields, meadow, woodland and waste — and it is the one
/// number that turns a cell of the planet into a count of villages.
pub const KM2_PER_VILLAGE: f64 = 5.0;

/// How much of what farming yields is owed to the land. The same third `economy` uses, for the
/// same reason.
const LAND_SHARE: f64 = 0.35;

/// How much of what making yields is owed to the ground — timber and rock to work.
///
/// Smaller than farming's share of land: a forge needs ore, but most of what a maker adds is
/// hands and skill, and a town with poor ground can import its stock. Large enough that a town
/// on wooded, rocky ground is better at making than a town on a river plain, which is §28's
/// whole point carried up a scale.
const GROUND_SHARE: f64 = 0.20;

/// How much each sector's output rises with the capital each of its workers has, as the
/// exponent on (1 + capital per worker). Farming least — the land is the limit — and reckoning
/// most: a clerk with a calculating machine is worth many without.
const CAPITAL_DEPTH: [f64; Sector::COUNT] = [0.15, 0.35, 0.0, 0.40];

/// The ground under a town, and how much of it there is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Endowment {
    /// Food a hand gets off one village's worth of this land in a year, at bare technique.
    pub food_yield: f64,
    /// Timber and rock a hand gets off one village's worth of it.
    pub stock_yield: f64,
    /// How many villages' worth of ground the town has.
    pub villages: f64,
}

impl Endowment {
    /// Read the ground exactly as the people-level world reads it.
    pub fn of(terrain: &Terrain, area_km2: f64) -> Endowment {
        let ground = economy::ground_of(terrain, economy::Technique::BARE);
        Endowment {
            food_yield: ground.food as f64,
            stock_yield: ground.stock as f64,
            villages: (area_km2 / KM2_PER_VILLAGE).max(1.0),
        }
    }
}

/// A year's output, sector by sector, in each sector's own unit.
///
/// Food is in years of one person's eating, which is the unit everything in this workspace is
/// counted in. The other three are in what one person makes of them in a year at bare
/// technique; what they are *worth* is a price, decided in a market.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Output {
    pub food: f64,
    pub wares: f64,
    pub services: f64,
    pub reckoning: f64,
}

impl Output {
    pub fn of(&self, sector: Sector) -> f64 {
        match sector {
            Sector::Farming => self.food,
            Sector::Making => self.wares,
            Sector::Serving => self.services,
            Sector::Reckoning => self.reckoning,
        }
    }
}

/// How capital is shared between sectors: in proportion to how much each sector's workers get
/// out of it. Services use none.
pub fn share_capital(capital: f64, workers: &[f64; Sector::COUNT]) -> [f64; Sector::COUNT] {
    let weights: Vec<f64> = (0..Sector::COUNT)
        .map(|s| CAPITAL_DEPTH[s] * workers[s].max(0.0))
        .collect();
    let total: f64 = weights.iter().sum();
    let mut out = [0.0; Sector::COUNT];
    if total <= 0.0 {
        return out;
    }
    for s in 0..Sector::COUNT {
        out[s] = capital.max(0.0) * weights[s] / total;
    }
    out
}

fn lift(capital: f64, workers: f64, depth: f64) -> f64 {
    if workers <= 0.0 || depth <= 0.0 {
        return 1.0;
    }
    (1.0 + capital.max(0.0) / workers).powf(depth)
}

/// What a town makes in a year.
///
/// `workers` and `capital` are per sector; `technique` multiplies each sector's output and is
/// one at bare technique. With everybody farming and no capital this is, per hand, exactly the
/// people-level `economy`'s one-good output.
pub fn produce(
    ground: &Endowment,
    workers: &[f64; Sector::COUNT],
    capital: &[f64; Sector::COUNT],
    technique: &[f64; Sector::COUNT],
) -> Output {
    let hands = |s: Sector| workers[s as usize].max(0.0);
    let deep = |s: Sector| lift(capital[s as usize], hands(s), CAPITAL_DEPTH[s as usize]);
    let farmers = hands(Sector::Farming);
    let makers = hands(Sector::Making);

    let food = if farmers > 0.0 {
        technique[0]
            * ground.food_yield
            * ground.villages.powf(LAND_SHARE)
            * farmers.powf(1.0 - LAND_SHARE)
            * deep(Sector::Farming)
    } else {
        0.0
    };
    let wares = if makers > 0.0 {
        technique[1]
            * (ground.stock_yield * ground.villages).powf(GROUND_SHARE)
            * makers.powf(1.0 - GROUND_SHARE)
            * deep(Sector::Making)
    } else {
        0.0
    };
    Output {
        food,
        wares,
        services: technique[2] * hands(Sector::Serving),
        reckoning: technique[3] * hands(Sector::Reckoning) * deep(Sector::Reckoning),
    }
}

/// What one more worker adds to each sector's output, in that sector's unit, with capital
/// following workers — a new farmer arrives with a farmer's share of the tools.
///
/// Farming and making have diminishing returns because land and ground do not grow with the
/// people working them; serving and reckoning do not, because nothing they need is fixed.
pub fn marginal(output: &Output, workers: &[f64; Sector::COUNT], technique: &[f64; Sector::COUNT], ground: &Endowment, capital: &[f64; Sector::COUNT]) -> [f64; Sector::COUNT] {
    // Where a sector is empty, the first worker's product is what one worker alone would make
    // there — the same "what if I did this instead" the people-level trades ask.
    let at = |s: Sector| -> f64 {
        let i = s as usize;
        if workers[i] >= 1.0 {
            return output.of(s) / workers[i];
        }
        let mut one = [0.0; Sector::COUNT];
        one[i] = 1.0;
        let mut kit = [0.0; Sector::COUNT];
        kit[i] = if workers[i] > 0.0 { capital[i] / workers[i] } else { 0.0 };
        produce(ground, &one, &kit, technique).of(s)
    };
    [
        (1.0 - LAND_SHARE) * at(Sector::Farming),
        (1.0 - GROUND_SHARE) * at(Sector::Making),
        at(Sector::Serving),
        at(Sector::Reckoning),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain() -> Endowment {
        Endowment {
            food_yield: 5.0,
            stock_yield: 2.0,
            villages: 1000.0,
        }
    }

    /// A town of villages is its villages: double the land and the hands and every output
    /// doubles, so what a person gets does not depend on how the planet was cut into cells.
    #[test]
    fn a_town_is_the_sum_of_its_villages() {
        let workers = [600.0, 200.0, 100.0, 20.0];
        let capital = share_capital(500.0, &workers);
        let technique = [1.2, 1.0, 1.0, 1.5];
        let small = produce(&plain(), &workers, &capital, &technique);
        let twice = Endowment {
            villages: 2000.0,
            ..plain()
        };
        let big = produce(
            &twice,
            &workers.map(|w| 2.0 * w),
            &capital.map(|k| 2.0 * k),
            &technique,
        );
        for s in Sector::ALL {
            let ratio = big.of(s) / small.of(s);
            assert!((ratio - 2.0).abs() < 1e-9, "{} doubled by {ratio}", s.label());
        }
    }

    /// Everybody farming with nothing owned is the people-level economy's one-good model, per
    /// hand, to the last decimal.
    #[test]
    fn everybody_farming_is_the_village_economy() {
        let terrain = Terrain::middling(0);
        let ground = Endowment::of(&terrain, KM2_PER_VILLAGE);
        assert_eq!(ground.villages, 1.0);
        for people in [10.0f64, 40.0, 300.0] {
            let here = produce(&ground, &[people, 0.0, 0.0, 0.0], &[0.0; 4], &[1.0; 4]);
            let village = economy::produce(&terrain, people as f32);
            assert!(
                (here.food - village.output as f64).abs() < 1e-3 * village.output as f64,
                "{people}: {} against the village's {}",
                here.food,
                village.output
            );
        }
    }

    #[test]
    fn crowding_the_land_has_diminishing_returns_and_the_trades_that_need_no_land_do_not() {
        let technique = [1.0; 4];
        let few = produce(&plain(), &[100.0, 100.0, 100.0, 100.0], &[0.0; 4], &technique);
        let many = produce(&plain(), &[200.0, 200.0, 200.0, 200.0], &[0.0; 4], &technique);
        assert!(many.food < 2.0 * few.food);
        assert!(many.wares < 2.0 * few.wares);
        assert!((many.services - 2.0 * few.services).abs() < 1e-9);
        assert!((many.reckoning - 2.0 * few.reckoning).abs() < 1e-9);
    }

    #[test]
    fn capital_goes_where_it_is_used() {
        let shared = share_capital(1000.0, &[100.0, 100.0, 100.0, 100.0]);
        assert_eq!(shared[Sector::Serving as usize], 0.0);
        assert!(shared[Sector::Reckoning as usize] > shared[Sector::Farming as usize]);
        assert!((shared.iter().sum::<f64>() - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn the_first_worker_in_an_empty_trade_is_worth_something() {
        let workers = [500.0, 0.0, 0.0, 0.0];
        let technique = [1.0; 4];
        let output = produce(&plain(), &workers, &[0.0; 4], &technique);
        let worth = marginal(&output, &workers, &technique, &plain(), &[0.0; 4]);
        for s in Sector::ALL {
            assert!(worth[s as usize] > 0.0, "{} worth nothing", s.label());
        }
    }
}
