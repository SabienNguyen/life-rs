//! How exchange comes to need a medium, and what the medium becomes.
//!
//! Nothing here issues money. Money is something people start *accepting*: a good they take in
//! exchange not because they want it but because they expect somebody else to take it from
//! them. That is Menger's account, and it has a property the design needs — it cannot be
//! decreed and it cannot be ignored. A medium is worth accepting only if enough others accept
//! it, so for a long time nobody does, and then, once trade is thick enough, almost everybody
//! does within a generation.
//!
//! ## The dynamics
//!
//! Every market has, for each candidate medium, the share of its trade that uses it. The rest
//! is barter. Using a medium pays in proportion to how many others already use it — the more
//! people take it, the easier it is to pass on — times how *saleable* the good is and how much
//! of the market's income passes through exchange at all; and it costs what holding the good
//! costs, since grain rots and metal does not. Shares grow where they pay better than the
//! average and shrink where they pay worse, which is the replicator equation, plus a trickle of
//! people who hold the durable good as a store of value whatever anybody else does.
//!
//! The result is bistable. Where little is traded, the trickle settles at a few per cent and
//! barter persists indefinitely. Where enough is traded, the same trickle is past the point at
//! which accepting pays for itself, and the medium takes over.
//!
//! ## Then a currency
//!
//! Once a country's trade runs on a medium, somebody who can stamp it saves everybody the cost
//! of weighing and testing every piece. That is worth doing only past a certain volume, which is
//! why small places use their neighbour's coin. A currency has a stock of money and a price
//! level, related by the oldest identity in the subject: money times how often it changes hands
//! is what is bought with it.

use sim_core::Rng;

/// A good that can come to be used as money.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(usize)]
pub enum Medium {
    /// Food, as grain: wanted by everybody, bulky, and it rots.
    Grain,
    /// Wares, as metal: compact, divisible, and it keeps.
    Metal,
}

impl Medium {
    pub const ALL: [Medium; 2] = [Medium::Grain, Medium::Metal];

    pub const fn label(self) -> &'static str {
        match self {
            Medium::Grain => "grain",
            Medium::Metal => "metal",
        }
    }

    /// How easily it passes from hand to hand, 0 to 1. A physical fact about the good, like
    /// what a tool is made of: metal is compact and divides into any size; grain is wanted by
    /// everybody but needs a cart to carry a year's worth.
    pub const fn saleability(self) -> f64 {
        match self {
            Medium::Grain => 0.6,
            Medium::Metal => 1.0,
        }
    }

    /// What holding it costs a year, as a share of it. Stored grain loses a fifth a year to
    /// rot and rats; metal a couple of per cent to wear and theft.
    pub const fn carrying(self) -> f64 {
        match self {
            Medium::Grain => 0.20,
            Medium::Metal => 0.02,
        }
    }

    /// The word a currency made of it is counted in, before any people has its own word.
    pub const fn stem(self) -> &'static str {
        match self {
            Medium::Grain => "Measure",
            Medium::Metal => "Piece",
        }
    }
}

/// How fast shares move towards what pays, per year: people copy what works for their
/// neighbours within a few years of seeing it work.
const PULL: f64 = 4.0;

/// The trickle into a durable medium from people who hold it as a store of value regardless.
///
/// It is what tips a market once trade is thick enough, and together with `PULL` it sets where
/// that is. The low state — a few holders, mostly barter — exists only while
/// `pull · (exchanged · saleability · share² − carrying · share) + trickle` has a root, which is
/// while the share of income exchanged is below `pull · carrying² / (4 · saleability ·
/// trickle)`. With metal's carrying cost that is two fifths of a market's income, which is
/// where the historical record puts the monetisation of agrarian economies; with grain's it is
/// past one, so grain never tips on its own. Above the line a market goes over within a
/// lifetime.
const HOARDING: f64 = 0.001;

/// How much of each market's trade uses each medium.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Acceptance(pub [f64; 2]);

impl Default for Acceptance {
    fn default() -> Acceptance {
        Acceptance([0.0; 2])
    }
}

impl Acceptance {
    /// Share of trade using any medium at all.
    pub fn monetised(&self) -> f64 {
        self.0.iter().sum::<f64>().min(1.0)
    }

    /// The medium most of trade uses, if most of trade uses one.
    pub fn money(&self) -> Option<Medium> {
        Medium::ALL
            .into_iter()
            .find(|m| self.0[*m as usize] > 0.5)
    }

