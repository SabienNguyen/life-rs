//! Goods that are made out of other goods, and the trades that make them.
//!
//! Until now this world had **one good**. Everybody did the same undifferentiated work,
//! everybody produced the same undifferentiated output, and the only thing that
//! distinguished two workers was how much of it they made. That is not an economy. An
//! economy is people doing different things *because other people are doing the other
//! things*, and the whole of it turns on one fact: you cannot make tools until somebody
//! else is growing enough food to feed you while you make them.
//!
//! ## The chain
//!
//! ```text
//!   land + hands              → stock     (timber, stone, clay, ore)
//!   stock + hands             → tools     ── and tools multiply everything above
//!   land + hands  (× tools)   → food      ── what everybody must have
//!   food + hands              → meals     ── frees everybody else's time
//!   hands                     → upkeep    ── what keeps tools from wearing out
//! ```
//!
//! Four links deep, with a loop in it: tools make it easier to get the stock that tools are
//! made of, and easier to grow the food that feeds the people making them. That loop is why
//! an economy can *compound*, and it is the thing this model has never had.
//!
//! ## Why the goods are authored and the jobs are not
//!
//! What a thing is made of is a physical fact, like the seven `Deed`s or the five factors of
//! a temperament: those are the primitives the model is built out of. What is emphatically
//! **not** authored is who makes what, how many of each a place has, whether it has any at
//! all, what its people call them, or whether the chain gets past its first link. A hungry
//! village has no smiths, because it cannot spare the hands, and nothing anywhere says so —
//! it falls out of subsistence coming first.
//!
//! ## Subsistence first is what makes specialisation *earned*
//!
//! Every trade but farming is a claim on somebody else's surplus. So the return to a trade
//! is scaled by how badly the place wants what it makes, and a place short of food wants
//! nothing else at all. That single rule produces the whole historical shape: villages at
//! the edge are all farmers; a place with good land and a road grows smiths, then cooks;
//! and a bad year takes the cooks first.
//!
//! ## It reduces to what came before
//!
//! With everybody farming and no tools in the place, `food` is exactly the Cobb–Douglas
//! output of the one-good model. That is deliberate and it is what protects every number
//! §21 and §22 calibrated: a world that never specialises is bit-for-bit the world that
//! existed before this file.

/// What one person must have in a year, in the units everything here is measured in.
///
/// The unit is arbitrary and this fixes it: one is what one person eats. So an output of a
/// hundred with a hundred mouths is a place with no surplus, which is most of history.
pub const SUBSISTENCE: f32 = 1.0;

/// Something that can be made.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(usize)]
pub enum Good {
    /// What everybody must have, every year, before anything else is worth making.
    Food,
    /// Timber, stone, clay, ore — what tools are made out of.
    Stock,
    /// The first thing in this world that lasts longer than the year it was made in.
    Tools,
    /// Food somebody else prepared. Nobody needs it; everybody who has it has more day.
    Meals,
    /// Keeping what exists from falling apart. The only good that is pure service.
    Upkeep,
}

impl Good {
    pub const ALL: [Good; 5] = [
        Good::Food,
        Good::Stock,
        Good::Tools,
        Good::Meals,
        Good::Upkeep,
    ];
    pub const COUNT: usize = Good::ALL.len();

    pub const fn label(self) -> &'static str {
        match self {
            Good::Food => "food",
            Good::Stock => "stock",
            Good::Tools => "tools",
            Good::Meals => "meals",
            Good::Upkeep => "upkeep",
        }
    }

    /// What a year of one person's work in this trade needs of other goods.
    ///
    /// In the same unit everything else is in: what one person eats in a year. Zero for the
    /// goods that come straight off the land, which is what makes those the bottom of the
    /// chain and everything else dependent on them.
    pub const fn needs(self) -> Option<(Good, f32)> {
        match self {
            // Off the land. Nothing above them.
            Good::Food | Good::Stock => None,
            // A year of smithing consumes about its own weight in raw material.
            Good::Tools => Some((Good::Stock, STOCK_PER_TOOL)),
            // A cook does not create food, they change its form — so what they need is
            // food to work with, one year's worth for every mouth they serve. It binds
            // exactly when the place is short, which is when a cook is the wrong thing to
            // be.
            Good::Meals => Some((Good::Food, SUBSISTENCE)),
            // Pure labour. A hand and a broom.
            Good::Upkeep => None,
        }
    }
}

