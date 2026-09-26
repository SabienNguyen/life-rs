//! What the world at the scale of nations claims, checked as claims.

use std::sync::OnceLock;

use commerce::payments;
use sim_core::WorldSeed;

use crate::*;

/// A world that founds a chain, run until it does and thirty years beyond.
///
/// Shared, because getting there is five centuries of history — cheap at this scale, a second or
/// two, but not something every test should pay for again.
fn chained() -> &'static Nations {
    static WORLD: OnceLock<Nations> = OnceLock::new();
    WORLD.get_or_init(|| {
        let mut world = Nations::found(WorldSeed::from_u128(0x11));
        while world.networks.is_empty() && world.year < 800 {
            world.year();
        }
        assert!(!world.networks.is_empty(), "seed 0x11 founds a chain within eight centuries");
        world.run(30);
        world
    })
}

/// The first two centuries of a world, before anything much has happened.
fn early() -> &'static Nations {
    static WORLD: OnceLock<Nations> = OnceLock::new();
    WORLD.get_or_init(|| {
        let mut world = Nations::found(WorldSeed::from_u128(0x21));
        world.run(200);
        world
    })
}

#[test]
fn nations_stand_on_the_planet_the_people_level_world_founds() {
    let seed = WorldSeed::from_u128(0x21);
    let nations = early();
    let surface = sim::Surface::genesis(seed);
    let grid = surface.planet.grid();
    assert_eq!(nations.surface.planet.grid().len(), grid.len());
    for cell in grid.cells().step_by(17) {
        assert_eq!(
            nations.surface.planet.height_above_sea_m(cell),
            surface.planet.height_above_sea_m(cell),
            "cell {cell}"
        );
    }
    // And every town stands on dry, habitable ground, one to a cell, none neighbouring another.
    for (a, town) in nations.towns.iter().enumerate() {
        assert!(nations.surface.planet.is_land(town.cell));
        for (b, other) in nations.towns.iter().enumerate() {
            if a != b {
                assert!(!grid.neighbours(town.cell).contains(&other.cell));
            }
        }
    }
}

#[test]
fn the_same_seed_is_the_same_world() {
    let run = || {
        let mut world = Nations::found(WorldSeed::from_u128(0x5eed));
        world.run(80);
        world.readings.clone()
    };
    assert_eq!(run(), run());
}

#[test]
fn every_town_is_in_one_state_and_every_state_is_in_one_country() {
    let world = early();
    let mut seen = vec![0; world.towns.len()];
    for (s, state) in world.states.iter().enumerate() {
        assert!(state.towns.contains(&state.hub), "a hub is in its own state");
        for t in &state.towns {
            seen[*t] += 1;
            assert_eq!(world.towns[*t].state, s);
            assert_eq!(world.towns[*t].country, state.country);
        }
        assert!(world.countries[state.country].states.contains(&s));
    }
    assert!(seen.iter().all(|n| *n == 1), "{seen:?}");
    // States are bigger than towns and smaller than countries — the level is not empty.
    assert!(world.states.len() < world.towns.len());
    assert!(world.states.len() > world.countries.len());
}

/// Money comes out of trade being thick, not out of anybody being rich: the first town to
/// trade mostly in a medium does so centuries before the world's income leaves subsistence.
#[test]
fn money_is_used_long_before_anybody_is_rich() {
    let world = chained();
    let first_money = world
        .history
        .iter()
        .find_map(|e| match e {
            Event::Monetised { year, .. } => Some(*year),
            _ => None,
        })
        .expect("money appears");
    let trap = world
        .history
        .iter()
        .find_map(|e| match e {
            Event::TrapOpened { year } => Some(*year),
            _ => None,
        })
        .expect("the trap opens");
    assert!(first_money + 100 < trap, "money in {first_money}, growth in {trap}");
    assert!(world.currencies.iter().all(|c| c.minted < trap));
}

/// The trap: for its first centuries a world's income stays near what it takes to eat however
/// much it learns, because every gain is eaten by the children it feeds — and then it opens.
#[test]
fn the_trap_holds_and_then_opens() {
    let world = chained();
    let early: Vec<&Reading> = world.readings.iter().filter(|r| (20..150).contains(&r.year)).collect();
    let mean = early.iter().map(|r| r.income).sum::<f64>() / early.len() as f64;
    assert!(mean < 1.6, "income in the first centuries averaged {mean:.2}");
    let people_then = early.first().unwrap().people;
    let people_after = early.last().unwrap().people;
    assert!(people_after > people_then, "the gains went into people");
    let last = world.readings.last().unwrap();
    assert!(last.income > 5.0, "and the trap opened: {:.2}", last.income);
}

