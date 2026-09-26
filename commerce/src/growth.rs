//! How many people there are, and what they work out.
//!
//! The Malthusian trap and the way out of it, in two curves.
//!
//! **Births answer income, and then stop answering it.** Near subsistence a little more food
//! means more children who live, so population rises until income falls back — the check §21
//! built for the people-level world, and the reason every agrarian society in history sat near
//! the edge of what its land could feed. But the response is a hump, not a line: past a few
//! times subsistence, families have fewer children rather than more. That is the demographic
//! transition, and it is observed everywhere income has risen far enough to see it.
//!
//! **Ideas come from people with time to think, and there are diminishing returns to how many
//! of them there are.** Somebody working something out is still the only thing that moves what
//! is possible (§29), but at the scale of nations it is thousands of somebodies a year, so what
//! is kept here is the rate: proportional to the number of people with slack, raised to a power
//! below one because many of them are working out the same thing.
//!
//! Put together they give the shape of the actual record. While ideas come slower than a
//! population can grow into them, every gain in technique is eaten by the children it feeds and
//! income stays at subsistence however clever the world gets. Once ideas come faster than
//! births can catch up — because there are more people, and because the richer have time to
//! think — income starts to rise, the rise slows births, and the slower births let income rise
//! faster. Nobody has to decide when that happens.

/// The fastest a population grows, net, in a year: about what the fastest-growing countries
/// of the twentieth century managed at the peak of their transitions.
const MOST_GROWTH: f64 = 0.02;

/// How far above subsistence income has to be for growth to be fastest, in multiples of
/// subsistence. Past two and a half times what it takes to eat, families start to have fewer
/// children rather than more.
const PEAK_ABOVE: f64 = 1.5;

/// Net growth in a year, from income per head in years of food.
///
/// Zero at subsistence, rising to `MOST_GROWTH` at `1 + PEAK_ABOVE`, and falling away towards
/// zero beyond it. Famine is separate — see `famine` — because going short kills people in a
/// way that being poor does not.
pub fn natural_increase(income: f64) -> f64 {
    if income <= 1.0 {
        return 0.0;
    }
    let x = (income - 1.0) / PEAK_ABOVE;
    MOST_GROWTH * x * (1.0 - x).exp()
}

/// The share of a town that dies in a year of going short by `hunger`, as a share of what it
/// needs to eat.
///
/// A quarter of the shortfall: a town a fifth short loses one in twenty. Famines kill through
/// disease among the weakened far more than through starvation outright, which is why the toll
/// is a fraction of the shortfall rather than all of it.
pub fn famine(hunger: f64) -> f64 {
    0.25 * hunger.clamp(0.0, 1.0)
}

/// How much of somebody's time is their own: the share of income above what it takes to eat.
pub fn slack(income: f64) -> f64 {
    if income <= 1.0 {
        0.0
    } else {
        1.0 - 1.0 / income
    }
}

/// The share of any population with time to think whatever the harvest: priests, lords,
/// merchants, the idle rich.
///
/// §48.7 found that the people-level world stopped inventing because the gate read a place's
/// *average* slack, and a crowded place's average is zero even where some people in it are
/// plainly comfortable. At this scale the same error is available and this is the repair: a
/// fiftieth of everybody always has an evening spare.
const LEISURED: f64 = 0.02;

/// People working in a trade who are also, in effect, thinking about it.
pub fn thinkers(workers: f64, income: f64) -> f64 {
    workers.max(0.0) * (LEISURED + (1.0 - LEISURED) * slack(income))
}

/// How the rate of discovery scales with the number thinking. Well below one, because two
/// people working out the same thing in two towns is one idea, not two — and at the scale of
/// nations most of what is being worked out is being worked out several times over.
const RETURNS: f64 = 0.75;

/// How much harder each idea is to find than the last, as the exponent on how far a trade has
/// already come.
///
/// The "ideas are getting harder to find" result, in its simplest form: the frontier's own
/// height divides the rate, by its square root, so a trade ten times as advanced as bare
/// technique needs about three times the thinkers to move as fast. Without it growth runs away
/// with population and a world of billions invents at double digits a year.
const HARDER: f64 = 0.5;

/// The rate at which thinkers move a trade's frontier.
///
/// Set so that an agrarian world of a hundred million, nearly all at subsistence, improves at
/// about a twentieth of a per cent a year — the long pre-modern rate — and so that the same
/// arithmetic, fed a world of billions with time to think and ten times the technique, gives
/// about two per cent. Everything between is the curve, not a choice.
const IDEAS: f64 = 1.1e-8;

/// How much of what is worked out in one trade helps the others.
const SPILLOVER: f64 = 0.2;

