//! Paying somebody far away, and what it would take to stop needing anybody in between.
//!
//! A payment between two countries does not go from one merchant to the other. It goes from a
//! merchant to a house, from that house to a house it deals with abroad, perhaps through a third
//! that both of them trust more than they trust each other, and from there to the merchant who
//! is owed. Every house along the way charges, and every house that has to wait on another it
//! does not quite trust charges for the risk of not being paid. That is correspondent banking,
//! and it is the thing a ledger nobody keeps is an alternative to.
//!
//! ## When a ledger nobody keeps is worth keeping
//!
//! Only when three things are true at once, and this module states each as a comparison rather
//! than a date:
//!
//! 1. **There is no keeper everybody trusts.** If one house is trusted by all the others, they
//!    will simply keep their accounts with it, and a shared ledger costs more for nothing. A
//!    chain is what distrustful parties build, and a world with a trusted centre does not
//!    build one.
//! 2. **There are enough of them.** Byzantine agreement needs `3f + 1` parties to survive `f`
//!    faulty ones; with fewer than four there is nothing a chain can do that one of them keeping
//!    the books could not.
//! 3. **Checking it is cheap enough.** Every validator checks every payment's signature. By hand,
//!    one Ed25519 check is about two thousand multiplications of seventy-seven-digit numbers,
//!    roughly two years of a clerk's working life; what reckoning technique buys is doing it
//!    faster, and it compounds — better methods on better machines — so the cost falls as a
//!    power of the technique. Until it is small against what a payment is worth, the chain is
//!    dearer than the distrust it replaces.

/// A house's charge for relaying one payment, as a share of it.
pub const FEE: f64 = 0.01;

/// What a house charges on top for waiting on a counterparty it does not trust at all.
pub const RISK: f64 = 0.08;

/// What changing one currency into another costs at an ordinary counter.
pub const FX_SPREAD: f64 = 0.02;

/// What changing a currency into the one everybody invoices in costs: less, because that
/// market is the deepest.
pub const VEHICLE_SPREAD: f64 = 0.005;

/// What an issuer charges to mint or redeem a stable token.
pub const TOKEN_SPREAD: f64 = 0.002;

/// Person-years of a clerk's work to check one signature by hand.
///
/// Derived, not chosen: a verification is about 2,600 multiplications of 255-bit numbers, a
/// 255-bit number is seventy-seven decimal digits, and a schoolbook multiplication and
/// reduction of two of them is about twelve thousand digit operations — an hour and three
/// quarters at a steady two a second. Two thousand six hundred of those is a little over four
/// thousand hours: two working years.
pub const HAND_CHECK: f64 = 2.0;

/// How fast checking gets cheaper as reckoning technique rises, as a power of it.
///
/// The one judgement in this module. Reckoning improves the method and the machine at once — a
/// clerk with tables, then a clerk with a calculating engine, then an engine with no clerk — so
/// the cost of a fixed computation falls much faster than the technique that measures it. A
/// cube means a people three times as good at reckoning as bare technique checks a signature
/// in under a month, and ten times as good in under a day; the historical fall in the cost of a
/// multiplication over the century before the first ledgers was many orders of magnitude, so
/// this is if anything slow.
pub const COMPOUNDING: f64 = 3.0;

/// Trust above which a house is a keeper others would simply use.
pub const TRUSTED: f64 = 0.75;

/// Byzantine agreement survives `f` faults with `3f + 1` parties, so with one fault tolerated —
/// the least that makes a shared ledger mean anything — it needs four.
pub const FEWEST_FOUNDERS: usize = 4;

/// The most parties that can found a chain together: its largest validator set.
pub const MOST_FOUNDERS: usize = 21;

/// How much better than the alternative a chain has to be before people who distrust each
/// other will go to the trouble of founding one together.
pub const WORTH_THE_TROUBLE: f64 = 3.0;

