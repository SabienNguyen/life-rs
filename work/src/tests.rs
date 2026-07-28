//! What a chain of goods has to get right.
//!
//! The claim under all of it is that **nobody is told to be anything**. Every trade in every
//! place exists because there was food enough to spare a hand for it and something worth
//! making. So the tests that matter are the ones about the order of the chain: land before
//! stock, stock before tools, food before anybody who is not growing it.

use super::*;

fn hands(farmers: f32, hewers: f32, smiths: f32, cooks: f32, keepers: f32) -> Hands {
    let mut hands = Hands::default();
    hands.set(Trade::Farmer, farmers);
    hands.set(Trade::Hewer, hewers);
    hands.set(Trade::Smith, smiths);
    hands.set(Trade::Cook, cooks);
    hands.set(Trade::Keeper, keepers);
    hands
}

/// A hand at the bottom of the chain on ordinary land gets about this in a year.
const ORDINARY: f32 = 1.4;

#[test]
fn a_place_where_everybody_farms_is_the_old_one_good_world() {
    // The compatibility that protects every number §21 and §22 calibrated: a world that
    // never specialises has to be the world that existed before there were trades.
    let (made, _) = make(&Hands::all_farming(30.0), Ground::even(ORDINARY), &Holdings::default());
    assert!((made.of(Good::Food) - 30.0 * ORDINARY).abs() < 1e-4);
    for good in [Good::Stock, Good::Tools, Good::Meals, Good::Upkeep] {
        assert_eq!(made.of(good), 0.0, "{good:?} appeared with nobody making it");
    }
}

#[test]
fn a_smith_with_no_stock_makes_no_tools() {
    // The whole of the supply chain in one test. You cannot fill a link before the link
    // below it, however badly the place wants what the link makes.
    let (made, after) = make(&hands(20.0, 0.0, 5.0, 0.0, 0.0), Ground::even(ORDINARY), &Holdings::default());
    assert_eq!(made.of(Good::Tools), 0.0, "tools out of nothing");
    assert_eq!(all_tools(&after), 0.0);

    // Put hewers in and the same smiths produce.
    let (made, after) = make(&hands(20.0, 5.0, 5.0, 0.0, 0.0), Ground::even(ORDINARY), &Holdings::default());
    assert!(made.of(Good::Tools) > 0.0, "hewers and smiths and still no tools");
    assert!(all_tools(&after) > 0.0, "nothing was left standing at the end of the year");
}

#[test]
fn specialising_costs_food_and_that_is_the_point() {
    // Every trade but farming is a claim on somebody else's surplus, and it has to show up
    // as one — otherwise specialisation is free and every place does all of it.
    let all_farming = make(&Hands::all_farming(30.0), Ground::even(ORDINARY), &Holdings::default()).0;
    let mixed = make(&hands(20.0, 5.0, 5.0, 0.0, 0.0), Ground::even(ORDINARY), &Holdings::default()).0;
    assert!(
        mixed.of(Good::Food) < all_farming.of(Good::Food),
        "ten people stopped farming and the harvest did not fall"
    );
}

#[test]
fn tools_are_what_lets_it_pay_for_itself() {
    // And the reason a chain is worth having: the hands that came off the land come back as
    // a multiplier on the hands that stayed.
    let equipped = Holdings::equipping(36.0, &Hands::all_farming(30.0));
    let bare = make(&Hands::all_farming(30.0), Ground::even(ORDINARY), &Holdings::default()).0;
    let kitted = make(&Hands::all_farming(30.0), Ground::even(ORDINARY), &equipped).0;
    assert!(
        kitted.of(Good::Food) > bare.of(Good::Food) * 1.3,
        "a place with tools grew {:.1} against {:.1} with none",
        kitted.of(Good::Food),
        bare.of(Good::Food)
    );

    // A specialised place with the tools its smiths made beats one that never spared them.
    let specialised = make(&hands(24.0, 3.0, 3.0, 0.0, 0.0), Ground::even(ORDINARY), &equipped).0;
    assert!(
        specialised.of(Good::Food) > bare.of(Good::Food),
        "the chain never paid for itself: {:.1} against {:.1}",
        specialised.of(Good::Food),
        bare.of(Good::Food)
    );
}

