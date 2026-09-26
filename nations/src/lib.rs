//! The world at the scale of nations.
//!
//! §6 always said most of a planet would be simulated statistically: people in the places
//! nobody is watching are cohorts, not persons. The people-level `World` is five quarters of one
//! region. This is everywhere else — every cell of the same planet that a people could live on,
//! a town on each, and the towns doing to each other what §48.2 found no region in this world
//! had ever done to another: trading, paying, lending their money, and in the end keeping a
//! ledger together that none of them keeps alone.
//!
//! ## Four levels, none of them drawn
//!
//! - **Local**: a town. One per habitable cell, standing on the same planet `World::genesis`
//!   founds for the same seed, with the same habitability and the same ground.
//! - **State**: a market area. Every town is drawn to the largest market it can reach, weighted
//!   by the square of how near it is; a town drawn to itself is a hub, and a hub with the towns
//!   drawn to it is a state. As a city grows, its state widens. Nothing names a province.
//! - **Country**: a people who can reach each other, exactly as `World::countries` reads one —
//!   the same `culture` machinery, the same reach.
//! - **World**: every country's market, joined through whichever is largest.
//!
//! Goods move up and down that tree through `commerce::market`, and every link costs what
//! carriage over that distance costs at the world's technique, plus — where it crosses a
//! border — what it costs to be paid by somebody you do not quite trust, in a money that is not
//! yours.
//!
//! ## What a year is
//!
//! Everything in `year`, in an order that matters: the map (who is in which state and
//! country), then production, the market, spending, and what that does to people — who works at
//! what, what they save, how many are born and die and move — then what they learn, what they
//! come to use as money, what they pay each other abroad and how far they trust each other for
//! it, and last whether any of that is worth a ledger nobody keeps. Each step reads last year's
//! values of the ones after it, which is what a year is.

use std::collections::BTreeMap;

use commerce::market::{Offer, Tastes, Tree};
use commerce::money::{self, Acceptance, Currency, Medium};
use commerce::production::{self, Endowment, Output, Sector};
use commerce::{growth, payments};
use sim::Surface;
use sim_core::{Domain, Rng, WorldSeed};
use society::Terrain;

pub mod network;
pub use network::Network;

#[cfg(test)]
mod tests;

/// How much of a town works: the rest are children, the old, and those who keep a house.
pub const WORKING: f64 = 0.6;

/// Habitability a cell must have to hold a town: the bar deep time uses to settle new ground.
const SETTLE_ABOVE: f32 = 0.22;

/// A bound on the bookkeeping, not a claim about the world; it binds only on a planet almost
/// all of which is worth living on.
const MOST_TOWNS: usize = 64;

/// How far a people is from its ceiling at the founding: most of the way, which is where two
/// thousand years of farming leave anybody.
const FOUNDED_AT: f64 = 0.6;

/// How far a market's pull carries at bare technique, in kilometres: a town this far from a hub
/// needs a hub four times its own size to be drawn in, and one twice as far, nine times.
///
/// It has to be measured against the grid. Towns here stand at least two cells apart — some two
/// thousand kilometres — so a reach much shorter than that makes every town its own state and
/// the level between a town and a country is empty.
const MARKET_REACH_KM: f64 = 2_500.0;

/// How far an inland town is from the sea, on average: half a cell.
const COAST_KM: f64 = 500.0;

/// What carrying goods a thousand kilometres costs at bare technique, as a share of what they
/// are worth: dearly over land, a fifth as much by sea. Both fall as making improves — roads,
/// wagons, ships, and in the end engines.
const LAND_CARRIAGE: f64 = 0.20;
const SEA_CARRIAGE: f64 = 0.04;

/// What any market charges to handle a trade, per link, whatever the distance.
const HANDLING: f64 = 0.02;

/// How much capital wears out in a year. A little less than the people-level tools' tenth,
/// because a town's capital is buildings and roads as well as hand tools.
const WEAR: f64 = 0.08;

/// How fast workers move towards the trades that pay, per year, per unit of how much better
/// they pay. People are slow, and the slowness is what lets an occupational structure exist.
const RETRAINING: f64 = 0.10;

/// The least share of a town in any trade: there is always somebody trying it.
const FEWEST_HANDS: f64 = 0.002;

/// How much of a country's money has to be somebody's debt to a house, against its coin, before
/// the houses manage it rather than the metal: three times over. A money is managed by whoever
/// holds the country's deposits, and until most of what people pay with is credit rather than
/// metal, nobody does — which is where banking systems stood when they grew central houses.
const MANAGED_AT: f64 = 3.0;

/// How much of what houses have lent is repaid in a year: a tenth, so a loan runs about ten
/// years and what is outstanding is about ten years of lending.
const REPAID: f64 = 0.1;

/// A town this short of food in a year is a famine worth writing down.
const FAMINE: f64 = 0.2;

/// How much a harvest varies from year to year, as a standard deviation of its logarithm: a
/// tenth, which is the order of what pre-modern grain yields did. Shared across a state, because
/// weather is.
const HARVESTS_VARY: f64 = 0.10;

/// How readily people substitute one country's wares for another's — the Armington elasticity.
///
/// Five, the middle of what trade studies estimate. It is what turns the cost of crossing a
/// border into how much less people buy across it: at a wedge of a third, a foreign ware sells
/// about a third as well as it would at home.
const VARIETY: f64 = 5.0;

/// How many years the world's income is averaged over before a rise in it counts as the trap
/// opening, so that a founding's first adjustments are not mistaken for history.
const SUSTAINED: usize = 10;

