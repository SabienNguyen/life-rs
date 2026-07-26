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

/// How many people it takes to keep an ordinary body of technique alive.
///
/// The Tasmanian number, and the reason technology is a *population* variable rather than
/// a clock. Technique is not written down here; it lives in people who know it, and every
/// one of them is an imperfect copy of the person they learned from. A large group has
/// enough learners that the best copy in each generation is nearly as good as the original;
/// a small one does not, and loses a little every generation until the technique is gone.
/// Tasmania was cut off at about four thousand people and over eight thousand years lost
/// bone tools, cold-weather clothing, fishing and hafted implements — not through any
/// catastrophe, through arithmetic.
const MINDS_TO_KEEP: f32 = 900.0;

/// How fast technique accumulates where there are minds enough, per year.
///
/// Slow. Pre-modern growth in productivity is a fraction of a per cent a century, and
/// anything faster here would run a stone-age valley to industry inside a simulated
/// lifetime.
const LEARNING: f32 = 0.0026;

/// How fast it is lost where there are not.
///
/// Faster than it is gained, which is the asymmetry that makes the record look the way it
/// does: accumulating takes millennia and losing takes generations.
const FORGETTING: f32 = 0.011;

/// The most technique can multiply what land yields.
///
/// Three. This is a pre-industrial ceiling on purpose — crop rotation, the mouldboard
/// plough, drainage and selective breeding between them roughly trebled European yields
/// over a thousand years, and everything past that needs the chemistry and the machines
/// §23 puts out of scope.
const TECHNIQUE_CEILING: f32 = 3.0;

/// What a people know how to do, as a multiplier on what their land yields.
///
/// One is bare subsistence farming. It is deliberately *not* a tech tree: there are no
/// discoveries, no prerequisites and no names, because §23 draws that boundary and a tree
/// is a project of its own. What is here is the part that has consequences at this scale —
/// that technique **accumulates where there are people to carry it and is lost where there
/// are not**, and that raising what land yields does not make anybody better off for long.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Technique(f32);

impl Technique {
    /// Knowing how to farm, and nothing beyond it.
    pub const BARE: Technique = Technique(1.0);

    pub fn level(self) -> f32 {
        self.0
    }

    /// A year of a people either learning or forgetting.
    ///
    /// Which of those happens is decided by how many of them there are — and by how well
    /// connected they are, because a technique lost in one valley can be relearned from
    /// the next one if anybody is travelling. That second term is why isolation is the
    /// thing that impoverishes rather than poverty itself.
    pub fn after_a_year(self, minds: f32, reach: f32) -> Technique {
        // Connection multiplies the pool of people you can learn from. An isolated group
        // is on its own; a well-connected one draws on everybody its roads reach.
        let carriers = minds * (0.5 + 1.5 * reach.clamp(0.0, 1.0));
        let level = if carriers >= MINDS_TO_KEEP {
            // Room to grow, and less of it the more there already is to know: each new
            // technique is harder than the last, which is why growth was so slow for so
            // long.
            let room = (TECHNIQUE_CEILING - self.0).max(0.0) / TECHNIQUE_CEILING;
            self.0 + LEARNING * room * (carriers / MINDS_TO_KEEP).min(4.0)
        } else {
            // Below the threshold, the copies degrade. Never below bare subsistence —
            // people do not forget how to eat.
            let shortfall = 1.0 - carriers / MINDS_TO_KEEP;
            self.0 - FORGETTING * shortfall * (self.0 - 1.0).max(0.0)
        };
        Technique(level.clamp(1.0, TECHNIQUE_CEILING))
    }
}

impl Default for Technique {
    fn default() -> Technique {
        Technique::BARE
    }
}

/// How hard births respond to how well the place is doing.
///
/// The steepness of the check. Too shallow and it is not a feedback; too steep and the
/// population oscillates or dies.
const RESPONSE: f32 = 1.7;

/// The most and least a place's economy can do to how many children are born there.
const FEWEST: f32 = 0.45;
const MOST: f32 = 1.65;

/// How many children are born here, against how many are born in an ordinary place *of
/// this world*.
///
/// The centring point is the world's own typical place, and getting there took three goes
/// that are worth recording because they are the same mistake in three costumes.
///
/// The first multiplied every birth by `1 − 0.72·(1 − opportunity)`. Opportunity averages a
/// little under a half, so the typical place — which should have been left alone — took a
/// forty per cent cut. The mean surviving population fell from a hundred and eighty to
/// eighty-two and three worlds in eight emptied.
///
/// The second centred on a *constant* living standard, chosen from what preindustrial
/// people plausibly had. But whether a model's places sit above or below any particular
/// constant is a fact about the model, not about history, and these sit below it: the
/// population runs to what the land carries, so surplus per head is small nearly
/// everywhere. Centring on 0.24 when the typical place is at 0.12 is the first mistake
/// again with better manners, and it took the mean population to eighty-eight.
///
/// So the centre is measured, not chosen. Against the world's own mean the multiplier
/// averages one **by construction**, whatever level that world happens to sit at, and the
/// check can only ever move births from worse places to better ones. Which is what a
/// Malthusian check is: the absolute number of people is set by mortality and by the land,
/// and what the check decides is *where they are born*.
pub fn births_relative(prosperity: f32, typical: f32) -> f32 {
    let spare = prosperity.clamp(0.0, 1.0) - typical.clamp(0.0, 1.0);
    (1.0 + RESPONSE * spare).clamp(FEWEST, MOST)
}

/// What a place makes, before trade.
///
/// `workers` is people, not households — the economy counts hands.
pub fn produce(terrain: &Terrain, workers: f32) -> Ledger {
    produce_knowing(terrain, workers, Technique::BARE)
}

/// What a place makes, given what its people know how to do.
pub fn produce_knowing(terrain: &Terrain, workers: f32, technique: Technique) -> Ledger {
    if workers <= 0.0 {
        return Ledger::EMPTY;
    }
    // The land, as an effective quantity. Fertility is what grows on it and harshness is
    // how much of the year is worth working; a productive place with a brutal season is
    // less than its soil suggests, which is most of the boreal world.
    let land = terrain.fertility.max(0.0) * (1.0 - 0.6 * terrain.hardship());

    // Cobb–Douglas. Neither factor is substitutable for the other, which is the point:
    // hands with no land make nothing and land with no hands makes nothing.
    let output =
        YIELD * technique.level() * land.powf(LAND_SHARE) * workers.powf(1.0 - LAND_SHARE);
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
    let knowing: Vec<(Terrain, f32, Technique)> = places
        .iter()
        .map(|(t, w)| (t.clone(), *w, Technique::BARE))
        .collect();
    year_knowing(&knowing)
}

/// A year for a region whose places each know something different.
pub fn year_knowing(places: &[(Terrain, f32, Technique)]) -> Vec<Ledger> {
    let mut ledgers: Vec<Ledger> = places
        .iter()
        .map(|(terrain, workers, technique)| produce_knowing(terrain, *workers, *technique))
        .collect();
    let reach: Vec<f32> = places
        .iter()
        .map(|(t, _, _)| t.reach.clamp(0.0, 1.0))
        .collect();
    trade(&mut ledgers, &reach);
    ledgers
}

#[cfg(test)]
mod tests;