#[test]
fn capital_runs_down_without_anybody_keeping_it() {
    // The other half of what makes tools capital rather than a number that only grows.
    let mut holdings = Holdings::equipping(30.0, &Hands::all_farming(30.0));
    for _ in 0..10 {
        holdings = make(&Hands::all_farming(30.0), Ground::even(ORDINARY), &holdings).1;
    }
    assert!(all_tools(&holdings) < 12.0, "tools kept themselves: {}", all_tools(&holdings));

    // And a keeper holds it very nearly level.
    let mut kept = Holdings::equipping(30.0, &hands(28.0, 0.0, 0.0, 0.0, 2.0));
    for _ in 0..10 {
        kept = make(&hands(28.0, 0.0, 0.0, 0.0, 2.0), Ground::even(ORDINARY), &kept).1;
    }
    assert!(
        all_tools(&kept) > 24.0,
        "two keepers could not hold thirty tools together: {}",
        all_tools(&kept)
    );
}

#[test]
fn a_hungry_place_has_nothing_to_gain_from_anything_but_farming() {
    // Subsistence first, and not as a rule anybody wrote: a starving place values a tool at
    // nothing because a tool is next year's problem, so every trade above the land is worth
    // less than the one that feeds people.
    let hands_now = hands(24.0, 2.0, 2.0, 1.0, 1.0);
    let (made, holdings) = make(&hands_now, Ground::even(0.2), &Holdings::default());
    let worth = worth_taking_up(&made, &holdings, &hands_now, Ground::even(0.2));
    let best = Trade::ALL
        .into_iter()
        .max_by(|a, b| worth[*a as usize].total_cmp(&worth[*b as usize]))
        .unwrap();
    assert_eq!(best, Trade::Farmer, "worth: {worth:?}");
    assert!(hunger(&made, hands_now.total()) > 0.5);
}

#[test]
fn a_want_falls_as_it_is_met() {
    // The property the first version of this did not have, and the one that keeps a trade
    // from swallowing a town. Each cook added has to be worth less than the one before.
    let mut previous = f32::MAX;
    for cooks in [0.0, 2.0, 5.0, 10.0, 20.0] {
        let hands_now = hands(40.0 - cooks, 0.0, 0.0, cooks, 0.0);
        let (made, holdings) = make(&hands_now, Ground::even(3.0), &Holdings::default());
        let worth = worth_taking_up(&made, &holdings, &hands_now, Ground::even(3.0));
        let cooking = worth[Trade::Cook as usize] - worth[Trade::Farmer as usize];
        // Non-increasing rather than strictly falling: past the point where every mouth in
        // the place is already served, another cook adds exactly nothing, and nothing is a
        // perfectly good flat line.
        assert!(
            cooking <= previous + 1e-3,
            "the {cooks}th cook was worth more than the one before: {cooking} against {previous}"
        );
        previous = cooking;
    }
    assert!(previous < 0.0, "a town of half cooks still wanted cooks");
}

#[test]
fn nobody_takes_up_a_trade_whose_input_nobody_makes() {
    // What a person actually looks at when choosing. A place with no hewers should not make
    // smithing look worthwhile however badly it wants tools.
    let hands_now = Hands::all_farming(30.0);
    let (made, holdings) = make(&hands_now, Ground::even(3.0), &Holdings::default());
    let worth = worth_taking_up(&made, &holdings, &hands_now, Ground::even(3.0));
    assert_eq!(
        worth[Trade::Smith as usize],
        0.0,
        "smithing looked worth doing in a place with no stock"
    );
    assert!(
        worth[Trade::Hewer as usize] > 0.0,
        "and hewing, which needs nothing, did not"
    );
}