/// A town: the people of one cell of the planet, and what they do.
#[derive(Clone, Debug)]
pub struct Town {
    pub name: String,
    pub cell: u32,
    pub terrain: Terrain,
    pub coastal: bool,
    pub ground: Endowment,
    pub area_km2: f64,
    pub people: f64,
    /// Share of workers in each trade.
    pub shares: [f64; Sector::COUNT],
    /// Wares held as capital.
    pub capital: f64,
    /// What each trade knows how to do. Held by the town, shared across its country each year.
    pub technique: [f64; Sector::COUNT],
    /// How much of its trade uses each medium.
    pub acceptance: Acceptance,
    /// The year it first mostly traded in a medium, and which.
    pub monetised: Option<(u64, Medium)>,
    /// Last year, as read.
    pub output: Output,
    /// Income per head, in years of food.
    pub income: f64,
    /// Share short of what its people needed to eat.
    pub hunger: f64,
    /// The price of a ware in food, here.
    pub price: f64,
    /// Share of income that passed through exchange.
    pub exchanged: f64,
    /// Share of what was exchanged that exchanging lost.
    pub loss: f64,
    /// Wares shipped to its market; negative is wares bought.
    pub shipped: f64,
    /// What each trade paid a worker last year, in food.
    pub wages: [f64; Sector::COUNT],
    pub state: usize,
    pub country: usize,
    exchange_value: f64,
    service_price: f64,
    /// Wares bought last year, to use and to invest, in food.
    wares_bought: f64,
    /// What it put by last year, in food, and the share of that its houses could not lend.
    pub saved: f64,
    pub unlent: f64,
}

impl Town {
    pub fn workers(&self) -> [f64; Sector::COUNT] {
        self.shares.map(|s| s * self.people * WORKING)
    }

    /// Everything it produced, valued at its own prices, in years of food.
    pub fn product(&self) -> f64 {
        self.output.food
            + self.price * self.output.wares
            + self.service_price * self.output.services
    }
}

/// A market area: a hub and the towns drawn to it.
#[derive(Clone, Debug)]
pub struct State {
    pub hub: usize,
    pub towns: Vec<usize>,
    pub country: usize,
}

/// A people who can reach each other, and their money.
#[derive(Clone, Debug)]
pub struct Country {
    /// The lowest-numbered town in it, which is what carries its identity from year to year.
    pub key: usize,
    pub name: String,
    pub towns: Vec<usize>,
    /// Its largest town.
    pub capital: usize,
    pub states: Vec<usize>,
    pub currency: Option<usize>,
    pub people: f64,
    pub product: f64,
    /// What it sold abroad last year, in years of food at world prices.
    pub exports: f64,
    /// What paying abroad cost it last year, as a share of a payment.
    pub pay_cost: f64,
}

/// Something that happened to the world, worth a line in its history.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    /// A town's trade came to run mostly on a medium.
    Monetised { year: u64, town: usize, medium: Medium },
    /// A country stamped its money.
    Coined { year: u64, currency: usize },
    /// A currency became the one the world invoices in.
    Reserve { year: u64, currency: usize },
    /// A town went hungry.
    Famine { year: u64, town: usize, hunger: f64 },
    /// The world's income per head first passed twice what it takes to eat.
    TrapOpened { year: u64 },
    /// A ledger nobody keeps was founded.
    Founded { year: u64, network: usize },
    /// A stable token was registered on one.
    Issued { year: u64, network: usize, symbol: String },
    /// A house started validating.
    Joined { year: u64, network: usize, town: usize },
    /// A block could not be finalised: more than a third of the stake was absent.
    Stalled { year: u64, network: usize, height: u64 },
}

impl Event {
    pub fn year(&self) -> u64 {
        match self {
            Event::Monetised { year, .. }
            | Event::Coined { year, .. }
            | Event::Reserve { year, .. }
            | Event::Famine { year, .. }
            | Event::TrapOpened { year }
            | Event::Founded { year, .. }
            | Event::Issued { year, .. }
            | Event::Joined { year, .. }
            | Event::Stalled { year, .. } => *year,
        }
    }
}

/// One year of the whole world, as a line in a table.
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    pub year: u64,
    pub people: f64,
    pub product: f64,
    pub income: f64,
    pub hunger: f64,
    /// Share of the world's product sold across a border.
    pub traded: f64,
    /// Share of workers in each trade.
    pub shares: [f64; Sector::COUNT],
    /// People-weighted technique in each trade.
    pub technique: [f64; Sector::COUNT],
    /// People-weighted share of trade using a medium.
    pub monetised: f64,
    pub countries: usize,
    pub states: usize,
    pub currencies: usize,
    /// Share of cross-border payments settled on a chain.
    pub on_chain: f64,
    /// Blocks committed, across every chain.
    pub blocks: u64,
}

/// The world, at the scale of nations.
pub struct Nations {
    pub seed: WorldSeed,
    pub surface: Surface,
    pub year: u64,
    pub towns: Vec<Town>,
    pub states: Vec<State>,
    pub countries: Vec<Country>,
    pub currencies: Vec<Currency>,
    /// Which town issued each currency.
    pub issuers: Vec<usize>,
    /// What the houses of each currency's country have lent and not yet been repaid, in food.
    pub deposits: Vec<f64>,
    /// The currency the world invoices in, if one has emerged.
    pub reserve: Option<usize>,
    /// Last year's payments abroad, between country keys, in years of food.
    pub payments: BTreeMap<(usize, usize), f64>,
    /// How well each pair of countries knows each other, by key.
    familiarity: BTreeMap<(usize, usize), f64>,
    pub cultures: culture::Cultures,
    pub networks: Vec<Network>,
    /// Why no chain has been founded, as of the last year anybody asked.
    pub not_yet: Option<payments::NotYet>,
    pub history: Vec<Event>,
    pub readings: Vec<Reading>,
    /// The price of a ware in food at the world market.
    pub world_price: f64,
    /// Whether goods can move between towns at all.
    ///
    /// A switch on the world rather than a constant, for the reason the people-level world gives
    /// its own switches: an ablation nobody can run without editing the source is an ablation
    /// nobody runs. With it off every link in the market tree costs as much as a link can, and
    /// each town lives on what it grows.
    pub trade_is_possible: bool,
    /// Whether being paid abroad costs anything — the ablation that asks what borders cost a
    /// world. With it on, a payment across a border costs what one at home does: nothing more
    /// than the carriage and handling every link pays.
    pub borders_are_free: bool,
    /// Whether houses may found a ledger nobody keeps — the ablation that asks what one is worth.
    pub chains_are_possible: bool,
    tastes: Tastes,
    distances: Vec<Vec<f64>>,
    reach_km: f64,
    trap_opened: bool,
}

