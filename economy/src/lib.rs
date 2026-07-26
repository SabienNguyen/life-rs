//! What a place makes, and what that does to the people in it.
//!
//! Design principle one says a neighbourhood is poor "because of what happened to its
//! economy". Until now that was not true. A place's prosperity was read off the standing
//! of the people in it and nothing else, which is a loop with no outside: residents are
//! well off because the place is, and the place is well off because the residents are. It
//! sustains any level it happens to reach and it has nothing to say about *why* that level
//! and not another.
//!
//! This is the outside. A place produces, from the land it stands on and the hands it has,
//! and what it produces beyond what those hands must eat is a **surplus** — which is the
//! only thing anybody has ever built a town out of.
//!
//! ## The three things it gets right
//!
//! **Land and labour together, with diminishing returns to labour.** Output is
//! `land^α · labour^(1−α)`, the oldest functional form in the subject and the right one:
//! neither factor can be bought off with the other, and doubling the workers on fixed land
//! does not double what comes off it. That last clause is the Malthusian core, and it is
//! what makes crowding cost something *economically* rather than only as a nuisance —
//! before this, a place could absorb any number of people at no cost to what each of them
//! got.
//!
//! **Subsistence comes out first.** People eat before there is a surplus, so a place with
//! poor land and many mouths has no surplus at all however hard it works — which is not the
//! same as being slightly worse off, and is the difference between a poor town and a
//! failing one.
//!
//! **Trade is what a coast is for.** A place's market is its own surplus plus a share of
//! what its neighbours have, weighted by how easily anyone reaches it. Two identical
//! valleys, one on a road and one not, are not identical places, and this is the only
//! mechanism in the model that says so in money rather than in social capital.
//!
//! ## What is not here
//!
//! Prices, money, capital, firms, wages, credit, ownership, and every institution. There is
//! one good and no way to store it between years. That is deliberate and it is the boundary
//! §23 draws — a simple economy is in scope for this phase and a tech tree is "a project of
//! its own" — but it is worth being precise about what the boundary costs: **without
//! capital there is no accumulation, so nothing here can compound**. A rich place is rich
//! because its land and its position are good, not because it was rich last century. Real
//! inequality is mostly the latter.

use society::Terrain;

/// How much of output is owed to the land rather than to the hands working it.
///
/// The exponent on land in `land^α · labour^(1−α)`. Estimates for pre-industrial agrarian
/// economies put land's share near a third and labour's near a half, with capital taking
/// the rest — and with no capital here the two have to sum to one, so a third to land is
/// the honest reading of the same evidence.
const LAND_SHARE: f32 = 0.35;

/// What one person must have in a year, in the same units output is measured in.
///
/// The unit is arbitrary and this fixes it: one is what one person eats. So an output of a
/// hundred with a hundred mouths is a place with no surplus, which is most of history.
pub const SUBSISTENCE: f32 = 1.0;

/// The scale of what land yields, in units where one is what a person eats in a year.
///
/// Solved rather than guessed, because the two exponents fix everything else. Output goes
/// as `workers^0.65` and subsistence as `workers`, so for any given land there is a
/// workforce past which nothing is spare — that is the Malthusian ceiling and it is what
/// this constant places. Set it so that thirty people on good land keep about half a year's
/// food each in hand, which is roughly what a decent pre-industrial harvest left, and
/// everything else follows: twenty on ordinary land are comfortable, four hundred on the
/// same ground are starving.
const YIELD: f32 = 5.3;

/// How much of a neighbour's surplus can reach a place, at best.
///
/// A third. Trade over land before railways is expensive enough that most of what is grown
/// is eaten within a few days' walk of where it grew; what moves is the margin. Making this
/// large would quietly abolish geography by letting every place draw on every other.
const TRADE_REACH: f32 = 0.33;

/// What a place produced, and what became of it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ledger {
    /// Everything the place made this year.
    pub output: f32,
    /// What its people had to eat.
    pub subsistence: f32,
    /// What was left. Negative means the place did not feed itself.
    pub surplus: f32,
    /// Surplus plus what trade brought in, which is what the place actually has to spend.
    pub market: f32,
    /// How many workers there were.
    pub workers: f32,
}