/// Person-years of reckoning to check one signature, at a given reckoning technique.
pub fn check_cost(technique: f64) -> f64 {
    HAND_CHECK / technique.max(1.0).powf(COMPOUNDING)
}

/// How far two parties' trust can go, however much they trade: less between peoples than
/// within one, and less the further apart they are.
pub fn trust_ceiling(same_people: bool, distance_km: f64) -> f64 {
    let base = if same_people { 0.95 } else { 0.8 };
    base * (-distance_km.max(0.0) / 8_000.0).exp()
}

/// How familiarity grows with the share of a party's business done with another, and fades.
const ACQUAINTANCE: f64 = 0.2;
const FORGETTING: f64 = 0.03;

/// A year of two parties dealing with each other: familiarity grows with the share of their
/// business they do together and fades otherwise. Trust is the ceiling times familiarity, never
/// zero — strangers still extend a little.
pub fn familiarity_after(familiar: f64, share_of_business: f64) -> f64 {
    let share = share_of_business.clamp(0.0, 1.0);
    (familiar + ACQUAINTANCE * share.sqrt() * (1.0 - familiar) - FORGETTING * familiar).clamp(0.0, 1.0)
}

pub fn trust(ceiling: f64, familiar: f64) -> f64 {
    ceiling * (0.25 + 0.75 * familiar.clamp(0.0, 1.0))
}

/// The cheapest way to get a payment from `from` to `to` through houses: directly, or through
/// one other house both trust better. Returns the cost as a share of the payment and who, if
/// anybody, it went through.
pub fn cheapest_route(trust: &[Vec<f64>], from: usize, to: usize, fx: f64) -> (f64, Option<usize>) {
    let leg = |a: usize, b: usize| FEE + RISK * (1.0 - trust[a][b].clamp(0.0, 1.0));
    let mut best = (leg(from, to) + fx, None);
    for via in 0..trust.len() {
        if via == from || via == to {
            continue;
        }
        let cost = leg(from, via) + leg(via, to) + fx;
        if cost < best.0 {
            best = (cost, Some(via));
        }
    }
    best
}

/// What a payment of `size` costs on a chain with `validators` checking it, as a share of it:
/// minting and redeeming the stable token, changing into and out of the currency it is pegged
/// to where the two ends use another, and the fee that pays for every validator to check it.
pub fn chain_cost(size: f64, validators: usize, technique: f64, wage: f64, changes: usize) -> f64 {
    let checking = validators.max(1) as f64 * check_cost(technique) * wage.max(0.0);
    2.0 * TOKEN_SPREAD + changes as f64 * VEHICLE_SPREAD + checking / size.max(1e-9)
}

/// A party that might found a chain: a house, with the country it is in, how much reckoning it
/// can do in a year (in reckoners), and how much it pays and is paid abroad.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    pub country: usize,
    pub reckoners: f64,
    pub technique: f64,
    pub volume: f64,
}

/// A chain worth founding, and by whom.
#[derive(Clone, Debug, PartialEq)]
pub struct Founding {
    /// Indices into the candidates, largest business first.
    pub founders: Vec<usize>,
    /// What moving their payments onto it would save them in a year, in years of food.
    pub saving: f64,
    /// What checking it would cost them in a year.
    pub cost: f64,
}

/// Why not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotYet {
    /// Fewer than four parties with business abroad.
    TooFew,
    /// All of them in one country, where a house can keep the books.
    OneCountry,
    /// One of them is trusted by all the rest.
    TrustedKeeper(usize),
    /// None of them can check a block in the time it is meant to take.
    CannotCheck,
    /// Cheaper to keep paying through houses.
    NotWorthIt,
}