impl Nations {
    /// Put people on every cell of the planet worth living on.
    ///
    /// The planet is `Surface::genesis(seed)` — the same one `World::genesis(seed)` stands on —
    /// so the five quarters of the people-level world are somewhere on this map.
    pub fn found(seed: WorldSeed) -> Nations {
        Nations::on(seed, Surface::genesis(seed))
    }

    /// The same, on a surface already made.
    pub fn on(seed: WorldSeed, surface: Surface) -> Nations {
        let habitability =
            settlement::Habitability::of(&surface.planet, &surface.climate, &surface.life);
        let planet = &surface.planet;
        let grid = planet.grid();
        let mut candidates: Vec<(u32, f32)> = grid
            .cells()
            .filter(|c| habitability.score(*c) >= SETTLE_ABOVE)
            .map(|c| (c, habitability.score(c)))
            .collect();
        candidates.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));

        let mut naming = seed.stream(Domain::Naming, 0xc033_e700, 0);
        let mut towns: Vec<Town> = Vec::new();
        for (cell, _) in candidates {
            if towns.len() >= MOST_TOWNS {
                break;
            }
            // Two towns in neighbouring cells are one town.
            if towns
                .iter()
                .any(|t| t.cell == cell || grid.neighbours(t.cell).contains(&cell))
            {
                continue;
            }
            let position = grid.position(cell);
            let terrain = Terrain {
                cell,
                latitude: position.latitude().to_degrees() as f32,
                longitude: position.longitude().to_degrees() as f32,
                elevation_m: planet.height_above_sea_m(cell),
                fertility: habitability.fertility(cell),
                reach: habitability.reach(cell),
                harshness: habitability.harshness(cell),
                carrying: 1,
                biome: surface.life.biome(cell).label(),
            };
            let coastal = grid.neighbours(cell).iter().any(|n| !planet.is_land(*n));
            let area_km2 = grid.area_km2(cell, geo::EARTH_RADIUS_KM);
            let ground = Endowment::of(&terrain, area_km2);
            let name = settlement::naming::name_for(&terrain, coastal, &mut naming);
            let people = FOUNDED_AT * ceiling(&ground);
            towns.push(Town {
                name,
                cell,
                terrain,
                coastal,
                ground,
                area_km2,
                people,
                shares: [0.82, 0.08, 0.07, 0.03],
                capital: 0.1 * people * WORKING,
                technique: [1.0; Sector::COUNT],
                acceptance: Acceptance::default(),
                monetised: None,
                output: Output::default(),
                income: 1.0,
                hunger: 0.0,
                price: 1.0,
                exchanged: 0.0,
                loss: 0.0,
                shipped: 0.0,
                wages: [0.0; Sector::COUNT],
                state: 0,
                country: 0,
                exchange_value: 0.0,
                service_price: 1.0,
                wares_bought: 0.0,
                saved: 0.0,
                unlent: money::UNBANKED,
            });
        }

        let n = towns.len();
        let mut distances = vec![vec![0.0; n]; n];
        for a in 0..n {
            for b in 0..n {
                distances[a][b] = grid.distance_km(towns[a].cell, towns[b].cell, geo::EARTH_RADIUS_KM);
            }
        }
        // The reach a country is held together by, as `World::within_reach` reads it.
        let reach_km = 3.0 * grid.spacing_km(geo::EARTH_RADIUS_KM);
        let hearth = towns
            .iter()
            .max_by(|a, b| a.people.total_cmp(&b.people))
            .map(|t| t.name.clone())
            .unwrap_or_else(|| "Firstfolk".to_string());
        let cultures = culture::Cultures::beginning(n, hearth);

        let mut nations = Nations {
            seed,
            surface,
            year: 0,
            towns,
            states: Vec::new(),
            countries: Vec::new(),
            currencies: Vec::new(),
            issuers: Vec::new(),
            deposits: Vec::new(),
            reserve: None,
            payments: BTreeMap::new(),
            familiarity: BTreeMap::new(),
            cultures,
            networks: Vec::new(),
            not_yet: None,
            history: Vec::new(),
            readings: Vec::new(),
            world_price: 1.0,
            trade_is_possible: true,
            borders_are_free: false,
            chains_are_possible: true,
            tastes: Tastes::ORDINARY,
            distances,
            reach_km,
            trap_opened: false,
        };
        nations.map();
        nations
    }

    /// Run for a number of years.
    pub fn run(&mut self, years: u64) {
        for _ in 0..years {
            self.year();
        }
    }

    pub fn within_reach(&self, a: usize, b: usize) -> bool {
        self.distances[a][b] <= self.reach_km
    }

    pub fn distance_km(&self, a: usize, b: usize) -> f64 {
        self.distances[a][b]
    }

    pub fn people(&self) -> f64 {
        self.towns.iter().map(|t| t.people).sum()
    }

    fn stream(&self, purpose: u64) -> Rng {
        self.seed.stream(Domain::Commerce, purpose, self.year)
    }

    /// How this year's weather treated a town's fields: a draw shared by its whole state, and a
    /// smaller one of its own. The harvest is what famine is made of, and what trade exists to
    /// smooth.
    fn harvest(&self, town: usize) -> f64 {
        let state = self.towns[town].state as u64;
        let shared = self
            .seed
            .stream(Domain::Weather, 0x5747_0000 + state, self.year)
            .normal();
        let local = self
            .seed
            .stream(Domain::Weather, town as u64, self.year)
            .normal();
        (HARVESTS_VARY * (0.8 * shared + 0.6 * local)).exp()
    }

    /// Who belongs to which country and which state, read off where people are now.
    fn map(&mut self) {
        let n = self.towns.len();
        let souls: Vec<u32> = self
            .towns
            .iter()
            .map(|t| t.people.round().clamp(0.0, u32::MAX as f64) as u32)
            .collect();
        let found = self.cultures.countries(&souls, |a, b| self.within_reach(a, b));
        // What paying abroad cost each country last year, and what it sold abroad, carry over to
        // the country with the same key: this year's market is priced with the one and this
        // year's catching up is paced by the other, and neither is worked out again until the
        // year's trade is known. Redrawing the countries used to set both back to nothing, so no
        // border ever cost anything, trade never brought anybody's technique on, and the chain
        // lowered a number nothing read. A country new this year starts where the others stood
        // on average, and as if it had sold nothing.
        let carried: BTreeMap<usize, (f64, f64)> = self
            .countries
            .iter()
            .map(|c| (c.key, (c.pay_cost, c.exports)))
            .collect();
        let usual = if carried.is_empty() {
            0.0
        } else {
            carried.values().map(|(cost, _)| cost).sum::<f64>() / carried.len() as f64
        };
        let mut countries: Vec<Country> = Vec::new();
        for c in found {
            let mut towns = c.places.clone();
            towns.sort_unstable();
            let key = towns[0];
            let capital = *towns
                .iter()
                .max_by(|a, b| {
                    self.size(**a)
                        .total_cmp(&self.size(**b))
                        .then(b.cmp(a))
                })
                .expect("a country has a town");
            countries.push(Country {
                key,
                name: culture::naming::name_a_country(&self.towns[capital].name),
                towns,
                capital,
                states: Vec::new(),
                currency: None,
                people: 0.0,
                product: 0.0,
                exports: carried.get(&key).map(|(_, sold)| *sold).unwrap_or(0.0),
                pay_cost: carried.get(&key).map(|(cost, _)| *cost).unwrap_or(usual),
            });
        }
        // Largest first, then by key, so the order is stable.
        countries.sort_by(|a, b| {
            let (pa, pb) = (
                a.towns.iter().map(|t| self.towns[*t].people).sum::<f64>(),
                b.towns.iter().map(|t| self.towns[*t].people).sum::<f64>(),
            );
            pb.total_cmp(&pa).then(a.key.cmp(&b.key))
        });
        for (at, country) in countries.iter().enumerate() {
            for t in &country.towns {
                self.towns[*t].country = at;
            }
        }

        // States: every town drawn to the largest market it can reach within its country.
        let reach = self.market_reach_km();
        let mut hub: Vec<usize> = (0..n).collect();
        for t in 0..n {
            let country = &countries[self.towns[t].country];
            let pull = |h: usize| {
                let d = self.distances[t][h] / reach;
                self.size(h) / ((1.0 + d) * (1.0 + d))
            };
            hub[t] = *country
                .towns
                .iter()
                .filter(|h| **h == t || self.within_reach(t, **h))
                .max_by(|a, b| pull(**a).total_cmp(&pull(**b)).then(b.cmp(a)))
                .expect("a town can reach itself");
        }
        // Follow each chain of pulls to a town that pulls itself.
        let root = |mut t: usize| {
            for _ in 0..n {
                if hub[t] == t {
                    break;
                }
                t = hub[t];
            }
            t
        };
        let mut states: Vec<State> = Vec::new();
        let mut state_of: BTreeMap<usize, usize> = BTreeMap::new();
        for t in 0..n {
            let r = root(t);
            let at = *state_of.entry(r).or_insert_with(|| {
                states.push(State {
                    hub: r,
                    towns: Vec::new(),
                    country: self.towns[r].country,
                });
                states.len() - 1
            });
            states[at].towns.push(t);
            self.towns[t].state = at;
        }
        for (at, state) in states.iter().enumerate() {
            countries[state.country].states.push(at);
        }
        // A country's money is whichever currency one of its towns issued — the largest issuer's
        // if more than one did.
        for country in &mut countries {
            country.currency = (0..self.currencies.len())
                .filter(|c| country.towns.contains(&self.issuers[*c]))
                .max_by(|a, b| {
                    self.size(self.issuers[*a])
                        .total_cmp(&self.size(self.issuers[*b]))
                        .then(b.cmp(a))
                });
            country.people = country.towns.iter().map(|t| self.towns[*t].people).sum();
            country.product = country.towns.iter().map(|t| self.towns[*t].product()).sum();
        }
        self.states = states;
        self.countries = countries;
    }

    /// How big a town's market is: what it produced last year, or its people before it has
    /// produced anything.
    fn size(&self, town: usize) -> f64 {
        let t = &self.towns[town];
        let product = t.product();
        if product > 0.0 { product } else { t.people }
    }

    /// What carrying goods between two towns costs, as a share of their worth: overland, or to
    /// the coast, by sea and inland again, whichever is cheaper. A long haul is almost always by
    /// sea — which is the whole of why coasts were rich — and between two landmasses it is the
    /// only way at all, which the land route's price says on its own.
    fn carriage(&self, a: usize, b: usize) -> f64 {
        if a == b {
            return 0.0;
        }
        let (ta, tb) = (&self.towns[a], &self.towns[b]);
        let thousands = self.distances[a][b] / 1000.0;
        let to_coast = |t: &Town| if t.coastal { 0.0 } else { COAST_KM / 1000.0 * LAND_CARRIAGE };
        let overland = thousands * LAND_CARRIAGE;
        let by_sea = thousands * SEA_CARRIAGE + to_coast(ta) + to_coast(tb);
        let making = 0.5
            * (ta.technique[Sector::Making as usize] + tb.technique[Sector::Making as usize]);
        (overland.min(by_sea) / making.max(1.0).sqrt()).min(0.9)
    }

    /// How far a market's pull carries: further as carriage gets cheaper, so market areas widen
    /// and merge as a world learns to move things.
    fn market_reach_km(&self) -> f64 {
        let people = self.people().max(1.0);
        let making = self
            .towns
            .iter()
            .map(|t| t.people * t.technique[Sector::Making as usize])
            .sum::<f64>()
            / people;
        MARKET_REACH_KM * making.max(1.0).sqrt()
    }

    /// A year.
    pub fn year(&mut self) {
        self.year += 1;
        self.map();
        let n = self.towns.len();
        let world_capital = self
            .countries
            .iter()
            .max_by(|a, b| a.product.total_cmp(&b.product).then(b.key.cmp(&a.key)))
            .map(|c| c.capital)
            .unwrap_or(0);

        // What every town makes this year, and what exchanging it will cost.
        let mut offers = Vec::with_capacity(n);
        let mut outputs = Vec::with_capacity(n);
        for t in 0..n {
            let town = &self.towns[t];
            let workers = town.workers();
            let capital = production::share_capital(town.capital, &workers);
            let mut output =
                production::produce(&town.ground, &workers, &capital, &town.technique);
            output.food *= self.harvest(t);
            let coined = self.countries[town.country].currency.is_some();
            let friction = money::friction(&town.acceptance, coined);
            let loss = money::exchange_loss(friction, town.exchange_value, output.reckoning);
            // Farmers eat their own; everything else a town makes changes hands at least once.
            let sold_food = output.food * (1.0 - town.shares[Sector::Farming as usize]);
            offers.push(Offer {
                food: output.food - loss * sold_food,
                wares: output.wares * (1.0 - loss),
                mouths: town.people,
            });
            outputs.push((output, friction, loss));
        }

        // The market tree: towns, then states, then countries, then the world.
        let (s0, c0) = (n, n + self.states.len());
        let root = c0 + self.countries.len();
        let mut parent = vec![None; root + 1];
        let mut wedge = vec![0.0; root + 1];
        for t in 0..n {
            let state = &self.states[self.towns[t].state];
            parent[t] = Some(s0 + self.towns[t].state);
            wedge[t] = HANDLING + self.carriage(t, state.hub);
        }
        for (s, state) in self.states.iter().enumerate() {
            let capital = self.countries[state.country].capital;
            parent[s0 + s] = Some(c0 + state.country);
            wedge[s0 + s] = HANDLING + self.carriage(state.hub, capital);
        }
        for (c, country) in self.countries.iter().enumerate() {
            parent[c0 + c] = Some(root);
            let far = self.carriage(country.capital, world_capital);
            // Crossing a border costs what being paid abroad costs, which is what a chain is for.
            let paying = if self.countries.len() > 1 { country.pay_cost } else { 0.0 };
            wedge[c0 + c] = HANDLING + far + paying;
        }
        if !self.trade_is_possible {
            wedge.fill(0.95);
        }
        let tree = Tree::new(parent, wedge, n);
        let clearing = tree.clear(&offers, &self.tastes);
        self.world_price = clearing.price[root];

        // What each town did with it, and what that does to its people.
        let mut rng = self.stream(1);
        for t in 0..n {
            let (output, friction, loss) = outputs[t];
            let q = clearing.price[t];
            let spent = commerce::market::spend(&offers[t], q, &self.tastes);
            let town = &mut self.towns[t];
            let people = town.people.max(1.0);
            let service_price = if output.services > 0.0 {
                spent.services / output.services
            } else {
                1.0
            };
            let income = (spent.food + q * (spent.wares + spent.invested) + spent.services) / people;
            // What passed through exchange: everything but what farmers ate of their own, and
            // everything shipped.
            let exchanged_value = output.food * (1.0 - town.shares[0])
                + q * output.wares
                + spent.services
                + (clearing.shipped[t] * q).abs();
            let total = income * people;

            // The town's houses keep count of what changes hands and lend out what is saved,
            // with the same clerks, in proportion to how much of each there is.
            let saved = q * spent.invested;
            let counting = exchanged_value + saved;
            let (for_trade, for_saving) = if counting > 0.0 {
                (exchanged_value / counting, saved / counting)
            } else {
                (1.0, 0.0)
            };
            let unlent = money::exchange_loss(
                money::UNBANKED,
                saved,
                output.reckoning * for_saving,
            );

            // What each trade paid a worker, in food.
            let workers = town.workers();
            let capital = production::share_capital(town.capital, &workers);
            let marginal =
                production::marginal(&output, &workers, &town.technique, &town.ground, &capital);
            let keep = 1.0 - loss;
            let reckoning_worth = for_trade
                * money::worth_of_reckoning(friction, exchanged_value, output.reckoning * for_trade)
                + for_saving
                    * money::worth_of_reckoning(
                        money::UNBANKED,
                        saved,
                        output.reckoning * for_saving,
                    );
            let wages = [
                marginal[0],
                q * marginal[1] * keep,
                service_price * marginal[2] * keep,
                reckoning_worth * marginal[3],
            ];

            // People move towards what pays, slowly, and never all the way out of anything.
            let mean: f64 = (0..Sector::COUNT).map(|s| town.shares[s] * wages[s]).sum();
            if mean > 0.0 {
                let mut shares = town.shares;
                for s in 0..Sector::COUNT {
                    let pull = (wages[s] / mean - 1.0).clamp(-1.0, 1.0);
                    shares[s] = (shares[s] * (1.0 + RETRAINING * pull)).max(FEWEST_HANDS);
                }
                let sum: f64 = shares.iter().sum();
                town.shares = shares.map(|s| s / sum);
            }

            town.capital = town.capital * (1.0 - WEAR) + spent.invested * (1.0 - unlent);
            let births = growth::natural_increase(income);
            let deaths = growth::famine(spent.hunger);
            town.people = (town.people * (1.0 + births - deaths)).max(1.0);

            let first = town.monetised.is_none();
            let available = [output.food > 0.0, output.wares > 0.0];
            let share_exchanged = if total > 0.0 {
                (exchanged_value / total).clamp(0.0, 1.0)
            } else {
                0.0
            };
            town.acceptance.year(share_exchanged, available, &mut rng);
            if first && let Some(medium) = town.acceptance.money() {
                town.monetised = Some((self.year, medium));
                self.history.push(Event::Monetised {
                    year: self.year,
                    town: t,
                    medium,
                });
            }
            if spent.hunger > FAMINE {
                self.history.push(Event::Famine {
                    year: self.year,
                    town: t,
                    hunger: spent.hunger,
                });
            }

            town.output = output;
            town.income = income;
            town.hunger = spent.hunger;
            town.price = q;
            town.exchanged = share_exchanged;
            town.loss = loss;
            town.shipped = clearing.shipped[t];
            town.wages = wages;
            town.exchange_value = exchanged_value;
            town.service_price = service_price;
            town.wares_bought = q * (spent.wares + spent.invested);
            town.saved = saved;
            town.unlent = unlent;
        }

        self.move_house();
        self.learn();
        self.coin();
        self.pay_abroad(&clearing, c0);
        self.keep_ledgers();
        self.drift();
        self.read();
    }

    /// People drift towards the richer towns of their own country.
    fn move_house(&mut self) {
        for country in &self.countries {
            let mut people: Vec<f64> = country.towns.iter().map(|t| self.towns[*t].people).collect();
            let income: Vec<f64> = country.towns.iter().map(|t| self.towns[*t].income).collect();
            growth::resettle(&mut people, &income);
            for (t, p) in country.towns.iter().zip(people) {
                self.towns[*t].people = p;
            }
        }
    }

    /// What every country works out, and what it picks up from those it trades with.
    fn learn(&mut self) {
        // Each country's frontier is the best any of its towns knows, moved by everybody in it
        // with time to think.
        let mut frontier: Vec<[f64; Sector::COUNT]> = Vec::with_capacity(self.countries.len());
        for country in &self.countries {
            let mut best = [1.0f64; Sector::COUNT];
            let mut thinking = [0.0f64; Sector::COUNT];
            for t in &country.towns {
                let town = &self.towns[*t];
                let workers = town.workers();
                for s in 0..Sector::COUNT {
                    best[s] = best[s].max(town.technique[s]);
                    thinking[s] += growth::thinkers(workers[s], town.income);
                }
            }
            let found = growth::discoveries(&thinking, &best);
            for s in 0..Sector::COUNT {
                best[s] *= 1.0 + found[s];
            }
            frontier.push(best);
        }
        // And what trade carries between them.
        let caught: Vec<[f64; Sector::COUNT]> = (0..self.countries.len())
            .map(|c| {
                let country = &self.countries[c];
                let openness = if country.product > 0.0 {
                    (country.exports / country.product).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let mut mine = frontier[c];
                for (d, other) in self.countries.iter().enumerate() {
                    if d == c || !self.trade_between(country.key, other.key) {
                        continue;
                    }
                    for s in 0..Sector::COUNT {
                        mine[s] = growth::catch_up(mine[s], frontier[d][s], openness);
                    }
                }
                mine
            })
            .collect();
        for (c, country) in self.countries.iter().enumerate() {
            for t in &country.towns {
                self.towns[*t].technique = caught[c];
            }
        }
    }

    fn trade_between(&self, a: usize, b: usize) -> bool {
        self.payments.get(&(a.min(b), a.max(b))).is_some_and(|v| *v > 0.0)
    }

    /// Countries whose trade runs on a medium, and is large enough to pay for a mint, stamp it.
    fn coin(&mut self) {
        for c in 0..self.countries.len() {
            let country = &self.countries[c];
            let exchanged: f64 = country
                .towns
                .iter()
                .map(|t| self.towns[*t].exchange_value)
                .sum();
            // The country's acceptance, weighted by how much each town trades.
            let mut shares = [0.0; 2];
            if exchanged > 0.0 {
                for t in &country.towns {
                    let town = &self.towns[*t];
                    for (share, accepted) in shares.iter_mut().zip(town.acceptance.0) {
                        *share += accepted * town.exchange_value / exchanged;
                    }
                }
            }
            let acceptance = Acceptance(shares);
            if country.currency.is_none() && money::worth_coining(&acceptance, exchanged) {
                let medium = acceptance.money().expect("worth coining means there is money");
                let (name, symbol) = self.currency_name(c, medium);
                let issuer = self.countries[c].capital;
                self.currencies.push(Currency::mint(name, symbol, medium, self.year, exchanged));
                self.issuers.push(issuer);
                self.deposits.push(0.0);
                let id = self.currencies.len() - 1;
                self.countries[c].currency = Some(id);
                self.history.push(Event::Coined {
                    year: self.year,
                    currency: id,
                });
            }
        }
        // Every currency's year: commodity money grows with the medium it is made of, and a
        // country whose houses are large enough manages its own.
        for id in 0..self.currencies.len() {
            let Some(c) = self.countries.iter().position(|c| c.currency == Some(id)) else {
                continue;
            };
            let country = &self.countries[c];
            let exchanged: f64 = country
                .towns
                .iter()
                .map(|t| self.towns[*t].exchange_value)
                .sum();
            // What the country's houses lent this year, in food.
            let lent: f64 = country
                .towns
                .iter()
                .map(|t| self.towns[*t].saved * (1.0 - self.towns[*t].unlent))
                .sum();
            let medium = self.currencies[id].medium;
            // What the medium is worth to make this year, in food.
            let made: f64 = country
                .towns
                .iter()
                .map(|t| {
                    let town = &self.towns[*t];
                    match medium {
                        Medium::Grain => town.output.food,
                        Medium::Metal => town.output.wares * town.price,
                    }
                })
                .sum();
            let currency = &mut self.currencies[id];
            let in_food = currency.money / currency.level.max(1e-9);
            self.deposits[id] = self.deposits[id] * (1.0 - REPAID) + lent;
            if !currency.managed && self.deposits[id] >= MANAGED_AT * in_food {
                currency.managed = true;
            }
            let medium_growth = money::commodity_growth(medium, made, in_food);
            currency.year(exchanged, medium_growth);
        }
    }

    /// A name for a country's money: its capital's name and what the money is made of.
    fn currency_name(&self, country: usize, medium: Medium) -> (String, String) {
        let place = &self.countries[country].name;
        let name = format!("{place} {}", medium.stem().to_lowercase());
        let mut symbol: String = place
            .chars()
            .filter(|c| c.is_ascii_alphabetic())
            .take(3)
            .collect::<String>()
            .to_uppercase();
        while symbol.len() < 3 {
            symbol.push('X');
        }
        let mut unique = symbol.clone();
        let mut n = 1;
        while self.currencies.iter().any(|c| c.symbol == unique) {
            n += 1;
            unique = format!("{symbol}{n}");
        }
        (name, unique)
    }

    /// Who paid whom abroad, what it cost, and what that did to how far they trust each other.
    fn pay_abroad(&mut self, clearing: &commerce::market::Clearing, c0: usize) {
        let q = self.world_price;
        // A country shipping wares out is paid for them; one taking wares in pays with food.
        let sold: Vec<f64> = (0..self.countries.len())
            .map(|c| (clearing.shipped[c0 + c] * q).max(0.0))
            .collect();
        let bought: Vec<f64> = (0..self.countries.len())
            .map(|c| (-clearing.shipped[c0 + c] * q).max(0.0))
            .collect();
        let total_sold: f64 = sold.iter().sum();
        let mut payments: BTreeMap<(usize, usize), f64> = BTreeMap::new();
        for (c, country) in self.countries.iter().enumerate() {
            for (d, other) in self.countries.iter().enumerate() {
                if c == d || total_sold <= 0.0 {
                    continue;
                }
                // d buys c's wares in proportion to what d buys, and pays for them; c buys d's
                // food with what it earned, and pays for that. Two payments, one each way.
                let flow = sold[c] * bought[d] / total_sold;
                if flow > 0.0 {
                    *payments
                        .entry((country.key.min(other.key), country.key.max(other.key)))
                        .or_insert(0.0) += 2.0 * flow;
                }
            }
        }
        for c in 0..self.countries.len() {
            self.countries[c].exports = sold[c] + bought[c];
        }

        // And the trade the two-good market cannot see: countries' wares are not the same wares,
        // so people buy some of each other's whatever the net position — two-way trade in
        // varieties, in proportion to what each makes, discounted by what crossing costs. Only
        // the part that balances is counted, since the imbalance is the net trade above.
        let spend: Vec<f64> = self
            .countries
            .iter()
            .map(|c| c.towns.iter().map(|t| self.towns[*t].wares_bought).sum())
            .collect();
        let made: Vec<f64> = self
            .countries
            .iter()
            .map(|c| {
                c.towns
                    .iter()
                    .map(|t| self.towns[*t].output.wares * self.towns[*t].price)
                    .sum()
            })
            .collect();
        let k = self.countries.len();
        let mut bought = vec![vec![0.0; k]; k];
        for c in 0..k {
            let reach: Vec<f64> = (0..k)
                .map(|d| {
                    let wedge = if c == d {
                        0.0
                    } else {
                        2.0 * HANDLING
                            + self.carriage(self.countries[c].capital, self.countries[d].capital)
                            + self.countries[c].pay_cost
                    };
                    made[d] * (1.0 + wedge).powf(1.0 - VARIETY)
                })
                .collect();
            let total: f64 = reach.iter().sum();
            if total <= 0.0 {
                continue;
            }
            for d in 0..k {
                if d != c {
                    bought[c][d] = spend[c] * reach[d] / total;
                }
            }
        }
        let mut varieties = vec![0.0; k];
        for c in 0..k {
            for d in (c + 1)..k {
                let both_ways = 2.0 * bought[c][d].min(bought[d][c]);
                if both_ways <= 0.0 {
                    continue;
                }
                let (a, b) = (self.countries[c].key, self.countries[d].key);
                *payments.entry((a.min(b), a.max(b))).or_insert(0.0) += both_ways;
                varieties[c] += both_ways / 2.0;
                varieties[d] += both_ways / 2.0;
            }
        }
        for (country, sold) in self.countries.iter_mut().zip(&varieties) {
            country.exports += sold;
        }
        // Familiarity grows with the share of each country's business done with the other.
        let keys: Vec<usize> = self.countries.iter().map(|c| c.key).collect();
        let business: BTreeMap<usize, f64> = keys
            .iter()
            .map(|k| {
                let mine: f64 = payments
                    .iter()
                    .filter(|((a, b), _)| a == k || b == k)
                    .map(|(_, v)| v)
                    .sum();
                (*k, mine)
            })
            .collect();
        for (i, a) in keys.iter().enumerate() {
            for b in keys.iter().skip(i + 1) {
                let pair = (*a.min(b), *a.max(b));
                let flow = payments.get(&pair).copied().unwrap_or(0.0);
                let share = flow / business[a].max(business[b]).max(1e-9);
                let known = self.familiarity.get(&pair).copied().unwrap_or(0.0);
                self.familiarity
                    .insert(pair, payments::familiarity_after(known, share));
            }
        }
        self.payments = payments;
        self.choose_reserve();

        // What paying abroad costs each country, for next year's border.
        let trust = self.country_trust();
        for c in 0..self.countries.len() {
            let mut weighted = 0.0;
            let mut volume = 0.0;
            for d in 0..self.countries.len() {
                if c == d {
                    continue;
                }
                let pair = (keys[c].min(keys[d]), keys[c].max(keys[d]));
                let flow = self.payments.get(&pair).copied().unwrap_or(0.0);
                let bank = payments::cheapest_route(&trust, c, d, self.fx(c, d)).0;
                let cost = self.settlement_cost(pair, bank);
                weighted += flow * cost;
                volume += flow;
            }
            self.countries[c].pay_cost = if self.borders_are_free {
                0.0
            } else if volume > 0.0 {
                weighted / volume
            } else if self.countries.len() > 1 {
                // Nothing paid abroad last year: what the first payment would cost.
                (0..self.countries.len())
                    .filter(|d| *d != c)
                    .map(|d| payments::cheapest_route(&trust, c, d, self.fx(c, d)).0)
                    .fold(f64::MAX, f64::min)
            } else {
                // Nobody abroad to pay.
                0.0
            };
        }
    }

    /// What a payment between a pair costs once any chain has taken its share of them.
    fn settlement_cost(&self, pair: (usize, usize), bank: f64) -> f64 {
        let mut cost = bank;
        for network in &self.networks {
            let share = network.share_of(pair);
            if share > 0.0 {
                cost = (1.0 - share) * cost + share * network.cost;
            }
        }
        cost
    }

    /// What changing money costs between two countries: nothing within one currency, an
    /// ordinary spread between two, and the same between two that have no currency at all and
    /// must weigh each other's metal.
    pub(crate) fn fx(&self, a: usize, b: usize) -> f64 {
        match (self.countries[a].currency, self.countries[b].currency) {
            (Some(x), Some(y)) if x == y => 0.0,
            _ => payments::FX_SPREAD,
        }
    }

    /// What paying abroad would cost with no chain at all — routed through whichever houses
    /// are cheapest — averaged over every ordered pair of countries. `None` for a world of one
    /// country, which has nobody abroad to pay.
    pub fn cost_through_houses(&self) -> Option<f64> {
        let n = self.countries.len();
        if n < 2 {
            return None;
        }
        let trust = self.country_trust();
        let mut total = 0.0;
        for a in 0..n {
            for b in (0..n).filter(|b| *b != a) {
                total += payments::cheapest_route(&trust, a, b, self.fx(a, b)).0;
            }
        }
        Some(total / (n * (n - 1)) as f64)
    }

    /// How far each country's houses trust each other's, by position in `countries`.
    pub fn country_trust(&self) -> Vec<Vec<f64>> {
        let n = self.countries.len();
        let mut trust = vec![vec![1.0; n]; n];
        for (a, row) in trust.iter_mut().enumerate() {
            for (b, cell) in row.iter_mut().enumerate() {
                if a == b {
                    continue;
                }
                let (ka, kb) = (self.countries[a].key, self.countries[b].key);
                let familiar = self
                    .familiarity
                    .get(&(ka.min(kb), ka.max(kb)))
                    .copied()
                    .unwrap_or(0.0);
                let same = self.cultures.of_place(self.countries[a].capital)
                    == self.cultures.of_place(self.countries[b].capital);
                let distance = self.distances[self.countries[a].capital][self.countries[b].capital];
                *cell = payments::trust(payments::trust_ceiling(same, distance), familiar);
            }
        }
        trust
    }

    /// The currency the world invoices in: whichever carries the most trade, weighted by how far
    /// the others trust its issuer — and it takes a clear lead to unseat one already in use,
    /// because everybody's contracts are written in it.
    fn choose_reserve(&mut self) {
        if self.currencies.is_empty() {
            return;
        }
        let trust = self.country_trust();
        let score = |id: usize| -> f64 {
            let Some(c) = self.countries.iter().position(|c| c.currency == Some(id)) else {
                return 0.0;
            };
            let trusted = (0..self.countries.len())
                .filter(|d| *d != c)
                .map(|d| trust[d][c])
                .sum::<f64>()
                / (self.countries.len().max(2) - 1) as f64;
            (self.countries[c].exports + 0.1 * self.countries[c].product) * (0.5 + trusted)
        };
        let best = (0..self.currencies.len())
            .max_by(|a, b| score(*a).total_cmp(&score(*b)).then(b.cmp(a)))
            .expect("there is a currency");
        let change = match self.reserve {
            None => true,
            Some(now) => now != best && score(best) > 1.25 * score(now),
        };
        if change && score(best) > 0.0 {
            self.reserve = Some(best);
            self.history.push(Event::Reserve {
                year: self.year,
                currency: best,
            });
        }
    }

    fn keep_ledgers(&mut self) {
        network::year(self);
    }

    /// A year of each people's ways — drift and contact, exactly as the people-level world runs
    /// it — which is what decides, next year, who is in which country.
    fn drift(&mut self) {
        let n = self.towns.len();
        let souls: Vec<u32> = self
            .towns
            .iter()
            .map(|t| t.people.round().clamp(0.0, u32::MAX as f64) as u32)
            .collect();
        let total: f64 = self.towns.iter().map(|t| t.people).sum();
        let doing: Vec<[f32; culture::WAYS]> = (0..n).map(|t| self.cultures.practised(t)).collect();
        let contact: Vec<f32> = (0..n)
            .map(|t| {
                let within: f64 = (0..n)
                    .filter(|o| *o != t && self.within_reach(t, *o))
                    .map(|o| self.towns[o].people)
                    .sum();
                let apart = (total - self.towns[t].people).max(1.0);
                self.towns[t].terrain.reach * (within / apart) as f32
            })
            .collect();
        let mut rng = self.stream(2);
        self.cultures.step(&doing, &contact, &souls, self.year, &mut rng);
    }

    fn read(&mut self) {
        let people = self.people();
        let product: f64 = self.towns.iter().map(|t| t.product()).sum();
        let weigh = |f: &dyn Fn(&Town) -> f64| -> f64 {
            if people <= 0.0 {
                return 0.0;
            }
            self.towns.iter().map(|t| t.people * f(t)).sum::<f64>() / people
        };
        let income = weigh(&|t| t.income);
        let workers: f64 = people * WORKING;
        let mut shares = [0.0; Sector::COUNT];
        let mut technique = [0.0; Sector::COUNT];
        for s in 0..Sector::COUNT {
            shares[s] = self
                .towns
                .iter()
                .map(|t| t.workers()[s])
                .sum::<f64>()
                / workers.max(1.0);
            technique[s] = weigh(&|t| t.technique[s]);
        }
        let abroad: f64 = self.payments.values().sum::<f64>() / 2.0;
        let on_chain: f64 = self
            .payments
            .iter()
            .map(|(pair, v)| v * self.networks.iter().map(|n| n.share_of(*pair)).sum::<f64>().min(1.0))
            .sum::<f64>()
            / 2.0;
        let recent: Vec<f64> = self
            .readings
            .iter()
            .rev()
            .take(SUSTAINED - 1)
            .map(|r| r.income)
            .chain(std::iter::once(income))
            .collect();
        let sustained = recent.len() >= SUSTAINED
            && recent.iter().sum::<f64>() / recent.len() as f64 > 2.0
            && self.year > 2 * SUSTAINED as u64;
        if !self.trap_opened && sustained {
            self.trap_opened = true;
            self.history.push(Event::TrapOpened { year: self.year });
        }
        self.readings.push(Reading {
            year: self.year,
            people,
            product,
            income,
            hunger: weigh(&|t| t.hunger),
            traded: if product > 0.0 { abroad / product } else { 0.0 },
            shares,
            technique,
            monetised: weigh(&|t| t.acceptance.monetised()),
            countries: self.countries.len(),
            states: self.states.len(),
            currencies: self.currencies.len(),
            on_chain: if abroad > 0.0 { on_chain / abroad } else { 0.0 },
            blocks: self.networks.iter().map(|n| n.chain.height()).sum(),
        });
    }
}

/// How many a town's ground feeds with everybody farming at bare technique.
fn ceiling(ground: &Endowment) -> f64 {
    // food(0.6 N) = N with food = yield · villages^0.35 · farmers^0.65.
    let per = ground.food_yield * WORKING.powf(0.65);
    per.max(0.0).powf(1.0 / 0.35) * ground.villages
}