    /// One year of people deciding what to take in exchange.
    ///
    /// `exchanged` is the share of the market's income that passes through exchange, 0 to 1;
    /// `available` whether each medium is made here at all — nobody hoards metal in a market
    /// that has never seen any. `rng` supplies a small year-to-year wobble, because a market
    /// is people and the tipping point is not reached on a schedule.
    pub fn year(&mut self, exchanged: f64, available: [bool; 2], rng: &mut Rng) {
        let exchanged = exchanged.clamp(0.0, 1.0);
        let fitness: Vec<f64> = Medium::ALL
            .iter()
            .map(|m| exchanged * m.saleability() * self.0[*m as usize] - m.carrying())
            .collect();
        // Barter has fitness zero: it costs nothing to keep and gains nothing from company.
        let mean: f64 = (0..2).map(|i| self.0[i] * fitness[i]).sum();
        let barter = (1.0 - self.monetised()).max(0.0);
        let mut next = self.0;
        for i in 0..2 {
            let a = self.0[i];
            let wobble = 1.0 + 0.1 * (rng.unit_f64() - 0.5);
            let trickle = if available[i] { HOARDING * barter } else { 0.0 };
            next[i] = (a + PULL * a * (fitness[i] - mean) * wobble + trickle).clamp(0.0, 1.0);
        }
        let total: f64 = next.iter().sum();
        if total > 1.0 {
            for share in &mut next {
                *share /= total;
            }
        }
        self.0 = next;
    }
}

/// What trading costs, as a share of what changes hands, before anybody keeps any accounts.
///
/// Barter loses most: finding somebody who has what you want and wants what you have is the
/// whole cost, and it is large. Accepted metal cuts it to a third; a coin that nobody has to
/// weigh cuts it again.
pub const BARTER: f64 = 0.30;
pub const MONEY: f64 = 0.10;
pub const COINED: f64 = 0.04;

/// The cost of exchanging, as a share of what changes hands, given how monetised the market is
/// and whether its money is coined.
pub fn friction(acceptance: &Acceptance, coined: bool) -> f64 {
    let mut cost = BARTER * (1.0 - acceptance.monetised());
    for medium in Medium::ALL {
        let share = acceptance.0[medium as usize];
        let rate = if coined && Some(medium) == acceptance.money() {
            COINED
        } else {
            MONEY / medium.saleability()
        };
        cost += share * rate;
    }
    cost
}

/// How much exchange one unit of reckoning handles at half the friction.
///
/// Twenty: a clerk who keeps the accounts of twenty years' worth of trade halves what it costs
/// to do. It puts reckoning at a few per cent of a trading economy's workforce, which is what
/// commerce and finance together have been in every economy that recorded them.
pub const THROUGHPUT: f64 = 20.0;

/// The share of what changes hands that is lost to exchanging it, given the friction of the
/// money in use and how much reckoning there is to keep count.
///
/// Reckoning is what makes a given medium work better: the same coins go further where somebody
/// keeps books, extends credit and settles accounts, and with no reckoning at all a market pays
/// the whole friction of its money.
pub fn exchange_loss(friction: f64, exchanged: f64, reckoning: f64) -> f64 {
    if exchanged <= 0.0 {
        return 0.0;
    }
    friction * exchanged / (exchanged + THROUGHPUT * reckoning.max(0.0))
}

/// What one more unit of reckoning saves, in food: the marginal fall in what exchange loses.
pub fn worth_of_reckoning(friction: f64, exchanged: f64, reckoning: f64) -> f64 {
    let denominator = exchanged + THROUGHPUT * reckoning.max(0.0);
    if denominator <= 0.0 {
        return 0.0;
    }
    friction * THROUGHPUT * exchanged * exchanged / (denominator * denominator)
}

/// What a mint costs to run for a year, in years of food: the smiths and assayers and the
/// guard on the door.
pub const MINT_COST: f64 = 2_000.0;

/// Whether a country's trade is worth coining: what stamping the money would save on what
/// changes hands, against what the mint costs.
pub fn worth_coining(acceptance: &Acceptance, exchanged_value: f64) -> bool {
    let Some(medium) = acceptance.money() else {
        return false;
    };
    let saving = (MONEY / medium.saleability() - COINED) * exchanged_value * acceptance.monetised();
    saving > MINT_COST
}

/// How much money people hold against what they buy: a quarter of a year's transactions.
pub const HELD: f64 = 0.25;

/// What a currency's first unit is worth: a hundredth of a year's food, a few days of eating,
/// which is the size of unit people actually count in.
pub const FIRST_UNIT: f64 = 0.01;

/// The inflation a managed currency is aimed at.
pub const TARGET_INFLATION: f64 = 0.02;

/// A country's money.
#[derive(Clone, Debug, PartialEq)]
pub struct Currency {
    pub name: String,
    /// Three or four letters, for writing prices and stable tokens.
    pub symbol: String,
    pub medium: Medium,
    pub minted: u64,
    /// Units in existence.
    pub money: f64,
    /// Units a year's food costs.
    pub level: f64,
    /// Whether a house manages the supply rather than the metal.
    pub managed: bool,
    /// What the level did last year.
    pub inflation: f64,
    last_transactions: f64,
    last_growth: f64,
}

