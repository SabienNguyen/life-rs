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

pub use work::{Good, Ground, Hands, Holdings, Made, SUBSISTENCE, Trade};

use society::Terrain;

/// How much of output is owed to the land rather than to the hands working it.
///
/// The exponent on land in `land^α · labour^(1−α)`. Estimates for pre-industrial agrarian
/// economies put land's share near a third and labour's near a half, with capital taking
/// the rest — and with no capital here the two have to sum to one, so a third to land is
/// the honest reading of the same evidence.
const LAND_SHARE: f32 = 0.35;

// `SUBSISTENCE` lives in `work`, which is the crate that fixes the unit everything else is
// counted in. Re-exported above so that nothing downstream has to know that.

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

    /// How far short of feeding its people a place falls, per head, as a fraction of what
    /// one person needs for a year. Zero when it manages.
    ///
    /// This is the *positive* check, and it is the half of Malthus that was missing. The
    /// other half — `births_relative` — is centred on the world's own middle, so it
    /// averages one by construction and can only ever decide **where** children are born.
    /// A world that is uniformly poorer gets a multiplier of one everywhere, which is
    /// correct for what that function is for and useless as a ceiling. Nothing was stopping
    /// population at all: places ran to three and five times what their ground would hold,
    /// growth accelerated to nearly two per cent a year, and it never levelled.
    ///
    /// It was being thrown away rather than missing. `prosperity` takes `per_head().max(0)`,
    /// so a place that cannot feed itself reports exactly what a place that just breaks even
    /// reports. Everything downstream read prosperity and nothing read this, so famine and
    /// bare sufficiency were the same number.
    ///
    /// Measured after trade, because a place that cannot feed itself but can buy food is not
    /// hungry — which is most of why trade matters.
    pub fn want(&self) -> f32 {
        if self.workers <= 0.0 {
            0.0
        } else {
            (-self.per_head()).max(0.0)
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

/// The same number, for anybody who needs it outside this crate.
///
/// How many people it takes to keep an ordinary body of technique alive — and therefore also
/// the scale at which a people has enough of itself to be having ideas in the first place.
pub const MINDS_ENOUGH: f64 = MINDS_TO_KEEP as f64;

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

/// How far an ordinary tradition can get with nobody ever having a new idea.
///
/// Three. Crop rotation, the mouldboard plough, drainage and selective breeding between them
/// roughly trebled European yields over a thousand years — but every one of those was worked
/// out by *somebody*, and what this number really measures is how much a large well-connected
/// population can wring out of a body of practice by copying each other well. It used to be a
/// hard ceiling and it is now a *starting* one: see `Technique::frontier`.
const FIRST_CEILING: f32 = 3.0;

/// How much one person working something out moves the limit of what is possible.
///
/// A **proportion**, and that is the whole of §29. An absolute step would make knowledge
/// arithmetic, and arithmetic knowledge always loses to a population that grows
/// geometrically — which is precisely why no world here has ever left the age it started in.
/// A proportion compounds, and compounding is the only shape that has ever escaped the trap.
///
/// Small, because most good ideas are small: it takes about seventy of them to double what a
/// people can do.
const BREAKTHROUGH: f32 = 0.01;

/// What a people know how to do, trade by trade.
///
/// Two numbers per trade rather than one, and the second is what this world was missing.
///
/// - **known** is what is actually practised. It rises by ordinary copying where there are
///   enough people to copy well, and falls where there are not — the Tasmanian result, which
///   is the whole of why technique is a *population* variable and not a clock.
/// - **frontier** is the most that could be practised. It used to be a constant three for
///   everybody for ever, which is why every world in this simulation was permanently
///   medieval: a people could get better at what it already did, and could never come to do
///   anything else.
///
/// Nothing moves the frontier except **somebody working something out** — a particular person,
/// in a particular year, in a particular place, who had the slack to think and thought of
/// something. See §29. It moves by a proportion rather than a step, so knowledge compounds,
/// and whether a world ever escapes its Malthusian ceiling is then an outcome of how much
/// surplus it managed to hold on to rather than a date somebody wrote down.
///
/// Per trade, because knowing how to farm better is not knowing how to smith better. A people
/// with no smiths never improves smithing, and two worlds that specialised differently end up
/// good at different things — which is a thing civilisations do and this model could not
/// previously express.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Technique {
    known: [f32; Trade::COUNT],
    frontier: [f32; Trade::COUNT],
}

impl Default for Technique {
    fn default() -> Technique {
        Technique::BARE
    }
}

impl Technique {
    /// Knowing how to farm, and nothing beyond it.
    pub const BARE: Technique = Technique {
        known: [1.0; Trade::COUNT],
        frontier: [FIRST_CEILING; Trade::COUNT],
    };

    /// What is practised in one trade.
    pub fn at(&self, trade: Trade) -> f32 {
        self.known[trade as usize]
    }

    /// The most that could be practised in one trade.
    pub fn frontier(&self, trade: Trade) -> f32 {
        self.frontier[trade as usize]
    }

    /// What is practised, averaged over everything a people does.
    ///
    /// For reading, and for the tests that predate there being more than one of these. What
    /// production uses is `at`, trade by trade.
    pub fn level(&self) -> f32 {
        self.known.iter().sum::<f32>() / Trade::COUNT as f32
    }

    /// How far past an ordinary tradition this people has been pushed, averaged.
    ///
    /// One for a people nobody has ever had an idea in. It is the number that says whether a
    /// world is still in the age it started in.
    pub fn reach_of_knowledge(&self) -> f32 {
        self.frontier.iter().sum::<f32>() / (Trade::COUNT as f32 * FIRST_CEILING)
    }

    /// Somebody worked something out.
    ///
    /// The only thing in the model that moves a limit rather than a level. It is deliberately
    /// not a discovery *of* anything named: what it is is that the people who do this trade
    /// can now go further than they could, and how far they actually get is still a matter of
    /// there being enough of them to carry it.
    pub fn worked_out(&mut self, trade: Trade) {
        self.frontier[trade as usize] *= 1.0 + BREAKTHROUGH;
    }

    /// A year of a people either learning or forgetting.
    ///
    /// **How much of a frontier a people can carry depends on how many of them there are.**
    /// This used to be a cliff: above `MINDS_TO_KEEP` carriers a people climbed towards the
    /// ceiling, and below it everything decayed to bare subsistence. That is stronger than
    /// the evidence it comes from. Tasmania is a claim about four thousand people *losing* a
    /// complex toolkit over eight thousand years, not a claim that two hundred people know
    /// nothing — and the cliff had a consequence nobody had looked at: every country in every
    /// world this simulation runs is smaller than the threshold, so **no world had ever
    /// practised anything above bare subsistence at all**. The whole of technique was inert.
    ///
    /// So what a people can hold is now a share, proportional to its carriers up to the point
    /// where it can hold everything. A hamlet keeps a hamlet's worth of a frontier and a
    /// nation keeps all of it, which preserves the Tasmanian result — cut a people off and
    /// shrink it and it loses what it can no longer carry — while letting a small people have
    /// *some* technique, which small peoples do.
    ///
    /// Connection multiplies the pool of people you can learn from: an isolated group is on
    /// its own, a well-connected one draws on everybody its roads reach. That second term is
    /// why isolation is the thing that impoverishes rather than poverty itself.
    pub fn after_a_year(mut self, minds: f32, reach: f32) -> Technique {
        let carriers = minds * (0.5 + 1.5 * reach.clamp(0.0, 1.0));
        // How much technique this many people can hold at all, whatever is known to be
        // possible. Absolute in the population rather than a share of the frontier: a share
        // would let forty people carry a quarter of an industrial civilisation, which is the
        // opposite of what Tasmania says. `MINDS_TO_KEEP` carriers hold exactly the old
        // ceiling of three, and holding more than that takes more of them.
        let holdable = 1.0 + (FIRST_CEILING - 1.0) * carriers / MINDS_TO_KEEP;
        // Small peoples learn slowly as well as holding less, but not so slowly that they
        // never arrive: a floor, or a hamlet takes ten thousand years to reach a hamlet's
        // ceiling and the distinction between holding little and holding nothing is lost.
        let pace = (carriers / MINDS_TO_KEEP).clamp(0.25, 4.0);

        for trade in Trade::ALL {
            let at = trade as usize;
            // The most this many people can hold of what is known to be possible.
            let sustainable = self.frontier[at].min(holdable);
            let known = self.known[at];
            let level = if known < sustainable {
                // Room to grow, and less of it the more there already is to know: each new
                // technique is harder than the last, which is why growth was so slow for so
                // long.
                let room = (sustainable - known) / sustainable;
                known + LEARNING * room * pace
            } else {
                // Above what they can carry, the copies degrade. Never below bare
                // subsistence — people do not forget how to eat.
                known - FORGETTING * (known - sustainable)
            };
            self.known[at] = level.clamp(1.0, sustainable.max(1.0));
        }
        self
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
///
/// Everybody farming, which is what this was before there was anything else to be.
pub fn produce_knowing(terrain: &Terrain, workers: f32, technique: Technique) -> Ledger {
    produce_working(
        terrain,
        &Hands::all_farming(workers),
        technique,
        &Holdings::default(),
    )
    .0
}

/// How much timber a year of cutting gets out of each kind of country.
///
/// The one number the biome label has ever been asked for. `Terrain` carried it from the
/// beginning and said in as many words that it was "for reading rather than for arithmetic",
/// which was honest and is no longer true: what grows on the ground decides what can be got
/// off it that is not food, and that is what makes one place a hewer's and another a
/// farmer's.
///
/// A rainforest is not the best of these. Standing timber is thickest there and getting it
/// out is worst, and a temperate forest with a dry season and a frozen river to float logs
/// down was where pre-industrial Europe actually cut its wood.
fn timber(biome: &str) -> f32 {
    match biome {
        "temperate forest" | "seasonal forest" => 1.0,
        "taiga" | "temperate rainforest" => 0.85,
        "rainforest" => 0.6,
        "shrubland" | "savanna" => 0.3,
        "grassland" => 0.15,
        // Tundra, desert, ice and anything under water. Driftwood and dung.
        _ => 0.05,
    }
}

/// How much stone and ore a year of quarrying gets out, by how high and hard the ground is.
///
/// Height is the proxy and it is a good one: mountains are where rock is at the surface, and
/// the same uplift that puts it there puts the ore with it. A river plain has a hundred
/// metres of its own silt over anything worth digging.
fn rock(elevation_m: f32, harshness: f32) -> f32 {
    let high = (elevation_m / 1200.0).clamp(0.0, 1.0);
    // Bare ground gives up its stone more easily than ground under a forest floor, which is
    // why the harsh places are not simply worse at everything.
    (0.15 + 0.85 * high) * (0.7 + 0.3 * harshness.clamp(0.0, 1.0))
}

/// What this ground gives a hand in a year, good by good.
///
/// Food is fertility and always was. Stock is timber plus rock, and the two together are why
/// a wooded hillside and a river plain are different places to live rather than the same
/// place with different numbers — and why, once they can reach each other, one of them ends up
/// full of hewers and the other full of farmers.
pub fn ground_of(terrain: &Terrain, technique: Technique) -> Ground {
    let workable =
        |land: f32, trade: Trade| YIELD * technique.at(trade) * land.max(0.0).powf(LAND_SHARE);
    let land = terrain.fertility.max(0.0) * (1.0 - 0.6 * terrain.hardship());
    Ground {
        food: workable(land, Trade::Farmer),
        stock: workable(
            timber(terrain.biome).max(rock(terrain.elevation_m, terrain.harshness)),
            Trade::Hewer,
        ),
        // One for one with food, since the two are already in the same unit — scaled by how
        // much of anything actually moves and by whether anybody can get here. A place off
        // every road sells nothing, whatever it is sitting on.
        sells_for: TRADE_REACH * terrain.reach.clamp(0.0, 1.0),
    }
}

/// What a hand at the bottom of the chain gets off this land in a year.
///
/// Cobb–Douglas, per worker. Only the hands actually *on the land* count towards the
/// crowding, because that is where the diminishing return comes from: a smith does not make
/// the fields smaller. Everybody still eats, which is counted separately and is what makes
/// every trade above the land a claim on somebody else's surplus.
fn per_hand(terrain: &Terrain, land_hands: f32, technique: Technique) -> Ground {
    if land_hands <= 0.0 {
        return Ground::default();
    }
    // Cobb–Douglas, per worker. Neither factor is substitutable for the other, which is the
    // point: hands with no land make nothing and land with no hands makes nothing. Only the
    // hands actually on the land count towards the crowding, because that is where the
    // diminishing return comes from — a smith does not make the fields smaller.
    let crowding = land_hands.powf(-LAND_SHARE);
    let ground = ground_of(terrain, technique);
    Ground {
        food: ground.food * crowding,
        stock: ground.stock * crowding,
        // Not crowded: what a unit fetches does not depend on how many hands cut it.
        sells_for: ground.sells_for,
    }
}

/// What a place makes, given who is doing what and what it owns.
///
/// Returns what became of the year and what the place still holds at the end of it — the
/// tools are the only thing in this world that survives a year, and they are what lets an
/// economy compound.
///
/// With everybody farming and nothing owned this is exactly the one-good model, which is
/// what protects every number §21 and §22 calibrated.
pub fn produce_working(
    terrain: &Terrain,
    hands: &Hands,
    technique: Technique,
    holdings: &Holdings,
) -> (Ledger, Made, Holdings) {
    let workers = hands.total();
    if workers <= 0.0 {
        return (Ledger::EMPTY, Made::default(), Holdings::default());
    }
    let on_the_land = hands.at(Trade::Farmer) + hands.at(Trade::Hewer);
    let ground = per_hand(terrain, on_the_land, technique);
    let (made, after) = work::make(hands, ground, holdings);

    let output = made.of(Good::Food);
    let subsistence = workers * SUBSISTENCE;
    let surplus = output - subsistence;
    (
        Ledger {
            output,
            subsistence,
            surplus,
            // Trade fills this in; alone, a place has only what it made.
            market: surplus,
            workers,
        },
        made,
        after,
    )
}

/// What one more hand in each trade would be worth here.
pub fn worth_of_trades(
    terrain: &Terrain,
    hands: &Hands,
    technique: Technique,
    holdings: &Holdings,
    made: &Made,
) -> [f32; Trade::COUNT] {
    let on_the_land = hands.at(Trade::Farmer) + hands.at(Trade::Hewer);
    let ground = per_hand(terrain, on_the_land.max(1.0), technique);
    work::worth_taking_up(made, holdings, hands, ground)
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

/// Places with material and no food buy food from places with food and no material.
///
/// The other half of trade, and the half that makes geography pay. `trade` pools *access*:
/// everybody who can be reached draws a share of everybody else's spare food, whatever they
/// have to offer. That is right for what it models — a road means the harvest two valleys
/// over is not irrelevant to you — but it is not an exchange, and without an exchange a place
/// whose ground gives timber and no wheat simply starves next to a place with the opposite
/// problem.
///
/// This is a barter, and unlike the pool above it is **conserved**: what one place hands over
/// another receives, both ends weighted by how easily they are reached. One unit of material
/// for one unit of food, which needs no currency and no price — the two are already in the
/// same unit, because the unit is what one person's year of work produces.
///
/// It only ever fires between places that differ. Two identical valleys trade nothing, which
/// is correct and is why this does nothing at all in a world whose ground is uniform.
pub fn barter(ledgers: &mut [Ledger], stock: &mut [f32], reach: &[f32]) {
    debug_assert_eq!(ledgers.len(), stock.len());
    debug_assert_eq!(ledgers.len(), reach.len());

    // What is on offer, and what is being asked for. A place bids only what it can actually
    // pay with and only for what it actually lacks.
    let for_sale: f32 = ledgers
        .iter()
        .zip(reach)
        .map(|(l, r)| l.surplus.max(0.0) * r)
        .sum();
    let bids: Vec<f32> = ledgers
        .iter()
        .zip(stock.iter())
        .zip(reach)
        .map(|((l, have), r)| ((-l.surplus).max(0.0)).min(*have) * r)
        .collect();
    let asked: f32 = bids.iter().sum();
    if for_sale <= 0.0 || asked <= 0.0 {
        return;
    }

    // Whichever side is the shorter, that is how much moves. Scaled by `TRADE_REACH` for the
    // same reason the pool is: most of what is grown is eaten within a few days' walk.
    let moved = (TRADE_REACH * for_sale).min(asked);

    for (at, bid) in bids.iter().enumerate() {
        let got = moved * bid / asked;
        ledgers[at].surplus += got;
        ledgers[at].market += got;
        stock[at] -= got;
    }
    // And the sellers, in proportion to what each had to sell.
    for at in 0..ledgers.len() {
        let sold = moved * ledgers[at].surplus.max(0.0) * reach[at] / for_sale;
        if sold <= 0.0 {
            continue;
        }
        ledgers[at].surplus -= sold;
        ledgers[at].market -= sold;
        stock[at] += sold;
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
    let working: Vec<(Terrain, Hands, Technique, Holdings)> = places
        .iter()
        .map(|(t, w, k)| {
            (
                t.clone(),
                Hands::all_farming(*w),
                *k,
                Holdings::default(),
            )
        })
        .collect();
    year_working(&working).into_iter().map(|(l, _, _)| l).collect()
}

/// A year for a region whose places each have their own trades and their own capital.
pub fn year_working(
    places: &[(Terrain, Hands, Technique, Holdings)],
) -> Vec<(Ledger, Made, Holdings)> {
    let mut worked: Vec<(Ledger, Made, Holdings)> = places
        .iter()
        .map(|(terrain, hands, technique, holdings)| {
            produce_working(terrain, hands, *technique, holdings)
        })
        .collect();
    let reach: Vec<f32> = places
        .iter()
        .map(|(t, _, _, _)| t.reach.clamp(0.0, 1.0))
        .collect();
    let mut ledgers: Vec<Ledger> = worked.iter().map(|(l, _, _)| *l).collect();
    trade(&mut ledgers, &reach);
    // And then the exchange, which needs the pooled access to have happened first: a place
    // only sells what it still has spare after its neighbours' harvest has reached it.
    let mut stock: Vec<f32> = worked.iter().map(|(_, _, h)| h.stock).collect();
    barter(&mut ledgers, &mut stock, &reach);
    for at in 0..worked.len() {
        worked[at].0 = ledgers[at];
        worked[at].2.stock = stock[at].max(0.0);
    }
    worked
}

#[cfg(test)]
mod tests;