/// Structural change: as the world gets richer, fewer of its people farm.
#[test]
fn the_richer_the_world_the_fewer_its_farmers() {
    let world = chained();
    let first = &world.readings[10];
    let last = world.readings.last().unwrap();
    assert!(first.shares[Sector::Farming as usize] > 0.7);
    assert!(last.shares[Sector::Farming as usize] < 0.3);
    assert!(last.shares[Sector::Serving as usize] > first.shares[Sector::Serving as usize]);
}

/// The demographic transition: once people are rich, population stops growing as fast as it
/// did on the way.
#[test]
fn births_slow_once_people_are_rich() {
    let world = chained();
    let growth_over = |from: &Reading, years: u64| {
        let later = world
            .readings
            .iter()
            .find(|r| r.year == from.year + years)
            .unwrap_or(world.readings.last().unwrap());
        (later.people / from.people).powf(1.0 / (later.year - from.year).max(1) as f64) - 1.0
    };
    let fastest = world
        .readings
        .iter()
        .filter(|r| r.year + 30 <= world.year)
        .map(|r| growth_over(r, 30))
        .fold(0.0f64, f64::max);
    let rich = world
        .readings
        .iter()
        .find(|r| r.income > 15.0)
        .expect("the world gets rich");
    let then = growth_over(rich, 20);
    assert!(then < fastest / 2.0, "grew {then:.4} a year once rich against {fastest:.4} at most");
}

/// Markets end famines. The same world with nothing able to move between towns goes hungry far
/// more often, because a bad harvest can no longer be met from somebody else's good one.
#[test]
fn markets_end_famines() {
    let famines = |trade: bool| {
        let mut world = Nations::found(WorldSeed::from_u128(0xbeef));
        world.trade_is_possible = trade;
        world.run(150);
        world
            .history
            .iter()
            .filter(|e| matches!(e, Event::Famine { .. }))
            .count()
    };
    let (with, without) = (famines(true), famines(false));
    assert!(without > 2 * with.max(1), "{without} famines without trade, {with} with");
}

/// A chain is what parties who do not trust each other build: its founders span countries, and
/// none of them is a keeper the rest would simply have used.
#[test]
fn a_chain_is_founded_by_parties_who_distrust_each_other() {
    let world = chained();
    let network = &world.networks[0];
    assert!(network.founders.len() >= payments::FEWEST_FOUNDERS);
    let countries: std::collections::BTreeSet<usize> = network
        .founders
        .iter()
        .map(|t| world.towns[*t].country)
        .collect();
    assert!(countries.len() >= 2, "founders from {} countries", countries.len());
    assert!(
        world.history.iter().any(|e| matches!(e, Event::Founded { .. })),
        "the founding is part of the world's history"
    );
}

/// Everything the world has done on its chain can be checked by somebody holding nothing but
/// the genesis: every block, signature and root, from the first.
#[test]
fn the_chain_a_world_keeps_can_be_checked_from_its_genesis() {
    let network = &chained().networks[0];
    assert!(network.chain.height() >= 12 * 30, "thirty years of monthly blocks");
    assert_eq!(network.chain.verify(), Ok(()));
    assert_eq!(network.chain.ledger.broken_law(), None);
}

/// The stable token is never more than its reserve — the attestor states it and the chain
/// refuses a mint past it — and the world's issuer never lets it fall short.
#[test]
fn the_stable_token_is_backed_one_for_one() {
    let network = &chained().networks[0];
    let token = network.token.as_ref().expect("a token was issued");
    let on_chain = network.chain.token(token.id).expect("and is on the ledger");
    assert!(on_chain.supply > 0, "and used");
    assert!(on_chain.reserves >= on_chain.supply);
    assert!((on_chain.backing() - 1.0).abs() < 1e-9);
    assert!(on_chain.minted > on_chain.supply, "tokens are redeemed as well as minted");
}

/// What a chain is for: paying abroad costs less once there is one.
#[test]
fn a_chain_makes_paying_abroad_cheaper() {
    let world = chained();
    let founded = world.networks[0].founded;
    assert!(world.networks[0].carried > 0.0, "the chain carries payments");
    let now: f64 = world.countries.iter().map(|c| c.pay_cost).sum::<f64>() / world.countries.len() as f64;
    // What routing through houses would cost, with no chain at all, between the same countries.
    let bank = world.cost_through_houses().expect("more than one country");
    assert!(
        now < 0.8 * bank,
        "{now:.3} with the chain against {bank:.3} through houses, founded {founded}"
    );
}

/// A world that is one country has a house everybody in it can pay through, and never builds a
/// ledger nobody keeps.
#[test]
fn a_world_that_is_one_country_keeps_its_books_in_one_house() {
    let mut world = Nations::found(WorldSeed::from_u128(0x2b));
    world.run(600);
    assert_eq!(world.countries.len(), 1);
    assert!(world.networks.is_empty());
    assert_eq!(world.not_yet, Some(payments::NotYet::OneCountry));
}