#[test]
fn a_starving_place_makes_farming_the_only_thing_worth_doing() {
    let hands_now = hands(20.0, 4.0, 3.0, 2.0, 1.0);
    let (made, holdings) = make(&hands_now, Ground::even(0.6), &Holdings::default());
    let worth = worth_taking_up(&made, &holdings, &hands_now, Ground::even(0.6));
    let best = Trade::ALL
        .into_iter()
        .max_by(|a, b| worth[*a as usize].total_cmp(&worth[*b as usize]))
        .unwrap();
    assert_eq!(best, Trade::Farmer, "worth: {worth:?}");
}

#[test]
fn every_link_of_the_chain_can_be_reached_from_the_bottom() {
    // Left to itself, a place with food to spare should walk up the chain: hewers first
    // because they need nothing, then smiths because there is stock, then the rest. Nobody
    // is told the order; it falls out of what each trade needs.
    let mut hands_now = Hands::all_farming(40.0);
    let mut holdings = Holdings::default();
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..60 {
        let (made, after) = make(&hands_now, Ground::even(3.0), &holdings);
        holdings = after;
        let worth = worth_taking_up(&made, &holdings, &hands_now, Ground::even(3.0));
        let best = Trade::ALL
            .into_iter()
            .max_by(|a, b| worth[*a as usize].total_cmp(&worth[*b as usize]))
            .unwrap();
        seen.insert(best);
        // One person changes trade a year, out of the largest trade.
        let from = Trade::ALL
            .into_iter()
            .max_by(|a, b| hands_now.at(*a).total_cmp(&hands_now.at(*b)))
            .unwrap();
        if from != best && hands_now.at(from) > 1.0 {
            hands_now.set(from, hands_now.at(from) - 1.0);
            hands_now.set(best, hands_now.at(best) + 1.0);
        }
    }
    assert!(
        seen.contains(&Trade::Hewer) && seen.contains(&Trade::Smith),
        "a comfortable place never got past farming: {seen:?}"
    );
    assert!(
        all_tools(&holdings) > 1.0,
        "sixty years and the place owns nothing: {}",
        all_tools(&holdings)
    );
}

#[test]
fn the_marginal_comparison_is_the_switching_question() {
    // `worth_taking_up` values each trade by running the year with one more hand in it, and
    // somebody deciding whether to leave A for B compares those two numbers. What they
    // actually face is different on its face — a year with *their* hands moved from A to B,
    // one fewer at the old bench rather than one more everywhere.
    //
    // Those are the same question to first order, since both differences come out as the
    // marginal value of B less the marginal value of A, but "to first order" is an argument
    // and this is a measurement. It matters because §30.5.1 claimed the difference was a
    // bug, on exactly that argument, without checking. It is not one, and the cobweb has a
    // single cause rather than two.
    let ground = Ground::even(1.0);
    let shapes = [
        hands(40.0, 8.0, 8.0, 6.0, 4.0),
        hands(20.0, 20.0, 20.0, 20.0, 20.0),
        hands(70.0, 2.0, 2.0, 2.0, 2.0),
        hands(10.0, 30.0, 30.0, 10.0, 10.0),
    ];

    let (mut compared, mut disagreed) = (0, 0);
    let mut worst: f32 = 0.0;
    for shape in shapes {
        // Equipped for the shape it is in, since that is what a place at rest looks like.
        let holdings = Holdings {
            stock: 40.0,
            ..Holdings::equipping(30.0, &shape)
        };
        let workers = shape.total();
        let (made, held) = make(&shape, ground, &holdings);
        let base = value_of(&made, &held, workers, ground, 0.5);
        let marginal: Vec<f32> = Trade::ALL
            .into_iter()
            .map(|t| {
                let mut more = shape;
                more.set(t, more.at(t) + 1.0);
                let (m, h) = make(&more, ground, &holdings);
                value_of(&m, &h, workers + 1.0, ground, 0.5) - base
            })
            .collect();

        for from in Trade::ALL {
            if shape.at(from) < 1.0 {
                continue;
            }
            for to in Trade::ALL.into_iter().filter(|t| *t != from) {
                let mut moved = shape;
                moved.set(from, moved.at(from) - 1.0);
                moved.set(to, moved.at(to) + 1.0);
                let (m, h) = make(&moved, ground, &holdings);
                let truth = value_of(&m, &h, workers, ground, 0.5) - base;
                let proxy = marginal[to as usize] - marginal[from as usize];
                worst = worst.max((truth - proxy).abs());
                compared += 1;
                if (truth > 0.0) != (proxy > 0.0) {
                    disagreed += 1;
                }
            }
        }
    }
    assert!(compared > 50, "not enough cases to say anything: {compared}");
    assert!(
        disagreed == 0,
        "{disagreed} of {compared} switches disagree on whether they are worth making, \
worst gap {worst:.4}"
    );
}