/// What somebody does with their working life.
///
/// One per good, because a trade is exactly "the people who make this". There is no trade
/// for doing nothing and none for doing everything: somebody who has not settled on one is
/// a farmer, which is what everybody was before there was anything else to be.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[repr(usize)]
pub enum Trade {
    /// Grows food. The default, the fallback, and for most of history everybody.
    #[default]
    Farmer,
    /// Cuts, quarries and digs what other trades work with.
    Hewer,
    /// Makes tools out of it.
    Smith,
    /// Feeds other people for a living.
    Cook,
    /// Mends, cleans and keeps. The trade that makes the others' work last.
    Keeper,
}

impl Trade {
    pub const ALL: [Trade; 5] = [
        Trade::Farmer,
        Trade::Hewer,
        Trade::Smith,
        Trade::Cook,
        Trade::Keeper,
    ];
    pub const COUNT: usize = Trade::ALL.len();

    pub const fn makes(self) -> Good {
        match self {
            Trade::Farmer => Good::Food,
            Trade::Hewer => Good::Stock,
            Trade::Smith => Good::Tools,
            Trade::Cook => Good::Meals,
            Trade::Keeper => Good::Upkeep,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Trade::Farmer => "farmer",
            Trade::Hewer => "hewer",
            Trade::Smith => "smith",
            Trade::Cook => "cook",
            Trade::Keeper => "keeper",
        }
    }

    /// What the position is, in a word, before any people has its own word for it.
    ///
    /// The same arrangement §26 uses for social positions: the meaning lives here and the
    /// sound lives in `culture::naming`, so that two peoples call the same trade two things.
    pub const fn stem(self) -> &'static str {
        match self {
            Trade::Farmer => "Tiller",
            Trade::Hewer => "Hewer",
            Trade::Smith => "Smith",
            Trade::Cook => "Cook",
            Trade::Keeper => "Warden",
        }
    }
}

/// What a hand at the bottom of the chain gets off *this* ground in a year, good by good.
///
/// Two numbers rather than one, and that is the whole of regional specialisation. The ground
/// under a place is not equally good at everything: a river plain grows a great deal and has
/// no stone in it, a wooded hillside is the reverse, and until now both were described by a
/// single figure called "what the land yields" — so every place in every world was good at
/// exactly the same things in exactly the same proportion, and geography could not produce a
/// division of labour between *places* however much it produced within one.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Ground {
    /// Per hand farming.
    pub food: f32,
    /// Per hand cutting, quarrying or digging.
    pub stock: f32,
    /// How much food a unit of unworked material fetches from the neighbours, 0 to 1.
    ///
    /// The other reason to cut timber, and the one that makes a road worth having. Without
    /// it the only thing material is good for is the tools *this* place can use, so a wooded
    /// hillside with a market a day's walk away hews exactly as much as one with no
    /// neighbours at all — and the whole point of two places being different is that they
    /// can live off each other.
    pub sells_for: f32,
}

impl Ground {
    /// Land equally good at both, which is what every place was before this.
    pub fn even(per_hand: f32) -> Ground {
        Ground {
            food: per_hand,
            stock: per_hand,
            sells_for: 0.0,
        }
    }
}

/// How many hands are in each trade in a place.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hands(pub [f32; Trade::COUNT]);

impl Hands {
    /// Everybody farming, which is what a place is until it can afford not to be.
    pub fn all_farming(workers: f32) -> Hands {
        let mut hands = Hands::default();
        hands.0[Trade::Farmer as usize] = workers.max(0.0);
        hands
    }

    pub fn at(&self, trade: Trade) -> f32 {
        self.0[trade as usize]
    }

    pub fn set(&mut self, trade: Trade, hands: f32) {
        self.0[trade as usize] = hands.max(0.0);
    }

    pub fn total(&self) -> f32 {
        self.0.iter().sum()
    }
}