impl Ledger {
    /// Nothing made, nobody there.
    pub const EMPTY: Ledger = Ledger {
        output: 0.0,
        subsistence: 0.0,
        surplus: 0.0,
        market: 0.0,
        workers: 0.0,
    };

    /// Surplus per head — the number that decides whether this is somewhere to be.
    ///
    /// Not total output. A large poor town and a small rich one differ in exactly this and
    /// in nothing else that matters to somebody deciding where to live.
    pub fn per_head(&self) -> f32 {
        if self.workers <= 0.0 {
            0.0
        } else {
            self.market / self.workers
        }
    }

    /// Whether the place feeds itself.
    pub fn self_sufficient(&self) -> bool {
        self.surplus >= 0.0
    }

    /// How prosperous this reads as, 0 to 1.
    ///
    /// Saturating, because the difference between no surplus and a little is enormous and
    /// the difference between a lot and more is not. A place with nothing spare is at
    /// zero however many people it holds.
    pub fn prosperity(&self) -> f32 {
        let spare = self.per_head().max(0.0);
        spare / (spare + 0.9)
    }
}

/// What a place makes, before trade.
///
/// `workers` is people, not households — the economy counts hands.
pub fn produce(terrain: &Terrain, workers: f32) -> Ledger {
    if workers <= 0.0 {
        return Ledger::EMPTY;
    }
    // The land, as an effective quantity. Fertility is what grows on it and harshness is
    // how much of the year is worth working; a productive place with a brutal season is
    // less than its soil suggests, which is most of the boreal world.
    let land = terrain.fertility.max(0.0) * (1.0 - 0.6 * terrain.hardship());

    // Cobb–Douglas. Neither factor is substitutable for the other, which is the point:
    // hands with no land make nothing and land with no hands makes nothing.
    let output = YIELD * land.powf(LAND_SHARE) * workers.powf(1.0 - LAND_SHARE);
    let subsistence = workers * SUBSISTENCE;
    let surplus = output - subsistence;

    Ledger {
        output,
        subsistence,
        surplus,
        // Trade fills this in; alone, a place has only what it made.
        market: surplus,
        workers,
    }
}

/// Let places trade with each other.
///
/// Every place keeps its own surplus and receives a share of everyone else's, in proportion
/// to how reachable *both* ends are — a road needs two ends. Places in deficit draw on the
/// same pool, which is what lets a town on a good road survive a bad year on poor land and
/// an isolated one not.
///
/// The pool is not conserved and is not meant to be: this is a share of what is *available*
/// to a place, not a shipment. Modelling the freight would need prices, and prices need a
/// great deal more than one good.
pub fn trade(ledgers: &mut [Ledger], reach: &[f32]) {
    debug_assert_eq!(ledgers.len(), reach.len());
    let spare: f32 = ledgers
        .iter()
        .zip(reach)
        .map(|(l, r)| l.surplus.max(0.0) * r)
        .sum();

    let partners = (ledgers.len().max(2) - 1) as f32;
    for (index, ledger) in ledgers.iter_mut().enumerate() {
        let mine = ledger.surplus.max(0.0) * reach[index];
        // What everybody else has spare and can get here. Both ends weighted, so an
        // unreachable place neither sends nor receives.
        let elsewhere = (spare - mine).max(0.0);
        ledger.market = ledger.surplus + TRADE_REACH * reach[index] * elsewhere / partners;
    }
}

/// Run a whole region's economy for a year.
///
/// The two steps have to happen in this order and separately: everybody produces, and only
/// then does anybody trade. Interleaving them would let a place trade away what it has not
/// grown yet, which is a different and much later kind of economy.
pub fn year(places: &[(Terrain, f32)]) -> Vec<Ledger> {
    let mut ledgers: Vec<Ledger> = places
        .iter()
        .map(|(terrain, workers)| produce(terrain, *workers))
        .collect();
    let reach: Vec<f32> = places.iter().map(|(t, _)| t.reach.clamp(0.0, 1.0)).collect();
    trade(&mut ledgers, &reach);
    ledgers
}

#[cfg(test)]
mod tests;