#[test]
fn a_plough_is_no_use_to_a_quarryman() {
    // §27.9's last bullet: tools used to be one number, so a place that had spent a century
    // farming was, on the day it turned to hewing, exactly as well equipped for hewing as it
    // had been for farming. Capital that transfers perfectly between trades is not capital.
    let ground = Ground::even(ORDINARY);
    let farming = Hands::all_farming(30.0);
    let equipped = Holdings::equipping(36.0, &farming);

    // A village of farmers with ploughs does well, which is the old claim and still holds.
    let fed = make(&farming, ground, &equipped).0.of(Good::Food);
    let barehanded = make(&farming, ground, &Holdings::default()).0.of(Good::Food);
    assert!(fed > barehanded * 1.3, "ploughs should pay: {fed:.1} against {barehanded:.1}");

    // Turn the same village to quarrying overnight and the ploughs are worth nothing to
    // them. They own thirty-six tools and none of them are the right ones.
    let quarrying = hands(0.0, 30.0, 0.0, 0.0, 0.0);
    let with_ploughs = make(&quarrying, ground, &equipped).0.of(Good::Stock);
    let with_nothing = make(&quarrying, ground, &Holdings::default()).0.of(Good::Stock);
    assert!(
        (with_ploughs - with_nothing).abs() < 1e-4,
        "a farm's tools should be no help at the quarry face: {with_ploughs:.2} against \
{with_nothing:.2}"
    );

    // And a place equipped *for* quarrying does as well at it as the farmers did at farming,
    // so this is stickiness rather than a penalty on hewing.
    let picks = Holdings::equipping(36.0, &quarrying);
    let with_picks = make(&quarrying, ground, &picks).0.of(Good::Stock);
    assert!(
        with_picks > with_nothing * 1.3,
        "picks should pay a quarry what ploughs pay a farm: {with_picks:.2} against \
{with_nothing:.2}"
    );
}

#[test]
fn at_rest_it_is_the_pool_it_replaced() {
    // The other half of the claim, and the one that makes this safe to land: a place that
    // has been doing the same thing for years is equipped in proportion to its hands, and
    // then tools-per-hand is the same number in every trade — which is exactly what one
    // pooled figure meant. The change bites only on places that *change*.
    let ground = Ground::even(ORDINARY);
    let mixed = hands(20.0, 10.0, 4.0, 4.0, 2.0);
    let mut holdings = Holdings::equipping(20.0, &mixed);
    for _ in 0..40 {
        holdings = make(&mixed, ground, &holdings).1;
    }
    let per_farmer = holdings.tools[Trade::Farmer as usize] / mixed.at(Trade::Farmer);
    let per_hewer = holdings.tools[Trade::Hewer as usize] / mixed.at(Trade::Hewer);
    assert!(
        (per_farmer - per_hewer).abs() < 0.05 * per_farmer.max(1e-6),
        "a settled place should be evenly equipped: {per_farmer:.3} a farmer against \
{per_hewer:.3} a hewer"
    );
}