/// What a place has that outlives the year.
///
/// One entry, and it is the whole of capital in this world. §22 said plainly that without
/// capital nothing here can compound — a rich place was rich because of its land and its
/// road, never because it was rich last century. This is the correction, and it is
/// deliberately the smallest possible one: a stock of tools, which is made by people, wears
/// out, is kept up by other people, and multiplies what everybody else's hands produce.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Holdings {
    /// Tools in hand, in years of one person's work to make — **per trade**.
    ///
    /// A plough is not a saw. They used to be one number, so a place that had spent a
    /// century farming was, on the day it turned to hewing, exactly as well equipped for
    /// hewing as it had been for farming. Capital that transfers perfectly between trades is
    /// not capital, it is a bonus attached to a place.
    ///
    /// Kept per trade, the stickiness is the mechanism: new tools are made for the trades
    /// people are actually working, so at rest each trade is equipped in proportion to its
    /// hands and this behaves exactly as one pooled number did. It bites only when a place
    /// *changes* what it does — and then a village that has all turned cook finds it owns
    /// ploughs, and has to spend a decade of smithing before cooking pays what it promised.
    ///
    /// That cost is also a brake on §30.5.1's cobweb, which is the reverse of what usually
    /// happens when something is added here: everybody moving into the trade that looks best
    /// this year now arrives to find the place has no tools for it.
    pub tools: [f32; Trade::COUNT],
    /// Timber, stone and ore cut but not yet worked.
    ///
    /// It has to keep, or the chain cannot start: a hewer in a place with no smith would be
    /// producing nothing anybody could use, so hewing would never be worth taking up, so no
    /// smith would ever have anything to work with. A pile of timber is worth something
    /// before anybody has made anything of it, and that is what lets the second link of the
    /// chain be filled before the third.
    pub stock: f32,
}

impl Holdings {
    /// A place equipped with `total` tools, spread over the hands that would use them.
    ///
    /// The same distribution `make` maintains, so this is what a place at rest looks like —
    /// and it is what lets a caller say "thirty tools" and mean what that meant when tools
    /// were one number.
    pub fn equipping(total: f32, hands: &Hands) -> Holdings {
        let wanted: f32 = USES_TOOLS.iter().map(|t| hands.at(*t)).sum();
        let mut tools = [0.0; Trade::COUNT];
        for trade in USES_TOOLS {
            tools[trade as usize] = if wanted > 0.0 {
                total * hands.at(trade) / wanted
            } else {
                total / USES_TOOLS.len() as f32
            };
        }
        Holdings { tools, stock: 0.0 }
    }
}

/// The trades a tool is any use to.
///
/// Farming and hewing: the two that take something out of the ground, which is where an edge
/// or a lever multiplies what a pair of hands can do. A cook's pot and a keeper's kit exist,
/// but nothing in this model has them multiplying anything, so making tools for them would
/// be making tools nobody uses.
pub const USES_TOOLS: [Trade; 2] = [Trade::Farmer, Trade::Hewer];

/// How much stock a year of one person's smithing turns into tools.
///
/// About its own weight in raw material — a year at a forge or a bench gets through roughly
/// what a year of cutting and quarrying brings in.
const STOCK_PER_TOOL: f32 = 0.8;

/// What a pile of unworked material is worth against the tools it will become.
///
/// Half. It has to be under one or the chain stalls at its second link — see `value_of` —
/// and it has to be over nothing or the second link never gets filled at all.
const WORTH_UNWORKED: f32 = 0.5;

/// How much of a place's tools are lost in a year with nobody keeping them.
///
/// A tenth. Pre-industrial tools are wood, leather, stone and a little metal, and they are
/// used every day; something like a decade is what one lasts without repair, and rather less
/// with hard use. This is what makes `Keeper` a trade rather than a courtesy: without upkeep
/// a place's capital runs down as fast as it can be built.
const WEAR: f32 = 0.10;

/// How much wear a year of one person's upkeep prevents, in the same units tools are in.
///
/// Set so that one keeper in twenty workers holds a well-equipped place exactly level —
/// twenty workers well equipped is `WELL_EQUIPPED × 20` tools, which wear at `WEAR` a year,
/// and one hand has to cover that. About a twentieth of a workforce on maintenance and
/// repair is the pre-industrial figure, and this is what makes `Keeper` a trade rather than
/// a courtesy: without it a place's capital runs down as fast as it can be built.
const MENDING: f32 = 2.4;