/// How far each trade's frontier moves in a year, as a proportion, given how many are thinking
/// about each and how far each has come.
pub fn discoveries(thinking: &[f64; 4], technique: &[f64; 4]) -> [f64; 4] {
    let total: f64 = thinking.iter().sum();
    let mut out = [0.0; 4];
    for (s, own) in thinking.iter().enumerate() {
        let effective = own + SPILLOVER * (total - own);
        out[s] = IDEAS * effective.max(0.0).powf(RETURNS) / technique[s].max(1.0).powf(HARDER);
    }
    out
}

/// How fast a country closes the gap to a frontier somebody it trades with holds, per year, at
/// full contact: two per cent, the "iron law" of convergence that shows up in almost every
/// cross-country sample. Technique travels by contact (§29.5.1), and contact is not only
/// goods — it is merchants, letters and people moving — so any trade at all brings a quarter of
/// it, and trade worth a twentieth of a country's product brings all of it.
const CATCHING_UP: f64 = 0.02;

/// A country's technique in a trade after a year of contact with the best it trades with.
pub fn catch_up(own: f64, best_in_contact: f64, openness: f64) -> f64 {
    if best_in_contact <= own {
        return own;
    }
    let contact = (0.25 + 5.0 * openness.max(0.0)).min(1.0);
    own + CATCHING_UP * contact * (best_in_contact - own)
}

/// How much of a town moves to a richer town in the same country in a year, per unit of the
/// gap in income between them. Slow: people follow what a place has been like for a
/// generation, not what last year's harvest did (§30.5).
pub const DRIFTING_TO_WORK: f64 = 0.02;

/// Who moves where inside one country: every town's population nudged towards the richer
/// ones, with the country's total unchanged.
pub fn resettle(people: &mut [f64], income: &[f64]) {
    let total: f64 = people.iter().sum();
    if total <= 0.0 || people.len() < 2 {
        return;
    }
    let mean = people
        .iter()
        .zip(income)
        .map(|(p, y)| p * y)
        .sum::<f64>()
        / total;
    if mean <= 0.0 {
        return;
    }
    for (p, y) in people.iter_mut().zip(income) {
        *p *= (1.0 + DRIFTING_TO_WORK * (y / mean - 1.0)).clamp(0.9, 1.1);
    }
    let after: f64 = people.iter().sum();
    for p in people.iter_mut() {
        *p *= total / after;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The hump: nothing at subsistence, most at two and a half times it, and nearly nothing
    /// again for the rich.
    #[test]
    fn births_answer_income_and_then_stop_answering_it() {
        assert_eq!(natural_increase(1.0), 0.0);
        let peak = natural_increase(2.5);
        assert!((peak - MOST_GROWTH).abs() < 1e-12);
        assert!(natural_increase(1.2) < peak);
        assert!(natural_increase(5.0) < peak);
        assert!(natural_increase(20.0) < 0.001);
    }

    #[test]
    fn going_short_kills_and_being_poor_does_not() {
        assert_eq!(famine(0.0), 0.0);
        assert!(famine(0.2) > 0.04);
        assert_eq!(natural_increase(0.8), 0.0);
    }

    /// The trap, as arithmetic: an agrarian world near subsistence improves at a few
    /// hundredths of a per cent a year, and a rich crowded one at over a per cent.
    #[test]
    fn a_rich_crowded_world_works_things_out_faster_than_a_poor_one() {
        let poor_workers = 60e6 / 4.0;
        let rich_workers = 3e9 / 4.0;
        let poor = discoveries(&[thinkers(poor_workers, 1.05); 4], &[1.0; 4]);
        let rich = discoveries(&[thinkers(rich_workers, 6.0); 4], &[10.0; 4]);
        assert!(poor[0] < 0.001, "poor world {:.5}", poor[0]);
        assert!(rich[0] > 0.008 && rich[0] < 0.05, "rich world {:.5}", rich[0]);
        // And harder as it goes: the same thinkers move a frontier twice as far on less.
        let early = discoveries(&[thinkers(rich_workers, 6.0); 4], &[1.0; 4]);
        assert!(early[0] > rich[0]);
    }

    #[test]
    fn nobody_moves_where_everybody_is_as_well_off() {
        let mut people = [100.0, 200.0, 300.0];
        resettle(&mut people, &[2.0, 2.0, 2.0]);
        assert!((people[0] - 100.0).abs() < 1e-9);
        let mut drifting = [100.0, 100.0];
        resettle(&mut drifting, &[1.0, 3.0]);
        assert!(drifting[1] > drifting[0]);
        assert!((drifting.iter().sum::<f64>() - 200.0).abs() < 1e-9, "moving is not dying");
    }

    #[test]
    fn a_people_catches_up_faster_the_more_it_trades() {
        assert!(catch_up(2.0, 5.0, 0.0) > 2.0, "any contact teaches something");
        assert!(catch_up(2.0, 5.0, 1.0) > catch_up(2.0, 5.0, 0.0), "and more contact more");
        assert_eq!(catch_up(5.0, 2.0, 1.0), 5.0, "nobody unlearns by trading");
    }
}