/// Whether a chain is worth founding among these parties, given how much they trust each other,
/// what their payments cost now, and what they would cost on a chain.
///
/// `checks_a_year` is how many signatures each validator would have to check in a year at the
/// cadence the chain would keep; `bank_cost` and `chain_cost` are the average shares of a
/// payment each way costs.
pub fn worth_founding(
    candidates: &[Candidate],
    trust: &dyn Fn(usize, usize) -> f64,
    checks_a_year: f64,
    bank_cost: f64,
    chain_cost: f64,
    wage: f64,
) -> Result<Founding, NotYet> {
    // Anybody who could check the chain at all, largest business first.
    let mut able: Vec<usize> = (0..candidates.len())
        .filter(|i| candidates[*i].volume > 0.0)
        .collect();
    if able.len() < FEWEST_FOUNDERS {
        return Err(NotYet::TooFew);
    }
    able.retain(|i| {
        let c = &candidates[*i];
        c.reckoners >= checks_a_year * check_cost(c.technique)
    });
    if able.len() < FEWEST_FOUNDERS {
        return Err(NotYet::CannotCheck);
    }
    able.sort_by(|a, b| candidates[*b].volume.total_cmp(&candidates[*a].volume).then(a.cmp(b)));
    able.truncate(MOST_FOUNDERS);

    let first_country = candidates[able[0]].country;
    if able.iter().all(|i| candidates[*i].country == first_country) {
        return Err(NotYet::OneCountry);
    }
    for keeper in &able {
        if able
            .iter()
            .filter(|h| *h != keeper)
            .all(|h| trust(*h, *keeper) >= TRUSTED)
        {
            return Err(NotYet::TrustedKeeper(*keeper));
        }
    }

    let volume: f64 = able.iter().map(|i| candidates[*i].volume).sum();
    let saving = volume * (bank_cost - chain_cost).max(0.0);
    let technique = able
        .iter()
        .map(|i| candidates[*i].technique)
        .fold(f64::MAX, f64::min);
    let cost = able.len() as f64 * checks_a_year * check_cost(technique) * wage;
    if saving <= WORTH_THE_TROUBLE * cost || saving <= 0.0 {
        return Err(NotYet::NotWorthIt);
    }
    Ok(Founding {
        founders: able,
        saving,
        cost,
    })
}

/// How fast payments move onto a chain once it pays to use it, per year.
const TAKING_TO_IT: f64 = 0.15;

/// The share of a pair's payments on a chain, a year on: towards the share of the cost it saves.
pub fn adoption(share: f64, bank_cost: f64, chain_cost: f64) -> f64 {
    let target = if bank_cost <= 0.0 {
        0.0
    } else {
        ((bank_cost - chain_cost) / bank_cost).clamp(0.0, 1.0)
    };
    (share + TAKING_TO_IT * (target - share)).clamp(0.0, 1.0)
}

/// What stake a chain's validators want to hold against what passes over it in a year: a
/// fiftieth. Enough that corrupting two thirds of it costs more than most of what one could
/// steal by doing so.
pub const SECURITY: f64 = 0.02;

/// How long, in years, a user holds the coin it pays fees in.
pub const FEE_FLOAT: f64 = 0.25;