impl Currency {
    /// Stamp a country's medium. The first unit is worth `FIRST_UNIT` of a year's food, and
    /// there is as much of it as people want to hold.
    pub fn mint(name: String, symbol: String, medium: Medium, year: u64, transactions: f64) -> Currency {
        let level = 1.0 / FIRST_UNIT;
        Currency {
            name,
            symbol,
            medium,
            minted: year,
            money: HELD * transactions.max(1.0) * level,
            level,
            managed: false,
            inflation: 0.0,
            last_transactions: transactions.max(1.0),
            last_growth: 0.0,
        }
    }

    /// A year of a currency. Commodity money grows with the medium it is made of; managed money
    /// grows with what the house expects the economy to need — last year's growth — plus its
    /// target, and misses by however much the year surprised it.
    pub fn year(&mut self, transactions: f64, medium_growth: f64) {
        let transactions = transactions.max(1.0);
        let growth = if self.managed {
            self.last_growth + TARGET_INFLATION
        } else {
            medium_growth
        };
        self.money *= 1.0 + growth.clamp(-0.5, 1.0);
        let level = self.money / (HELD * transactions);
        self.inflation = level / self.level - 1.0;
        self.level = level;
        self.last_growth = (transactions / self.last_transactions - 1.0).clamp(-0.5, 0.5);
        self.last_transactions = transactions;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim_core::{Domain, WorldSeed};

    fn rng() -> Rng {
        WorldSeed::from_u128(0x3e7a1).stream(Domain::Chance, 0, 0)
    }

    fn run(exchanged: f64, years: usize) -> Acceptance {
        let mut acceptance = Acceptance::default();
        let mut rng = rng();
        for _ in 0..years {
            acceptance.year(exchanged, [true, true], &mut rng);
        }
        acceptance
    }

    /// Where little changes hands, barter persists however long you wait; where much does,
    /// metal takes over. Nothing in between is chosen.
    #[test]
    fn money_comes_when_trade_is_thick_enough_and_not_before() {
        let thin = run(0.15, 500);
        assert!(thin.monetised() < 0.1, "{thin:?}");
        assert_eq!(thin.money(), None);
        let thick = run(0.7, 200);
        assert_eq!(thick.money(), Some(Medium::Metal), "{thick:?}");
        assert!(thick.monetised() > 0.9);
    }

    /// Grain rots. Given metal, a market never settles on grain.
    #[test]
    fn the_medium_that_keeps_is_the_one_that_wins() {
        let thick = run(0.9, 300);
        assert!(thick.0[Medium::Metal as usize] > thick.0[Medium::Grain as usize] * 10.0);
    }

    /// A market that makes no metal has nothing to hoard, and stays on barter.
    #[test]
    fn nobody_hoards_what_they_have_never_seen() {
        let mut acceptance = Acceptance::default();
        let mut rng = rng();
        for _ in 0..300 {
            acceptance.year(0.8, [true, false], &mut rng);
        }
        assert_eq!(acceptance.0[Medium::Metal as usize], 0.0);
    }

    #[test]
    fn money_and_coin_each_cut_the_cost_of_trading() {
        let barter = friction(&Acceptance::default(), false);
        let metal = Acceptance([0.0, 1.0]);
        let money = friction(&metal, false);
        let coin = friction(&metal, true);
        assert!(barter > money && money > coin, "{barter} {money} {coin}");
    }

    /// Reckoning is worth most where trade is heaviest and there is least of it.
    #[test]
    fn reckoning_is_worth_most_where_it_is_scarce() {
        let scarce = worth_of_reckoning(0.1, 1000.0, 1.0);
        let plenty = worth_of_reckoning(0.1, 1000.0, 1000.0);
        assert!(scarce > plenty * 10.0);
        assert!(exchange_loss(0.1, 1000.0, 0.0) > exchange_loss(0.1, 1000.0, 100.0));
    }

    #[test]
    fn a_small_market_is_not_worth_a_mint() {
        let metal = Acceptance([0.0, 1.0]);
        assert!(!worth_coining(&metal, 1_000.0));
        assert!(worth_coining(&metal, 1_000_000.0));
        assert!(!worth_coining(&Acceptance([0.3, 0.1]), 1_000_000.0), "no money, nothing to stamp");
    }

    /// Money times velocity is what is bought: double the money with nothing more to buy and
    /// the price level doubles.
    #[test]
    fn more_money_for_the_same_goods_is_inflation() {
        let mut coin = Currency::mint("Test".into(), "TST".into(), Medium::Metal, 0, 1e6);
        assert!((coin.level - 100.0).abs() < 1e-9);
        coin.year(1e6, 1.0);
        assert!((coin.inflation - 1.0).abs() < 1e-9);
        let mut steady = Currency::mint("Test".into(), "TST".into(), Medium::Metal, 0, 1e6);
        steady.year(2e6, 1.0);
        assert!(steady.inflation.abs() < 1e-9, "twice the money for twice the trade");
    }
}