/// The most tools can multiply what hands produce.
///
/// The same shape and the same modesty as `TECHNIQUE_CEILING`: doubling, not more. A stone
/// sickle against bare hands is a large multiple; everything past a good set of hand tools
/// needs the power sources §23 puts out of scope. Technique is *knowing how*; this is
/// *having the thing*, and the two multiply, which is why a place that knows a great deal
/// and owns nothing is still poor.
const TOOL_LIFT: f32 = 1.0;

/// Tools per worker at which half of that lift is reached.
const WELL_EQUIPPED: f32 = 1.2;

/// How much of a person's working year goes on feeding themselves.
///
/// Grinding, fetching water, tending a fire. A sixth of the day is the low end of what the
/// ethnographic record gives for households doing it one at a time, and it is the whole
/// product of the cooking trade: a cook makes no food, they hand back the time.
const COOKING_SHARE: f32 = 0.15;

/// How many mouths one cook can serve.
///
/// A dozen. Doing for twelve households what each would otherwise do for itself is the
/// oldest economy of scale there is, and the number is what makes the trade possible at all:
/// at one mouth a cook saves exactly the time they spend and nobody would ever be one. It is
/// also why cooking is the first trade a place takes up once it is *dense* rather than
/// merely rich — twelve mouths have to be within reach of one kitchen.
const MOUTHS_PER_COOK: f32 = 12.0;

/// A saturating lift: `at` per worker gives half of `most`.
fn lift(per_worker: f32, most: f32, at: f32) -> f32 {
    let per = per_worker.max(0.0);
    1.0 + most * per / (per + at)
}

/// What the tools for one trade are worth to the hands working in it.
pub fn toolage(holdings: &Holdings, hands: &Hands, trade: Trade) -> f32 {
    let working = hands.at(trade);
    if working <= 0.0 {
        return 1.0;
    }
    lift(
        holdings.tools[trade as usize] / working,
        TOOL_LIFT,
        WELL_EQUIPPED,
    )
}

/// Everything the place owns, whatever it is for.
pub fn all_tools(holdings: &Holdings) -> f32 {
    holdings.tools.iter().sum()
}

/// What a year's production came to, good by good.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Made(pub [f32; Good::COUNT]);

impl Made {
    pub fn of(&self, good: Good) -> f32 {
        self.0[good as usize]
    }

    fn set(&mut self, good: Good, how_much: f32) {
        self.0[good as usize] = how_much.max(0.0);
    }
}