/// What one coin is worth, in the currency tokens are pegged to: the stake people want to
/// hold against what the chain carries, plus the coin held to pay fees, over the coin there is.
pub fn coin_value(carried_a_year: f64, fees_a_year: f64, coins: f64) -> f64 {
    if coins <= 0.0 {
        return 0.0;
    }
    (SECURITY * carried_a_year + FEE_FLOAT * fees_a_year) / coins
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checking_by_hand_is_years_and_by_engine_is_nothing() {
        assert!((check_cost(1.0) - 2.0).abs() < 1e-12);
        assert!(check_cost(3.0) < 2.0 / 12.0, "under a month at three times bare technique");
        assert!(check_cost(10.0) < 1.0 / 365.0, "under a day at ten");
    }

    /// Trust is built by trading and bounded by distance.
    #[test]
    fn trust_grows_with_business_and_is_capped_by_distance() {
        let mut familiar = 0.0;
        for _ in 0..40 {
            familiar = familiarity_after(familiar, 0.5);
        }
        assert!(familiar > 0.7);
        let near = trust(trust_ceiling(false, 500.0), familiar);
        let far = trust(trust_ceiling(false, 9_000.0), familiar);
        assert!(near > far * 2.0);
        let mut strangers = 0.8;
        for _ in 0..100 {
            strangers = familiarity_after(strangers, 0.0);
        }
        assert!(strangers < 0.1, "unused acquaintance fades");
    }

    /// Two parties who do not trust each other pay through a third that both trust.
    #[test]
    fn a_payment_goes_the_way_that_is_trusted() {
        let trust = vec![
            vec![1.0, 0.1, 0.9],
            vec![0.1, 1.0, 0.9],
            vec![0.9, 0.9, 1.0],
        ];
        let (cost, via) = cheapest_route(&trust, 0, 1, 0.0);
        assert_eq!(via, Some(2));
        assert!(cost < FEE + RISK * 0.9);
    }

    fn four_strangers() -> Vec<Candidate> {
        (0..4)
            .map(|i| Candidate {
                country: i % 2,
                reckoners: 1e6,
                technique: 10.0,
                volume: 1e6,
            })
            .collect()
    }

    #[test]
    fn distrustful_parties_found_a_chain_when_checking_is_cheap() {
        let founding = worth_founding(&four_strangers(), &|_, _| 0.3, 1000.0, 0.08, 0.01, 1.0)
            .expect("four strangers with cheap checking and dear banking");
        assert_eq!(founding.founders.len(), 4);
        assert!(founding.saving > founding.cost);
    }

    #[test]
    fn a_keeper_everybody_trusts_is_used_instead() {
        let trusted = |_: usize, keeper: usize| if keeper == 2 { 0.9 } else { 0.3 };
        assert_eq!(
            worth_founding(&four_strangers(), &trusted, 1000.0, 0.08, 0.01, 1.0),
            Err(NotYet::TrustedKeeper(2))
        );
    }

    #[test]
    fn three_is_too_few_and_one_country_is_one_bookkeeper() {
        let three = &four_strangers()[..3];
        assert_eq!(
            worth_founding(three, &|_, _| 0.3, 1000.0, 0.08, 0.01, 1.0),
            Err(NotYet::TooFew)
        );
        let mut home = four_strangers();
        for c in &mut home {
            c.country = 0;
        }
        assert_eq!(
            worth_founding(&home, &|_, _| 0.3, 1000.0, 0.08, 0.01, 1.0),
            Err(NotYet::OneCountry)
        );
    }

    /// With reckoning at bare technique, checking a year of blocks is beyond any house.
    #[test]
    fn a_chain_checked_by_hand_is_beyond_anybody() {
        let mut clerks = four_strangers();
        for c in &mut clerks {
            c.technique = 1.0;
            c.reckoners = 1_000.0;
        }
        assert_eq!(
            worth_founding(&clerks, &|_, _| 0.3, 1000.0, 0.08, 0.01, 1.0),
            Err(NotYet::CannotCheck)
        );
    }

    #[test]
    fn payments_move_to_what_is_cheaper_at_the_rate_people_move() {
        let mut share = 0.0;
        for _ in 0..50 {
            share = adoption(share, 0.08, 0.01);
        }
        assert!((share - 7.0 / 8.0).abs() < 0.01);
        assert_eq!(adoption(0.5, 0.01, 0.08), 0.5 - TAKING_TO_IT * 0.5, "and back off what is dearer");
    }

    #[test]
    fn a_coin_is_worth_what_it_secures_over_how_many_there_are() {
        let small = coin_value(1e6, 1e4, 1e4);
        let large = coin_value(1e8, 1e4, 1e4);
        let diluted = coin_value(1e6, 1e4, 2e4);
        assert!(large > small * 50.0);
        assert!((diluted - small / 2.0).abs() < 1e-9);
    }
}