/// What one place made this year, and what its capital came to afterwards.
///
/// `primary` is what a hand at the bottom of the chain gets off this land in a year — the
/// Cobb–Douglas yield per worker, handed in rather than recomputed, so that the one-good
/// model stays the single authority on what land and labour are worth together.
///
/// The order is forced and it is the whole model: the goods that come off the land are made
/// first, because everything else is made *out of* them, and a trade that cannot get its
/// input simply does not produce. Nobody is told to farm; a smith with no stock makes no
/// tools, and next year smithing is not worth doing.
pub fn make(hands: &Hands, ground: Ground, holdings: &Holdings) -> (Made, Holdings) {
    let workers = hands.total();
    let mut made = Made::default();
    if workers <= 0.0 {
        return (made, Holdings::default());
    }

    // Off the land, each trade with the tools that were made for *it*. This is where a tool
    // actually helps, and a sickle is no use to somebody quarrying stone.
    let hewn = hands.at(Trade::Hewer) * ground.stock * toolage(holdings, hands, Trade::Hewer);
    made.set(Good::Stock, hewn);
    let raw_food =
        hands.at(Trade::Farmer) * ground.food * toolage(holdings, hands, Trade::Farmer);

    // Out of what came off the land. Leontief: a trade makes what its hands could make, or
    // what its inputs allow, whichever is less. There is no substituting labour for stock.
    // Out of what came off the land *and* out of what was left over from before.
    let (mut stock_left, food_left) = (holdings.stock + hewn, raw_food);

    let (input, per) = Good::Tools.needs().unwrap_or((Good::Stock, 0.0));
    debug_assert_eq!(input, Good::Stock);
    let smithing = hands.at(Trade::Smith);
    let tools = if per > 0.0 {
        smithing.min(stock_left / per)
    } else {
        smithing
    };
    stock_left -= tools * per;
    made.set(Good::Tools, tools);

    // Cooking, measured in mouths served. A cook consumes no food — they prepare what was
    // already grown and hand back the hours it would have taken everybody else — but they
    // cannot serve more mouths than there is food for, which is what ties the trade to the
    // harvest and what makes a bad year take the cooks first.
    let (_, per_mouth) = Good::Meals.needs().unwrap_or((Good::Food, SUBSISTENCE));
    let served = (hands.at(Trade::Cook) * MOUTHS_PER_COOK)
        .min(workers)
        .min(food_left / per_mouth.max(1e-6));
    made.set(Good::Meals, served);
    made.set(Good::Upkeep, hands.at(Trade::Keeper));

    // Every mouth served is a mouth that did not spend its own year's hours over a fire, and
    // that time comes back as more of everything the place does.
    let fed = 1.0 + COOKING_SHARE * (made.of(Good::Meals) / workers).clamp(0.0, 1.0);
    made.set(Good::Food, food_left * fed);

    // And what is left standing at the end of it. Tools made this year join the stock; wear
    // takes its share of what there was, less whatever the keepers held together.
    // What the keepers held together, as a share of the year's wear — not of the tools. The
    // two differ by a factor of ten and getting it wrong makes upkeep almost worthless.
    let held = all_tools(holdings);
    let wear = WEAR * held;
    let kept = (MENDING * made.of(Good::Upkeep) / wear.max(1e-6)).clamp(0.0, 1.0);

    // Who this year's tools were made for: the trades people are actually working, in
    // proportion to how many are working them. Not a decision anybody makes — a smith makes
    // what the people around them are asking for, and asking is what having hands in a trade
    // *is*. Deliberately not "whichever trade would gain most", which would be a choice read
    // afresh every year off a quantity the choice itself moves. §31.1.
    //
    // The consequence is that at rest this is exactly the old single pool: equip every trade
    // in proportion to its hands and every trade's tools-per-hand is the same number. What
    // differs is a place that *changes* what it does, which now carries the wrong tools for
    // a decade.
    let mut tools = holdings.tools;
    let wanted: f32 = Trade::ALL
        .into_iter()
        .filter(|t| USES_TOOLS.contains(t))
        .map(|t| hands.at(t))
        .sum();
    for trade in Trade::ALL {
        let share = if wanted > 0.0 && USES_TOOLS.contains(&trade) {
            hands.at(trade) / wanted
        } else {
            0.0
        };
        let mine = tools[trade as usize];
        let lost = if held > 0.0 { wear * (mine / held) } else { 0.0 };
        tools[trade as usize] =
            (mine - lost * (1.0 - kept) + made.of(Good::Tools) * share).max(0.0);
    }
    let after = Holdings {
        tools,
        // What was cut and not worked keeps, less what the weather takes. Nobody roofs a
        // timber pile, so upkeep does not reach it.
        stock: (stock_left * (1.0 - WEAR)).max(0.0),
    };
    (made, after)
}

/// How long a tool goes on being useful, in years.
///
/// The reciprocal of wear. It matters because a tool is the one thing here that is worth
/// more than what it does this year, and valuing it at one year's use is exactly the mistake
/// that makes an economy never build anything.
const TOOL_LIFE: f32 = 1.0 / WEAR;

/// What a year in a place came to, in food.
///
/// Food is the numeraire and it needs no defending: it is the thing everybody must have, it
/// is the unit `SUBSISTENCE` is one of, and every other good here exists to get more of it.
/// Tools count at what they will yield over their life rather than at what they yielded this
/// year, which is the whole difference between a thing you own and a thing you consumed.
///
/// `spare` is what the place is not desperate for. A hungry place values a tool at nothing,
/// because a tool is next year's problem and it is hungry now — and that single weighting is
/// why famine takes the smiths first.
fn value_of(made: &Made, holdings: &Holdings, workers: f32, ground: Ground, spare: f32) -> f32 {
    let land_food = made.of(Good::Food);
    // What a given quantity of tools is worth: the extra food it will pull out of the same
    // hands, for as long as it lasts. Saturating, because `toolage` is.
    // Asked of a total rather than per trade: what a place would give for one more tool
    // does not depend on which shed it ends up in, because a smith making tools makes them
    // for whoever is asking. Valuing them per trade here would price the mis-equipment of a
    // place that has just changed its work, which is a real cost but is one the *production*
    // already charges — counting it twice would have a village that turned cook also decide
    // its ploughs had never been worth anything.
    let tools_worth = |tools: f32| {
        let equipped = lift(tools / workers, TOOL_LIFT, WELL_EQUIPPED);
        land_food * (equipped - 1.0) / equipped.max(1e-6) * TOOL_LIFE
    };

    let standing = tools_worth(all_tools(holdings));
    // And what the pile of unworked material is worth: what it would add *if it were all
    // worked*, less the years of somebody's hands that would take, and discounted because it
    // is not the thing yet.
    //
    // Saturating with the rest, and that is the whole of why this is written this way. Valued
    // at a flat rate per unit, a stockpile is worth its size — so a place hewed timber for
    // ever, valued the heap at twenty years of its own harvest, and never made a single tool
    // because finishing one revalued the heap downwards. A hundred and eighty units of timber
    // in a village of forty is not worth a hundred and eighty times one unit. It is worth
    // about what one is.
    let all_worked = holdings.stock / STOCK_PER_TOOL;
    let unworked = (WORTH_UNWORKED * (tools_worth(all_tools(holdings) + all_worked) - standing)
        - all_worked * ground.food)
        .max(0.0);
    // Or what it fetches, if there is anybody to sell it to. Whichever is worth more, because
    // a place with both a use for its timber and a buyer for it will do whichever pays — and
    // this is the term that lets somewhere live off ground that grows nothing.
    let sold = holdings.stock * ground.sells_for;

    land_food + spare.clamp(0.0, 1.0) * (standing + unworked.max(sold))
}

/// How short of feeding itself a place is, from 0 to 1.
pub fn hunger(made: &Made, workers: f32) -> f32 {
    if workers <= 0.0 {
        return 0.0;
    }
    (1.0 - made.of(Good::Food) / (workers * SUBSISTENCE)).clamp(0.0, 1.0)
}

/// What one more hand in each trade would be worth here.
///
/// **Not a price.** There is no currency in this world and none is invented: this asks the
/// only question anybody choosing a trade could actually answer, which is *what happens if I
/// do this instead*. One more hand is put into each trade in turn, the year is run again, and
/// what the place ends up with is compared. The supply chain then enforces itself with
/// nothing written down — a smith where there is no stock adds nothing, so smithing is worth
/// nothing, so nobody smiths.
///
/// This replaced a table of target quantities per good, and the table was wrong in a way that
/// is worth recording: it priced meals by how far short of a target the place was, so cooking
/// stayed worth doing however many cooks there were. Six worlds ran to **fifty-one cooks
/// against forty-seven farmers**, and the population fell by a third. A want that does not
/// fall as it is met is not a want.
pub fn worth_taking_up(
    made: &Made,
    holdings: &Holdings,
    hands: &Hands,
    ground: Ground,
) -> [f32; Trade::COUNT] {
    let workers = hands.total().max(1.0);
    let spare = 1.0 - hunger(made, workers);
    let mut worth = [0.0; Trade::COUNT];
    for trade in Trade::ALL {
        let mut more = *hands;
        more.set(trade, more.at(trade) + 1.0);
        let (made_then, holdings_then) = make(&more, ground, holdings);
        worth[trade as usize] =
            value_of(&made_then, &holdings_then, workers + 1.0, ground, spare);
    }
    // Against the least of them, so the numbers read as "how much better than the worst
    // thing you could do", and so `SWITCHING` compares like with like across places.
    let worst = worth.iter().cloned().fold(f32::MAX, f32::min);
    for w in &mut worth {
        *w -= worst;
    }
    worth
}

#[cfg(test)]
mod tests;
