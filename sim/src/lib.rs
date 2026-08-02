//! The world, and the systems that advance it.
//!
//! `World` owns every entity in arenas and hands out handles. Systems take `&mut World`
//! and resolve handles as they need them — nothing holds a reference to anything else,
//! which is what will let households, kinship, and food webs be cycles later on.
//!
//! Nothing here prints. Systems append to the chronicle; rendering is the frontend's
//! job. That separation is what makes a run comparable to another run, which is how the
//! reproducibility guarantee gets tested rather than merely asserted.
//!
//! Two rhythms run side by side, which is the scale ladder in miniature: people act at
//! the pace of what they are doing (minutes to hours), and age at the pace of a year.
//! Neither polls.

pub mod deep;
pub mod provenance;

pub use provenance::{DeepProvenance, NotASave, Provenance};
#[cfg(test)]
mod deep_tests;

use genetics::{Architecture, FounderPool};
use person::{Cause, Deed, FERTILE_FROM, Person, PersonId, Situation};
use planet::{DayPhase, Planet, PlanetId};
use sim_core::{Arena, Chronicle, Domain, Duration, Rng, Salience, Scheduler, Time, WorldSeed};
use society::{Census, Place, PlaceId, Society};

/// Worlds start with a history behind them, so that a founding population can be
/// adults of varying ages rather than a single cohort of newborns.
///
/// A whole number of Earth days, so the clock starts at local midnight on a year
/// boundary rather than partway through a day the observer never saw.
pub const FOUNDING: Time = Time::from_secs(36_500 * 86_400);

/// How long a pregnancy runs.
pub const GESTATION: Duration = Duration::from_days(273);

/// Annual chance that a fertile, partnered woman in good health conceives.
///
/// Set deliberately above replacement rather than tuned to sit on it. With no feedback,
/// fertility near replacement is a knife edge: the population is a branching process, so
/// slightly below it dies out and slightly above it grows exponentially, with drift
/// deciding which. Measured here, 0.16 grows about fivefold per two centuries while 0.13
/// went extinct — there is no stable value in between to find.
///
/// Real populations are steady because fertility *responds* to conditions — density,
/// food, child mortality — and that negative feedback is what a constant cannot supply.
/// The economy has arrived and this feedback still has not, which is worth being precise
/// about because it was attempted four times. `economy::births_relative` is written and
/// tested and deliberately not called from here: every centring tried was a cull by a
/// different route, and §21 records all four with their measurements.
const CONCEPTION_PER_YEAR: f32 = 0.16;

/// How far below a neighbourhood's average a household can fall before being priced
/// out of it.
///
/// Admission alone sorts nobody: it only ever tested newcomers, and there were none —
/// founders start somewhere, children inherit their parents' quarter, and once inside
/// a household was never asked again. So the best place accumulated everyone and the
/// five quarters ended up indistinguishable. Rents rise; people who fall behind leave.
/// The margin is hysteresis, so a household near the line does not shuttle every year.
const DISPLACEMENT_MARGIN: f32 = 0.18;

/// How much a household minds crowding, per multiple of a place's capacity.
///
/// The missing negative feedback. Admission keeps newcomers out of a full place, but
/// nothing was pushing anyone out of one, and a new household founded by a couple
/// inherits its parents' neighbourhood without passing admission at all. So the best
/// quarter accumulated everyone: measured at 160 years, all 1,260 survivors lived in one
/// place at twenty-five times its capacity while the other four stood empty.
///
/// It bites only *past* capacity, and at the sizes this project runs it therefore never
/// bites at all. Housing is built out to meet demand, so a quarter that absorbs its
/// neighbours' households absorbs them while staying inside its own walls: measured across
/// four seeds, no place in any of those worlds ever exceeded its capacity, so the term was
/// identically zero for the whole of the project's history. What that leaves is an
/// absorbing state — a quarter emptied to nobody has no residents to make it worth
/// anything, so its `env` freezes at whatever it fell to and nothing draws anyone back —
/// and a fifth to a third of every world is permanently dead ground.
///
/// Making it continuous and convex (`aversion · occupancy²`, the hundredth household
/// costing more than the tenth) fixes exactly that, and was tried at 0.05, 0.20 and 0.40,
/// and was reverted at all three. It costs the thing this project will not spend. With
/// crowding driving migration at every occupancy rather than only past capacity, the two
/// detail tiers put households in different quarters, and under a thin detail budget
/// **25 people starved at 0.20 and 13 at 0.05, against none under an ample budget, across
/// six seeds, never once the other way**. The gate form measures 2 and 0 on the same six.
/// That is §21.1 — the observer deciding who dies — and it is not for sale.
///
/// So the mechanism stays inert and is labelled inert, pending a way to make crowding felt
/// that does not route the coarse tier's differences into a death. §30.5 has the sweep.
const CROWDING_AVERSION: f32 = 0.5;

/// How much better a neighbourhood has to be before a household will move to it.
///
/// Without a threshold, households shuffle endlessly between places that differ in the
/// third decimal, and churn — which erodes community — becomes an artefact of the
/// sorting loop rather than a fact about the world.
const MOVE_THRESHOLD: f32 = 0.05;

/// How much somebody's temperament decides what a year of work is worth to them.
///
/// It was 0.5, and that was the channel making children resemble parents more than §15 allows.
/// Conscientiousness is heritable, so if it decides most of how well anybody does, then most
/// of how well anybody does is handed down through the genome alone — measured at **0.51 of
/// outcome variance against a ceiling of 0.45**, with intergenerational elasticity at 0.66
/// against 0.50. Nothing else came close: taking away a patron's lift moved elasticity by
/// 0.04, and removing mentoring entirely moved it by nothing.
///
/// Lowering it is not free and the variance does not vanish — it moves to whatever else
/// explains an outcome, and with nothing else to take it up it went to chance. That is why
/// this and the estate had to be built together: one opens the room and the other fills it.
/// §15.2 has the four numbers before and after.
const TEMPERAMENT_AT_WORK: f32 = 0.5;

/// How much what a household owns adds to the childhood it can give, per unit of estate.
///
/// §14 makes the *quarter* almost all of what shapes a child, which was always a little too
/// clean: two families on the same street do not raise children identically, and what they
/// have is a large part of why. This is that difference, and it is the only thing an estate
/// does — it deliberately buys no admission, since wealth arriving at a funeral moving
/// somebody's house the same afternoon is how it broke the world the first time.
const WHAT_A_HOUSEHOLD_ADDS: f32 = 0.25;

/// The yearly chance of a taking, per unit of the pressure behind it.
///
/// Small, and rare on purpose. A taking is the sort of thing a place remembers for a
/// generation, not an annual event, and a rate high enough to see every year would make it
/// weather rather than history.
const TAKING: f32 = 0.02;

/// What share of an estate a taking carries off.
///
/// Not all of it. A raid takes what can be carried and leaves people alive on their ground —
/// which is what makes it repeatable, and what makes it different from the taking of ground
/// that §32 says is still missing.
const PLUNDER: f32 = 0.35;

/// The standing at which a household is keeping itself and no more.
///
/// Anything above this in a year is surplus and a share of it can be put by; at or below it
/// there is nothing to save. Set at the middle of the range rather than derived, because
/// `standing` is a scale of its own with no units to anchor to — and stated here rather than
/// buried so that the arbitrariness is visible.
const SUBSISTENCE_STANDING: f32 = 0.5;

/// What a spell of work adds to standing, where there is work worth having.
///
/// Derived rather than guessed. Gain saturates and decay is proportional, so standing
/// settles where `W·g·q = d·s` — with W the roughly four hundred spells of work in a
/// year, q the local opportunity and d the yearly decay. These values put a typical
/// worker in a middling neighbourhood near 0.5. Two earlier attempts were wrong in
/// opposite directions: one implied an equilibrium of 0.99 and turned every world into
/// an affluent enclave inside a decade, the other left no equilibrium at all.
const WORK_GAIN: f32 = 0.0017;

/// What a year takes back. Standing is a position needing upkeep, not a hoard.
const STANDING_DECAY: f32 = 0.15;

/// The share of their parents' standing a child starts from.
///
/// The most direct of the three routes by which advantage passes down — the other two
/// being the genes they inherit and the neighbourhood they grow up in.
///
/// Measured, it turns out to be the *weakest* of the three: moving it between 0.20 and
/// 0.55 shifts intergenerational elasticity only from 0.55 to 0.62. Advantage here
/// travels mostly through the neighbourhood a child is raised in, not through what is
/// handed to them at birth — which is worth knowing before reaching for this dial to
/// change how mobile a world is.
const INHERITED_STANDING: f32 = 0.35;

// ---- escape routes (§14.4) ---------------------------------------------------------
//
// Without these, a world is deterministic doom: advantage travels down through the
// neighbourhood a child is raised in, nothing travels the other way, and measured
// mobility never recovers. Three routes, chosen because none of them reward the people
// already ahead.
//
// These values sit on a frontier rather than at an optimum, and the trade is not a
// tuning artefact — it is arithmetic. An escape route works precisely by decoupling
// where someone ends up from where they began, so anything that lowers
// intergenerational elasticity also lowers the share of outcome that upbringing can
// explain, and raises the share left to chance. Measured across four settings:
//
//   routes off      elasticity 0.62   genes 0.39 / circumstance 0.39 / luck 0.46
//   these values    elasticity 0.55   genes 0.42 / circumstance 0.37 / luck 0.46
//   stronger        elasticity 0.40   genes 0.41 / circumstance 0.15 / luck 0.55
//   stronger still  elasticity 0.33   genes 0.39 / circumstance 0.07 / luck 0.59
//
// §15 asks for elasticity 0.20–0.50 *and* circumstance near 0.40 *and* luck near 0.30.
// This model cannot currently deliver all three at once. These values buy the closest
// thing to the design's central claim — that neither genes nor circumstance decides
// a life — and leave elasticity and luck each a little outside their bands, which the
// harness reports rather than hides.

/// Yearly chance of an unearned gain — a windfall, a good turn, being in the right place.
const WINDFALL_CHANCE: f64 = 0.015;
const WINDFALL: f32 = 0.12;

/// And of the reverse. Slightly likelier than the windfall, because ruin is.
const SETBACK_CHANCE: f64 = 0.020;
const SETBACK: f32 = 0.12;

/// Yearly chance, per unit of local bonding capital, that a young adult is taken up by
/// someone who can open doors.
///
/// Scaled by *bonding* capital rather than bridging, which is the whole point. Bridging
/// ties belong to the already-comfortable, so routing patronage through them would only
/// have widened the gap. Dense mutual-dependence community is what poor neighbourhoods
/// actually have, and turning it into a way out is what makes them produce escapees
/// rather than only outcomes.
/// Now scaled by what a *particular* person is worth to you rather than by the average
/// connectedness of the street. The old flip fired against bonding capital of about a half,
/// so about 0.028 a year for everybody alike; this fires against one patron's standing and
/// what they think of you, and at zero for somebody who has cultivated nobody.
const MENTOR_CHANCE: f64 = 0.09;

/// What a patron is worth, per unit of their own standing.
///
/// This used to be a flat multiplier, and making it depend on *who took you up* is the
/// whole point of there being a patron at all. A well-placed patron opens doors a poor one
/// cannot, so where you grew up reaches your outcome through the quality of the people you
/// could get to know there — which is what `bonding_capital` was a stand-in for and is now
/// the thing itself.
///
/// Measured rather than picked: with a flat multiplier and a real patron, patronage became
/// the dominant term in attainment *and* uncorrelated with anywhere, and §15's shared
/// environment share collapsed from a fifth to one part in a hundred thousand. Upbringing
/// had stopped predicting an outcome at all. Scaling by the patron restores it, because a
/// patron is a person and people are not distributed evenly across places.
///
/// A typical patron of middling standing is worth about what the flat 2.1 was.
const PATRONAGE: f32 = 2.2;

/// How much older, and how much better off, somebody has to be to be a patron rather than
/// a friend. Patronage is a relation between unequals; between equals it is just company.
const MENTOR_SENIORITY: f64 = 12.0;
const MENTOR_MEANS: f32 = 0.10;

/// What being taken up puts you in your patron's debt, in days.
///
/// Large, because it is: this is the largest favour anybody in this world does anybody
/// else, and an obligation that size is not discharged in a season. Whether it is ever
/// discharged at all is what decides how the two of them end up regarding each other, and
/// nothing here decides that in advance.
const MENTOR_FAVOUR: f32 = 40.0;

/// Ages at which someone will still uproot themselves for work.
const RESTLESS_UNTIL: f64 = 32.0;

/// How much better another trade has to look before somebody will change theirs.
///
/// **A proportion, not an amount.** It was an amount — a fifth — back when what a trade was
/// worth was a price between nought and one; now it is a quantity of food, which in a large
/// town runs to hundreds, and a fifth of a loaf is not a reason to change your life. A flat
/// threshold that small is no threshold at all, and it showed: a town of a hundred carried
/// twenty-seven keepers for tools that needed three, because every year everybody moved into
/// whatever was marginally ahead.
const SWITCHING: f32 = 0.20;

/// The yearly chance that somebody who has a reason to change trades actually does.
///
/// Low. Changing what you do for a living is a thing people do once or twice, and the
/// slowness is not a nuisance to be tuned away — it is what lets an occupational structure
/// exist at all rather than sloshing between trades every few years.
const RETRAINING: f64 = 0.08;

/// The yearly chance that somebody not yet settled into a trade reconsiders theirs.
///
/// Higher than `RETRAINING` — the young have less to put down — but not one, which is what it
/// was. Everybody in a place reads the same valuation in the same instant, so if they all act
/// on it in the same year they all move into whatever was short and it is short no longer:
/// **88% of a year's changes of trade went to the same trade**, and the signal pointed the
/// other way the year after. A cobweb, and the standard cause of one is that the decision is
/// simultaneous. Nobody reconsiders their livelihood on a schedule shared with their
/// neighbours, and staggering it is what turns an oscillation into a convergence.
const TRYING_THINGS: f64 = 0.25;

/// The yearly chance that somebody with a whole year of slack, in a well-connected place,
/// works something out.
///
/// Everything else scales this down: how much surplus their place actually had, how open they
/// are to a new way of doing a thing, and how easily its people reach each other.
///
/// This is the one number in §29 that had to be chosen rather than derived, and it is worth
/// being plain about what it is choosing. A real village of three hundred produced, over a
/// century, essentially no attributable lasting improvement — what reached it came from
/// populations a thousand times larger. Set to that, the mechanism would be correct and
/// permanently invisible in any world this machine can run. So it is set instead to about
/// **one lasting improvement per comfortable country per human lifetime**, which is generous
/// by a wide margin, and the reason for the generosity is written down here rather than
/// hidden in the number.
const WORKING_IT_OUT: f64 = 0.0008;

/// How much surplus per head counts as having time to think.
///
/// Half a year's food in hand. Below it people are busy staying alive, and the model says so
/// rather than assuming a scholar class into existence: **slack is the input to discovery**,
/// which is why the trap is a trap and why leaving it needs a run of good harvests first.
const TIME_TO_THINK: f32 = 0.5;

/// What a trade being wanted is worth to what a year's work returns.
///
/// The economy has to reach a person's own outcome or the division of labour is decoration.
/// It reaches it here: a hand in a trade the place badly wants earns more than a hand in one
/// it does not. Clamped, because this multiplies an already-calibrated figure and an
/// unbounded term here would be a second `WORK_GAIN` in disguise.
const PAY_BY_TRADE: std::ops::RangeInclusive<f32> = 0.6..=1.6;

/// The most that being spoken for can be worth when somewhere decides whether to take you.
///
/// Of the same size as `DISPLACEMENT_MARGIN` and `YOUNG_MOVER_SLACK`, the other two thumbs
/// on this scale, and for the same reason: these adjust who is admitted at the margin. A
/// term that could exceed what a household has would not be an adjustment, it would be a
/// replacement — see `World::backing` for what happened when it was one.
const VOUCHING: f32 = 0.15;

/// How much more readily somewhere takes in the young.
///
/// They are renting a room, not buying a house. Without this the spatial trap is
/// absolute: you cannot move to where the work is until you have the standing that
/// moving there would earn you.
const YOUNG_MOVER_SLACK: f32 = 0.30;

// ---- level of detail (§6, §8) ------------------------------------------------------

/// How finely a place is being simulated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Detail {
    /// Every person, every few hours: needs, utility scoring, the lot.
    Full,
    /// A year at a time, in aggregate. Individuals still exist and still have names,
    /// ages, genomes and families — they simply are not deliberated over.
    Coarse,
}

/// How many people may be simulated finely at once.
///
/// Cost scales with people times years, and a finely simulated person costs some four
/// thousand decisions a year whether or not anyone is watching. This is the ceiling on
/// how many are worth that.
pub const FULL_DETAIL_BUDGET: usize = 400;

/// Spells of work a year, at full opportunity.
///
/// Calibrated against the fine simulation rather than chosen — the coarse tier has to
/// produce the year the fine tier would have produced, and this is the number that makes
/// the two agree. See the equivalence test.
///
/// It was 300, and that was a fifth of a lifetime's standing too little. The gap did not
/// show because the equivalence test allowed a tenth of absolute standing at thirty years,
/// while the shortfall is *proportional* and so grows with the span: at sixty years an
/// unwatched adult reached 0.374 against a watched one's 0.476, and everything standing
/// feeds — affluence, a quarter's character, who is admitted where, every §15 measurement —
/// was quietly lower wherever nobody was looking.
///
/// At 380 the same comparison gives 0.472 against 0.476, and across three seeds the
/// remaining gaps are +0.004, −0.020 and −0.007: no longer one-directional, so what is left
/// is noise rather than bias.
const WORK_SPELLS_PER_YEAR: f32 = 380.0;

/// Spells of each deed an unwatched person gets through in a year, at ordinary appetite in
/// an unremarkable place.
///
/// The same calibration as `WORK_SPELLS_PER_YEAR` and for the same reason: an unwatched
/// person still has neighbours and still spends their days on something. If their ties stood
/// still while a watched person's grew, then who your friends are would depend on who the
/// observer happened to be looking at — the exact bug class that once had the observer
/// setting the death rate. The same now goes for what a life is *spent* on, since §26 reads
/// a person's position in their society off exactly that.
///
/// Measured, not chosen: see `measure_what_a_year_is_spent_on`. Only the four deeds anybody
/// has a choice about are booked; nobody's position in a society follows from how much they
/// slept. Work is absent because `WORK_SPELLS_PER_YEAR` already books it, calibrated against
/// standing rather than against a tally.
const SPELLS_A_YEAR: [f32; Deed::COUNT] = {
    let mut spells = [0.0; Deed::COUNT];
    spells[Deed::Wash as usize] = 730.0;
    spells[Deed::Socialize as usize] = 620.0;
    spells[Deed::Wander as usize] = 128.0;
    spells
};

/// How many separate people a year of company is spread over.
///
/// Company is settled once a year, in this many draws, each carrying its share of whatever
/// evenings the year held. Not once per evening: an evening cost some hundred map edits —
/// choosing company, meeting, and both parties' gossip — and at six hundred evenings a year
/// each for four hundred people that was the most expensive thing in the simulation by an
/// order of magnitude, to model the difference between seeing a friend on Tuesday and on
/// Wednesday.
///
/// Sixteen because `choose_company` concentrates evenings on the people you already know,
/// so a year of company is not spread over the town; a handful is what that concentration
/// actually produces.
const COMPANY_A_YEAR: u32 = 16;

/// How readily a disposition becomes a thing somebody actually did.
///
/// An appetite is not an occasion. Somebody generous does not hand something over on all
/// sixteen evenings a year they spend in company, and this is the factor that turns a
/// standing willingness into an event that happens now and then — a full-blooded appetite
/// acts about once a year, an ordinary one about once a decade. It is the single number
/// that sets how eventful this world's social life is, and it is the first thing to move if
/// the acts count in `vitals` comes out absurd in either direction.
const AN_OCCASION: f32 = 0.06;

/// What a gift is worth, in days of the giver's work.
///
/// Something over a week — a real favour, well under `PATIENCE`, and small enough that
/// `Bonds::helped`'s warmth does not saturate on it. Everything about the size of this number
/// is set by the ledger it goes on rather than by what feels generous, because a favour whose
/// size the reciprocity machinery cannot represent is not a favour, it is a distortion.
const GIFT_IN_DAYS: f32 = 8.0;

/// What a lesson is worth to whoever is taught, in standing.
const TEACHING: f32 = 0.03;
/// And how much of an upbringing it counts for, in years, for a pupil still young enough
/// for it to count at all.
const TEACHING_YEARS: f32 = 0.25;

/// How hard a shunning lands.
const SHUNNING: f32 = 0.5;

/// The share of an estate one person can carry off from another by hand.
///
/// Well under `PLUNDER`, which is what a whole settlement takes from a whole settlement. A
/// raid empties a place; a robbery takes what a man can carry.
const BY_HAND: f32 = 0.30;

/// How likely any one person standing there is to actually notice, before the act's own
/// openness is applied.
///
/// Well under one. Being in the same company as somebody is not watching them, and a village
/// where every neighbour clocks every transaction is a panopticon rather than a village.
const SOMEBODY_NOTICES: f32 = 0.35;

/// And how many can see one thing.
///
/// The first version had no cap and averaged **nine witnesses per act** — because the list it
/// draws from is everybody you know here plus a dozen faces out of the crowd, which is a
/// settlement rather than a doorway. Two or three is what a thing done in front of people is
/// actually done in front of.
const HOW_MANY_SEE: usize = 3;

/// How far seeing something moves what a witness thinks of whoever did it.
///
/// Small, because regard is the number that *travels* and a quantity that could be rewritten
/// by one sighting would be noise rather than a reputation — the same argument `HEARSAY` makes
/// for itself, at the same order of magnitude.
const WHAT_A_WITNESS_MAKES_OF_IT: f32 = 0.06;

/// What a living looks like at the start of an adult life, and how much more by the end —
/// the means at which somebody of a given age reads as neither well-off nor badly-off (§42.4).
///
/// Measured, not chosen. Mean `means()` by fifth of a life, over three worlds:
/// `(children)` then `0.60`, `0.70`, `0.78`, `0.79` —
/// so par runs from about 0.57 early to about 0.81 late, which is what these two give. The
/// saturating form needs a middle and never a maximum, which matters because `means()` has no
/// ceiling — it is `standing + estate * WORTH_AT_A_DOOR` and runs past 1.9, and dividing an
/// unbounded quantity by a guessed maximum is the error §36.6 spent three rounds on.
const A_LIVING_STARTING_OUT: f32 = 0.45;
const A_LIVING_BY_THE_END: f32 = 0.40;

/// How much of an obligation has to go unmet before anybody counts it as a slight.
const WITHHOLDING_NOTICED: f32 = 0.30;
/// The gap in means at which somebody is half as envious as they could possibly be — §36.6.
///
/// **Measured rather than chosen.** Across three worlds of ninety years the gap between an
/// adult and the best-off person they know has a median of 0.958, so this is one, and the median
/// person therefore reads a half by construction. That is the only defensible way to pick it:
/// the alternative is a number that feels right, and `means()` has no natural ceiling to
/// measure a gap against — it is `standing + estate * WORTH_AT_A_DOOR` and runs to 1.93.
const A_GAP_WORTH_MINDING: f32 = 1.0;
/// How many neighbours somebody weighs up before deciding who to spend an evening with.
///
/// Nobody surveys a town. This is what keeps the cost of a social life bounded by Dunbar
/// rather than by the size of the settlement — without it, an evening in a city of two
/// thousand costs two thousand times an evening in a hamlet, for no more society.
const NEIGHBOURS_CONSIDERED: usize = 12;

/// How much of what an ally has over you they will take off your shoulders in a bad year.
///
/// Per unit of standing they have above yours, scaled by how warmly they hold you. Not a
/// transfer of standing — nothing anybody owns changes hands. What moves is the *shortfall*:
/// see `share_the_shortfall`, where every day of hunger lifted off one person is a day put
/// onto another, so that a famine kills the same number and no longer picks at random.
const RELIEF: f32 = 0.6;

/// What share of their standing somebody will spend in a year on settling up.
///
/// A ceiling, and the reason a debt can outlive the person who incurred it. Without one,
/// everybody clears everything in a few good years, nobody is ever a creditor for long, and
/// the whole of reciprocity collapses into an accounting identity.
const AFFORDABLE: f32 = 0.35;

/// Something that happened, as the simulation records it — structured, not prose.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Happening {
    WorldBegins {
        planet: PlanetId,
    },
    PhaseBegins {
        planet: PlanetId,
        phase: DayPhase,
    },
    PersonArrives {
        person: PersonId,
    },
    PersonDoes {
        person: PersonId,
        deed: Deed,
    },
    PersonDies {
        person: PersonId,
        cause: Cause,
    },
    PersonPairs {
        person: PersonId,
        with: PersonId,
    },
    PersonBorn {
        child: PersonId,
        mother: PersonId,
        father: PersonId,
    },
    PersonMoves {
        person: PersonId,
        to: PlaceId,
    },
    PersonMentored {
        person: PersonId,
        by: PersonId,
    },
    /// Somebody worked out a better way to do the thing they do.
    ///
    /// The rarest thing in the chronicle and the only one that changes what is *possible*
    /// rather than what happened. Nothing else in this world moves a limit.
    PersonWorksItOut {
        person: PersonId,
        trade: economy::Trade,
    },
    /// Somebody who had already settled into a trade gave it up for another.
    ///
    /// Only for the settled. The young trying things is how anybody arrives at a trade at all
    /// and is not an event in a life; a smith of forty putting down the hammer is.
    PersonRetrains {
        person: PersonId,
        from: economy::Trade,
        to: economy::Trade,
    },
    PlaceChanges {
        place: PlaceId,
        into: society::Archetype,
    },
    /// Somebody's neighbours came and took what they had.
    ///
    /// The first thing in this world that happens *to* people rather than being chosen by
    /// them. §24.4 kept conquest out on the grounds that it needed a state and an army, and
    /// that was the wrong prerequisite — what it needs is something worth taking, which is
    /// why it could not be built before there were estates.
    PlaceTaken {
        /// Where it was taken from.
        place: PlaceId,
        /// And where the takers came from.
        by: PlaceId,
    },
    /// Somebody did something to somebody, on purpose.
    ///
    /// The first thing in this chronicle with a person on both ends of it that neither of
    /// them is obliged to by kinship, need or the machinery of the year. Everything else two
    /// people do here they do because they are married, related, or both hungry; this is one
    /// of them deciding about the other. See `person::acts`.
    PersonActsOn {
        person: PersonId,
        toward: PersonId,
        act: person::acts::Toward,
    },
}

impl Happening {
    /// Who and what this is about, so the chronicle can file it under them.
    ///
    /// A biography is the log filtered by participant, and this is the filter. Note that
    /// a birth is about three people: without the parents in the list, a life's record
    /// would not mention its own children.
    pub fn subjects(&self) -> Subjects {
        use sim_core::chronicle::Subject;
        let one = |a: Subject| Subjects::of(&[a]);
        match self {
            Happening::WorldBegins { planet } => one(planet.to_bits()),
            Happening::PhaseBegins { planet, .. } => one(planet.to_bits()),
            Happening::PersonArrives { person }
            | Happening::PersonDoes { person, .. }
            | Happening::PersonDies { person, .. }
            | Happening::PersonWorksItOut { person, .. }
            | Happening::PersonRetrains { person, .. } => one(person.to_bits()),
            // Taking somebody up is an event in the patron's life as much as in theirs.
            Happening::PersonMentored { person, by } => {
                Subjects::of(&[person.to_bits(), by.to_bits()])
            }
            Happening::PersonPairs { person, with } => {
                Subjects::of(&[person.to_bits(), with.to_bits()])
            }
            Happening::PersonBorn {
                child,
                mother,
                father,
            } => Subjects::of(&[child.to_bits(), mother.to_bits(), father.to_bits()]),
            Happening::PersonMoves { person, to } => {
                Subjects::of(&[person.to_bits(), to.to_bits()])
            }
            Happening::PlaceChanges { place, .. } => one(place.to_bits()),
            Happening::PlaceTaken { place, by } => Subjects::of(&[place.to_bits(), by.to_bits()]),
            // Both, always. Being given to and being robbed are events in a life whoever
            // else was involved, and a record filed only under whoever moved first would
            // leave every victim's biography silent about the thing done to them.
            Happening::PersonActsOn { person, toward, .. } => {
                Subjects::of(&[person.to_bits(), toward.to_bits()])
            }
        }
    }
}

/// Who a happening concerns, without touching the heap.
///
/// This was a `Vec` and it was the single largest source of allocation in the simulation:
/// one heap allocation and one free for **every event recorded**, twenty-six million of
/// them in a sixty-year world, almost all of which hold exactly one identifier. Nothing
/// concerns more than three parties — a birth, which is the child and both parents — so the
/// whole thing fits in a fixed array and never leaves the stack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Subjects {
    who: [sim_core::chronicle::Subject; 3],
    many: u8,
}

impl Subjects {
    fn of(who: &[sim_core::chronicle::Subject]) -> Subjects {
        let mut all = [0; 3];
        all[..who.len()].copy_from_slice(who);
        Subjects {
            who: all,
            many: who.len() as u8,
        }
    }

    pub fn as_slice(&self) -> &[sim_core::chronicle::Subject] {
        &self.who[..self.many as usize]
    }

    pub fn contains(&self, who: &sim_core::chronicle::Subject) -> bool {
        self.as_slice().contains(who)
    }
}

impl Happening {
    /// Who this concerns, if anyone. A biography is the log filtered by this.
    pub fn subject(&self) -> Option<PersonId> {
        match self {
            Happening::PersonArrives { person }
            | Happening::PersonDoes { person, .. }
            | Happening::PersonDies { person, .. }
            | Happening::PersonPairs { person, .. }
            | Happening::PersonMoves { person, .. }
            | Happening::PersonMentored { person, .. }
            | Happening::PersonWorksItOut { person, .. }
            | Happening::PersonActsOn { person, .. } => Some(*person),
            Happening::PersonBorn { child, .. } => Some(*child),
            _ => None,
        }
    }
}

/// Work the scheduler has queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Task {
    /// A planet announcing itself, once, at the start of the world.
    PlanetAwakens(PlanetId),
    /// A planet crossing into the next quarter of its day.
    PlanetPhase(PlanetId),
    /// A person finishing what they were doing and choosing the next thing.
    PersonActs(PersonId),
    /// A person getting a year older, and rolling against mortality.
    PersonAges(PersonId),
    /// A pregnancy coming to term.
    Birth { mother: PersonId, father: PersonId },
    /// A yearly census: every place reads itself off its residents, and households
    /// consider moving.
    Reckoning,
}

pub struct World {
    pub seed: WorldSeed,
    pub planets: Arena<Planet>,
    pub people: Arena<Person>,
    pub society: Society,
    pub places: Arena<Place>,
    pub chronicle: Chronicle<Happening>,
    architecture: &'static Architecture,
    pool: FounderPool,
    scheduler: Scheduler<Task>,
    next_stream: u64,
    /// Mothers with a birth already queued. Ordered, so iteration cannot vary.
    expecting: std::collections::BTreeSet<PersonId>,
    /// What each place read as at the last reckoning, so a change of character is
    /// noticed rather than merely happening.
    was: std::collections::BTreeMap<PlaceId, society::Archetype>,
    /// How many people may be simulated finely at once.
    budget: usize,
    /// Which places are being simulated finely.
    detail: std::collections::BTreeMap<PlaceId, Detail>,
    /// Places the observer has asked to see. Always simulated finely.
    watched: std::collections::BTreeSet<PlaceId>,
    /// Households that arrived somewhere since the last reckoning.
    arrivals: std::collections::BTreeMap<PlaceId, u32>,
    /// What was done where, since the last reckoning. Norms are read off this.
    deeds_done: std::collections::BTreeMap<PlaceId, [u32; Deed::COUNT]>,
    /// The peoples of this world and which place practises what.
    ///
    /// `None` until somewhere is actually inhabited — a world with nobody in it has no
    /// culture, and starting one anyway would mean writing down a people before there was
    /// anybody to be them.
    cultures: Option<culture::Cultures>,
    /// What each place knows how to do.
    ///
    /// Kept per place but *advanced per country*, because the Tasmanian result is about how
    /// many people a technique can be copied between, and a country is exactly the set of
    /// people who can reach each other to copy it. A place that empties keeps what it knew,
    /// for the same reason its manners survive it.
    technique: std::collections::BTreeMap<PlaceId, economy::Technique>,
    /// Places in the order culture knows them.
    ///
    /// `Cultures` indexes places by position and never forgets one, so the roster only
    /// grows: a place that empties keeps its slot, along with the manners of whoever used
    /// to live there. Arena ids are not dense and places are founded as the world spreads,
    /// so the mapping has to be kept rather than derived.
    roster: Vec<PlaceId>,
    /// Who holds what about whom.
    ///
    /// The one part of this world that is neither a person nor a place: the edges. Kept
    /// here rather than on the people because a tie is not a possession — it runs between
    /// two of them, and half of one stored on each is two facts that can disagree.
    pub bonds: bonds::Bonds,
    /// Who is to hand, per place, as of the last reckoning.
    ///
    /// Rebuilt yearly rather than maintained, because the only thing that moves anybody
    /// between places is `sort_households`, which runs yearly. A newborn is not company
    /// and the recently dead are dropped by `Bonds::year`, so a list a few months stale is
    /// a list of the neighbours.
    neighbours: std::collections::BTreeMap<PlaceId, Vec<PersonId>>,
    /// How many evenings of company each person has kept since the last reckoning.
    ///
    /// Counted rather than acted on, because who somebody spends an evening with is settled
    /// once a year for everybody at once — see `COMPANY_A_YEAR`. What this preserves is the
    /// part that has to come from the person: an extravert chooses `Deed::Socialize` more
    /// often, so an extravert has more friends, and that is an outcome of their temperament
    /// rather than a rule about temperaments.
    evenings: std::collections::BTreeMap<PersonId, u32>,
    /// What each place owns that outlives the year.
    ///
    /// The only capital in this world, and the first thing in it that can compound: tools,
    /// made by smiths out of what hewers cut, worn down by use, held together by keepers,
    /// and multiplying what everybody else's hands get off the land. §22 said plainly that
    /// without capital nothing here could compound — a rich place was rich because of its
    /// land and its road, never because it was rich last century. This is the correction.
    holdings: std::collections::BTreeMap<PlaceId, economy::Holdings>,
    /// What one more hand in each trade would be worth in each place, as of the last
    /// reckoning. What anybody choosing a trade is actually looking at.
    worth: std::collections::BTreeMap<PlaceId, [f32; economy::Trade::COUNT]>,
    /// Where everybody stands in the world's regard, from 0 to 1, as of the last reckoning.
    ///
    /// A **rank**, not the raw figure, and for the reason everything else about a position
    /// is a rank. In a world at the Malthusian edge nearly everybody owes somebody something
    /// they cannot repay, so nearly everybody's raw regard is slightly negative — which
    /// makes "thought poorly of" true of the whole population and therefore a statement
    /// about nobody. What a town can actually act on is whether somebody is worse thought of
    /// *than the rest*.
    ///
    /// Kept rather than asked, because a reputation is the one fact about a person that
    /// cannot be read off their own ties — it lives in everybody else's — and answering it
    /// from scratch means walking the whole graph. Walked once a year for everybody at once.
    repute: std::collections::BTreeMap<PersonId, f32>,
    /// Shortfall somebody has taken on for somebody else's sake, not yet gone through.
    ///
    /// Hunger given away is not hunger destroyed — see `share_the_shortfall`. It waits here
    /// until the giver's own birthday comes round and they go without instead.
    shouldered: std::collections::BTreeMap<PersonId, f32>,
    /// Scratch: who somebody is weighing up this evening. Kept on the world purely so that
    /// an evening in company costs no allocation, of which there are some hundreds of
    /// millions in a run.
    company: Vec<PersonId>,
    /// Whether people act on each other at all — see `person::acts`.
    ///
    /// A switch on the world rather than a script that edits a constant and rebuilds. Two
    /// ablations in this project have left the working tree holding an edited constant after
    /// the container running them restarted, and an ablation nobody can run without editing
    /// the source is an ablation nobody runs. Switching this off costs nothing and moves
    /// nothing else: acts draw from their own stream, so a world with them off follows the
    /// same trajectory as one that never had them.
    pub acts_are_possible: bool,
    /// How many times each act in `person::acts` has ever been done to anybody.
    ///
    /// A tally on the world rather than a count of the chronicle, and that is a decision
    /// worth stating. Most of these are not pivotal events — an evening in which somebody
    /// was cold to somebody is not a turning point in either life — so they are recorded at
    /// `Notable`, and every long run in this project uses `record_only(Pivotal)` and would
    /// drop them. An instrument that can only see a mechanism at a detail setting nobody
    /// uses cannot tell "switched off" from "never fired", which is §31.2's whole lesson.
    /// Six words of counter can.
    pub acted: [u32; person::acts::Toward::COUNT],
    /// Whether a life changes who somebody is — see `Person::weather`.
    ///
    /// Its own switch, because "people do things to each other", "a society finds out" and
    /// "people are changed by it" are three separate claims and §31.2's table wants a row for
    /// each.
    pub people_change: bool,
    /// Whether anybody measures themselves against anybody in particular — see
    /// `dreams::Dream::WhatTheyHave`.
    ///
    /// Its own switch, and this one earns it more than the others do. The person somebody
    /// envies is by construction the best-off person they know, and robbery already covets
    /// means — so a world where the envied get robbed far more than anybody else is exactly
    /// what a world with **no envy at all** would also look like. The two are told apart by
    /// switching this off and measuring the same rate, which is the only way the difference
    /// between "envy aims" and "the rich are worth robbing" can be seen at all.
    pub people_envy: bool,
    /// Whether only the single strongest longing gets a say in what somebody does, which is
    /// how it worked until §36.6.
    ///
    /// Here so that the *cost* of dropping winner-take-all stays measurable rather than
    /// remembered. It is not a mechanism anybody argues for — it is the old behaviour, kept
    /// switchable because the change to it moved giving and shunning by more than the noise
    /// floor and a number that large should not live only in a document. Set it and the array
    /// handed to the scorer keeps its maximum and zeroes the rest.
    pub only_the_strongest_dream: bool,
    /// Whether somebody's own standing buys them time to think — §48.4.
    ///
    /// Its own switch, because it changes the one thing in this model that decides whether a
    /// world has a history or only a demography. With it off, the input to discovery is a
    /// per-place average and a crowded quarter cannot invent however rich the people in it.
    pub people_think_on_their_own_means: bool,
    /// Whether an ordinary evening in somebody's company moves what you rate them at — §42.4.
    ///
    /// Its own switch because it is the source of a quantity that had none, and the ablation
    /// is the whole argument: with it off, `regard` sits at zero on 97.7% of live ties and
    /// `hearsay` spreads nothing, which is the world every measurement in §40 and §42.2 was
    /// taken in.
    pub reputation_is_earned: bool,
    /// Whether anybody standing there notices what is done in front of them — see
    /// `let_them_see`.
    ///
    /// Its own switch rather than sharing `acts_are_possible`, because they are separate
    /// claims: one is whether people do things to each other, the other is whether a society
    /// finds out. §31.2's table wants a row for each.
    pub witnesses_notice: bool,
    /// How many acts anybody who was not part of them ever saw — see `let_them_see`.
    pub witnessed: u32,
    /// And how many times somebody stood beside a person going short, in a place whose ways
    /// say you do not do that, and did nothing.
    ///
    /// The wrong nobody chooses — see `person::acts::withheld`. Counted for the same reason
    /// and kept separately, because it is the one wrong in this world whose weight depends
    /// on where you are standing.
    pub withheld: u32,
    /// Every evening anybody spent with anybody, and how many of those were spent with the
    /// one person they envy — see `dreams::Dream::WhatTheyHave`.
    ///
    /// Four counters rather than one, because envy's claim is not *that people rob* — they
    /// already did — but that robbery **lands on a particular person**. A tally of robberies
    /// cannot tell a world where envy aims from a world where it only agitates, and those are
    /// different claims about what a society is. What distinguishes them is a rate against a
    /// rate: how often the envied are robbed, against how often everybody else is. §31.2 says
    /// to add the line before ablating the mechanism, and this is the line.
    pub occasions: u64,
    pub met_the_envied: u64,
    pub robbed_the_envied: u32,
    /// And of those evenings, the ones where the envy was strong enough to say anything at
    /// all — over `dreams::WORTH_WANTING`.
    ///
    /// The difference between this and `met_the_envied` is the difference between a mechanism
    /// that is *weak* and one that never runs, and two rounds of chasing the wrong fix were
    /// spent because nothing here could tell them apart. A rate whose numerator is zero says
    /// nothing about the arithmetic above it.
    pub told_the_envied: u64,
    /// The ground the world stands on, if it stands on any.
    surface: Option<Surface>,
    /// How many people it was founded with. Kept because a world cannot be made again
    /// without it, and making it again is what a save is.
    founded_with: usize,
}

/// The solid planet under a populated world.
///
/// A still frame of the deep-time stack rather than a running copy of it. The lithosphere
/// and the climate move on a clock eleven rungs above this one — a megayear is thirty
/// thousand human lifetimes — so from the point of view of anybody living here the
/// continents are exactly where they were yesterday and will be exactly there tomorrow.
/// Running the two together would be spending most of the machine on a coastline nobody
/// alive will see move.
///
/// What this is *for* is the join: it is what makes "where do they live" answerable with
/// a place on a real planet rather than with a name somebody typed.
pub struct Surface {
    pub planet: geo::Lithosphere,
    pub climate: climate::Climate,
    pub life: biome::Biosphere,
    /// The star this world goes round, and which of its planets this is.
    pub system: cosmos::System,
    pub world: usize,
}

impl Surface {
    /// The star.
    pub fn star(&self) -> cosmos::Star {
        self.system.star
    }

    /// This world's orbit.
    pub fn orbit(&self) -> cosmos::Orbit {
        self.system.worlds[self.world]
    }

    /// Let the planet get on with it.
    ///
    /// Plates move, mountains rise and wear down, the sea comes and goes, the star
    /// brightens, the thermostat answers, and the biomes are re-read off the result. This
    /// is the same loop the deep-time globe runs; what is new is that there are people
    /// standing on it while it happens.
    ///
    /// The star ages too, which is the part that eventually ends everything: a world stays
    /// habitable for a finite time and this is the mechanism that runs it out.
    pub fn step_myr(&mut self, dt: f32, rng: &mut Rng) {
        debug_assert!(dt > 0.0, "time only runs forwards");
        self.planet.step_myr(dt, rng);
        self.system.star.age_gyr += dt as f64 / 1000.0;
        self.climate.step_myr(&self.planet, dt, rng);
        // The one wire that runs back down the stack: rivers cut in proportion to how much
        // falls on them, and only the climate knows that. Without it a desert wears down as
        // fast as a rainforest and the continents retire into the sea.
        let runoff: Vec<f32> = self
            .planet
            .grid()
            .cells()
            .map(|c| self.climate.rain_mm(c) / climate::moisture::REFERENCE_RAIN_MM)
            .collect();
        self.planet.set_runoff(&runoff);
        self.life = biome::Biosphere::read(&self.planet, &self.climate);
    }
}

/// How many records a world will hold before it starts forgetting the small and old.
///
/// The chronicle was never compacted. `compact_chronicle` has existed the whole time, with
/// a comment saying that retaining every routine act for decades "is not affordable until
/// compaction exists" — and compaction did exist, and nothing ever called it. A sixty-year
/// world of two hundred people logs twenty-six million records at forty-eight bytes each;
/// a hundred and fifty years of four hundred logged two hundred and thirteen million, which
/// is ten gigabytes of memory nobody asked for.
///
/// A **safety valve, not housekeeping**, and the number is set from measurement rather than
/// taste. Compaction rebuilds every surviving record and the whole index, so its cost falls
/// on the total ever recorded and not on the budget: trimming to one million cost 18% of
/// the running time, and so did trimming to eight million. There is no budget at which
/// routine compaction is cheap. Set high enough that an ordinary run never reaches it and
/// only a run heading for gigabytes ever pays, which is the right place for a valve.
///
/// Nothing above `Pivotal` is ever dropped, so what goes is what the design says should go.
const CHRONICLE_BUDGET: usize = 20_000_000;

/// How far over budget the chronicle drifts before it is trimmed, so the rebuild is
/// amortised across those records rather than repeated at the threshold.
const CHRONICLE_SLACK: usize = CHRONICLE_BUDGET / 2;

/// How fine the grid under a populated world is.
///
/// Level three: six hundred and forty-two cells, about eight hundred thousand square
/// kilometres each. Coarse for geophysics and right for this — the question being asked
/// of it is "which corner of which continent", and a finer grid costs a settling climate
/// on every world founded, including every one a test founds.
const SETTLED_GRID: u8 = 3;
/// How many plates a populated world's planet is broken into.
const SETTLED_PLATES: usize = 8;
/// The share of the surface that starts as continent.
const SETTLED_LAND: f32 = 0.40;
/// How promising a world has to be before people are put on it.
///
/// The anthropic filter, stated as a number. Most stars have nowhere worth living and
/// most of the worlds that qualify are marginal; a world with a civilisation on it is by
/// construction one of the good ones, so worlds are drawn until a good one turns up rather
/// than the first one that is merely possible.
const WORTH_SETTLING: f64 = 0.35;
/// How many systems to look through before giving up and taking the best seen.
///
/// A bound rather than a limit anybody is expected to hit: it exists so that a
/// pathological seed cannot spin here forever.
const SYSTEMS_TO_SEARCH: usize = 4_000;
/// How many candidate worlds to actually solve a climate for.
///
/// The flux band below narrows the field, but it cannot settle the question on its own:
/// how much carbon dioxide a thermostat needs to hold a given temperature also depends on
/// how much weatherable rock the planet has, and that varies by a factor of two between
/// seeds. So the last step is to ask the climate rather than to predict it, and asking
/// costs a solve. Four is enough that a breathable world turns up on every seed tried and
/// few enough that founding a world stays cheap.
const WORLDS_TO_TRY: usize = 4;
/// The most carbon dioxide people can live under, in parts per million.
///
/// One per cent. Above it the air is measurably harmful and by four it is lethal; the
/// exact threshold is a matter of exposure, and one per cent is the standard occupational
/// ceiling.
const BREATHABLE_CO2_PPM: f32 = 10_000.0;
/// The least carbon dioxide a biosphere can live under, in parts per million.
///
/// Ordinary photosynthesis stops fixing carbon below about this, which is why a very
/// brightly lit world is not habitable either: the thermostat draws the air down to keep
/// it cool and starves the plants doing it.
const GREEN_CO2_PPM: f32 = 150.0;
/// The band of starlight a world can carry **people** in, relative to what the Earth gets.
///
/// Measured across three planets rather than assumed, and startlingly narrow — and the
/// reason it is narrow is the interesting part, because it is not temperature.
///
/// The habitable zone in `cosmos` uses the standard astronomical bounds and its outer edge
/// sits at about a third of Earth's sunlight. That figure means *liquid water somewhere*,
/// and it gets there by letting the planet accumulate several bars of carbon dioxide. The
/// thermostat here will do exactly that, and it works: a world at nine tenths of Earth's
/// light settles at a comfortable thirteen degrees. It does so under **seven per cent
/// carbon dioxide**, which is four times the concentration that kills a human being.
///
/// The other end is the mirror image. Past about one and an eighth of Earth's light the
/// thermostat has drawn carbon dioxide down below a hundred and fifty parts per million,
/// which is where ordinary photosynthesis stops working. The planet is warm, blue, and
/// starving.
///
/// So the temperature is a red herring at both ends: everywhere from three quarters to a
/// quarter again of Earth's light comes out temperate, because that is what a thermostat
/// is *for*. What is habitable is the much narrower band where the atmosphere it needs to
/// do that is one a person could breathe and a plant could use. That band is this one, and
/// it holds across every planet tested to within a couple of per cent.
const LIVEABLE_FLUX: std::ops::Range<f64> = 0.97..1.12;
/// How far apart two settlements must be, in rings of the grid.
///
/// How far apart two places can be and still be one country, in cells rather than in miles.
///
/// It wants to be about a fortnight on foot — the classical radius of a state held together
/// by people walking, which is where Rome's core and the medieval kingdoms all sit, because
/// they are all limited by the same legs. Six hundred kilometres, written down as such.
///
/// It cannot be written down as such. The planet under a populated world runs at grid level
/// three, where one cell is **961 km across** — wider than France. Settlements land one to
/// four thousand kilometres apart because that is the finest the ground can distinguish, so
/// an absolute threshold of six hundred kilometres does not describe a small country: it
/// guarantees that no two places are ever in the same one. Every quarter became its own
/// country, and because technique is carried by a country's population, no world could ever
/// hold enough minds together to learn anything. The Tasmanian mechanism was switched on and
/// unreachable.
///
/// So the link is expressed in what the grid can actually say — neighbouring ground — and
/// scales with resolution: raise the level and this tightens towards the fortnight it wants
/// to be. What it costs is honesty about what a country means here, which §23 records: at
/// this resolution a country is a handful of adjacent regions, not a polity anybody walked
/// across.
///
/// It bounds the *links*, not the country — `World::countries` walks a chain of them, so a
/// ribbon of places each within reach of the next is one country however long the ribbon is.
const NEIGHBOURING_GROUND: f64 = 1.6;

/// One ring. At this grid a ring is most of a country, and neighbouring cells would be
/// the same place.
const SETTLEMENTS_APART: usize = 1;
/// How many neighbourhoods a world is founded with.
const QUARTERS: usize = 5;

impl Surface {
    /// Make the ground a world will stand on.
    ///
    /// Drawn and solved once, not run. Running it was tried and measured, because the
    /// worlds this founds carry several thousand parts per million of carbon dioxide and
    /// that looked like a thermostat that had not been given time. It is not. Six hundred
    /// megayears of plates and climate leaves the same planet at fifty-eight hundred parts
    /// per million rather than fewer, because what the thermostat is regulating against is
    /// *weatherable land*, and six hundred megayears of erosion cuts the land from a third
    /// of the surface to a sixth. The carbon dioxide is high because there is little rock
    /// to draw it down, which is the carbonate–silicate cycle working rather than failing.
    /// A hot, wet, low-relief world with a Cambrian atmosphere is a legitimate planet, and
    /// it is what these seeds produce.
    ///
    /// What running it *did* cost was thirteen seconds per world at the grid the plates
    /// need, on every world any test founds. That is the trade, and it is written down
    /// here so it does not have to be discovered twice.
    pub fn genesis(seed: WorldSeed) -> Surface {
        // Find a sky first. Most stars have nowhere worth living — that is the whole
        // point of `cosmos` and it is why this is a search rather than a construction —
        // and a world with people on it is by construction one of the lucky ones. Looking
        // until one turns up is the anthropic principle written as a loop, and it is
        // honest in a way that hardcoding a sun is not: the star that comes out is drawn
        // from the real distribution, conditioned on somebody being there to see it.
        let mut sky = seed.stream(Domain::World, 0, 0);
        let mut candidates: Vec<(cosmos::System, usize, f64)> = Vec::new();
        for _ in 0..SYSTEMS_TO_SEARCH {
            let system = cosmos::System::drawn(&mut sky);
            let Some(index) = system.best_world() else {
                continue;
            };
            let world = system.worlds[index];
            // What the astronomy allows, narrowed to what this climate can hold.
            let flux = world.flux(&system.star) / cosmos::SOLAR_CONSTANT_WM2;
            if !LIVEABLE_FLUX.contains(&flux) {
                continue;
            }
            // Ranked on this simulation's own criterion rather than the astronomy's.
            // `cosmos` scores the middle of the habitable zone highest, which is right as
            // astronomy and wrong here: the band this climate can hold a *breathable*
            // world in sits against the zone's inner edge, so the two criteria pull in
            // opposite directions and between them admitted almost nothing. The body and
            // the star's remaining time carry over unchanged; only the placement differs.
            let lit = 1.0 - ((flux - 1.03) / 0.12).abs().min(1.0);
            let promise = lit * cosmos::habitability::body_and_time(&system.star, &world);
            let good = promise >= WORTH_SETTLING;
            candidates.push((system, index, promise));
            // Stop once there are enough genuinely good ones to choose between. A
            // marginal world is kept as a fallback rather than as a preference — the
            // filter is a lot to ask at once, and a seed that cannot meet all of it
            // should get the best available rather than nothing.
            if good && candidates.iter().filter(|(_, _, p)| *p >= WORTH_SETTLING).count()
                >= WORLDS_TO_TRY
            {
                break;
            }
        }
        assert!(
            !candidates.is_empty(),
            "four thousand systems and not one world anybody could live on"
        );
        // Best first, so a seed whose every candidate has a difficult atmosphere still
        // gets the best of them rather than the last tried.
        candidates.sort_by(|(_, _, a), (_, _, b)| b.total_cmp(a));
        candidates.truncate(WORLDS_TO_TRY);

        let mut rng = seed.stream(Domain::Terrain, 0, 0);
        let mut planet =
            geo::Lithosphere::genesis(SETTLED_GRID, SETTLED_PLATES, SETTLED_LAND, &mut rng);
        // One step before the climate is asked anything: the carbon cycle is fed by
        // volcanism along plate boundaries, and a planet whose boundaries have not been
        // worked out yet has no volcanoes, no carbon dioxide, and freezes solid.
        planet.step_myr(4.0, &mut rng);

        // Now ask each candidate's climate what atmosphere it needs, and take the first
        // that needs one people could breathe. The flux band gets this right most of the
        // time and cannot get it right always, because a planet with less weatherable rock
        // needs more carbon dioxide at the same distance.
        let mut chosen: Option<(cosmos::System, usize, climate::Climate)> = None;
        for (system, index, _) in candidates {
            let climate = climate::Climate::around(
                &planet,
                system.star,
                system.worlds[index].semi_major_au,
                climate::insolation::EARTH_OBLIQUITY,
            );
            let air = climate.co2_ppm();
            let breathable = (GREEN_CO2_PPM..BREATHABLE_CO2_PPM).contains(&air);
            let first = chosen.is_none();
            if breathable || first {
                chosen = Some((system, index, climate));
            }
            if breathable {
                break;
            }
        }

        let (system, world, climate) = chosen.expect("a candidate was checked");
        let life = biome::Biosphere::read(&planet, &climate);
        Surface {
            planet,
            climate,
            life,
            system,
            world,
        }
    }
}

impl World {
    pub fn new(seed: WorldSeed) -> World {
        World {
            seed,
            planets: Arena::new(),
            people: Arena::new(),
            society: Society::new(),
            places: Arena::new(),
            chronicle: Chronicle::new(),
            architecture: genetics::standard_architecture(),
            pool: FounderPool::uniform(),
            scheduler: Scheduler::starting_at(FOUNDING),
            next_stream: 0,
            expecting: std::collections::BTreeSet::new(),
            was: std::collections::BTreeMap::new(),
            budget: FULL_DETAIL_BUDGET,
            detail: std::collections::BTreeMap::new(),
            watched: std::collections::BTreeSet::new(),
            arrivals: std::collections::BTreeMap::new(),
            deeds_done: std::collections::BTreeMap::new(),
            cultures: None,
            roster: Vec::new(),
            technique: std::collections::BTreeMap::new(),
            bonds: bonds::Bonds::new(),
            neighbours: std::collections::BTreeMap::new(),
            holdings: std::collections::BTreeMap::new(),
            worth: std::collections::BTreeMap::new(),
            repute: std::collections::BTreeMap::new(),
            evenings: std::collections::BTreeMap::new(),
            shouldered: std::collections::BTreeMap::new(),
            company: Vec::new(),
            acts_are_possible: true,
            people_change: true,
            people_envy: true,
            reputation_is_earned: true,
            people_think_on_their_own_means: true,
            only_the_strongest_dream: false,
            witnesses_notice: true,
            witnessed: 0,
            acted: [0; person::acts::Toward::COUNT],
            withheld: 0,
            occasions: 0,
            met_the_envied: 0,
            robbed_the_envied: 0,
            told_the_envied: 0,
            surface: None,
            founded_with: 0,
        }
    }

    /// The planet this world's people stand on, if they stand on one.
    pub fn surface(&self) -> Option<&Surface> {
        self.surface.as_ref()
    }

    pub fn now(&self) -> Time {
        self.scheduler.now()
    }

    /// How finely a place is being simulated. Unknown places default to fine, so a
    /// world that has never thought about detail behaves exactly as it did before.
    pub fn detail_of(&self, place: PlaceId) -> Detail {
        self.detail.get(&place).copied().unwrap_or(Detail::Full)
    }

    /// Change how many people may be simulated finely. Zero coarsens everything that
    /// is not explicitly watched.
    pub fn set_detail_budget(&mut self, people: usize) {
        self.budget = people;
    }

    /// Ask to see a place. It will be simulated finely regardless of budget.
    pub fn watch(&mut self, place: PlaceId) {
        let already_fine = self.detail_of(place) == Detail::Full;
        if self.watched.insert(place) && !already_fine {
            self.promote(place);
        }
    }

    /// Stop watching. The place drops back to coarse at the next reckoning if the
    /// budget is short.
    pub fn unwatch(&mut self, place: PlaceId) {
        self.watched.remove(&place);
    }

    pub fn is_watched(&self, place: PlaceId) -> bool {
        self.watched.contains(&place)
    }

    /// How many people are being simulated finely.
    pub fn full_detail_population(&self) -> usize {
        self.people
            .iter()
            .filter(|(id, p)| {
                p.is_alive()
                    && self
                        .society
                        .place_of(*id)
                        .is_none_or(|place| self.detail_of(place) == Detail::Full)
            })
            .count()
    }

    /// Decide which places are worth simulating finely, and move them.
    ///
    /// Watched places first, then the rest in a fixed order until the budget runs out.
    /// Deterministic: the order is arena order, never a hash iteration.
    fn assign_detail(&mut self) {
        let mut spent = 0usize;
        let ids: Vec<PlaceId> = self.places.ids().collect();

        let population = |world: &World, place: PlaceId| {
            world
                .society
                .households_in(place)
                .flat_map(|(_, h)| h.members.iter())
                .filter(|m| world.people.get(**m).is_some_and(|p| p.is_alive()))
                .count()
        };

        // Watched places are never denied.
        for id in &ids {
            if self.watched.contains(id) {
                spent += population(self, *id);
            }
        }

        for id in ids {
            let wanted = if self.watched.contains(&id) {
                Detail::Full
            } else {
                let here = population(self, id);
                // Nobody there is nothing to detail, whatever the budget says.
                if here == 0 {
                    Detail::Coarse
                } else if spent + here <= self.budget {
                    spent += here;
                    Detail::Full
                } else {
                    Detail::Coarse
                }
            };

            let current = self.detail_of(id);
            if wanted != current {
                self.detail.insert(id, wanted);
                if wanted == Detail::Full {
                    self.promote(id);
                }
                // Demotion needs no work: a person's next act checks the tier and
                // simply stops rescheduling itself.
            }
        }
    }

    /// Hand somebody who has been living coarsely over to the fine tier.
    ///
    /// A coarse person's clock is only stamped forward once a year, at their birthday. The
    /// fine tier's first act is `catch_up`, which accrues every need across the whole span
    /// since that stamp — so a person who crosses tiers at any other moment is billed for
    /// up to a year of hunger and thirst in one step, and then charged a year of health
    /// decline against it. It kills them, and it kills children fastest.
    ///
    /// Three ways to cross, and each had to be found separately: a place is promoted, a
    /// household moves out of a coarse quarter into a fine one, or two people pair and the
    /// new household takes the *other* one's place.
    ///
    /// `from` is where they were, and it is what makes this safe to call. Applying it to
    /// somebody already being simulated finely wipes a life mid-stride — every world
    /// collapsed to a dozen souls when this fired on every mover rather than only on the
    /// ones actually arriving from coarse ground. A person with no place at all is *not*
    /// coarse: `live_coarsely` skips them, so the fine tier has been running them all
    /// along.
    fn arrive_from_coarse(&mut self, at: Time, id: PersonId, from: Option<PlaceId>, into: PlaceId) {
        if self.detail_of(into) != Detail::Full {
            return;
        }
        if !from.is_some_and(|was| self.detail_of(was) == Detail::Coarse) {
            return;
        }
        let Some(person) = self.people.get_mut(id) else {
            return;
        };
        if !person.is_alive() {
            return;
        }
        // They have been coping, by assumption, right up to this moment. Hand them across
        // in that state rather than billing them for the assumption.
        person.get_by(at);
        self.scheduler.schedule_at(at, Task::PersonActs(id));
    }

    /// Put a place's residents back on the fine schedule.
    ///
    /// Everybody here is by definition arriving from the coarse tier, so everybody is
    /// caught up to the present before being scheduled — see `arrive_from_coarse` for why
    /// that is load-bearing rather than tidiness.
    fn promote(&mut self, place: PlaceId) {
        self.detail.insert(place, Detail::Full);
        let now = self.scheduler.now();
        let residents: Vec<PersonId> = self
            .society
            .households_in(place)
            .flat_map(|(_, h)| h.members.iter().copied())
            .collect();
        for id in residents {
            let Some(person) = self.people.get_mut(id) else {
                continue;
            };
            if !person.is_alive() {
                continue;
            }
            // They have been coping, by assumption, right up to this moment. Hand them to
            // the fine tier in that state rather than billing them for the assumption.
            person.get_by(now);
            self.scheduler.schedule_at(now, Task::PersonActs(id));
        }
    }

    /// Stop recording anything below this level. See `Chronicle::set_floor` — running
    /// for decades with every routine act retained is not affordable until compaction
    /// exists, so a long run has to say what it does not care about.
    /// Everything needed to make this world again.
    ///
    /// The save file, and the whole save file. See `provenance` for why it is five numbers
    /// rather than a serialised heap, and what that costs.
    pub fn provenance(&self) -> Provenance {
        Provenance {
            seed: self.seed,
            population: self.founded_with,
            elapsed: self.now().since(FOUNDING),
            detail_budget: self.budget,
            floor: self.chronicle.floor(),
        }
    }

    /// Make that world again.
    ///
    /// Bit-for-bit the world that was saved, because it is the same computation — which is
    /// the reproducibility guarantee cashed in rather than a new promise.
    pub fn reopen(save: &Provenance) -> World {
        let mut world = World::genesis(save.seed, save.population);
        world.record_only(save.floor);
        world.set_detail_budget(save.detail_budget);
        if save.elapsed > Duration::ZERO {
            world.run_for(save.elapsed);
        }
        world
    }

    pub fn record_only(&mut self, floor: Salience) {
        self.chronicle.set_floor(floor);
    }

    pub fn pending(&self) -> usize {
        self.scheduler.len()
    }

    /// How many people are still alive. The dead stay in the arena so their lives can
    /// still be read — death removes someone from the schedule, not from history.
    pub fn living(&self) -> usize {
        self.people.iter().filter(|(_, p)| p.is_alive()).count()
    }

    /// A stream for one purpose. Each call takes a fresh index, so adding a draw in one
    /// place cannot shift what any other place draws.
    pub fn stream(&mut self, domain: Domain) -> Rng {
        let index = self.next_stream;
        self.next_stream += 1;
        self.seed.stream(domain, index, 0)
    }

    /// A stream tied to one entity at one moment — reproducible without being shared,
    /// and independent of how the run happens to be chunked.
    fn moment_stream(&self, domain: Domain, entity: u64, at: Time) -> Rng {
        self.seed.stream(domain, entity, at.as_secs())
    }

    pub fn add_planet(&mut self, planet: Planet) -> PlanetId {
        self.planets.insert(planet)
    }

    pub fn add_person(&mut self, person: Person) -> PersonId {
        let id = self.people.insert(person);
        self.enlist(id);
        id
    }

    /// Put a person on the schedule: acting now, and aging on their own anniversary.
    fn enlist(&mut self, id: PersonId) {
        let now = self.scheduler.now();
        self.scheduler.schedule_at(now, Task::PersonActs(id));

        // Stagger anniversaries across the year so a large population does not roll
        // mortality all on one day.
        let offset = Duration::from_secs(
            (id.to_bits().wrapping_mul(2_654_435_761)) % sim_core::time::SECONDS_PER_YEAR,
        );
        self.scheduler
            .schedule_at(now + offset, Task::PersonAges(id));
    }

    /// Populate a world with an Earth-like planet, some neighbourhoods, and people.
    ///
    /// The neighbourhoods no longer start identical, and the difference between them at
    /// founding is entirely the ground: a settlement on a fertile coast is a better place
    /// to be than one on a cold plateau on the day both are founded, and nothing was
    /// authored to make it so. Everything that distinguishes them *afterwards* — which
    /// becomes the enclave, which the slum — still comes out of who ends up living in
    /// them, which is the loop that was already there.
    pub fn genesis(seed: WorldSeed, population: usize) -> World {
        let mut world = World::new(seed);
        world.founded_with = population;
        let earth = world.add_planet(Planet::earth());
        world
            .scheduler
            .schedule_at(FOUNDING, Task::PlanetAwakens(earth));

        // Make the ground first. Neither the people nor the places exist yet, so nothing
        // can have influenced where the continents ended up — which is the direction
        // causation has to run for a world to be somewhere rather than about somewhere.
        let surface = Surface::genesis(seed);
        let mut naming = seed.stream(Domain::Naming, 0, 0);
        let sites = settlement::survey(
            &surface.planet,
            &surface.climate,
            &surface.life,
            QUARTERS,
            SETTLEMENTS_APART,
            &mut naming,
        );
        world.surface = Some(surface);

        // Founding capacity is the same total housing as before, split by how good the
        // ground is rather than evenly — so the fertile coast is the big town and the
        // plateau is the hamlet, before anybody has moved anywhere.
        let total = (population.max(12) / 3).max(4) as f32 * QUARTERS as f32;
        let strength: f32 = sites.iter().map(|s| s.habitability).sum::<f32>().max(1e-6);
        for site in &sites {
            let share = (total * site.habitability / strength).round().max(4.0) as u32;
            world
                .places
                .insert(Place::on(&site.name, share, site.terrain.clone()));
        }
        // A planet with no habitable land is a possible planet. Its people have to live
        // somewhere all the same, so the abstract quarters are the fallback rather than
        // the default.
        if world.places.is_empty() {
            for name in ["Northside", "The Wharf", "Elmhurst", "Kingsfield", "Lowgate"] {
                world
                    .places
                    .insert(Place::new(name, ((population / 3).max(4)) as u32));
            }
        }
        let place_ids: Vec<PlaceId> = world.places.ids().collect();

        for i in 0..population {
            let mut rng = world.stream(Domain::Genetics);
            // Founders span the adult range, so the population has a shape from the
            // start rather than being one cohort that ages and dies together.
            let age_years = rng.range_f64(18.0, 70.0);
            let born = FOUNDING - Duration::from_secs((age_years * 31_557_600.0) as u64);
            let standing = rng.unit_f32();

            let architecture = world.architecture;
            let pool = world.pool.clone();
            let mut inhabitant = person::found(architecture, &pool, &mut rng, earth, born, 0.0);
            // Named by the people they belong to, not by a word list. At the founding
            // there is only one people and it has not diverged yet, so everybody sounds
            // like everybody — which is right, and stops being true the moment a quarter
            // goes its own way.
            let (given, family) = culture::naming::name_a_person(
                &[0.5; culture::WAYS],
                inhabitant.sex == person::Sex::Female,
                None,
                &mut rng,
            );
            inhabitant.name = format!("{given} {family}");
            // Their life before the world started was never simulated; do not bill
            // them for it, and treat their upbringing as already behind them.
            inhabitant.assume_settled(FOUNDING);
            inhabitant.set_standing(standing);
            inhabitant.mature();

            let id = world.add_person(inhabitant);
            let home = world.society.found_household(FOUNDING, 0.0);
            world.society.move_in(id, home);
            // Spread the founders around; sorting takes it from here.
            let quarter = place_ids[i % place_ids.len()];
            world.society.settle(home, quarter);
        }

        // The first reckoning is a year in, once there is a year to read.
        world
            .scheduler
            .schedule_at(FOUNDING + Duration::from_years(1), Task::Reckoning);
        world
    }

    /// Advance to `horizon`, running everything due at or before it.
    pub fn run_until(&mut self, horizon: Time) {
        while let Some((at, task)) = self.scheduler.next_event_until(horizon) {
            self.run_task(at, task);
        }
        self.scheduler.advance_to(horizon);
    }

    pub fn run_for(&mut self, span: Duration) {
        self.run_until(self.now() + span);
    }

    fn run_task(&mut self, at: Time, task: Task) {
        match task {
            Task::PlanetAwakens(id) => {
                let Some(planet) = self.planets.get(id) else {
                    return;
                };
                let next = planet.next_phase_change(at);
                self.chronicle
                    .record(at, Salience::Epochal, Happening::WorldBegins { planet: id });
                self.scheduler.schedule_at(next, Task::PlanetPhase(id));
            }

            Task::PlanetPhase(id) => {
                let Some(planet) = self.planets.get(id) else {
                    return;
                };
                let phase = planet.phase_at(at);
                let next = planet.next_phase_change(at);
                self.remember(
                    at,
                    Salience::Routine,
                    Happening::PhaseBegins { planet: id, phase },
                );
                self.scheduler.schedule_at(next, Task::PlanetPhase(id));
            }

            Task::PersonActs(id) => self.person_acts(at, id),
            Task::PersonAges(id) => self.person_ages(at, id),
            Task::Birth { mother, father } => self.birth(at, mother, father),
            Task::Reckoning => self.reckoning(at),
        }
    }

    /// The situation one person is actually in, right now.
    ///
    /// Exactly what the simulation hands them when they decide, built the same way from
    /// the same places — so an observer asking "why is she doing that" is answering about
    /// the world she is in rather than about a hypothetical neutral one. The four
    /// channels are in it, and the first of them is a *gate*: without this, an observer
    /// constructing its own plain situation would report that a child had merely ranked
    /// work poorly, when in fact work was never on offer.
    pub fn situation_for(&self, id: PersonId) -> Option<Situation> {
        let at = self.now();
        let person = self.people.get(id)?;
        let planet = self.planets.get(person.home)?;
        let phase = planet.phase_at(at);
        let dependent = person.stage(at).is_dependent();

        let mut env = self
            .society
            .place_of(id)
            .and_then(|p| self.places.get(p))
            .map(|place| place.env.surroundings(dependent))
            .unwrap_or_else(person::Surroundings::neutral);
        env.stress = (env.stress + person.needs().total_pressure()).clamp(0.0, 1.0);
        Some(Situation { phase, env })
    }

    fn person_acts(&mut self, at: Time, id: PersonId) {
        // A person whose handle no longer resolves, or whose world is gone, simply
        // stops being scheduled. The lookup failing is the mechanism.
        let Some(home) = self.people.get(id).map(|p| p.home) else {
            return;
        };
        let Some(planet) = self.planets.get(home) else {
            return;
        };
        let phase = planet.phase_at(at);

        let where_they_live = self.society.place_of(id);
        if where_they_live.is_some_and(|p| self.detail_of(p) == Detail::Coarse) {
            // Demoted while this was queued. Not rescheduling is the demotion.
            return;
        }
        let dependent = self
            .people
            .get(id)
            .is_some_and(|p| p.stage(at).is_dependent());

        // The four channels, filled from the neighbourhood they actually live in.
        let mut env = where_they_live
            .and_then(|p| self.places.get(p))
            .map(|place| place.env.surroundings(dependent))
            .unwrap_or_else(person::Surroundings::neutral);

        let (job_opportunity, schooling) = where_they_live
            .and_then(|p| self.places.get(p))
            .map(|place| (place.env.job_opportunity, place.env.education_access))
            .unwrap_or((0.5, 0.5));
        // What their trade is worth here, against what a trade is worth here on average.
        let paid = self.pay_for(id);

        let mut rng = self.moment_stream(Domain::Behavior, id.to_bits(), at);
        let Some(subject) = self.people.get_mut(id) else {
            return;
        };
        if !subject.is_alive() {
            return;
        }

        // Their own unmet need adds to whatever the neighbourhood already imposes.
        env.stress = (env.stress + subject.needs().total_pressure()).clamp(0.0, 1.0);
        let situation = Situation { phase, env };

        // Only people who were here before the simulation started introduce themselves.
        // Everyone since has a birth in the chronicle already, and a newborn announcing
        // that they are nought years old and level-headed reads as nonsense.
        let first = subject.first_sighting() && subject.parents.is_none();
        let finished = subject.settle_intent_only(at);
        let outcome = subject.step(at, &situation, &mut rng);
        let death = subject.death();

        // The tally of a life, kept as it happens. Free here — the person is already
        // borrowed — and a whole social position is read off it later.
        if let Some(done) = finished {
            subject.spent(done, 1);
        }

        // Work pays where there is work worth having — the same channel that decided
        // whether it was on offer decides what it returns.
        if finished == Some(Deed::Work) {
            // How much a spell of work is worth depends on the person as well as the
            // place. Without that, everyone with the same neighbourhood converges on
            // the same standing and there is no spread for sorting to act on — the
            // whole world settles into one indistinguishable suburb.
            //
            // The two terms are the two inheritances: conscientiousness comes down
            // through the genome, schooling through the neighbourhood a child grew up
            // in. Advantage passes along both, and along the transfer at birth.
            let diligence = (0.6 + TEMPERAMENT_AT_WORK * subject.personality.conscientiousness).clamp(0.2, 2.0);
            let taught = 0.5 + schooling;
            subject.earn(
                WORK_GAIN * job_opportunity * diligence * taught * subject.patronage() * paid,
            );
            // And a share of anything above keeping the household going is put by. Both tiers
            // save, because a mechanism wired into one of them is a mechanism that fires only
            // when somebody is looking — which is §21.1's fault in a new place, and was
            // exactly what happened here: this was in `live_coarsely` alone, so in a world
            // watched closely enough to measure, **nobody owned anything at all**.
            let spare = subject.standing() - SUBSISTENCE_STANDING;
            subject.put_by(spare / WORK_SPELLS_PER_YEAR);
        }

        // An evening in company is spent *with somebody*, and which somebody is the whole
        // of §25. Counted here and settled at the reckoning, all at once — see
        // `COMPANY_A_YEAR` for why it is not settled here.
        if finished == Some(Deed::Socialize) {
            *self.evenings.entry(id).or_insert(0) += 1;
        }

        if first {
            self.remember(
                at,
                Salience::Pivotal,
                Happening::PersonArrives { person: id },
            );
        }

        match outcome {
            Some(choice) => {
                if let Some(place) = where_they_live {
                    let counts = self.deeds_done.entry(place).or_insert([0; Deed::COUNT]);
                    counts[choice.deed as usize] += 1;
                }
                self.remember(
                    at,
                    Salience::Routine,
                    Happening::PersonDoes {
                        person: id,
                        deed: choice.deed,
                    },
                );
                self.scheduler
                    .schedule_at(at + choice.deed.duration(), Task::PersonActs(id));
            }
            None => {
                // Dying between birthdays. This has to go through the same route as
                // any other death: recording it and leaving the body partnered was
                // how a widow stayed married to a corpse until the next birthday
                // came round to notice — and if the run ended first, forever.
                if let Some((_, cause)) = death {
                    self.record_death(at, id, cause);
                }
            }
        }
    }

    fn person_ages(&mut self, at: Time, id: PersonId) {
        // Before anything else. Someone nobody is watching has not been feeding
        // themselves in the simulation, so catching them up first would charge them a
        // year of unrelieved hunger and kill them — which is exactly what it did, and
        // it emptied the world within a generation.
        self.live_coarsely(at, id);

        // Someone who died between birthdays, by whatever route. Let them go properly
        // and stop scheduling them.
        if self.people.get(id).is_some_and(|p| !p.is_alive()) {
            self.release(id);
            return;
        }

        // What the ground failed to grow, felt by the people standing on it. Applied to
        // everybody once a year, at both tiers alike — a hunger that only reached the
        // people somebody happened to be watching would be the same class of bug as the
        // one `arrive_from_coarse` exists to prevent.
        let want = self
            .society
            .place_of(id)
            .and_then(|place| self.places.get(place))
            .map(|place| place.want)
            .unwrap_or(0.0);
        // Plus whatever they took off somebody else's shoulders since their last birthday.
        let want = want + self.shouldered.remove(&id).unwrap_or(0.0);
        // Less whatever their own friends will take off theirs.
        let want = self.share_the_shortfall(id, want);

        let mut rng = self.moment_stream(Domain::Demography, id.to_bits(), at);
        let mut settling_up = false;
        let Some(subject) = self.people.get_mut(id) else {
            return;
        };
        if !subject.is_alive() {
            return;
        }

        if want > 0.0 {
            subject.go_hungry(want, at);
        } else {
            // The land is feeding them again. Lift the ceiling, and let the ordinary
            // recovery do the rest at its own pace — a famine that ends does not restore
            // anybody instantly.
            subject.eat_well();
            settling_up = true;
        }
        if !subject.is_alive() {
            self.record_death(at, id, Cause::Deprivation);
            self.release(id);
            return;
        }

        subject.catch_up(at);
        let cause = subject
            .survive(at, Duration::from_years(1), &mut rng)
            .or_else(|| {
                subject
                    .death()
                    .map(|(_, c)| c)
                    .filter(|_| !subject.is_alive())
            });

        match cause {
            Some(cause) => self.record_death(at, id, cause),
            None => {
                if let Some(person) = self.people.get_mut(id)
                    && !person.stage(at).is_dependent()
                {
                    // Adults only. A child cannot work — the option is gated off — so
                    // decaying their standing too meant every generation arrived at
                    // adulthood with nothing, and the whole society ratcheted to zero
                    // within a few generations. What a child has is their family's
                    // backing, and that does not erode because they are young.
                    person.slip(STANDING_DECAY);
                }
                // A good year is when a debt gets made good, because it is the only year
                // anybody has anything to spare.
                if settling_up {
                    self.settle_debts(id);
                }
                // What the year did to who they are. Before the year's decisions rather than
                // after, so somebody hardened by last winter meets this spring as the person
                // that made them.
                if self.people_change
                    && let Some(person) = self.people.get_mut(id)
                {
                    person.weather(at);
                }
                self.roll_fortune(at, id);
                self.seek_patron(at, id);
                self.seek_partner(at, id);
                self.try_conceive(at, id);
                self.scheduler
                    .schedule_at(at + Duration::from_years(1), Task::PersonAges(id));
            }
        }
    }

    /// Put something in the chronicle, filed under everyone it concerns.
    fn remember(&mut self, at: Time, salience: Salience, kind: Happening) {
        let about = kind.subjects();
        self.chronicle.record_about(at, salience, kind, about.as_slice());
        self.let_them_remember(at, kind);
    }

    /// And the people it happened to carry it themselves.
    ///
    /// The chronicle is the world's record: complete, ordered, external, and nobody in the
    /// world can consult it. This is the other half — what one person has of their own life,
    /// which is partial and fades. The two are written from the same call because they are
    /// the same events seen from different sides, and because a personal memory that had to
    /// be filled separately would drift out of step with the record of the same moment.
    ///
    /// Who remembers what is not symmetric. A man robbed remembers being robbed; the raiders
    /// remember nothing, since a raid is a Tuesday to them. A child does not remember being
    /// born but its parents remember the birth. Those asymmetries are the whole reason this
    /// is written by hand per happening rather than derived from `subjects()`.
    fn let_them_remember(&mut self, at: Time, kind: Happening) {
        use person::memory::What;
        let keep = |world: &mut World, who: PersonId, what: What, about: Option<PersonId>| {
            if let Some(person) = world.people.get_mut(who) {
                if person.is_alive() {
                    person.keep(what, about, at);
                }
            }
        };
        match kind {
            // The parents remember the birth. The child does not remember being born, which
            // is true and saves the commonest memory in the world from being the emptiest.
            Happening::PersonBorn { child, mother, father } => {
                keep(self, mother, What::Born, Some(child));
                keep(self, father, What::Born, Some(child));
            }
            // Everybody who knew them, weighted by how well — a death is the heaviest thing
            // in this model and the only one that reaches people who were merely acquainted.
            Happening::PersonDies { person, .. } => {
                let mourners: Vec<PersonId> = self
                    .bonds
                    .of(person)
                    .filter(|(_, tie)| tie.known > 0.35)
                    .map(|(who, _)| who)
                    .collect();
                for who in mourners {
                    keep(self, who, What::Died, Some(person));
                }
            }
            Happening::PersonPairs { person, with } => {
                keep(self, person, What::Paired, Some(with));
                keep(self, with, What::Paired, Some(person));
            }
            // Both sides, because being taken up and taking somebody up are both things a
            // life is organised around — §25 calls the first the largest single fact in one.
            Happening::PersonMentored { person, by } => {
                keep(self, person, What::TakenUp, Some(by));
                keep(self, by, What::TakenUp, Some(person));
            }
            Happening::PersonWorksItOut { person, .. } => {
                keep(self, person, What::WorkedItOut, None);
            }
            Happening::PersonMoves { person, .. } => {
                keep(self, person, What::Moved, None);
            }
            // Only the robbed. A raid is a Tuesday to the raiders and a year to remember for
            // everybody it was done to, and that asymmetry is most of what makes it a wrong.
            Happening::PlaceTaken { place, .. } => {
                let robbed: Vec<PersonId> = self
                    .society
                    .households_in(place)
                    .flat_map(|(_, h)| h.members.iter().copied())
                    .collect();
                for who in robbed {
                    keep(self, who, What::Robbed, None);
                }
            }
            // Both sides of a deliberate act, and which side gets what is the whole of it.
            // A wrong is kept by whoever did it *and* by whoever it was done to — the first
            // is conscience, which needs no witness, and the second is a grudge. A kindness
            // is kept only by whoever received it: being given to is a thing you remember
            // about somebody, and giving is a Tuesday.
            Happening::PersonActsOn { person, toward, act } => {
                if act.harm() > 0.0 {
                    keep(self, person, What::DidWrong, Some(toward));
                    // `keep` passes over the dead, so a killing leaves no second memory —
                    // which is right, and is why the killer is the only person in the world
                    // who knows what happened.
                    keep(self, toward, What::Wronged, Some(person));
                } else {
                    keep(self, toward, What::Carried, Some(person));
                }
            }
            _ => {}
        }
    }

    /// Forget enough of the small and old to stay inside the budget.
    ///
    /// Called on the caller's schedule rather than automatically, because compaction
    /// moves every record and therefore has to rebuild the index — which is cheap
    /// occasionally and ruinous every step.
    pub fn compact_chronicle(&mut self, budget: usize) -> usize {
        self.chronicle.compact(budget, Salience::Pivotal)
    }

    /// Everything the chronicle holds about one person — their life, as recorded.
    pub fn life_of(&self, person: PersonId) -> impl Iterator<Item = &sim_core::Record<Happening>> {
        self.chronicle.about(person.to_bits())
    }

    /// Detach someone from the living: no partner, no household.
    ///
    /// Idempotent, and deliberately separate from recording the death. There is more
    /// than one way to die here — the mortality roll at a birthday, deprivation noticed
    /// while acting — and rather than prove every route remembers to do this, the
    /// invariant is re-established wherever a corpse is next encountered. Reasoning
    /// about which path fires is how widows ended up married to the dead.
    fn release(&mut self, id: PersonId) {
        self.society.separate(id);
        self.society.move_out(id);
    }

    /// Everything that has to happen when someone dies.
    fn record_death(&mut self, at: Time, id: PersonId, cause: Cause) {
        self.settle_the_estate(id);
        self.release(id);
        self.remember(
            at,
            Salience::Pivotal,
            Happening::PersonDies { person: id, cause },
        );
    }

    /// What the dead leave, and who receives it.
    ///
    /// Divided equally among the children, which is a **kinship rule** — and worth saying so
    /// rather than letting it pass as arithmetic. §24.4 puts kinship rules out of scope, and
    /// this is the point at which that becomes impossible to maintain: the moment anything
    /// outlives its owner, somebody has to receive it, and every way of choosing is a rule
    /// about kin. Partible division among all children is the one that assumes least — it
    /// needs no notion of seniority, no sex, and no marriage — but a world of primogeniture
    /// would concentrate estates instead of dispersing them and would be a different world.
    /// That choice is now on the table whether or not anybody wanted it there.
    ///
    /// Nothing is created in the passing. What is not inherited is not destroyed either — a
    /// person who dies childless simply stops holding anything, which is the closest this
    /// model has to an estate returning to the common stock.
    fn settle_the_estate(&mut self, id: PersonId) {
        let estate = match self.people.get(id) {
            Some(person) if person.estate() > 0.0 => person.estate(),
            _ => return,
        };
        let heirs: Vec<PersonId> = self
            .society
            .children_of(id)
            .iter()
            .copied()
            .filter(|child| self.people.get(*child).is_some_and(|p| p.is_alive()))
            .collect();
        if heirs.is_empty() {
            return;
        }
        let share = estate / heirs.len() as f32;
        for heir in heirs {
            if let Some(person) = self.people.get_mut(heir) {
                person.inherit(share);
            }
        }
    }

    /// A whole year, for someone nobody is watching.
    ///
    /// The projection from fine to coarse. It has to produce the year the fine
    /// simulation would have produced — the same expected work, the same standing, a
    /// person who fed themselves — because a population that drifts while unobserved is
    /// a population you cannot trust when you look back at it.
    ///
    /// What it does not produce is the texture: no per-deed events, no scoring, no
    /// thousands of decisions. That is the whole saving.
    fn live_coarsely(&mut self, at: Time, id: PersonId) {
        let Some(place) = self.society.place_of(id) else {
            return;
        };
        if self.detail_of(place) == Detail::Full {
            return;
        }
        let Some(env) = self.places.get(place).map(|p| p.env.clone()) else {
            return;
        };
        let paid = self.pay_for(id);

        let Some(person) = self.people.get_mut(id) else {
            return;
        };
        if !person.is_alive() {
            return;
        }

        person.get_by(at);
        if person.stage(at).is_dependent() {
            return;
        }

        // How much work a year holds here, and what each spell is worth. The same terms
        // the fine tier applies one at a time.
        let surroundings = env.surroundings(false);
        let spells = WORK_SPELLS_PER_YEAR * surroundings.availability[Deed::Work as usize];
        let diligence = (0.6 + TEMPERAMENT_AT_WORK * person.personality.conscientiousness).clamp(0.2, 2.0);
        let taught = 0.5 + env.education_access;
        let gain =
            WORK_GAIN * env.job_opportunity * diligence * taught * person.patronage() * paid;
        person.earn_repeatedly(gain, spells);
        // And what a good year left over. Standing above what it costs to keep a household
        // going is the surplus; a tenth of it is put by. Below that there is nothing to save
        // and the estate does not move, which is what makes this a channel for advantage
        // rather than a second name for having worked.
        person.put_by(person.standing() - SUBSISTENCE_STANDING);

        // And a year of company. Unwatched people still have neighbours: if their ties
        // stood still while a watched person's grew, then who your friends are would depend
        // on who the observer happened to be looking at — the same fault that once had the
        // observer setting the death rate.
        //
        person.spent(Deed::Work, spells.round().max(0.0) as u32);

        // And the rest of the year. What somebody spends it on is the one thing the coarse
        // tier has to guess at, since nobody deliberated — guessed with the fine tier's own
        // expressions for how much a temperament wants a thing and what a place returns for
        // it, rather than new ones. An extravert is more sociable unwatched for exactly the
        // reason they are more sociable watched.
        for deed in Deed::CHOSEN {
            let base = SPELLS_A_YEAR[deed as usize];
            if base <= 0.0 {
                continue;
            }
            let appetite = deed.appeal(&person.personality, &person.values)
                * surroundings.payoff[deed as usize]
                * surroundings.availability[deed as usize];
            let times = (base * appetite).round().max(0.0) as u32;
            person.spent(deed, times);
            if deed == Deed::Socialize {
                *self.evenings.entry(id).or_insert(0) += times;
            }
        }
    }

    /// What a year in somebody's trade returns, against a year in an ordinary one.
    ///
    /// The place's own price for what they make, divided by what the trades there are worth
    /// on average. A smith where tools are wanted does well; the same smith where nobody
    /// wants tools does not, and that is the signal that eventually moves them.
    ///
    /// Clamped, because this multiplies a figure §15 and §21 already calibrated. An unbounded
    /// term here would be a second `WORK_GAIN` wearing a different hat.
    fn pay_for(&self, who: PersonId) -> f32 {
        let Some(place) = self.society.place_of(who) else {
            return 1.0;
        };
        let (Some(worth), Some(person)) = (self.worth.get(&place), self.people.get(who)) else {
            return 1.0;
        };
        let typical: f32 = worth.iter().sum::<f32>() / economy::Trade::COUNT as f32;
        if typical <= 1e-6 {
            return 1.0;
        }
        (worth[person.trade() as usize] / typical).clamp(*PAY_BY_TRADE.start(), *PAY_BY_TRADE.end())
    }

    /// An evening in company, and everything that follows from whose company it was.
    ///
    /// `Deed::Socialize` used to relieve a need and name nobody, so people in this world
    /// socialised alone. This is the repair, and it is the join between the two halves of
    /// the simulation: a deed chosen by one person's utilities becomes an edge in a graph
    /// that the rest of the world is then read off — who backs whom, who is owed what, who
    /// stands with whom when there is not enough good land.
    ///
    /// `evenings` is how many the one call stands for: one in the fine tier, a season's
    /// worth in the coarse. Everything below is per-evening and scales with it, so the two
    /// tiers make the same friendships out of the same year.
    fn spend_an_evening(&mut self, id: PersonId, rng: &mut Rng, evenings: u32, evening: u32) {
        let Some(place) = self.society.place_of(id) else {
            return;
        };
        let at = self.now();

        let mut to_hand = std::mem::take(&mut self.company);
        to_hand.clear();
        // The people you already know, who are still here. Kept whole rather than sampled:
        // your friends are not a thing you have to be reminded of.
        for (other, tie) in self.bonds.of(id) {
            if tie.holds() && self.society.place_of(other) == Some(place) {
                to_hand.push(other);
            }
        }
        // And a few faces out of the crowd, because a society in which nobody ever met
        // anybody new would have had no way to start.
        if let Some(here) = self.neighbours.get(&place)
            && !here.is_empty()
        {
            for _ in 0..NEIGHBOURS_CONSIDERED {
                let which = rng.range_u64(0, here.len() as u64 - 1) as usize;
                to_hand.push(here[which]);
            }
        }
        to_hand.sort_unstable();
        to_hand.dedup();
        let chosen = self.bonds.choose_company(id, &to_hand, rng);
        self.company = to_hand;

        let Some(other) = chosen else {
            return;
        };
        let (Some(one), Some(two)) = (self.people.get(id), self.people.get(other)) else {
            return;
        };
        if !two.is_alive() {
            return;
        }
        let suits = bonds::suits(&one.personality, &two.personality);
        // What each of them is visibly making of their life, on regard's -1 to 1 scale (§42.4).
        //
        // Saturating rather than scaled, because `means()` has no ceiling — it is
        // `standing + estate * WORTH_AT_A_DOOR` and runs past 1.9 — and dividing an unbounded
        // quantity by a guessed maximum is the error §36.6 spent three rounds on. This form
        // needs no maximum, only a middle, and the middle is measured: the median adult's
        // means is 0.70, so the median adult reads as nothing in particular.
        let now = self.now();
        let whole_life = life::Mortality::HUMAN.median_lifespan();
        let worth = |p: &person::Person| {
            let means = p.means().max(0.0);
            // Against what is normal **for somebody their age**, not against one fixed middle.
            // A twenty-year-old with little is not thought badly of for it and a sixty-year-old
            // with the same is, and a single middle is therefore a tax on being young: it made
            // `regard` a second name for wealth and cost patronage 40% of its cases, because
            // the people who seek a patron are by construction the people who have least.
            let through = (p.age(now).years() / whole_life).clamp(0.0, 1.0) as f32;
            let par = A_LIVING_STARTING_OUT + A_LIVING_BY_THE_END * through;
            means / (means + par) * 2.0 - 1.0
        };
        let rated = self
            .reputation_is_earned
            .then(|| [worth(one), worth(two)]);

        self.bonds.meet_repeatedly(id, other, suits, rated, evenings);
        // What each takes away about everybody else. Both directions, because both of them
        // were there — and this is the only channel in the simulation by which a fact about
        // one person reaches somebody who has never met them.
        self.bonds.hearsay_repeatedly(id, other, evenings);
        self.bonds.hearsay_repeatedly(other, id, evenings);
        // Being with somebody keeps what you hold about them sharp — which is why the
        // brother across the square is never forgiven and the one who moved away is.
        //
        // Once a year, not once an evening. `rehearse` halves the age of what it touches, so
        // sixteen of them a year would make anything held about a neighbour permanently new
        // and no grudge against anybody still alive would ever soften — which is the
        // opposite of the claim `memory` makes and would have been invisible in every
        // aggregate this project measures.
        if evening == 0 {
            if let Some(person) = self.people.get_mut(id) {
                person.rehearse(other, at);
            }
            if let Some(person) = self.people.get_mut(other) {
                person.rehearse(id, at);
            }
        }
        // And then one of them may do something about the other. On its *own* stream, not
        // this evening's: `spend_an_evening` draws from `rng` to pick company, and a single
        // extra draw here would reseed every choice made in the world after it. The first
        // measurement of this vocabulary reported migration up 39% and a third of the
        // smiths gone, and most of that was not the mechanism — it was the shift. A
        // mechanism that cannot be switched off without moving everything else cannot be
        // ablated, and §31.2 is the whole method.
        let mut theirs = self.moment_stream(
            Domain::Behavior,
            id.to_bits() ^ other.to_bits().rotate_left(17) ^ ((evening as u64) << 43) ^ 0xac_75,
            at,
        );
        self.act_toward(id, other, at, evening, &mut theirs);
    }

    /// What one person deliberately does to another, on an evening they spend together.
    ///
    /// The whole of `person::acts`, joined to a world. This function's only job is to gather
    /// what the choice is allowed to see, hand it over, and carry out the answer — and the
    /// gathering is written out longhand rather than passing `&Person` because a scorer that
    /// can reach the whole world is a scorer that will eventually read something nobody
    /// thought it could.
    ///
    /// **Why here and not in `Deed::ALL`.** Acts are aimed at a person and deeds are not, so
    /// the evening is where they belong on the merits. It is also the only place they *can*
    /// go without repricing everything else: deeds are chosen by softmax over relative
    /// scores, so an eighth deed re-normalises the other seven, and the one time that was
    /// tried it moved migration by 64% and was reverted (§26.11). Five acts scored
    /// independently move nothing.
    fn act_toward(&mut self, who: PersonId, other: PersonId, at: Time, evening: u32, rng: &mut Rng) {
        use person::acts::{Actor, Subject, Toward};

        if !self.acts_are_possible {
            return;
        }
        let Some(place) = self.society.place_of(who) else {
            return;
        };
        let (Some(here), Some(shortfall)) = (
            self.places.get(place).map(|p| p.env.norms),
            self.places.get(place).map(|p| p.want),
        ) else {
            return;
        };
        // What this place expects of people — which is not what the actor thinks it expects.
        // See `withheld`; the gap between the two is the whole of the migrant's problem.
        let expected_here = person::acts::what_is_expected(&here);

        // Who needs them. Taken before the borrow below, and the strongest single thing
        // anybody in this world has to lose.
        let dependents = self
            .society
            .children_of(who)
            .iter()
            .filter(|child| {
                self.people
                    .get(**child)
                    .is_some_and(|p| p.is_alive() && p.stage(at).is_dependent())
            })
            .count();
        let tie = self.bonds.tie(who, other);

        // What they are after, read afresh — see `person::dreams`. Taken before the borrows
        // below because it walks the society and the ties, and returned as a value so the
        // scoring cannot go looking for anything else.
        let mut come_to = self.what_they_have_come_to(who);
        // Who they measure themselves against is read whether or not the mechanism is on,
        // because it is also the *denominator* — the rate that says whether envy aims has to
        // be measured against the same evenings in both worlds, and a switch that took the
        // observation away with the mechanism would leave nothing to compare. So the reading
        // is kept and only what anybody does with it is switched off.
        let envied = come_to.as_ref().and_then(|it| it.envied).map(|envy| envy.of);
        if !self.people_envy {
            if let Some(it) = come_to.as_mut() {
                it.envied = None;
            }
        }
        let envies = self.people_envy.then_some(envied).flatten();
        let longing = come_to
            .as_ref()
            .zip(self.people.get(who))
            .map(|(come_to, p)| person::dreams::longings(p, come_to, at))
            .unwrap_or_default();
        // And the old behaviour, kept switchable — see `only_the_strongest_dream`.
        let longing = if self.only_the_strongest_dream {
            let mut only = [0.0; person::dreams::Dream::COUNT];
            if let Some(at) = longing
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .map(|(at, _)| at)
            {
                only[at] = longing[at];
            }
            only
        } else {
            longing
        };

        let (Some(one), Some(two)) = (self.people.get(who), self.people.get(other)) else {
            return;
        };
        // Children do not do these things to people. Not a claim about children — a claim
        // about this model, in which nobody under maturity has standing, an estate, a trade,
        // or anything much to give.
        if !one.has_matured() || !one.is_alive() || !two.is_alive() {
            return;
        }
        let whole_life = life::Mortality::HUMAN.median_lifespan();
        let ahead = |p: &Person| ((whole_life - p.age(at).years()) / whole_life).clamp(0.0, 1.0);
        let actor = Actor {
            values: &one.values,
            personality: &one.personality,
            held: one.held(),
            means: one.means(),
            want: shortfall,
            dependents,
            health: one.health().vitality,
            life_ahead: ahead(one) as f32,
            has_a_trade: one.has_matured(),
            own_ways: person::acts::what_is_expected(one.norms()),
            envies,
            dreams: longing,
        };
        let subject = Subject {
            who: other,
            warmth: tie.warmth,
            regard: tie.regard,
            debt: tie.debt,
            known: tie.known,
            means: two.means(),
            want: shortfall,
            age_years: two.age(at).years(),
            matured: two.has_matured(),
        };
        let appetite = person::acts::weigh(&actor, &subject, at);
        let missed = person::acts::withheld(&actor, &subject, expected_here);
        // What the actor's *own* upbringing said was owed, kept before the borrow ends.
        let by_their_lights = actor.own_ways;
        let chosen = person::acts::choose(&appetite, AN_OCCASION, rng);

        // Whether this evening was spent with the one person they measure themselves against,
        // counted whatever comes of it. The denominator has to be every occasion and not
        // every act, because the question envy answers is *who gets robbed* — and a rate whose
        // denominator is only the evenings something happened has already thrown away the
        // evenings nothing did, which is most of them.
        let facing_the_envied = envied == Some(other);
        self.occasions += 1;
        self.met_the_envied += u64::from(facing_the_envied);
        if facing_the_envied
            && longing[person::dreams::Dream::WhatTheyHave as usize]
                > person::dreams::WORTH_WANTING
        {
            self.told_the_envied += 1;
        }

        let did = match chosen {
            Some(act) if self.carry_out(who, other, act, at) => Some(act),
            // An act somebody decided on and could not manage is not an act. Giving is the
            // case that arises: the appetite is scored against everything somebody has, and
            // the gift comes out of their standing alone, so a person whose worth is all
            // estate can want to give and have nothing at hand to give with. Counting that
            // as a gift put the world's tally two ahead of the chronicle's, which is how it
            // was found — and *is* the reason for counting it twice.
            _ => None,
        };
        if facing_the_envied && did == Some(Toward::Rob) {
            self.robbed_the_envied += 1;
        }

        // And sometimes doing nothing is the thing that was done. This is the only wrong in
        // the world whose weight is local: the same shrug beside the same poor neighbour is
        // nothing in one valley and a disgrace in the next, because a people that lives in
        // each other's company has a claim on its members that a scattered one does not.
        //
        // Assessed **once a year**, not on each of sixteen evenings, and that is not a
        // tuning. Not helping somebody is a standing state rather than an event: counting it
        // once per evening makes the same failure sixteen wrongs, which is both false and
        // what produced twenty-four thousand of them in three worlds on the first run.
        if evening == 0 && did != Some(Toward::Give) && missed > WITHHOLDING_NOTICED {
            self.nobody_helped(who, other, by_their_lights, at);
        }
    }

    /// Carry out an act, and record it.
    ///
    /// What each act *costs* is the part worth reading. Giving and robbing both move
    /// something from one person to another and create nothing; teaching costs the teacher a
    /// share of a year's ground and gives the pupil rather more than it took, because that
    /// is what teaching is; shunning costs nothing and is therefore the cheapest thing in
    /// this vocabulary, which is why societies reach for it first.
    /// Returns whether it actually came about. The tally and the chronicle are both written
    /// from the end of this function so that they cannot disagree, and an act that turned out
    /// to be impossible is written to neither.
    fn carry_out(
        &mut self,
        who: PersonId,
        other: PersonId,
        act: person::acts::Toward,
        at: Time,
    ) -> bool {
        use person::acts::Toward;
        match act {
            Toward::Give => {
                // A gift is measured in **days of somebody's work**, because that is the unit
                // debts are kept in and the only unit in which a favour has a size.
                //
                // It was first a share of the giver's standing, and that was out by two
                // orders of magnitude. A day of work is worth `WORK_GAIN` of standing, so a
                // fifteenth of a comfortable person's position is *hundreds* of days — and
                // `Bonds::helped` warms the receiver by a twentieth per day. Every gift
                // arrived as instant devotion and left a debt nobody could ever clear, the
                // tie graph stopped meaning anything, and in one of eight worlds every
                // household ended up in a single quarter, which is §30.5's collapse.
                let gift = GIFT_IN_DAYS * WORK_GAIN;
                let enough = self
                    .people
                    .get(who)
                    .is_some_and(|p| p.standing() - gift >= SUBSISTENCE_STANDING);
                if !enough {
                    return false;
                }
                // An exact transfer. Nothing is created by kindness any more than it is by a
                // raid — what changes is who has it.
                if let Some(giver) = self.people.get_mut(who) {
                    let held = giver.standing();
                    giver.set_standing(held - gift);
                }
                // Into what they *own*, not into what they can do. Standing is a capacity and
                // it decays at `STANDING_DECAY` a year, so a gift booked as standing lifts a
                // marginal household over an admission bar and then lets it fall back —
                // §31.1's first rule, that a decision read afresh from a moving quantity
                // oscillates unless something damps it. It showed up as churn: 10.7% of moves
                // going straight back, against §30.4's bar of a tenth. An estate does not
                // decay, so what somebody is given stays given.
                if let Some(taker) = self.people.get_mut(other) {
                    taker.inherit(gift);
                }
                // And it goes on the ledger, because a gift in this world is a favour and
                // reciprocity is what decides what the two of them come to think of each
                // other.
                self.bonds.helped(who, other, GIFT_IN_DAYS);
            }
            Toward::Teach => {
                // On the scale `absorb` is written in, which is **not** the scale standing is
                // written in. `Environment::upbringing` is `(quality - 0.5) * 2.5`: signed,
                // centred on nothing-special, running about -1.25 to 1.25. Standing runs 0 to
                // 1 and averages near 0.4, so handing it over raw made every lesson a strong
                // *positive* shock to a quantity centred on zero — and being taught by a
                // middling neighbour counted as a better childhood than being raised in the
                // best quarter in the world.
                //
                // The same mistake as booking a gift in days of famine and as scoring one act
                // as a product against another as a sum: two quantities that are not on a
                // common scale, used as though they were. It cost six points of §15's
                // shared-environment share and took that band below its floor.
                let worth = self
                    .people
                    .get(who)
                    .map(|p| (p.standing() - 0.5) * 2.5)
                    .unwrap_or(0.0);
                if let Some(teacher) = self.people.get_mut(who) {
                    // Time given to somebody else is time not spent on your own ground.
                    teacher.slip(TEACHING);
                }
                if let Some(pupil) = self.people.get_mut(other) {
                    let age = pupil.age(at).years();
                    // Into the upbringing, which is where a lesson goes — and `absorb` is a
                    // no-op after maturity, which is why `weigh` will not offer this act
                    // toward a grown person at all. It deliberately does *not* hand over
                    // standing: standing is what somebody's own hands are worth and cannot
                    // be given, and a version that gave it moved migration more than every
                    // other act in this vocabulary put together.
                    pupil.absorb(worth, age, TEACHING_YEARS);
                }
            }
            Toward::Shun => {
                self.bonds.cut(who, other, SHUNNING);
            }
            Toward::Rob => {
                let taken = self
                    .people
                    .get_mut(other)
                    .map(|p| p.plundered(BY_HAND))
                    .unwrap_or(0.0);
                if let Some(thief) = self.people.get_mut(who) {
                    thief.inherit(taken);
                }
                self.bonds.wronged(other, who, Toward::Rob.harm());
            }
            Toward::Kill => {
                // Recorded first, so the death is in the record before the killing is. A
                // person who is dead cannot keep a memory of being wronged, which is exactly
                // the asymmetry that makes this the one wrong with no second party to it.
                self.record_death(at, other, Cause::Violence);
            }
        }
        // And whoever else was standing there.
        self.let_them_see(who, other, act, at);

        let weight = match act {
            // A life turns on being robbed and ends on the other one.
            Toward::Rob | Toward::Kill => Salience::Pivotal,
            _ => Salience::Notable,
        };
        self.acted[act as usize] = self.acted[act as usize].saturating_add(1);
        self.remember(
            at,
            weight,
            Happening::PersonActsOn {
                person: who,
                toward: other,
                act,
            },
        );
        true
    }

    /// Whoever else was there, and what it does to what they think.
    ///
    /// §35 built a vocabulary of things people do to each other and gave it **no witnesses at
    /// all**, and said so plainly: a killing is known only to the killer, because nothing in
    /// this world can tell anybody anything. The telling is still missing and this does not add
    /// it — there is no language here and inventing one is a different project. What this adds
    /// is the older thing underneath language: *somebody was standing there*.
    ///
    /// A witness needs no words. They see it, and what they think of the person who did it
    /// moves — which is `regard`, and regard is the one number on a tie that **travels**
    /// (`hearsay`). So one person seeing a robbery is enough for a town to come to think
    /// poorly of a thief, by a route that was already built and had, until now, only debt to
    /// carry. It is also the first thing in this world that gives *kindness* a reputation:
    /// being seen to give raises regard exactly as being seen to rob lowers it.
    ///
    /// Who is standing there is whoever is to hand this evening — the same list the evening's
    /// company was chosen from, which is the people you know here plus a few faces out of the
    /// crowd. Not everybody in the settlement: a village is not a room.
    fn let_them_see(&mut self, who: PersonId, other: PersonId, act: person::acts::Toward, at: Time) {
        let public = act.in_the_open();
        if !self.witnesses_notice || public <= 0.0 {
            return;
        }
        let mut rng = self.moment_stream(Domain::Behavior, who.to_bits() ^ 0x_5ee_2, at);
        // Taken from the scratch list rather than the whole place. It is already the right
        // set — who this person is among tonight — and it is already in hand.
        let around: Vec<PersonId> = self
            .company
            .iter()
            .copied()
            .filter(|face| *face != who && *face != other)
            .collect();
        if around.is_empty() {
            return;
        }
        let mut seen_by = 0;
        for face in around {
            if seen_by >= HOW_MANY_SEE {
                break;
            }
            if !rng.chance((public * SOMEBODY_NOTICES) as f64) {
                continue;
            }
            seen_by += 1;
            if !self.people.get(face).is_some_and(|p| p.is_alive() && p.has_matured()) {
                continue;
            }
            self.witnessed = self.witnessed.saturating_add(1);
            // What they make of it. Only wrongs reach here at all — see `Toward::in_the_open`,
            // where being seen to do the ordinary decent thing is worth nothing because it is
            // not news, and swamped everything else when it was worth something.
            self.bonds.saw(face, who, -act.harm() * WHAT_A_WITNESS_MAKES_OF_IT);
        }
    }

    /// Somebody went short beside somebody who could have helped, and nothing happened.
    ///
    /// Two sides that do not agree, on purpose. The person who went without is wronged by
    /// what *this place* holds people to; the person who did nothing feels it by what they
    /// were raised to hold people to. Somebody who moved here from a scattered people
    /// therefore transgresses without knowing they have — they withheld exactly as they
    /// always did — and their neighbours resent them for it while their conscience says
    /// nothing at all. That is the point of keeping the two numbers apart, and it is §17.2.1's
    /// `norms` finally doing something to somebody rather than merely differing.
    fn nobody_helped(&mut self, who: PersonId, from: PersonId, by_own: f32, at: Time) {
        use person::memory::What;
        self.withheld = self.withheld.saturating_add(1);
        // Only the memory, on both sides. Not the tie — and that is the difference between
        // this and being robbed. A slight of this kind is *carried* rather than acted on, and
        // what it does to the two of them afterwards has to go through somebody deciding to
        // do something about it: the grudge raises the appetite for shunning, and shunning is
        // what actually cools a tie. Damaging the tie here as well was double-counting, and
        // it cost the world a third more migration and six points of settlement concentration
        // for a wrong nobody had yet acted on.
        if let Some(slighted) = self.people.get_mut(from) {
            slighted.keep(What::Wronged, Some(who), at);
        }
        // And the conscience, at the actor's own rate rather than the local one.
        if by_own > WITHHOLDING_NOTICED
            && let Some(person) = self.people.get_mut(who)
        {
            person.keep(What::DidWrong, Some(from), at);
        }
    }

    /// Making good, in a year that allows it.
    ///
    /// `Bonds::repaid` existed, was tested, and was **called by nothing**, which left
    /// reciprocity as a one-way ratchet: you could be carried through a famine and you could
    /// never settle up, so every debtor's reputation fell for the rest of their life and
    /// everybody in every world was eventually thought poorly of. Half a mechanism reads
    /// exactly like a whole one until you look at what it produces.
    ///
    /// Repaying costs standing — a day of somebody else's hunger is worth about a day of
    /// your work — and is capped at what a year can spare, so somebody with little never
    /// clears what they owe. That is the trap, and it is the same trap that decides who a
    /// town shuts its door on.
    fn settle_debts(&mut self, id: PersonId) {
        let Some(mine) = self.people.get(id).map(|p| p.standing()) else {
            return;
        };
        // In days of work, which is the unit debts are kept in.
        let mut spare = mine * AFFORDABLE / WORK_GAIN;
        if spare <= 0.0 {
            return;
        }
        let mut owed: Vec<(PersonId, f32)> = self
            .bonds
            .of(id)
            .filter(|(_, tie)| tie.debt < 0.0)
            .map(|(creditor, tie)| (creditor, -tie.debt))
            .collect();
        if owed.is_empty() {
            return;
        }

        // Sorest first, and in full where it can be afforded. There was a second cap here
        // — a quarter of each debt a year — and it was the whole reason nobody in any world
        // was ever thought well of: it bound long before affordability did, so people with
        // ample means paid nine days against a debt of thirty-five and were resented for the
        // twenty-six they had not touched, every year, for life. Somebody who can settle up
        // settles up.
        owed.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let budget = spare;
        for (creditor, days) in owed {
            let pay = days.min(spare);
            if pay <= 0.0 {
                break;
            }
            spare -= pay;
            self.bonds.repaid(id, creditor, pay);
        }
        let spent = budget - spare;
        if spent > 0.0
            && let Some(person) = self.people.get_mut(id)
        {
            person.slip(spent * WORK_GAIN);
        }
    }

    /// Who goes without, when there is not enough.
    ///
    /// The land falls short by the same amount for everybody standing on it — `want` is per
    /// head — so on its own a famine kills at random. This is what makes it not random:
    /// somebody with more, who is fond of you, takes on a share of your shortfall, and it
    /// becomes theirs. **Nothing is created here.** Every day of hunger lifted off one
    /// person is a day put onto another, so the Malthusian brake is exactly as strong as it
    /// was — what changes is *who* it takes, which stops being a lottery and starts being a
    /// question of who has friends.
    ///
    /// This is also the only place ordinary reciprocity is generated. Company does not put
    /// people in each other's debt — an evening is not a favour, and treating it as one had
    /// everybody resenting everybody within a decade. Feeding somebody through a bad year
    /// is a favour, and whether they ever make it good decides what the two of them come to
    /// think of each other.
    fn share_the_shortfall(&mut self, id: PersonId, want: f32) -> f32 {
        if want <= 0.0 {
            return want;
        }
        let Some(mine) = self.people.get(id).map(|p| p.standing()) else {
            return want;
        };
        let Some(home) = self.society.place_of(id) else {
            return want;
        };

        // Taken out first: asking each ally in turn edits the ties while the walk is still
        // holding them. Once a year per person, so the small allocation is nothing.
        //
        // **Neighbours only.** Food moves between places by trade, which `economy` already
        // models and which `want` is measured after; letting obligation move it as well
        // counts the same sack of grain twice. It showed: a place a fifth short of feeding
        // itself had every one of its people at full health, because their friends two
        // valleys away had quietly absorbed the whole famine. Within a place food moves by
        // obligation, between places it moves by trade, and the two do not overlap.
        let allies: Vec<(PersonId, f32)> = self
            .bonds
            .of(id)
            .filter(|(ally, tie)| tie.allied() && self.society.place_of(*ally) == Some(home))
            .map(|(ally, tie)| (ally, tie.warmth))
            .collect();

        let mut relieved = 0.0;
        for (ally, warmth) in allies {
            // Only somebody with more to spare, and only in proportion to how warmly they
            // hold you. A friend who is as badly off as you are is no help.
            let Some(spare) = self
                .people
                .get(ally)
                .filter(|p| p.is_alive())
                .map(|p| p.standing() - mine)
                .filter(|spare| *spare > 0.0)
            else {
                continue;
            };
            let share = (RELIEF * warmth * spare).min(want - relieved);
            if share <= 0.0 {
                continue;
            }
            relieved += share;
            *self.shouldered.entry(ally).or_insert(0.0) += share;
            // In days of the year gone without, which is the unit debts are kept in.
            self.bonds.helped(ally, id, share * 365.0);
            if relieved >= want {
                break;
            }
        }
        want - relieved
    }

    /// Plain luck: the windfalls and ruins that no one earns.
    ///
    /// Uncorrelated with everything, which is the point — it is the only share of an
    /// outcome that cannot be inherited, and a world without it is a morality tale.
    fn roll_fortune(&mut self, at: Time, id: PersonId) {
        let mut rng = self.moment_stream(Domain::Chance, id.to_bits(), at);
        let Some(person) = self.people.get_mut(id) else {
            return;
        };
        if person.stage(at).is_dependent() {
            return;
        }
        if rng.chance(WINDFALL_CHANCE) {
            person.earn(WINDFALL);
        }
        if rng.chance(SETBACK_CHANCE) {
            person.slip(SETBACK);
        }
    }

    /// A young adult may be taken up by someone who can open doors.
    ///
    /// This used to be a coin flip against local bonding capital, with **no patron in it** —
    /// the single largest fact about a life here, worth more of the variance in attainment
    /// than every other input combined, and there was no mentor, only a multiplier. Now
    /// there is somebody: a specific person, older and better off, who already thinks well
    /// of you. That is what patronage is, and it means it can now only reach people who
    /// have made the acquaintance — which is the point.
    fn seek_patron(&mut self, at: Time, id: PersonId) {
        let Some(person) = self.people.get(id) else {
            return;
        };
        let age = person.age(at).years();
        if person.is_mentored() || !(FERTILE_FROM..RESTLESS_UNTIL).contains(&age) {
            return;
        }
        let mine = person.standing();

        // The best-placed person who knows you and thinks well of you.
        let mut patron: Option<(PersonId, f32)> = None;
        for (other, tie) in self.bonds.of(id) {
            if !tie.holds() {
                continue;
            }
            let Some(elder) = self.people.get(other) else {
                continue;
            };
            if !elder.is_alive()
                || elder.age(at).years() < age + MENTOR_SENIORITY
                || elder.standing() < mine + MENTOR_MEANS
            {
                continue;
            }
            // How well *they* hold *you* is what decides it, not how you feel about them:
            // this is being vouched for, and the mark of it is that it can be unrequited in
            // either direction. Both of what one person can hold about another count —
            // regard is what travels and warmth is what is felt, and a patron needs some of
            // one or the other and to actually know you.
            let theirs = self.bonds.tie(other, id);
            let worth = elder.standing() * theirs.known * (theirs.warmth + theirs.regard).max(0.0);
            if worth > 0.0 && patron.is_none_or(|(_, best)| worth > best) {
                patron = Some((other, worth));
            }
        }
        let Some((patron, worth)) = patron else {
            return;
        };

        let mut rng = self.moment_stream(Domain::Chance, id.to_bits() ^ 0x_1e17, at);
        if !rng.chance((MENTOR_CHANCE * f64::from(worth)).min(1.0)) {
            return;
        }
        let Some(worth_of_them) = self.people.get(patron).map(|p| p.standing()) else {
            return;
        };
        if self
            .people
            .get_mut(id)
            .is_some_and(|p| p.take_patron(1.0 + PATRONAGE * worth_of_them))
        {
            // A favour of this size is owed, and the chronicle files it under both of them:
            // taking somebody up is an event in the patron's life too.
            self.bonds.helped(patron, id, MENTOR_FAVOUR);
            self.remember(
                at,
                Salience::Pivotal,
                Happening::PersonMentored {
                    person: id,
                    by: patron,
                },
            );
        }
    }

    /// Look for someone to pair off with.
    ///
    /// Choice is assortative: among a handful of candidates, the most compatible wins.
    /// That is worth the extra work rather than pairing at random, because partners who
    /// resemble each other produce a wider spread of children than random pairing does,
    /// and the spread of a population is much of what the genetics is for.
    fn seek_partner(&mut self, at: Time, id: PersonId) {
        let Some(seeker) = self.people.get(id) else {
            return;
        };
        if self.society.is_partnered(id) || !seeker.is_marriageable(at) {
            return;
        }
        let (sex, age) = (seeker.sex, seeker.age(at).years());

        let eligible: Vec<PersonId> = self
            .people
            .iter()
            .filter(|(other_id, other)| {
                *other_id != id
                    && other.sex != sex
                    && other.is_marriageable(at)
                    && !self.society.is_partnered(*other_id)
                    && (other.age(at).years() - age).abs() <= 15.0
                    && !self.society.is_close_kin(id, *other_id)
            })
            .map(|(other_id, _)| other_id)
            .collect();

        if eligible.is_empty() {
            return;
        }

        // Consider a few, not everyone: nobody surveys the whole world before choosing.
        let mut rng = self.moment_stream(Domain::Behavior, id.to_bits() ^ 0x9a1d, at);
        let mut shortlist = eligible;
        rng.shuffle(&mut shortlist);
        shortlist.truncate(8);

        let Some(&chosen) = shortlist.iter().max_by(|a, b| {
            let score = |c: &PersonId| {
                self.people
                    .get(*c)
                    .map(|other| seeker.compatibility(other))
                    .unwrap_or(0.0)
            };
            score(a).total_cmp(&score(b))
        }) else {
            return;
        };

        self.society.pair(id, chosen);

        // A new household, with its own character, for the two of them and any
        // children they raise.
        // Captured before either of them moves into the new household, because moving in
        // clears where they were — and where they were is exactly what decides whether the
        // new place is a tier crossing for them.
        let were = (self.society.place_of(id), self.society.place_of(chosen));
        let inherited_place = were.0.or(were.1);
        let home = self.society.found_household(at, 0.0);
        self.society.move_in(id, home);
        self.society.move_in(chosen, home);
        if let Some(place) = inherited_place {
            self.society.settle(home, place);
            // The new household takes one partner's quarter, so the other may have just
            // crossed out of a coarse one into a fine one.
            self.arrive_from_coarse(at, id, were.0, place);
            self.arrive_from_coarse(at, chosen, were.1, place);
        }
        self.society.dissolve_empty();

        self.remember(
            at,
            Salience::Pivotal,
            Happening::PersonPairs {
                person: id,
                with: chosen,
            },
        );
    }

    /// Roll for a pregnancy, and queue the birth if one takes.
    fn try_conceive(&mut self, at: Time, id: PersonId) {
        if self.expecting.contains(&id) {
            return;
        }
        let Some(mother) = self.people.get(id) else {
            return;
        };
        if !mother.is_fertile(at) {
            return;
        }
        let Some(father_id) = self.society.partner_of(id) else {
            return;
        };
        let Some(father) = self.people.get(father_id) else {
            return;
        };
        if !father.is_alive() {
            return;
        }

        // What the place has spare. A household with food in hand has more children than
        // one without — through later marriage and worse nutrition rather than through
        // choice, but through something. This is the negative feedback the design has
        // wanted since Phase 2, and it is what stops a population running past its land
        // and levelling every difference between places by hunger.
        // `economy::births_relative` belongs here and is still not called, and this is now
        // the *fifth* centring measured rather than the fourth. It was switched on against
        // the world's own household-weighted mean — the most defensible centre there is —
        // on the expectation that the earlier four verdicts were confounded, because every
        // one of them was measured while the detail budget was quietly culling people.
        //
        // They were not confounded. On a sound world it still culls: with the check on, a
        // world of sixty founders was down to 46 souls at year ninety where it had 157
        // without it, and 15 at year one hundred and eighty where it had 629. See §21.2 for
        // all six seeds. Prosperity has too little spread between places for a multiplier
        // of this strength to be anything but noise with a downward bias, and the honest
        // conclusion after five attempts is that fertility is the wrong lever: what stops a
        // population is that the land will not feed it, and that is `Ledger::want`.
        let mut rng = self.moment_stream(Domain::Demography, id.to_bits() ^ 0xbabe, at);
        if !rng.chance(f64::from(CONCEPTION_PER_YEAR) * f64::from(mother.health().vitality)) {
            return;
        }

        self.expecting.insert(id);
        self.scheduler.schedule_at(
            at + GESTATION,
            Task::Birth {
                mother: id,
                father: father_id,
            },
        );
    }

    /// The yearly loop that closes people and places together.
    ///
    /// Three steps in order, and the order matters: every place first reads itself off
    /// the people currently in it, then children absorb the place they are growing up
    /// in, then households reconsider where they live. Sorting last means a household
    /// moves on this year's reading, not on a stale one.
    fn reckoning(&mut self, at: Time) {
        // Yearly, and only for a world that has recorded so much that the memory matters
        // more than the time — see `CHRONICLE_BUDGET`.
        if self.chronicle.len() > CHRONICLE_BUDGET + CHRONICLE_SLACK {
            self.compact_chronicle(CHRONICLE_BUDGET);
        }
        self.take_census(at);
        self.choose_trades(at);
        self.reckon_cultures(at);
        self.assign_detail();
        self.absorb_upbringings(at);
        self.work_things_out(at);
        self.take_what_can_be_taken(at);
        self.sort_households(at);
        self.reckon_bonds();
        self.scheduler
            .schedule_at(at + Duration::from_years(1), Task::Reckoning);
    }

    /// Everybody considers what to do for a living.
    ///
    /// This is where the division of labour comes from, and nothing in it names a job. Each
    /// adult looks at what one more hand in each trade would be worth where they live — which
    /// is the price of the thing times what they could actually make of it — and takes up the
    /// best of them if it is clearly better than what they are doing.
    ///
    /// Three things make it behave like a labour market rather than a stampede:
    ///
    /// - **Subsistence crowds everything else out.** A hungry place prices food at nearly one
    ///   and everything else at nearly nothing, so a village at the edge is all farmers and
    ///   nothing anywhere says so.
    /// - **A trade cannot be filled before the trade below it.** Smithing in a place with no
    ///   hewers is worth zero however badly the place wants tools.
    /// - **Changing trade is rare and costly to consider.** Without inertia the whole town
    ///   moves into whatever paid best last year, makes far too much of it, and moves out
    ///   again — a four-year cycle that never settles. People are slow, and the slowness is
    ///   what lets an occupational structure exist at all.
    fn choose_trades(&mut self, at: Time) {
        let ids: Vec<PersonId> = self.people.ids().collect();
        for id in ids {
            let Some(place) = self.society.place_of(id) else {
                continue;
            };
            let Some(worth) = self.worth.get(&place).copied() else {
                continue;
            };
            let Some(person) = self.people.get(id) else {
                continue;
            };
            if !person.is_alive() || person.stage(at).is_dependent() {
                continue;
            }
            let mine = person.trade();
            let settled = person.has_matured();

            let best = economy::Trade::ALL
                .into_iter()
                .max_by(|a, b| worth[*a as usize].total_cmp(&worth[*b as usize]))
                .unwrap_or_default();
            if best == mine {
                continue;
            }
            // Somebody who has not yet set into who they are takes up what pays without
            // needing to be dragged; everybody else needs a reason and a nudge.
            let better = worth[best as usize] - worth[mine as usize];
            if settled && better < SWITCHING * worth[mine as usize].abs().max(1.0) {
                continue;
            }
            // Whether they get round to it this year. Everybody rolls, not only the settled:
            // an unsettled person used to reconsider every year with certainty, which is what
            // made the whole cohort move as one body.
            let mut rng = self.moment_stream(Domain::Chance, id.to_bits() ^ 0x_7ade, at);
            if !rng.chance(if settled { RETRAINING } else { TRYING_THINGS }) {
                continue;
            }
            if let Some(person) = self.people.get_mut(id) {
                person.take_up(best);
            }
            // Only the settled: the young are still finding out what they are, and every one
            // of those would drown the record. Pivotal because what somebody does all year is
            // most of what their life consists of — and because a life that shows a trade
            // taken up and put down again every few years is the shape of a bug, which is
            // exactly what the moving-house record turned out to be.
            if settled {
                self.remember(
                    at,
                    Salience::Pivotal,
                    Happening::PersonRetrains {
                        person: id,
                        from: mine,
                        to: best,
                    },
                );
            }
        }
    }

    /// Hungry neighbours come and take what somebody else has.
    ///
    /// §24.4 kept conquest out of this world on the grounds that it needed a state, an army
    /// and a border — and that was the wrong list. What conquest needs is a **reason**, a
    /// **means**, and something worth **taking**, and the third was what was actually
    /// missing: until estates existed there was nothing here that could change hands.
    ///
    /// The other two this world already had. §25 says exclusion is its only sanction — no
    /// violence, no law, no court, just a door that does not open. So a taking is not a new
    /// kind of thing at all. **It is the negation of the one thing that was already
    /// political**: a door opened by force rather than passed.
    ///
    /// A *rare roll* rather than a threshold, and that is the whole of what keeps it from
    /// being §31.1's first failure again. A bar on pressure would fire and unfire as pressure
    /// hovers, which is the revolving door of §30.4 wearing armour. Rare events cannot
    /// flicker, which is why discovery and retraining are built the same way.
    ///
    /// It is self-limiting rather than a ratchet, which was the other thing to get right
    /// before writing it. Taking moves an estate from the taken to the takers, so the takers
    /// are better off and less hungry, so the pressure that caused it falls. The feedback is
    /// negative, and nothing here needed a damper bolted on afterwards.
    ///
    /// **This is the taking of things and not yet the taking of ground.** Transferring a
    /// place itself would mean displacing the households in it, and displacement runs through
    /// admission — the path by which five separate mechanisms have broken this world in one
    /// night. That half is deliberately not here, and §32 says what it would need.
    ///
    /// **And as it stands it never fires.** Built, wired into the year, consulted every
    /// reckoning, and zero takings in any world measured — including at a rate of 1.0, which
    /// makes it a closed gate rather than a small number. §32 has the diagnosis; the short
    /// version is that the conditions for a raid keep landing on different places from each
    /// other. It is kept, and labelled, on the same terms as `CROWDING_AVERSION` in §30.5:
    /// a mechanism that is right and inert is worth more visible than deleted, and `vitals`
    /// reports the count so that the day it starts firing is a day somebody notices.
    fn take_what_can_be_taken(&mut self, at: Time) {
        // Between *places*, not between countries — and that correction is the whole of what
        // this mechanism taught.
        //
        // The first version looked for a neighbouring country to raid, and measured **zero
        // adjacent cross-country pairs**, in every world, at every size. Not a rare event: a
        // structurally impossible one. A country here is a set of places that can reach each
        // other *and* share their ways, and §24 makes ways converge under contact — so any
        // two places close enough to raid have long since become the same country, and any
        // two countries are by construction out of each other's reach. §24.4 observed that
        // countries here "merge by converging, never by one taking another" and read it as a
        // missing feature; it is a theorem about how a country is defined.
        //
        // So a raid is between neighbours who can reach each other, whoever they call
        // themselves. Which is also the truer thing: a raiding party does not check whether
        // the next valley keeps the same customs.
        let slots: Vec<usize> = (0..self.roster.len()).collect();
        let mut takings: Vec<(PlaceId, PlaceId)> = Vec::new();
        for mine in &slots {
            for theirs in &slots {
                if mine == theirs || !self.within_reach(*mine, *theirs) {
                    continue;
                }
                let (Some(from), Some(to)) = (self.place_at(*theirs), self.place_at(*mine))
                else {
                    continue;
                };
                let ours = self.souls_at(*mine).unwrap_or(0) as f32;
                let theirs_souls = self.souls_at(*theirs).unwrap_or(0) as f32;
                if theirs_souls < 1.0 || ours < 1.0 {
                    continue;
                }
                // A reason: they have something, and there are more of us than them.
                //
                // Keyed on what the victim *has* rather than what the raider lacks, and that
                // came from measurement too. Keyed on the raider's hunger it could not fire
                // either: reach feeds what a place produces *and* decides who its neighbours
                // are, so hunger and adjacency are anti-correlated by construction and every
                // pair read `reach true, want 0.000` or `reach false, want 0.032`, never
                // both. Desperation cannot be the trigger in a world where isolation is what
                // causes the desperation — and raiding is what the strong do to the wealthy
                // anyway, not what the desperate do.
                let worth_taking = self
                    .society
                    .households_in(from)
                    .flat_map(|(_, h)| h.members.iter().copied())
                    .filter_map(|m| self.people.get(m))
                    .filter(|p| p.is_alive())
                    .map(|p| p.estate())
                    .sum::<f32>()
                    / theirs_souls;
                if worth_taking <= 0.0 {
                    continue;
                }
                let pressure =
                    worth_taking * ((ours / (ours + theirs_souls)) - 0.5).max(0.0) * 2.0;
                let mut rng = self.moment_stream(
                    Domain::Chance,
                    from.to_bits() ^ to.to_bits() ^ 0x_7a4e,
                    at,
                );
                if rng.chance((TAKING * pressure) as f64) {
                    takings.push((from, to));
                }
            }
        }

        for (from, to) in takings {
            let raided: Vec<PersonId> = self
                .society
                .households_in(from)
                .flat_map(|(_, h)| h.members.iter().copied())
                .filter(|m| self.people.get(*m).is_some_and(|p| p.is_alive()))
                .collect();
            let mut taken = 0.0;
            for who in raided {
                if let Some(person) = self.people.get_mut(who) {
                    taken += person.plundered(PLUNDER);
                }
            }
            if taken <= 0.0 {
                continue;
            }
            // What was taken is what is received. Nothing is created in a raid — which is the
            // whole of why it is worth doing to somebody and worth nothing to the world.
            let takers: Vec<PersonId> = self
                .society
                .households_in(to)
                .flat_map(|(_, h)| h.members.iter().copied())
                .filter(|m| self.people.get(*m).is_some_and(|p| p.is_alive()))
                .collect();
            if takers.is_empty() {
                continue;
            }
            let share = taken / takers.len() as f32;
            for taker in takers {
                if let Some(person) = self.people.get_mut(taker) {
                    person.inherit(share);
                }
            }
            self.remember(
                at,
                Salience::Historic,
                Happening::PlaceTaken { place: from, by: to },
            );
        }
    }

    /// Somebody works something out.
    ///
    /// The only thing in this world that moves a limit rather than a level, and the answer to
    /// why every world here was permanently medieval: `Technique` had a hard ceiling of three,
    /// so a people could get better at what it already did and could never come to do anything
    /// else. Now the ceiling is a *frontier* and this is the only thing that moves it.
    ///
    /// Four things decide whether anybody ever does, and none of them is a date:
    ///
    /// - **Slack.** Somebody has to have a year they did not spend staying alive. A place with
    ///   no surplus produces no advances however clever anybody in it is, which is why the
    ///   Malthusian trap is a trap: the surplus that would buy thinking gets eaten by the
    ///   children it also buys.
    /// - **Openness.** The trait for novelty, and the only place in the model where it decides
    ///   something that outlives the person who has it.
    /// - **Numbers.** More people have more ideas, and — through `MINDS_TO_KEEP` — more people
    ///   are needed to hold on to them afterwards. A lone genius in a hamlet is a lost idea.
    /// - **What they do all day.** An advance is in the discoverer's *own* trade. Nobody works
    ///   out a better forge who has never stood at one, so a world of farmers gets better at
    ///   farming and at nothing else, and two worlds that specialised differently end up good
    ///   at different things.
    ///
    /// The advance belongs to **everybody in touch**, which is the same unit
    /// `learn_and_forget` uses and not the same unit as a country: an idea passes between two
    /// villages that walk to each other whether or not they call themselves the same people.
    fn work_things_out(&mut self, at: Time) {
        // Everybody in touch, not everybody of one people — the same unit `learn_and_forget`
        // uses, and for the same reason. An idea passes between two villages that walk to each
        // other whether or not they call themselves the same thing.
        for country in self.neighbourhoods() {
            let places: Vec<PlaceId> = country
                .iter()
                .filter_map(|slot| self.roster.get(*slot).copied())
                .collect();
            if country
                .iter()
                .filter_map(|slot| self.souls_at(*slot))
                .sum::<u32>()
                == 0
            {
                continue;
            }

            let whole_life = life::Mortality::HUMAN.median_lifespan();
            let mut found: Vec<(PersonId, economy::Trade)> = Vec::new();
            for place in &places {
                // What this place had spare, per head. Nobody thinks on an empty stomach.
                let slack = self
                    .places
                    .get(*place)
                    .map(|p| (p.prosperity - p.want).clamp(0.0, 1.0))
                    .unwrap_or(0.0);
                // **No `continue` on an empty place** — §48.4. `slack` is a per-place
                // *average*, and gating on it meant one crowded quarter switched off the
                // thinking of everybody in it, the prosperous along with the hungry. Measured
                // over a hundred and sixty years: three quarters in five reach exactly zero
                // slack, the gate shuts on them, and the world's rate of invention falls to
                // nothing while its population quintuples. A trap that closes is right; one
                // that closes on the people who could plainly afford an idle evening is an
                // average standing in for a reading.
                //
                // The place's slack is now what somebody with nothing of their own has to
                // work with, and their own standing is the other half — see below.
                // How easily the people of this country reach each other. Not how many of
                // them there are — that is already counted by there being more of them to
                // roll for. What this is for is that ideas need somebody to have them *at*:
                // a hamlet at the end of a track and a town on a road produce different
                // numbers of good ideas from the same number of heads, and roads are this
                // model's whole vocabulary for that.
                let talking = self
                    .places
                    .get(*place)
                    .and_then(|p| p.terrain.as_ref())
                    .map(|t| 0.3 + 0.7 * t.reach.clamp(0.0, 1.0) as f64)
                    .unwrap_or(0.5);

                let here: Vec<PersonId> = self
                    .society
                    .households_in(*place)
                    .flat_map(|(_, h)| h.members.iter().copied())
                    .collect();
                for who in here {
                    let Some(person) = self.people.get(who) else {
                        continue;
                    };
                    if !person.is_alive() || person.stage(at).is_dependent() {
                        continue;
                    }
                        let curious = (1.0 + 0.6 * person.personality.openness).max(0.0) as f64;
                    // Their own margin, in the same units as the place's slack — which is the
                    // whole difficulty and the reason this is not simply `+ person.means()`.
                    // `slack` is years of food per head; `means()` is `standing + estate *
                    // WORTH_AT_A_DOOR` and reaches 1.93, on no scale in particular. Adding
                    // them is the error §36.6 spent three rounds on.
                    //
                    // §42.4 already built the bridge: how well somebody is doing *for their
                    // age*, saturating, -1 to 1. Only the positive half is used — doing worse
                    // than your neighbours does not take away time you never had — and it is
                    // multiplied by `TIME_TO_THINK`, so somebody at par contributes nothing
                    // and the best-off contributes exactly one span of it. By construction it
                    // lands in slack's units without anybody having to guess a maximum.
                    let mine = if self.people_think_on_their_own_means {
                        let means = person.means().max(0.0);
                        let through =
                            (person.age(at).years() / whole_life).clamp(0.0, 1.0) as f32;
                        let par = A_LIVING_STARTING_OUT + A_LIVING_BY_THE_END * through;
                        TIME_TO_THINK * (means / (means + par) * 2.0 - 1.0).max(0.0)
                    } else {
                        0.0
                    };
                    let idle = ((slack + mine) / TIME_TO_THINK).min(1.0) as f64;
                    if idle <= 0.0 {
                        continue;
                    }
                    let chance = WORKING_IT_OUT * idle * curious * talking;
                    let mut rng = self.moment_stream(Domain::Chance, who.to_bits() ^ 0x_1dea, at);
                    if rng.chance(chance) {
                        found.push((who, person.trade()));
                    }
                }
            }

            for (who, trade) in found {
                for place in &places {
                    self.technique.entry(*place).or_default().worked_out(trade);
                }
                self.remember(
                    at,
                    Salience::Historic,
                    Happening::PersonWorksItOut { person: who, trade },
                );
            }
        }
    }

    /// A year of ties fading, and a fresh count of who is next door.
    ///
    /// Last in the reckoning, after households have moved, so that the neighbours somebody
    /// spends the coming year among are the ones actually living there — not the ones who
    /// were there before the sorting.
    fn reckon_bonds(&mut self) {
        // Taken out and put back because the test of who is still alive has to read the
        // people, and the ties cannot be borrowed and read from the same world at once.
        let mut ties = std::mem::take(&mut self.bonds);
        ties.year(&|who| self.people.get(who).is_some_and(|p| p.is_alive()));
        self.bonds = ties;
        // A burden somebody died still carrying goes with them. Somebody else did not go
        // hungry for it, which is a small leak in the conservation `share_the_shortfall`
        // otherwise keeps — but the alternative is charging a corpse.
        self.shouldered
            .retain(|who, _| self.people.get(*who).is_some_and(|p| p.is_alive()));

        // **Among the grown** (§42.5). `everybodys_repute` returns everybody anybody holds a
        // tie about, and in a world of 113 adults that is 209 people — the rest are children.
        // Ranking adults against them put the median adult at 0.689 rather than 0.5, because
        // children have nothing yet and sit at the bottom, and `Dream::ToRise` reads
        // `1 - rank` and so quietly stopped believing anybody was near the bottom of anything.
        //
        // The bug is older than the fix that revealed it. While `regard` sat at zero for
        // everybody (§42.1), adults and children tied at the same value and the tie-break
        // scattered the adults evenly through the order, which put the median adult back at
        // 0.5 by accident. Two errors cancelling, again, and found the same way: by asking a
        // quantity what its distribution looked like rather than what its mean was.
        //
        // A child not in the map reads 0.5 from `repute_of`, which is the right answer to a
        // question nobody should be asking: they have not started.
        let grown = |who: PersonId| {
            self.people
                .get(who)
                .is_some_and(|p| p.is_alive() && p.has_matured())
        };
        let mut said: Vec<(PersonId, f32)> = self
            .bonds
            .everybodys_repute()
            .into_iter()
            .filter(|(who, _)| grown(*who))
            .map(|(who, (total, holders))| (who, total / holders.max(1) as f32))
            .collect();
        said.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
        let last = said.len().saturating_sub(1).max(1) as f32;
        // **People the world rates identically share a rank** (§42.1). The tie-break above
        // orders equal reputations by `PersonId`, which is an arena handle handed out in the
        // order people were born — and between a fifth and a half of the adults in a world
        // have a mean regard of *exactly* zero, because `regard` has almost no source and
        // sits where it was born. Spreading that block across a fifth of the hierarchy made
        // social rank correlate with birth order at up to 0.40, in a quantity read by
        // `Dream::ToRise`, `ToBeLookedTo`, household sorting and who a patron opens a door
        // for. It was a ranking of the arena, wearing the name of standing.
        //
        // The tie-break stays, because the sort still has to be deterministic; what changes
        // is that its result is no longer allowed to mean anything.
        self.repute.clear();
        let mut from = 0;
        while from < said.len() {
            let mut past = from + 1;
            while past < said.len() && said[past].1 == said[from].1 {
                past += 1;
            }
            let shared = (from + past - 1) as f32 / 2.0 / last;
            for (who, _) in &said[from..past] {
                self.repute.insert(*who, shared);
            }
            from = past;
        }

        let (people, neighbours) = (&self.people, &mut self.neighbours);
        neighbours.clear();
        for (_, household) in self.society.households() {
            let Some(place) = household.place else {
                continue;
            };
            let here = neighbours.entry(place).or_default();
            for member in &household.members {
                if people.get(*member).is_some_and(|p| p.is_alive()) {
                    here.push(*member);
                }
            }
        }

        self.keep_company();
    }

    /// Who everybody actually spent the year with.
    ///
    /// The evenings were chosen a deed at a time by four thousand separate decisions, or
    /// estimated in one go for the people nobody is watching. Either way they arrive here as
    /// a count, and here is where they become a society: `COMPANY_A_YEAR` draws of company,
    /// each carrying its share of the year.
    ///
    /// One path for both tiers. The tiers differ only in how the count was arrived at, which
    /// is the smallest difference they can differ by and still be two tiers — and it means
    /// there is no separate coarse social model that could quietly drift from the fine one.
    fn keep_company(&mut self) {
        let at = self.now();
        let kept = std::mem::take(&mut self.evenings);
        for (who, evenings) in kept {
            let each = evenings / COMPANY_A_YEAR;
            if each == 0 || !self.people.get(who).is_some_and(|p| p.is_alive()) {
                continue;
            }
            let mut rng = self.moment_stream(Domain::Behavior, who.to_bits() ^ 0x_50c1, at);
            for evening in 0..COMPANY_A_YEAR {
                self.spend_an_evening(who, &mut rng, each, evening);
            }
        }
    }

    /// What every place produced this year, before anybody is asked about anything.
    ///
    /// Computed first and separately from the census that reads places off their
    /// residents, because the whole point is that it does not depend on them: land,
    /// position and headcount, and none of those is an opinion. It is the one term in a
    /// neighbourhood's character that comes from outside the loop.
    fn economies(&mut self) -> std::collections::BTreeMap<PlaceId, economy::Ledger> {
        let now = self.now();
        let mut on_the_map: Vec<(PlaceId, society::Terrain, economy::Hands)> = Vec::new();
        for (id, place) in self.places.iter() {
            let Some(terrain) = place.terrain.clone() else {
                continue;
            };
            // Who is doing what. A child is not a hand and the dead are not either; nobody
            // else is left out, because a trade is what an adult spends their year on and
            // everybody spends their year on something.
            let mut hands = economy::Hands::default();
            for member in self
                .society
                .households_in(id)
                .flat_map(|(_, h)| h.members.iter())
            {
                let Some(person) = self.people.get(*member) else {
                    continue;
                };
                if !person.is_alive() || person.stage(now).is_dependent() {
                    continue;
                }
                let trade = person.trade();
                hands.set(trade, hands.at(trade) + 1.0);
            }
            on_the_map.push((id, terrain, hands));
        }

        let inputs: Vec<(society::Terrain, economy::Hands, economy::Technique, economy::Holdings)> =
            on_the_map
                .iter()
                .map(|(id, t, h)| {
                    (
                        t.clone(),
                        *h,
                        self.technique.get(id).copied().unwrap_or_default(),
                        self.holdings.get(id).copied().unwrap_or_default(),
                    )
                })
                .collect();
        let worked = economy::year_working(&inputs);

        let mut ledgers = std::collections::BTreeMap::new();
        for (at, (id, terrain, hands)) in on_the_map.into_iter().enumerate() {
            let (ledger, made, after) = worked[at];
            // What the place still owns at the end of the year. This is the line that makes
            // an economy able to compound.
            self.holdings.insert(id, after);
            self.worth.insert(
                id,
                economy::worth_of_trades(
                    &terrain,
                    &hands,
                    self.technique.get(&id).copied().unwrap_or_default(),
                    &after,
                    &made,
                ),
            );
            ledgers.insert(id, ledger);
        }
        ledgers
    }

    fn take_census(&mut self, at: Time) {
        let economies = self.economies();
        let place_ids: Vec<PlaceId> = self.places.ids().collect();
        for id in place_ids {
            let mut census = Census::default();

            for (home, household) in self.society.households_in(id) {
                census.households += 1;
                if household.founded.since(at).as_years() < 1.0
                    && household.founded >= at - Duration::from_years(1)
                {
                    census.arrivals += 1;
                }
                let _ = home;
                for member in &household.members {
                    if let Some(person) = self.people.get(*member)
                        && person.is_alive()
                        && !person.stage(at).is_dependent()
                    {
                        census.adults += 1;
                        census.mean_standing += person.standing();
                    }
                }
            }
            census.arrivals += self.arrivals.get(&id).copied().unwrap_or(0);
            if census.adults > 0 {
                census.mean_standing /= census.adults as f32;
            }

            // Norms are literally what people did here this year.
            if let Some(counts) = self.deeds_done.get(&id) {
                census.deeds = *counts;
            }

            // What the land and the position produced, which is the one term in a census
            // not read off the residents. A place with no ground under it has no economy
            // to speak of and keeps the unremarkable middle.
            census.prosperity = economies
                .get(&id)
                .map(|ledger: &economy::Ledger| ledger.prosperity())
                .unwrap_or(0.5);
            // And whether it fed them at all, which prosperity cannot say.
            census.want = economies
                .get(&id)
                .map(|ledger: &economy::Ledger| ledger.want())
                .unwrap_or(0.0);

            if let Some(place) = self.places.get_mut(id) {
                let before = place.archetype();
                place.observe(&census);
                let after = place.archetype();
                if self.was.insert(id, after) == Some(before) && before != after {
                    self.remember(
                        at,
                        Salience::Historic,
                        Happening::PlaceChanges {
                            place: id,
                            into: after,
                        },
                    );
                }
            }
        }
        self.arrivals.clear();
        self.deeds_done.clear();
    }

    /// A year of culture, and the loop it closes.
    ///
    /// §14 already had every piece of this except the memory. A place's `norms` are read
    /// off what its residents did, and `Deed::choose` weights every choice by how far it
    /// departs from them — so behaviour already followed norms and norms already followed
    /// behaviour. What was missing is that norms were rebuilt from scratch each year, which
    /// breaks the loop: a place could not carry a way of doing things through a generation,
    /// so nothing accumulated and nowhere was anywhere in particular for longer than a
    /// census.
    ///
    /// Running the year's deeds through `culture` and writing the result back into `norms`
    /// closes it. What a place did feeds what it believes, at two percent a year; what it
    /// believes feeds what its children do, at whatever their conformity is. That circuit
    /// is the whole mechanism — countries and peoples are what it produces, and nobody
    /// wrote either down.
    fn reckon_cultures(&mut self, at: Time) {
        // Places in a fixed order that never changes, so a culture index means the same
        // place for the life of the world.
        let known: std::collections::BTreeSet<PlaceId> = self.roster.iter().copied().collect();
        for id in self.places.ids() {
            if !known.contains(&id) {
                self.roster.push(id);
            }
        }
        if self.roster.is_empty() {
            return;
        }

        let mut doing = Vec::with_capacity(self.roster.len());
        let mut contact = Vec::with_capacity(self.roster.len());
        let mut reachers: Vec<PlaceId> = Vec::with_capacity(self.roster.len());
        let mut souls = Vec::with_capacity(self.roster.len());
        for id in &self.roster {
            let place = self.places.get(*id);
            doing.push(
                place
                    .map(|p| p.env.norms)
                    .unwrap_or([0.5; culture::WAYS]),
            );
            // Reach is the roads — but roads to *whom*. Terrain reach says how easy this
            // cell is to get about in, and on its own it made two settlements four thousand
            // kilometres apart, each with good going, count as being in constant touch with
            // each other. So every world stayed one people for ever: the mechanism was
            // sound and the number fed to it was a fact about the ground rather than about
            // anybody's neighbours.
            //
            // Contact is now the roads *times who is at the end of them*: how much of the
            // rest of the world's population a person here could actually reach. A place
            // nobody can get to has nobody to borrow a habit from, whatever its terrain.
            let roads = place
                .and_then(|p| p.terrain.as_ref())
                .map(|t| t.reach)
                .unwrap_or(0.5);
            contact.push(roads);
            reachers.push(*id);
            souls.push(
                self.society
                    .households_in(*id)
                    .flat_map(|(_, h)| h.members.iter())
                    .filter(|m| self.people.get(**m).is_some_and(|p| p.is_alive()))
                    .count() as u32,
            );
        }

        if souls.iter().all(|s| *s == 0) {
            return;
        }

        // Scale each place's contact by the share of everybody else it can actually get to.
        let elsewhere: u32 = souls.iter().sum();
        for (at, id) in reachers.iter().enumerate() {
            let mine = souls[at];
            let within: u32 = reachers
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != at && self.within_reach(at, *other))
                .map(|(other, _)| souls[other])
                .sum();
            let apart = elsewhere.saturating_sub(mine);
            let share = if apart == 0 {
                0.0
            } else {
                within as f32 / apart as f32
            };
            let _ = id;
            contact[at] *= share;
        }

        let cultures = self.cultures.get_or_insert_with(|| {
            // The first people are named after the first place anybody lives in, because
            // that is who they are: the people from there. Everything else in the world's
            // history of peoples descends from this one.
            let hearth = self
                .roster
                .iter()
                .zip(&souls)
                .find(|(_, s)| **s > 0)
                .and_then(|(id, _)| self.places.get(*id))
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "Firstfolk".to_string());
            culture::Cultures::beginning(0, hearth)
        });
        // A place founded since the last reckoning takes up the ways of the largest
        // inhabited place already known, because somebody walked there from somewhere and
        // the likeliest somewhere is the big one.
        let from = souls
            .iter()
            .take(cultures.places())
            .enumerate()
            .max_by_key(|(_, s)| **s)
            .map(|(at, _)| at);
        cultures.extend_to(self.roster.len(), from);

        let year = at.since(FOUNDING).as_years() as u64;
        let mut rng = self.seed.stream(Domain::Naming, year, 0);
        cultures.step(&doing, &contact, &souls, year, &mut rng);

        // And write it back, so what a place believes is what its people conform to. This
        // is the return leg of the loop and the only line that makes culture causal rather
        // than decorative.
        for (at, id) in self.roster.iter().enumerate() {
            let ways = cultures.practised(at);
            if let Some(place) = self.places.get_mut(*id) {
                place.env.norms = ways;
            }
        }

        self.learn_and_forget();
    }

    /// A year of a people either learning or forgetting.
    ///
    /// Advanced per country rather than per place, and that is the whole content of it.
    /// `MINDS_TO_KEEP` is the Tasmanian number — technique is not written down, it lives in
    /// people, and every one of them is an imperfect copy of whoever they learned from. What
    /// decides whether a body of technique grows or decays is how many carriers it has, and
    /// the carriers are everybody you can reach. A country *is* that set: people who do
    /// things the same way, close enough together to keep doing them the same way.
    ///
    /// So Tasmania is not a special case here, it is the ordinary case with the sea in it.
    /// A country that falls below the threshold loses what it knew, at the rate of its
    /// shortfall, and nothing had to be written down to say so.
    fn learn_and_forget(&mut self) {
        for country in self.neighbourhoods() {
            let minds: u32 = country
                .iter()
                .filter_map(|at| self.souls_at(*at))
                .sum();
            for at in &country {
                let Some(id) = self.roster.get(*at).copied() else {
                    continue;
                };
                // Reach is the roads again: a technique lost in one valley can be relearned
                // from the next one if anybody is travelling.
                let reach = self
                    .places
                    .get(id)
                    .and_then(|p| p.terrain.as_ref())
                    .map(|t| t.reach)
                    .unwrap_or(0.5);
                let known = self.technique.entry(id).or_default();
                *known = known.after_a_year(minds as f32, reach);
            }
        }
    }

    /// Places that can reach each other, whatever they think of each other.
    ///
    /// The unit technique travels in, and **not** the same unit as a country. A country is
    /// places that can reach each other *and share a people*; technique does not care whether
    /// two villages call themselves the same thing, only whether anybody walks between them.
    ///
    /// Using countries for this was quietly fatal. Culture fragments a world faster than
    /// anything else in it — nine hundred people spread over five quarters came out as
    /// countries of eighty — so the population that had to carry a body of technique was
    /// always a tenth of the population that could actually have carried it, and no world
    /// ever held anything. Tasmania is an argument about **contact**, not about identity.
    pub fn neighbourhoods(&self) -> Vec<Vec<usize>> {
        let n = self.roster.len();
        let mut seen = vec![false; n];
        let mut found = Vec::new();
        for start in 0..n {
            if seen[start] {
                continue;
            }
            seen[start] = true;
            let mut group = vec![start];
            let mut frontier = vec![start];
            while let Some(here) = frontier.pop() {
                for other in 0..n {
                    if seen[other] || !self.within_reach(here, other) {
                        continue;
                    }
                    seen[other] = true;
                    group.push(other);
                    frontier.push(other);
                }
            }
            found.push(group);
        }
        found
    }

    /// What a place knows how to do.
    pub fn technique_of(&self, place: PlaceId) -> economy::Technique {
        self.technique.get(&place).copied().unwrap_or_default()
    }

    /// The peoples of this world, in the order they arose.
    pub fn peoples(&self) -> &[culture::Culture] {
        self.cultures.as_ref().map(|c| c.all()).unwrap_or(&[])
    }

    /// The countries of this world, largest first, each named after its largest place.
    ///
    /// Derived on demand rather than stored, so a country cannot fall out of step with the
    /// places that make it up. Nothing here decides where a border goes: a country is
    /// whatever set of inhabited places shares a people and can walk to one another, and if
    /// that set changes because a town emptied or a valley drifted, the reading changes
    /// with it.
    pub fn countries(&self) -> Vec<culture::Country> {
        let Some(cultures) = self.cultures.as_ref() else {
            return Vec::new();
        };
        let souls: Vec<u32> = self
            .roster
            .iter()
            .map(|id| {
                self.society
                    .households_in(*id)
                    .flat_map(|(_, h)| h.members.iter())
                    .filter(|m| self.people.get(**m).is_some_and(|p| p.is_alive()))
                    .count() as u32
            })
            .collect();

        let mut countries = cultures.countries(&souls, |a, b| self.within_reach(a, b));
        for country in &mut countries {
            // After its largest place, which is how most real countries got their names —
            // and that place's own name came from the terrain it stands on, so the whole
            // chain from ground to country is derived.
            country.name = country
                .places
                .iter()
                .max_by_key(|at| souls.get(**at).copied().unwrap_or(0))
                .and_then(|at| self.roster.get(*at))
                .and_then(|id| self.places.get(*id))
                .map(|p| culture::naming::name_a_country(&p.name))
                .unwrap_or_default();
        }
        countries
    }

    /// Whether somebody could plausibly get from one place to the other and back often
    /// enough for the two to be one country.
    ///
    /// Great-circle distance, and deliberately not land connectivity: Denmark, Greece and
    /// Indonesia are all one country across water, and a strait is a road rather than a
    /// wall. What actually keeps a country together is how long the journey takes.
    ///
    /// Transitive, because `countries` walks the connections rather than testing every pair
    /// against a centre — so a chain of towns a fortnight apart is one long country even
    /// though its ends are half a world from each other, which is Chile.
    /// Public so a reader can ask which places could reach each other — §32's raid gate is
    /// built on it, and a gate nobody can inspect is a gate nobody can diagnose.
    pub fn within_reach(&self, a: usize, b: usize) -> bool {
        let of = |at: usize| {
            self.roster
                .get(at)
                .and_then(|id| self.places.get(*id))
                .and_then(|p| p.terrain.as_ref())
                .map(|t| t.cell)
        };
        let (Some(here), Some(there)) = (of(a), of(b)) else {
            // Places with no ground under them are not anywhere, so they cannot be a
            // fortnight from anywhere either. Every such place is its own country.
            return false;
        };
        let Some(surface) = self.surface.as_ref() else {
            return false;
        };
        let grid = surface.planet.grid();
        grid.distance_km(here, there, geo::EARTH_RADIUS_KM)
            <= NEIGHBOURING_GROUND * grid.spacing_km(geo::EARTH_RADIUS_KM)
    }

    /// How many live in the place at a given cultural index.
    ///
    /// The roster index rather than the `PlaceId`, because that is what a `Country` carries
    /// — it is a reading of `culture`'s own numbering and has no business knowing about
    /// arenas.
    pub fn souls_at(&self, at: usize) -> Option<u32> {
        let id = *self.roster.get(at)?;
        Some(
            self.society
                .households_in(id)
                .flat_map(|(_, h)| h.members.iter())
                .filter(|m| self.people.get(**m).is_some_and(|p| p.is_alive()))
                .count() as u32,
        )
    }

    /// What the place at a given cultural index is called.
    /// The place a country's roster index refers to.
    ///
    /// `Country` carries roster positions rather than arena handles, because `culture`
    /// indexes places by position and never forgets one. This is the way back.
    pub fn place_at(&self, at: usize) -> Option<PlaceId> {
        self.roster.get(at).copied()
    }

    pub fn place_named(&self, at: usize) -> Option<&str> {
        let id = *self.roster.get(at)?;
        self.places.get(id).map(|p| p.name.as_str())
    }

    /// What a place owns.
    pub fn holdings_of(&self, place: PlaceId) -> economy::Holdings {
        self.holdings.get(&place).copied().unwrap_or_default()
    }

    /// What one more hand in each trade would be worth in a place.
    pub fn worth_of(&self, place: PlaceId) -> Option<[f32; economy::Trade::COUNT]> {
        self.worth.get(&place).copied()
    }

    /// Everybody's position in the society of one place, and what that place's people call
    /// it.
    ///
    /// Read afresh, never stored, and read *against the neighbours* — see `bonds::roles`. So
    /// an elder who dies is replaced by whoever now best fits being an elder, with nothing
    /// anywhere that says a town must have one, or may have only one, or that the position
    /// exists at all. The position outliving the person is what makes it an institution; the
    /// fact that nothing had to be written down for it to is the point.
    pub fn society_of(&self, place: PlaceId) -> Vec<(PersonId, bonds::Position, bonds::Role)> {
        let now = self.now();
        let here: Vec<(PersonId, &Person)> = self
            .society
            .households_in(place)
            .flat_map(|(_, h)| h.members.iter().copied())
            .filter_map(|m| self.people.get(m).map(|p| (m, p)))
            .filter(|(_, p)| p.is_alive() && !p.stage(now).is_dependent())
            .collect();
        let facts: Vec<bonds::roles::Facts> = here
            .iter()
            .map(|(who, person)| bonds::roles::Facts {
                who: *who,
                person,
                age: person.age(now).years(),
            })
            .collect();
        bonds::roles::among(&self.bonds, &facts)
    }

    /// What somebody does for a living, and what their own people call it.
    ///
    /// The same arrangement as a social position: the meaning is `work`'s and the word is
    /// the people's, so two peoples who diverged in different directions call a smith two
    /// things and a people and its daughter call one nearly the same.
    pub fn trade_of(&self, who: PersonId) -> Option<(economy::Trade, String)> {
        let place = self.society.place_of(who)?;
        let trade = self.people.get(who)?.trade();
        let ways = self
            .people_of(place)
            .map(|people| people.ways)
            .unwrap_or([0.5; culture::WAYS]);
        Some((trade, culture::naming::name_a_role(&ways, trade.stem())))
    }

    /// What one person is, and what their own people call it.
    ///
    /// `None` for somebody who lives nowhere, and for a child: a position in a society is
    /// something adults have, and reading one off a nine-year-old would say more about the
    /// arithmetic than about the nine-year-old.
    pub fn standing_of(&self, who: PersonId) -> Option<(bonds::Role, String)> {
        let place = self.society.place_of(who)?;
        let (_, _, role) = self
            .society_of(place)
            .into_iter()
            .find(|(id, _, _)| *id == who)?;
        let ways = self
            .people_of(place)
            .map(|people| people.ways)
            .unwrap_or([0.5; culture::WAYS]);
        Some((role, culture::naming::name_a_role(&ways, role.stem())))
    }

    /// What somebody's life has come to, as far as a dream is concerned — see
    /// `person::dreams`.
    ///
    /// The half of a longing that is not memory. Gathered here rather than reached for by the
    /// reading, so that what a dream is allowed to see is a list somebody wrote down instead
    /// of whatever happens to be on the world.
    pub fn what_they_have_come_to(&self, who: PersonId) -> Option<person::dreams::Standing<'_>> {
        let person = self.people.get(who)?;
        let place = self.society.place_of(who);
        let whole_life = life::Mortality::HUMAN.median_lifespan();
        Some(person::dreams::Standing {
            values: &person.values,
            // A household of their *own*, which is not the same as living in one. Everybody
            // lives in one — a child lives in its parents' — so `home_of(..).is_some()` was
            // true of every adult in every world and the longing for a home was one nobody
            // ever had. What makes it theirs is that nobody who raised them is still in it.
            has_a_home: self.society.home_of(who).is_some_and(|home| {
                let parents = self.society.parents_of(who);
                !parents.is_some_and(|(mother, father)| {
                    self.society.home_of(mother) == Some(home)
                        || self.society.home_of(father) == Some(home)
                })
            }),
            has_somebody: self.society.partner_of(who).is_some(),
            rank: self.repute_of(who),
            want: place
                .and_then(|at| self.places.get(at))
                .map(|p| p.want)
                .unwrap_or(0.0),
            allies: self.bonds.of(who).filter(|(_, tie)| tie.allied()).count(),
            was_taken_up: person.is_mentored(),
            through_life: (person.age(self.now()).years() / whole_life).clamp(0.0, 1.0) as f32,
            // Who they measure themselves against. Walked over their own ties rather than over
            // the settlement, because envy is local — a person is not envious of the richest
            // man in the world, they are envious of the one they spend evenings with.
            //
            // Three things, each a fraction: how far above them the other is, how much of
            // their life that person occupies, and how little they like them. All three are
            // needed. The gap alone picks out a rich acquaintance nobody thinks about; the gap
            // with how well they are known picks out a rich friend, and being pleased for a
            // friend is not envy. It is the person who is *around* and *doing better* and *not
            // loved* who is minded.
            //
            // They are handed on **unmultiplied**, and only combined here to rank the
            // candidates. How strongly the longing is then felt is `dreams`' business, because
            // that is where it has to end up on one scale with the other six — and a number
            // that arrives pre-combined cannot be put back on their scale. See §36.6; the first
            // version multiplied here and was a product of sub-unit terms in a file where every
            // other longing is a fact times a set of weights.
            //
            // The gap goes through `A_GAP_WORTH_MINDING` and that is not decoration either. It
            // was the raw difference in means, on the stated grounds that all three factors
            // were fractions. **`means()` is not a fraction** — it is
            // `standing + estate * WORTH_AT_A_DOOR` and reaches 1.93 in a measured world, so
            // the median gap to the best-off person somebody knows is 0.958 and 46% of gaps
            // exceed one. The gap swamped the other two and was then clamped flat at the top:
            // the seventh appearance of the one bug this project makes, two quantities not on a
            // common scale used as though they were. A test written to check something else
            // found it, and only because it asserted on each factor rather than the product.
            envied: self
                .bonds
                .of(who)
                .filter(|(_, tie)| tie.holds())
                .filter_map(|(other, tie)| {
                    let them = self.people.get(other).filter(|p| p.is_alive())?;
                    let above = (them.means() - person.means()).max(0.0);
                    Some(person::dreams::Envy {
                        of: other,
                        above: above / (above + A_GAP_WORTH_MINDING),
                        known: tie.known.clamp(0.0, 1.0),
                        coolness: 1.0 - tie.warmth.clamp(0.0, 1.0),
                    })
                })
                .filter(|envy| envy.above > 0.0)
                // Ranked by the three together, which is what picks the person out; how
                // strongly it is then felt is `dreams`' business and not this one's.
                .max_by(|a, b| {
                    (a.above * a.known * a.coolness).total_cmp(&(b.above * b.known * b.coolness))
                }),
        })
    }

    /// Which people a place belongs to.
    pub fn people_of(&self, place: PlaceId) -> Option<&culture::Culture> {
        let cultures = self.cultures.as_ref()?;
        let at = self.roster.iter().position(|id| *id == place)?;
        cultures.get(cultures.of_place(at))
    }

    /// What a person would say when asked where they are from.
    ///
    /// Looked up rather than carried, because it is not a fact about them. Somebody who
    /// moves house on Tuesday is from somewhere else on Wednesday, and a person whose town
    /// drifts away from its neighbours over their lifetime ends up from a country that did
    /// not exist when they were born, without anything about them changing at all. That is
    /// the right shape for the thing, and it is why the eight-country enum this replaced
    /// could never have been made to work: it made nationality an inherited attribute.
    pub fn country_of(&self, person: PersonId) -> Option<String> {
        let place = self.society.place_of(person)?;
        let at = self.roster.iter().position(|id| *id == place)?;
        self.countries()
            .into_iter()
            .find(|c| c.places.contains(&at))
            .map(|c| c.name)
    }

    /// Children take on the character of wherever they are living, and everybody takes on
    /// its habits.
    fn absorb_upbringings(&mut self, at: Time) {
        let ids: Vec<PersonId> = self.people.ids().collect();
        for id in ids {
            let here = self
                .society
                .place_of(id)
                .and_then(|p| self.places.get(p))
                .map(|p| (p.env.upbringing(), p.env.job_opportunity, p.env.norms));
            let (quality, opportunity, norms) =
                here.unwrap_or((0.0, 0.0, [0.5; person::Deed::COUNT]));
            // What a household owns, per adult in it, added to the childhood it can give.
            //
            // This is where an estate belongs. A quarter's character is most of what shapes a
            // child (§14), and what their own household has is the rest of it — the same
            // claim §15 makes about circumstance, at the scale a family rather than a
            // neighbourhood operates on. It also enters *smoothly*: an estate arriving at a
            // funeral changes what a child absorbs from that year onwards rather than moving
            // anybody's house the same afternoon.
            let born_to = self
                .society
                .home_of(id)
                .and_then(|home| self.society.household(home))
                .map(|home| {
                    let (sum, count) = home
                        .members
                        .iter()
                        .filter_map(|m| self.people.get(*m))
                        .filter(|p: &&person::Person| p.is_alive())
                        .fold((0.0, 0), |(s, c), p| (s + p.estate(), c + 1));
                    if count == 0 { 0.0 } else { sum / count as f32 }
                })
                .unwrap_or(0.0);
            let quality = quality + born_to * WHAT_A_HOUSEHOLD_ADDS;

            let Some(person) = self.people.get_mut(id) else {
                continue;
            };
            if !person.is_alive() {
                continue;
            }
            let age = person.age(at).years();

            // Everybody, at every age, and never finished: what somebody takes to be normal
            // is learned by watching, so it lags where they are and remembers where they
            // were. §17.2 — the ambient version had a newcomer as steeped in local practice
            // as somebody born there, which is the one thing a norm is not.
            person.learn_norms(&norms, age, 1.0);

            if !person.stage(at).is_dependent() {
                person.work_amid(opportunity, 1.0);
            }
            if person.has_matured() {
                continue;
            }
            person.absorb(quality, age, 1.0);
            if age >= 20.0 {
                // The window closes: what was absorbed becomes who they are.
                person.mature();
            }
        }
    }

    /// What a household can bring to bear on getting into *somewhere in particular*.
    ///
    /// This is where society becomes politics. Admission is a scarcity — there is only so
    /// much good land and only so much room on it — and it used to be settled by wealth
    /// alone, which made this a market and not a society. Now the people already living
    /// there count: an ally inside vouches for you, in proportion to their own standing and
    /// to how warmly they hold you. A poor household with friends in a good quarter gets in
    /// over a richer one with none, and the way to get on is to be liked by somebody who is
    /// already getting on.
    ///
    /// Only ties *into the place being sought* count, which is what keeps this from becoming
    /// a second wealth term: your friends elsewhere cannot speak for you here.
    ///
    /// The best-backed member rather than the sum of them, because it takes one person to
    /// vouch for a household and a family of six is not six times as persuasive as one.
    ///
    /// **Capped, and the cap is load-bearing.** Uncapped, four allies of middling standing
    /// were worth more than a lifetime of work, so admission stopped depending on means at
    /// all: every household got into every quarter, the quarters stopped differing, and §15's
    /// upbringing gap fell to nothing — where a child grew up no longer showed up in their
    /// outcome, because everywhere had become the same place. A thumb on the scale is what
    /// this is for. No amount of vouching makes a pauper a landowner.
    ///
    /// Asked only about places the household does *not* already live in — see the caller.
    pub fn backing(&self, members: &[PersonId], into: PlaceId) -> f32 {
        // The best-regarded member speaks for the household and the worst-regarded is held
        // against it, which is what a household is: you cannot leave your brother behind.
        if members.is_empty() {
            return 0.0;
        }
        let who = *members
            .iter()
            .min_by(|a, b| self.repute_of(**a).total_cmp(&self.repute_of(**b)))
            .unwrap_or(&members[0]);
        let mut most: f32 = 0.0;
        for member in members {
            let inside = |ally: PersonId| {
                (self.society.place_of(ally) == Some(into))
                    .then(|| self.people.get(ally))
                    .flatten()
                    .filter(|p| p.is_alive())
                    .map(|p| p.standing())
            };
            most = most.max(bonds::standing_with_allies(&self.bonds, *member, 0.0, &inside));
        }
        // And what the place thinks of them, which can only ever cost. Being liked does not
        // get you in — having friends does, and that is the term above. Being *disliked*
        // keeps you out, and that is the only sanction in this world: no violence, no law,
        // no court, just a door that does not open for somebody the neighbours have turned
        // against. It is what makes reputation worth having, and it is why `regard` — which
        // travels between people who have never met — finally decides something.
        most.min(VOUCHING) + VOUCHING * ((self.repute_of(who) - 0.5) * 2.0).min(0.0)
    }

    /// Where somebody stands in the world's regard, from 0 (worst thought of) to 1 (best),
    /// with 0.5 the middle. Ordinary for anybody nobody has an opinion about.
    pub fn repute_of(&self, who: PersonId) -> f32 {
        self.repute.get(&who).copied().unwrap_or(0.5)
    }

    /// Households consider moving somewhere that suits them better.
    ///
    /// This is what produces sorting, and sorting is what makes neighbourhoods diverge
    /// rather than all drifting to the same middling average.
    fn sort_households(&mut self, at: Time) {
        let homes: Vec<society::HouseholdId> =
            self.society.households().map(|(id, _)| id).collect();

        for home in homes {
            let Some(household) = self.society.household(home) else {
                continue;
            };
            if household.members.is_empty() {
                continue;
            }
            let members = household.members.clone();
            let current = household.place;

            // What the household can sustain here, averaged over the adults who earn it.
            //
            // Tried and reverted: the *head's* standing rather than the mean, on the argument
            // that a door opens on the strength of who is asking. It measured worse for the
            // reason `backing`'s cap already records. The head is the strongest member, so
            // judging a household by them is judging every household by its best — admission
            // stops being selective, the quarters stop differing, and §15's shared-environment
            // share falls through its floor to 0.19. A household's ability to keep a roof is
            // its collective means, and the mean is the right statistic for that even though
            // it describes no individual.
            let standing = {
                // `standing`, not `means` — an estate deliberately does not open doors.
                //
                // It did, briefly, and it was the wrong place for it. An estate steps
                // *discontinuously* when a parent dies: a household's means jump the year of
                // a funeral, admission jumps with them, and somebody moves. Churn went from
                // 9% to 21% and one seed in three stopped fitting in its quarters at all.
                // §31.1's first rule is about quantities that move; this is the same rule for
                // quantities that *jump*, and admission has now been the path by which five
                // separate mechanisms broke this world.
                //
                // What wealth actually does to a life is not which door opens. It is how you
                // are raised — see `absorb_upbringings`, where it went instead.
                let (sum, count) = members
                    .iter()
                    .filter_map(|m| self.people.get(*m))
                    .filter(|p| p.is_alive() && !p.stage(at).is_dependent())
                    .fold((0.0, 0), |(s, c), p| (s + p.standing(), c + 1));
                if count == 0 { 0.0 } else { sum / count as f32 }
            };

            // The young will uproot for work, and are taken in more readily — they are
            // renting a room, not buying a house. Everyone else is choosing a place to
            // live, and is ranked out of the good ones by what they have.
            //
            // Every working adult, not the head. Tried that and measured it worse: the head
            // is read from standing, standing moves year to year, so which member is the
            // head can flip between a younger and an older one — and with it whether the
            // household is judged on what a place offers in work or on what it is like to
            // live in. A household that changes which question it is asking changes where it
            // wants to be, and 107 of 1,018 moves went straight back. See §26.10.
            let restless = members
                .iter()
                .filter_map(|m| self.people.get(*m))
                .filter(|p| p.is_alive() && !p.stage(at).is_dependent())
                .all(|p| p.age(at).years() < RESTLESS_UNTIL);

            let backing = |world: &World, into: PlaceId| world.backing(&members, into);

            let occupancy_of = |world: &World, id: PlaceId| {
                world
                    .places
                    .get(id)
                    .map(|p| world.society.households_in(id).count() as f32 / p.capacity as f32)
                    .unwrap_or(0.0)
            };
            // The same, but counting this household as living there — which it does not
            // yet, and would the moment it arrived.
            //
            // This distinction is the whole of the churn fix. Judged by the plain count, a
            // household compares the place it is packed into against a place with a
            // vacancy that it would itself fill on arrival: the discount is real when the
            // move is decided and gone by the time it is made. Then, standing in the new
            // place, the same asymmetry points back the way it came, so households
            // oscillated between two quarters for their whole lives — 65% of all moves
            // were a return to where the household had been two moves before, and the
            // aggregate never showed it because the flows in each direction cancelled.
            // Only reading one life end to end made it visible.
            //
            // Charging the arrival makes the comparison symmetric, so the gain from A to B
            // is exactly the negation of the gain from B to A, and a positive threshold
            // cannot be met in both directions. Moving back now requires the world itself
            // to have changed.
            let occupancy_with_me = |world: &World, id: PlaceId| {
                let joining = u32::from(current != Some(id));
                world
                    .places
                    .get(id)
                    .map(|p| {
                        (world.society.households_in(id).count() as u32 + joining) as f32
                            / p.capacity as f32
                    })
                    .unwrap_or(0.0)
            };
            // What a place is worth to this household: what it offers, less how packed
            // it is. Crowding has to enter here rather than only in admission, or a
            // desirable quarter fills without limit and nobody ever leaves.
            let appeal = |world: &World, id: PlaceId| {
                let Some(place) = world.places.get(id) else {
                    return f32::MIN;
                };
                let offered = if restless {
                    place.env.job_opportunity
                } else {
                    place.env.quality()
                };
                // Plus what the ground actually gives a person there, which until now
                // nothing in this decision consulted.
                //
                // §14's channels describe the *social* environment, and `affluence` is built
                // from what the residents have accumulated. It is not a measure of whether
                // you can eat, and using it as one had people moving towards the place that
                // was starving them because it looked rich. Measured on seed 0x221: one
                // quarter went from 11 households to 55, its output per head fell from 0.482
                // to 0.013 — thirty-seven times poorer — and its affluence *rose* from 0.493
                // to 0.630 the whole way down, while a neighbour with one household sat at
                // 0.862 and nobody went.
                //
                // `prosperity` is that missing number, it is per head, and it falls as hands
                // are added because `work::make` is Cobb–Douglas in land and labour. So it
                // is the counterforce to sorting that §30.5 went looking for and concluded
                // did not exist — it existed, and was computed every year, and was not
                // wired to the one decision that needed it. It needs no coefficient and it
                // comes out of `year_working`, which both detail tiers run alike.
                //
                // Smoothed, not raw. Raw prosperity answers the very move that reads it, so
                // it oscillates: 30,697 moves with 68% going straight back. `Place::fortune`
                // is what the place has been like for a working life, which is what anybody
                // is actually going on.
                offered + place.fortune
                    - CROWDING_AVERSION * (occupancy_with_me(world, id) - 1.0).max(0.0)
            };

            // What a place would take this household on: their own standing, what their
            // allies inside will lend them, and the extra room the young are given because
            // they are renting a room rather than buying a house.
            let means_at = |world: &World, id: PlaceId| {
                backing(world, id)
                    + standing
                    + if restless { YOUNG_MOVER_SLACK } else { 0.0 }
            };

            // Can they still afford where they are? Falling well behind the local average
            // means leaving, whether or not anywhere better will have them.
            //
            // Nobody can be turned out of somewhere that would admit them today. The two
            // tests used to disagree in two ways at once — the young were given
            // `YOUNG_MOVER_SLACK` to arrive and only the smaller `DISPLACEMENT_MARGIN` to
            // stay, and their allies' backing counted for getting in and not for staying.
            // Either gap on its own is a revolving door: admitted on Monday's terms, priced
            // out on Tuesday's, admitted again on Wednesday's. Between them they had
            // households commuting between two quarters for their whole lives — half to two
            // thirds of every move ever made was a household going straight back where it
            // had just come from.
            //
            // Allowed the *most* backing anybody could have rather than their own, which
            // `backing` bounds at `VOUCHING` in either direction. Two reasons, and the
            // second is the important one. Eviction is the harsher act, so where the two
            // tests could differ it should be the one that errs towards leaving people
            // where they are. And reading this household's actual allies here would put a
            // tie count in the path to a death: the coarse tier is known to understate how
            // tight a place is by up to a half, so an unwatched household would be turned
            // out of somewhere a watched one keeps, and some of them would starve for it.
            // Measured, when this test did read real backing: 25 people starved under a
            // thin detail budget against none under an ample one, over six seeds, never
            // once the other way. That is §21.1 — the observer deciding who dies — and no
            // amount of churn is worth reintroducing it.
            let grace = if restless {
                YOUNG_MOVER_SLACK + VOUCHING + DISPLACEMENT_MARGIN
            } else {
                VOUCHING + DISPLACEMENT_MARGIN
            };
            let priced_out = current.is_some_and(|c| {
                self.places
                    .get(c)
                    .is_some_and(|place| !place.admits(standing + grace, occupancy_of(self, c)))
            });

            let best = self
                .places
                .ids()
                .filter(|id| {
                    // Staying put needs no admitting — unless they have been priced out of
                    // it, in which case it is no longer an option either.
                    //
                    // Backing gates admission and never preference. That distinction is
                    // what makes it chain migration rather than a bonus for staying put
                    // dressed up as a bonus for having friends: your allies can get you in
                    // somewhere, and they cannot make you want to be there. `appeal` has no
                    // term for them at all.
                    (current == Some(*id) && !priced_out)
                        || self.places.get(*id).is_some_and(|p| {
                            // Counting this household as one of the place's own, since it
                            // would be one the moment it arrived. Judging a candidate on a
                            // vacancy you would yourself fill is the same revolving door
                            // from the other side.
                            p.admits(means_at(self, *id), occupancy_with_me(self, *id))
                        })
                })
                .max_by(|a, b| appeal(self, *a).total_cmp(&appeal(self, *b)));

            let Some(best) = best else { continue };
            if current == Some(best) {
                continue;
            }

            // Moving costs something, so only a real improvement is worth it —
            // otherwise households churn between near-identical places forever.
            // Against appeal, not raw quality. Comparing what the two places offer
            // while ignoring how packed they are rejected every move out of a crowded
            // quarter, which is how one neighbourhood came to hold everybody.
            let gain = appeal(self, best) - current.map(|c| appeal(self, c)).unwrap_or(f32::MIN);
            // Being priced out is not a preference, so the usual "is it worth the
            // move" test does not apply.
            if !priced_out && current.is_some() && gain < MOVE_THRESHOLD {
                continue;
            }

            self.society.settle(home, best);
            *self.arrivals.entry(best).or_insert(0) += 1;
            // Moving out of a coarse quarter into a fine one is a tier crossing, and an
            // unattended one is lethal — see `arrive_from_coarse`.
            for member in &members {
                self.arrive_from_coarse(at, *member, current, best);
            }
            for member in members {
                if self.people.get(member).is_some_and(|p| p.is_alive()) {
                    // Pivotal, not routine. §14 makes the neighbourhood a child grows
                    // up in the largest single influence on how they turn out, so
                    // changing it is one of the more consequential things that can
                    // happen to a family.
                    self.remember(
                        at,
                        Salience::Pivotal,
                        Happening::PersonMoves {
                            person: member,
                            to: best,
                        },
                    );
                }
            }
        }
    }

    fn birth(&mut self, at: Time, mother_id: PersonId, father_id: PersonId) {
        self.expecting.remove(&mother_id);

        // Either parent may have died while the pregnancy ran.
        let Some(mother) = self.people.get(mother_id) else {
            return;
        };
        let Some(father) = self.people.get(father_id) else {
            return;
        };
        if !mother.is_alive() {
            return;
        }

        let home = self.society.home_of(mother_id);
        let upbringing = self
            .society
            .place_of(mother_id)
            .and_then(|p| self.places.get(p))
            .map(|place| place.env.upbringing())
            .unwrap_or(0.0);
        let inherited = INHERITED_STANDING
            * (mother.standing()
                + self
                    .people
                    .get(father_id)
                    .map(|f| f.standing())
                    .unwrap_or(0.0))
            / 2.0;
        let mut rng = self.moment_stream(Domain::Genetics, mother_id.to_bits() ^ 0xb0_11, at);

        let child = person::born_to(
            self.architecture,
            (mother_id, mother),
            (father_id, father),
            &mut rng,
            at,
            upbringing,
        );

        let mut child = child;
        // A child takes its father's family name and a given name in the sound of the
        // people it is born to. Both matter: the surname is what makes the kin links
        // between people read as a family, and the sound is what makes two peoples who
        // parted company stop naming their children alike.
        let ways = self
            .society
            .place_of(mother_id)
            .and_then(|place| self.people_of(place).map(|people| people.ways))
            .unwrap_or([0.5; culture::WAYS]);
        let inherited_name = self
            .people
            .get(father_id)
            .and_then(|f| f.name.rsplit_once(' ').map(|(_, family)| family.to_string()));
        let (given, family) = culture::naming::name_a_person(
            &ways,
            child.sex == person::Sex::Female,
            inherited_name.as_deref(),
            &mut rng,
        );
        child.name = format!("{given} {family}");
        child.set_standing(inherited);
        let child_id = self.add_person(child);
        self.society.record_birth(child_id, mother_id, father_id);
        if let Some(home) = home {
            self.society.move_in(child_id, home);
        }

        self.remember(
            at,
            Salience::Pivotal,
            Happening::PersonBorn {
                child: child_id,
                mother: mother_id,
                father: father_id,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use life::{LifeStage, Need};
    use std::collections::BTreeSet;

    /// One world, lived for three generations, shared by every test needing lineages.
    ///
    /// Built once: the population grows as it runs, so paying for it per test is what
    /// made the suite unbearable.
    ///
    /// Serves both the family tests and the neighbourhood ones: seventy years is long
    /// enough for three generations *and* for identical quarters to pull apart, and one
    /// world is half the cost of two.
    ///
    /// Sixty founders, not thirty. Below about fifty the world reliably dwindles, and
    /// that is the simulation being right rather than wrong: the pairing market thins,
    /// and after a few generations close-kin exclusion rules out most of the remaining
    /// candidates, so a small isolated population struggles to reproduce itself. Real
    /// enough to keep — but it makes a small fixture a study of near-extinction rather
    /// than of families.
    fn lineages() -> &'static World {
        static WORLD: std::sync::LazyLock<World> = std::sync::LazyLock::new(|| {
            let mut world = World::genesis(WorldSeed::from_u128(0x11), 60);
            world.record_only(Salience::Pivotal);
            world.run_for(Duration::from_years(70));
            world
        });
        &WORLD
    }

    /// Eight full-sized worlds, founded once and shared — the fixture the settlement guards
    /// need, and the reason they can afford to be honest.
    ///
    /// §42.6 widened three guards from three seeds to enough seeds to mean something, because
    /// each had been set inside its own noise and was passing on whichever sample it happened
    /// to be handed. Doing that naively doubled the crate's test time: two guards, eight
    /// worlds each, sixteen runs of ninety years at a hundred and twenty founders, ten
    /// minutes between them.
    ///
    /// They want the *same* eight worlds. Founding them once is not an optimisation of the
    /// tests, it is what makes a properly-powered guard affordable at all — and a guard that
    /// is too slow to run is the same as no guard, which is how the suite ended up with three
    /// under-sampled ones in the first place.
    fn settlements() -> &'static [World] {
        static WORLDS: std::sync::LazyLock<Vec<World>> = std::sync::LazyLock::new(|| {
            [0x11u128, 0x21, 0x221, 0x31, 0x41, 0x5ee, 0x77, 0x8a]
                .into_iter()
                .map(|seed| {
                    let mut world = World::genesis(WorldSeed::from_u128(seed), 120);
                    world.record_only(Salience::Pivotal);
                    world.set_detail_budget(100_000);
                    world.run_for(Duration::from_years(90));
                    world
                })
                .collect()
        });
        &WORLDS
    }

    fn happenings(world: &World) -> Vec<Happening> {
        world.chronicle.iter().map(|r| r.kind).collect()
    }

    fn deeds_of(world: &World, id: PersonId) -> Vec<Deed> {
        world
            .chronicle
            .iter()
            .filter_map(|r| match r.kind {
                Happening::PersonDoes { person, deed } if person == id => Some(deed),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_world_starts_and_keeps_time() {
        let mut world = World::genesis(WorldSeed::from_u128(1), 1);
        world.run_for(Duration::from_hours(30));

        let planet = world.planets.ids().next().unwrap();
        use DayPhase::*;

        let phases: Vec<DayPhase> = world
            .chronicle
            .iter()
            .filter_map(|r| match r.kind {
                Happening::PhaseBegins { phase, .. } => Some(phase),
                _ => None,
            })
            .collect();
        assert_eq!(phases, vec![Morning, Afternoon, Evening, Night, Morning]);

        assert!(matches!(
            happenings(&world)[0],
            Happening::WorldBegins { planet: p } if p == planet
        ));
    }

    #[test]
    fn a_person_arrives_once_and_then_gets_on_with_it() {
        let mut world = World::genesis(WorldSeed::from_u128(2), 1);
        world.run_for(Duration::from_days(2));
        let id = world.people.ids().next().unwrap();

        let arrivals = world
            .chronicle
            .iter()
            .filter(|r| matches!(r.kind, Happening::PersonArrives { .. }))
            .count();
        assert_eq!(arrivals, 1);
        assert!(deeds_of(&world, id).len() > 5, "should have done things");
    }

    #[test]
    fn people_look_after_themselves() {
        // The behaviour loop, end to end: a month of unattended life.
        let mut world = World::genesis(WorldSeed::from_u128(3), 40);
        world.run_for(Duration::from_days(30));

        let starved = world
            .people
            .iter()
            .filter(|(_, p)| p.death().map(|(_, c)| c) == Some(Cause::Deprivation))
            .count();
        assert_eq!(
            starved, 0,
            "nobody should starve with food freely available"
        );

        for (_, p) in world.people.iter().filter(|(_, p)| p.is_alive()) {
            assert!(
                p.needs().vital_pressure() < 0.6,
                "{} is in trouble: {:?}",
                p.name,
                p.needs()
            );
        }
    }

    #[test]
    fn everyone_sleeps_and_eats_and_drinks() {
        let mut world = World::genesis(WorldSeed::from_u128(4), 10);
        world.run_for(Duration::from_days(5));

        for id in world.people.ids() {
            let deeds: std::collections::HashSet<Deed> = deeds_of(&world, id).into_iter().collect();
            for essential in [Deed::Sleep, Deed::Eat, Deed::Drink] {
                assert!(
                    deeds.contains(&essential),
                    "{:?} never got round to {essential:?}",
                    world.people.get(id).map(|p| &p.name)
                );
            }
        }
    }

    #[test]
    fn a_founding_population_has_a_shape() {
        let world = World::genesis(WorldSeed::from_u128(5), 300);
        let ages: Vec<f64> = world
            .people
            .iter()
            .map(|(_, p)| p.age(FOUNDING).years())
            .collect();

        let youngest = ages.iter().cloned().fold(f64::MAX, f64::min);
        let oldest = ages.iter().cloned().fold(f64::MIN, f64::max);
        assert!(youngest >= 18.0 && oldest <= 70.0, "{youngest}..{oldest}");
        assert!(
            oldest - youngest > 30.0,
            "founders should not be one cohort"
        );

        // And they are adults, not a nursery.
        assert!(
            world
                .people
                .iter()
                .all(|(_, p)| !p.stage(FOUNDING).is_dependent())
        );
    }

    #[test]
    fn people_age_and_eventually_die() {
        // Populations in the decade-scale tests are deliberately small. Everyone acts
        // every few hours whether or not it is recorded, so cost scales with
        // people x years; not simulating the unobserved is what level-of-detail buys,
        // and it does not exist yet.
        let mut world = World::genesis(WorldSeed::from_u128(6), 15);
        world.record_only(Salience::Pivotal);
        world.run_for(Duration::from_years(40));

        // Founders specifically. `deaths` used to count everybody who had ever died,
        // founders and their children together, and was then compared against the number
        // of *founders* — which happened to work while births were rare and stopped
        // working the moment a good place could have more of them. Two cohorts, one
        // number.
        let founders: Vec<_> = world
            .people
            .iter()
            .filter(|(_, p)| p.parents.is_none())
            .collect();
        let gone = founders.iter().filter(|(_, p)| !p.is_alive()).count();
        assert!(
            gone > 3,
            "40 years should thin a founding population: {gone} of {}",
            founders.len()
        );
        assert!(
            world.living() > 0,
            "but the world should not be empty afterwards"
        );

        // Old age should be the usual reason, not starvation.
        let by_cause = |want: Cause| {
            world
                .people
                .iter()
                .filter(|(_, p)| p.death().map(|(_, c)| c) == Some(want))
                .count()
        };
        assert!(by_cause(Cause::OldAge) > by_cause(Cause::Deprivation));
    }

    #[test]
    fn people_do_things_to_each_other_and_the_two_counts_of_it_agree() {
        // §35's whole claim in one run: the vocabulary fires, and the tally kept on the
        // world says the same as the record kept in the chronicle. Those are two independent
        // paths — a counter incremented in `carry_out` and a `Happening` filed by
        // `remember` — and a mechanism that reported five killings in a world where nobody
        // died of violence would be a bug in one of them, which is exactly why the number is
        // counted twice.
        let mut world = World::genesis(WorldSeed::from_u128(0x11), 90);
        world.record_only(Salience::Notable);
        world.set_detail_budget(100_000);
        world.run_for(Duration::from_years(70));

        use person::acts::Toward;
        assert!(
            world.acted[Toward::Give as usize] > 0,
            "somebody should have been kind to somebody in seventy years"
        );
        assert!(
            world.acted[Toward::Shun as usize] > 0,
            "and somebody should have fallen out with somebody"
        );

        let recorded = |act: Toward| {
            world
                .chronicle
                .iter()
                .filter(|r| matches!(r.kind, Happening::PersonActsOn { act: did, .. } if did == act))
                .count()
        };
        for act in Toward::ALL {
            assert_eq!(
                recorded(act),
                world.acted[act as usize] as usize,
                "{} — the tally and the record disagree",
                act.label()
            );
        }

        // And a killing is a death. Not a separate fact that happens to coincide: the act
        // *is* the death, so the causes of death have to account for every one of them.
        let by_violence = world
            .chronicle
            .iter()
            .filter(|r| matches!(r.kind, Happening::PersonDies { cause: Cause::Violence, .. }))
            .count();
        assert_eq!(
            by_violence,
            world.acted[Toward::Kill as usize] as usize,
            "somebody was killed without dying, or died without being killed"
        );
    }

    #[test]
    fn envy_is_aimed_at_a_named_person_who_is_known_and_better_off() {
        // §36.6's longing is the only one grown from somebody else's life, and the whole of its
        // value is that it names a person rather than a rank. That is what this pins: not how
        // strongly anybody feels it — which is measured in `what_they_want` and is not a claim
        // a test should hold — but that whoever it points at is somebody the envier actually
        // knows, is genuinely better off, and is not somebody they are fond of.
        //
        // Worth a test because all three could be lost by an edit that still compiles and
        // still produces a plausible number. A reading that quietly began pointing at the
        // richest person in the world would look identical from the outside, and would have
        // thrown away the reason envy was built.
        let mut world = World::genesis(WorldSeed::from_u128(0x221), 120);
        world.record_only(Salience::Pivotal);
        world.set_detail_budget(100_000);
        world.run_for(Duration::from_years(70));

        let mut checked = 0usize;
        for (who, person) in world.people.iter() {
            if !person.is_alive() || !person.has_matured() {
                continue;
            }
            let Some(envy) = world
                .what_they_have_come_to(who)
                .and_then(|come_to| come_to.envied)
            else {
                continue;
            };
            let envied = envy.of;
            checked += 1;
            assert_ne!(envied, who, "nobody envies themselves");
            let tie = world.bonds.tie(who, envied);
            assert!(
                tie.holds(),
                "envy points at somebody who is not in their life at all: known {:.3}",
                tie.known
            );
            let them = world.people.get(envied).expect("the envied person exists");
            assert!(
                them.is_alive(),
                "{} envies somebody who is dead",
                person.name
            );
            assert!(
                them.means() > person.means(),
                "{} envies {}, who has less: {:.3} against {:.3}",
                person.name,
                them.name,
                them.means(),
                person.means()
            );
            // Every piece a fraction, asserted separately. The first version of this checked
            // the three *multiplied together* and passed for the wrong reason: the gap ran to
            // 1.88 and the product hid it, because the other two shrank it back under one.
            // A composite in range says nothing about its parts.
            for (what, value) in [
                ("above", envy.above),
                ("known", envy.known),
                ("coolness", envy.coolness),
            ] {
                assert!(
                    (0.0..=1.0).contains(&value),
                    "{what} has to be a fraction, not {value} — `means()` has no ceiling and \
                     a gap taken raw is not on the scale the other two are"
                );
            }
            assert!(envy.above > 0.0, "envy with no gap in it is not envy");
            // And the fondness discount, which is what makes this envy rather than an income
            // comparison: a tie warm enough to be an ally cannot be the one that is minded
            // most, because `1 - warmth` would have to beat every cooler tie on the gap alone.
            assert!(
                tie.warmth < 1.0,
                "a tie at full warmth should have been discounted to nothing"
            );
        }
        // The reading has to happen to somebody, or the assertions above are vacuous — the
        // §31.2 failure in test form.
        assert!(
            checked > 20,
            "only {checked} adults measure themselves against anybody; the reading is not firing"
        );
    }

    #[test]
    fn reputation_is_written_by_ordinary_evenings_and_ranked_among_the_grown() {
        // §42, both halves, and both are claims about a **distribution** rather than about any
        // one person — which is why neither was caught for so long. Every reading of a
        // reputation in this project was a mean, and all three bugs here are invisible to one.
        let mut world = World::genesis(WorldSeed::from_u128(0x21), 120);
        world.record_only(Salience::Pivotal);
        world.set_detail_budget(100_000);
        world.run_for(Duration::from_years(70));
        let now = world.now();

        // §42.1 and §42.3: `regard` has a source. It was written by `saw`, `cut` and `helped`
        // alone — 2.3% of live ties, mean absolute value 0.0015 — while `hearsay` spent 1.79
        // million evenings faithfully spreading the zero everybody was born with.
        let (mut moved, mut ties, mut total) = (0usize, 0usize, 0.0f32);
        for (who, person) in world.people.iter() {
            if !person.is_alive() {
                continue;
            }
            for (_, tie) in world.bonds.of(who) {
                if !tie.holds() {
                    continue;
                }
                ties += 1;
                total += tie.regard.abs();
                if tie.regard.abs() > 0.01 {
                    moved += 1;
                }
            }
        }
        assert!(ties > 1_000, "only {ties} live ties — the world is too small to say");
        let alive = moved as f32 / ties as f32;
        assert!(
            alive > 0.5,
            "regard moved off zero on {moved} of {ties} live ties ({:.1}%) — it was 2.3% when \
             nothing but rare events wrote to it",
            100.0 * alive
        );
        assert!(
            total / ties as f32 > 0.05,
            "mean |regard| is {:.4}; warmth's is around 0.13, and a quantity two orders below \
             its sibling is not a quantity",
            total / ties as f32
        );

        // §42.2 and §42.4: rank is a percentile **of the grown**. Ranking adults against
        // children put the median adult at 0.689, and `Dream::ToRise` reads `1 - rank`, so
        // three quarters of the wanting-to-get-on in the world quietly went away.
        let mut ranks: Vec<f32> = world
            .people
            .iter()
            .filter(|(_, p)| p.is_alive() && p.has_matured())
            .map(|(who, _)| world.repute_of(who))
            .collect();
        assert!(ranks.len() > 40, "only {} adults", ranks.len());
        let mean = ranks.iter().sum::<f32>() / ranks.len() as f32;
        assert!(
            (mean - 0.5).abs() < 0.06,
            "the median grown adult sits at {mean:.3} of the hierarchy, not the middle — a \
             percentile is only uniform over the set it was actually taken across"
        );

        // And every rank is distinct, which is the §42.2 fix seen from the other side: while
        // a fifth to a half of adults had a mean regard of exactly zero, the sort fell through
        // to a `PersonId` tie-break and ranked them by the order they were born in.
        ranks.sort_by(f32::total_cmp);
        let shared = ranks.windows(2).filter(|w| w[0] == w[1]).count();
        assert!(
            shared * 5 < ranks.len(),
            "{shared} of {} adults share a rank with somebody; when regard has a source almost \
             nobody should",
            ranks.len()
        );
        let _ = now;
    }

    #[test]
    fn switching_the_vocabulary_off_leaves_a_world_that_still_works() {
        // The ablation, as a claim rather than as a thing somebody once ran. Every act's bar
        // put out of reach is not the same as deleting the code — the scoring still runs and
        // the stream is still drawn — so what this compares is the mechanism and not the
        // trajectory, which is the distinction §35.2 was built to make.
        //
        // What it asserts is deliberately weak: that a world with the vocabulary switched on
        // has the same population and the same rate of pointless migration as one without.
        // The two numbers that *do* move — how concentrated settlement ends up — are measured
        // in `vitals` over eight worlds, because at this size they are noise.
        let run = |bars: bool| {
            let mut world = World::genesis(WorldSeed::from_u128(0x21), 90);
            world.record_only(Salience::Pivotal);
            world.set_detail_budget(100_000);
            world.acts_are_possible = bars;
            world.run_for(Duration::from_years(70));
            let mut path: std::collections::BTreeMap<PersonId, Vec<PlaceId>> = Default::default();
            for record in world.chronicle.iter() {
                if let Happening::PersonMoves { person, to } = record.kind {
                    path.entry(person).or_default().push(to);
                }
            }
            let moves: usize = path.values().map(Vec::len).sum();
            let back: usize = path
                .values()
                .map(|steps| (2..steps.len()).filter(|i| steps[*i] == steps[i - 2]).count())
                .sum();
            (world.living(), moves, back)
        };
        let (with_people, with_moves, with_back) = run(true);
        let (without_people, without_moves, without_back) = run(false);

        let ratio = with_people as f32 / without_people.max(1) as f32;
        assert!(
            (0.85..1.18).contains(&ratio),
            "{with_people} living with the vocabulary against {without_people} without it"
        );
        let churn = |back: usize, moves: usize| back as f32 / moves.max(1) as f32;
        assert!(
            churn(with_back, with_moves) < 0.12,
            "{with_back} of {with_moves} moves went straight back — §30.4's bar is a tenth"
        );
        assert!(
            churn(without_back, without_moves) < 0.12,
            "and the world without it should be no worse: {without_back} of {without_moves}"
        );
    }

    #[test]
    fn the_dead_stop_acting_but_stay_readable() {
        // Routine recording is on here, because the property under test is precisely
        // that no routine act appears after a death. Small population, because that
        // makes every deed of every person for fifty years affordable to check.
        let mut world = World::genesis(WorldSeed::from_u128(7), 6);
        world.run_for(Duration::from_years(50));

        let dead: Vec<PersonId> = world
            .people
            .iter()
            .filter(|(_, p)| !p.is_alive())
            .map(|(id, _)| id)
            .collect();
        assert!(!dead.is_empty(), "fifty years should claim someone");

        for id in dead {
            let person = world.people.get(id).expect("the dead stay readable");
            let (died_at, _) = person.death().unwrap();
            assert!(!person.name.is_empty(), "still has a name to show");

            let acted_after = world.chronicle.iter().any(|r| {
                r.at > died_at
                    && matches!(r.kind, Happening::PersonDoes { person, .. } if person == id)
            });
            assert!(!acted_after, "{} kept going after dying", person.name);
        }
    }

    #[test]
    fn a_death_is_recorded_exactly_once() {
        let mut world = World::genesis(WorldSeed::from_u128(8), 12);
        world.record_only(Salience::Pivotal);
        world.run_for(Duration::from_years(40));

        let mut seen = std::collections::HashMap::new();
        for record in world.chronicle.iter() {
            if let Happening::PersonDies { person, .. } = record.kind {
                *seen.entry(person).or_insert(0) += 1;
            }
        }
        assert!(!seen.is_empty(), "somebody should have died in 40 years");
        for (person, count) in seen {
            assert_eq!(count, 1, "{person:?} died {count} times");
        }
    }

    #[test]
    fn behaviour_answers_to_circumstance() {
        // Same world, same person, different need: the choice must follow.
        let mut world = World::genesis(WorldSeed::from_u128(9), 1);
        let id = world.people.ids().next().unwrap();
        world
            .people
            .get_mut(id)
            .unwrap()
            .set_need(Need::Thirst, 0.99);
        world.run_for(Duration::from_minutes(10));

        assert!(
            deeds_of(&world, id).contains(&Deed::Drink),
            "a parched person should drink: {:?}",
            deeds_of(&world, id)
        );
    }

    #[test]
    fn the_same_seed_replays_exactly() {
        let seed = WorldSeed::from_u128(0xabc_def);
        let run = || {
            let mut w = World::genesis(seed, 12);
            w.run_for(Duration::from_days(40));
            (
                happenings(&w),
                w.people.iter().map(|(_, p)| p.clone()).collect::<Vec<_>>(),
            )
        };
        assert_eq!(run(), run(), "a world must be reproducible from its seed");
    }

    #[test]
    fn different_seeds_give_different_worlds() {
        let lives = |seed| {
            let mut w = World::genesis(WorldSeed::from_u128(seed), 10);
            w.run_for(Duration::from_days(10));
            happenings(&w)
        };
        assert_ne!(lives(1), lives(2));
    }

    #[test]
    fn fresh_worlds_are_not_variations_on_one_theme() {
        let names_of = |seed| {
            let w = World::genesis(seed, 6);
            w.people
                .iter()
                .map(|(_, p)| p.name.clone())
                .collect::<Vec<_>>()
        };
        assert_ne!(
            names_of(WorldSeed::from_entropy()),
            names_of(WorldSeed::from_entropy())
        );
    }

    #[test]
    fn running_in_pieces_matches_running_straight_through() {
        let seed = WorldSeed::from_u128(555);

        let mut whole = World::genesis(seed, 5);
        whole.run_for(Duration::from_days(20));

        let mut pieces = World::genesis(seed, 5);
        for _ in 0..20 {
            pieces.run_for(Duration::from_days(1));
        }

        assert_eq!(happenings(&whole), happenings(&pieces));
        assert_eq!(whole.now(), pieces.now());
        assert_eq!(whole.living(), pieces.living());
    }

    #[test]
    fn the_chronicle_zooms() {
        let mut world = World::genesis(WorldSeed::from_u128(10), 1);
        world.run_for(Duration::from_days(3));

        let epochal: Vec<_> = world.chronicle.at_least(Salience::Epochal).collect();
        assert_eq!(epochal.len(), 1);
        assert!(matches!(epochal[0].kind, Happening::WorldBegins { .. }));
        assert!(world.chronicle.at_least(Salience::Routine).count() > 20);
    }

    #[test]
    fn a_biography_is_the_log_filtered_by_subject() {
        let mut world = World::genesis(WorldSeed::from_u128(11), 5);
        world.run_for(Duration::from_days(3));
        let id = world.people.ids().next().unwrap();

        let mine = world
            .chronicle
            .iter()
            .filter(|r| r.kind.subject() == Some(id))
            .count();
        let all = world
            .chronicle
            .iter()
            .filter(|r| r.kind.subject().is_some())
            .count();
        assert!(mine > 0 && mine < all, "{mine} of {all}");
    }

    #[test]
    fn a_population_sustains_itself() {
        // Phase 2's headline: births now offset deaths, where Phase 1 could only decline.
        let world = lineages();
        let born = world
            .chronicle
            .iter()
            .filter(|r| matches!(r.kind, Happening::PersonBorn { .. }))
            .count();

        assert!(born > 20, "only {born} births in sixty-five years");
        assert!(
            world.people.len() > 60,
            "no one new ever existed: {}",
            world.people.len()
        );
        // Not strict growth: the founding cohort was eighteen to seventy at the start,
        // so most of it dies inside the first fifty years while its children are still
        // reaching childbearing age. The population dips through that transient before
        // it climbs. Holding most of its size across it is the real property.
        assert!(
            world.living() >= 40,
            "population collapsed to {} from 60",
            world.living()
        );
    }

    #[test]
    fn families_reach_a_third_generation() {
        let world = lineages();
        let grandparents = world
            .people
            .ids()
            .filter(|id| {
                world
                    .society
                    .children_of(*id)
                    .iter()
                    .any(|child| !world.society.children_of(*child).is_empty())
            })
            .count();
        assert!(grandparents > 0, "no lineage reached a third generation");

        let deep = world
            .people
            .ids()
            .filter(|id| world.society.ancestors_of(*id, 3).len() >= 4)
            .count();
        assert!(deep > 0, "nobody has four known grandparents");
    }

    #[test]
    fn nobody_pairs_with_close_kin() {
        let world = lineages();
        let mut pairings = 0;
        for record in world.chronicle.iter() {
            if let Happening::PersonPairs { person, with } = record.kind {
                pairings += 1;
                assert!(
                    !world.society.is_close_kin(person, with),
                    "{person:?} paired with close kin {with:?}"
                );
            }
        }
        assert!(pairings > 3, "only {pairings} pairings to check");
    }

    #[test]
    fn children_resemble_their_parents() {
        // Whether inheritance survives being wired through households and birth — the
        // coefficient itself is measured in the genetics crate, over thousands of
        // samples. A handful of families is far too few for a correlation, so this
        // compares distances instead: a child should sit closer to its own parents than
        // to a stranger, which is a much lower-variance thing to ask.
        let world = lineages();
        let architecture = genetics::standard_architecture();
        let value = |p: &Person| architecture.genetic_value(&p.genome, genetics::Trait::Openness);

        let others: Vec<f32> = world.people.iter().map(|(_, p)| value(p)).collect();
        let mut to_parents = Vec::new();
        let mut to_strangers = Vec::new();

        for (child_id, child) in world.people.iter() {
            let Some((mother, father)) = world.society.parents_of(child_id) else {
                continue;
            };
            let (Some(m), Some(f)) = (world.people.get(mother), world.people.get(father)) else {
                continue;
            };
            let midparent = (value(m) + value(f)) / 2.0;
            to_parents.push((value(child) - midparent).abs());

            // Everyone else, averaged — no single stranger, so no lucky draw.
            let mean_other = others.iter().sum::<f32>() / others.len() as f32;
            to_strangers.push((value(child) - mean_other).abs());
        }

        assert!(
            to_parents.len() > 8,
            "only {} families to measure",
            to_parents.len()
        );
        let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
        assert!(
            mean(&to_parents) < mean(&to_strangers),
            "children should sit closer to their parents ({:.3}) than to the population ({:.3})",
            mean(&to_parents),
            mean(&to_strangers)
        );
    }

    #[test]
    fn siblings_share_an_upbringing_without_it_being_identical() {
        // In Phase 2 full siblings had exactly the same shared term, because it was
        // fixed at birth from the household. It is now accumulated across a childhood,
        // so siblings born a decade apart experience the same neighbourhood in
        // different states — and one may be carried elsewhere partway through. Close,
        // not equal, is the right claim, and it is the developmental window working.
        let world = lineages();
        let mut siblings = Vec::new();
        let mut unrelated = Vec::new();

        let matured: Vec<(PersonId, f32)> = world
            .people
            .iter()
            .filter(|(_, p)| p.has_matured())
            .map(|(id, p)| (id, p.absorbed_upbringing()))
            .collect();

        for (id, mine) in &matured {
            let Some(parents) = world.society.parents_of(*id) else {
                continue;
            };
            for sibling in world.society.siblings_of(*id) {
                if world.society.parents_of(sibling) != Some(parents) {
                    continue;
                }
                if let Some((_, theirs)) = matured.iter().find(|(o, _)| *o == sibling) {
                    siblings.push((mine - theirs).abs());
                }
            }
            for (other, theirs) in &matured {
                if *other != *id && world.society.parents_of(*other) != Some(parents) {
                    unrelated.push((mine - theirs).abs());
                }
            }
        }

        assert!(siblings.len() > 2, "only {} sibling pairs", siblings.len());
        let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
        assert!(
            mean(&siblings) < mean(&unrelated),
            "siblings should be raised more alike ({:.3}) than strangers ({:.3})",
            mean(&siblings),
            mean(&unrelated)
        );
    }

    #[test]
    fn a_child_is_born_into_its_mothers_household() {
        let world = lineages();
        let mut checked = 0;
        for record in world.chronicle.iter() {
            let Happening::PersonBorn { child, mother, .. } = record.kind else {
                continue;
            };
            // Only recent births, and only children who have not yet left. The dead
            // move out, anyone who pairs off founds a household of their own, and a
            // widowed mother who pairs again moves to a new one — all correct, and all
            // reasons a present-day comparison says nothing about a birth long ago.
            if record.at < world.now() - Duration::from_years(6) {
                continue;
            }
            let Some(hers) = world.society.home_of(mother) else {
                continue;
            };
            let Some(offspring) = world.people.get(child) else {
                continue;
            };
            if !offspring.is_alive() || world.society.is_partnered(child) {
                continue;
            }
            assert_eq!(
                world.society.home_of(child),
                Some(hers),
                "a child should live with its mother"
            );
            checked += 1;
        }
        assert!(checked > 2, "only {checked} living mothers to check");
    }

    #[test]
    fn the_dead_leave_their_partner_and_their_house() {
        let world = lineages();
        let mut dead = 0;
        for (id, person) in world.people.iter() {
            if person.is_alive() {
                continue;
            }
            dead += 1;
            assert!(
                world.society.partner_of(id).is_none(),
                "{} is dead and still partnered",
                person.name
            );
            assert!(
                world.society.home_of(id).is_none(),
                "{} is dead and still housed",
                person.name
            );
        }
        assert!(dead > 5, "only {dead} deaths to check");
    }

    #[test]
    fn children_cannot_take_work() {
        // Channel one doing structural work: the option is absent, not unattractive.
        // Needs routine recording, so it runs its own small, short world.
        let mut world = World::genesis(WorldSeed::from_u128(0x55), 8);
        world.run_for(Duration::from_years(12));

        let now = world.now();
        let mut dependants = 0;
        for (id, person) in world.people.iter() {
            if !person.is_alive() || !person.stage(now).is_dependent() {
                continue;
            }
            dependants += 1;
            let worked = world.chronicle.iter().any(|r| {
                matches!(r.kind, Happening::PersonDoes { person: p, deed } if p == id && deed == Deed::Work)
            });
            assert!(!worked, "{} worked while still a child", person.name);
        }
        assert!(dependants > 0, "no children were born to check");
    }

    #[test]
    fn neighbourhoods_diverge_from_identical_beginnings() {
        // Every quarter starts unremarkable and identical. Nothing here writes "slum"
        // or "enclave" anywhere: the spread is what sorting and accumulation produce.
        let fresh = World::genesis(WorldSeed::from_u128(0x11), 4);
        let at_founding: Vec<f32> = fresh.places.iter().map(|(_, p)| p.env.affluence).collect();
        assert!(
            at_founding.windows(2).all(|w| w[0] == w[1]),
            "they should begin the same"
        );

        let world = lineages();
        let affluence: Vec<f32> = world.places.iter().map(|(_, p)| p.env.affluence).collect();
        let lowest = affluence.iter().cloned().fold(f32::MAX, f32::min);
        let highest = affluence.iter().cloned().fold(f32::MIN, f32::max);
        assert!(
            highest - lowest > 0.2,
            "neighbourhoods should pull apart: {affluence:?}"
        );

        let kinds: std::collections::HashSet<society::Archetype> =
            world.places.iter().map(|(_, p)| p.archetype()).collect();
        assert!(
            kinds.len() > 1,
            "they should not all read the same: {kinds:?}"
        );
    }

    #[test]
    fn where_a_child_grows_up_shapes_them() {
        // The developmental window, end to end. Same machinery, two upbringings.
        let world = lineages();
        let raised: Vec<(f32, f32)> = world
            .people
            .iter()
            .filter(|(_, p)| p.has_matured() && p.parents.is_some())
            .map(|(_, p)| (p.absorbed_upbringing(), p.origins.conscientiousness.shared))
            .collect();

        assert!(raised.len() > 5, "only {} raised here", raised.len());
        assert!(
            raised.iter().any(|(a, _)| *a != raised[0].0),
            "children should not all have absorbed the same place"
        );
        // The shared term is that absorption, not a birth-time snapshot.
        for (absorbed, shared) in &raised {
            assert!(
                (shared - absorbed * (0.20f32).sqrt()).abs() < 1e-4,
                "shared term should be the absorbed upbringing: {shared} vs {absorbed}"
            );
        }
    }

    #[test]
    fn a_hard_neighbourhood_suppresses_work() {
        // Channels one to three, together: the same person works less where there is
        // less to be had, and the gap is structural rather than a matter of character.
        let world = lineages();
        let poorest = world
            .places
            .iter()
            // By overall quality rather than by affluence alone, which is what "hard"
            // means and was not the same thing once there was an economy. Opportunity now
            // depends on what a place *produces* as well as on what its residents have, so
            // the poorest quarter by income is no longer reliably the one with least work
            // — a thinly populated place on good land can be poor and still have plenty to
            // do. Selecting on affluence and asserting about work was reading one column
            // and testing another.
            .min_by(|(_, a), (_, b)| a.env.quality().total_cmp(&b.env.quality()))
            .map(|(_, p)| p.env.clone())
            .unwrap();
        let richest = world
            .places
            .iter()
            .max_by(|(_, a), (_, b)| a.env.quality().total_cmp(&b.env.quality()))
            .map(|(_, p)| p.env.clone())
            .unwrap();

        let hard = poorest.surroundings(false);
        let easy = richest.surroundings(false);
        assert!(hard.availability[Deed::Work as usize] < easy.availability[Deed::Work as usize]);
        assert!(hard.payoff[Deed::Work as usize] < easy.payoff[Deed::Work as usize]);
        assert!(hard.discount_rate() > easy.discount_rate());
    }

    #[test]
    fn standing_settles_instead_of_saturating_or_collapsing() {
        // Two failure modes this went through on the way here: an equilibrium of 0.99
        // that made every quarter an enclave, and no equilibrium at all, which slid
        // every world to destitution within two generations.
        let world = lineages();
        let standings: Vec<f32> = world
            .people
            .iter()
            .filter(|(_, p)| p.is_alive() && !p.stage(world.now()).is_dependent())
            .map(|(_, p)| p.standing())
            .collect();
        assert!(standings.len() > 20, "not enough adults to judge");

        let mean = standings.iter().sum::<f32>() / standings.len() as f32;
        assert!(
            (0.10..0.85).contains(&mean),
            "mean standing {mean:.3} is a collapse or a saturation"
        );

        let spread = standings.iter().cloned().fold(f32::MIN, f32::max)
            - standings.iter().cloned().fold(f32::MAX, f32::min);
        assert!(
            spread > 0.2,
            "everyone ended up the same: spread {spread:.3}"
        );
    }

    #[test]
    fn some_people_are_taken_up_by_someone() {
        let world = lineages();
        let mentored = world
            .chronicle
            .iter()
            .filter(|r| matches!(r.kind, Happening::PersonMentored { .. }))
            .count();
        assert!(mentored > 0, "nobody ever found a patron");

        // And it is a way out, not a reward for already being ahead: everyone who found
        // one keeps it for life, and it multiplies what their work returns.
        let lucky = world.people.iter().filter(|(_, p)| p.is_mentored()).count();
        assert!(lucky > 0);
        assert!(
            lucky < world.people.len() / 2,
            "patronage should be uncommon, not the norm"
        );
    }

    #[test]
    fn patronage_favours_tight_communities_over_comfortable_ones() {
        // The inversion that makes the mechanism worth having. Bridging ties belong to
        // people who are already comfortable; routing a way out through them would only
        // widen the gap. Bonding capital is what a poor neighbourhood actually has.
        let world = lineages();
        let mut with_patron = Vec::new();
        let mut without = Vec::new();

        for (id, person) in world.people.iter() {
            if !person.has_matured() {
                continue;
            }
            let Some(bonding) = world
                .society
                .place_of(id)
                .and_then(|p| world.places.get(p))
                .map(|p| p.env.bonding_capital)
            else {
                continue;
            };
            if person.is_mentored() {
                with_patron.push(bonding);
            } else {
                without.push(bonding);
            }
        }

        if with_patron.len() >= 3 && !without.is_empty() {
            let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
            assert!(
                mean(&with_patron) > mean(&without) - 0.15,
                "patrons should not be concentrated in the comfortable places: {:.2} vs {:.2}",
                mean(&with_patron),
                mean(&without)
            );
        }
    }

    #[test]
    fn the_young_will_move_for_work() {
        // The spatial trap without this is absolute: you cannot move to where the work
        // is until you have the standing that moving there would have earned you.
        let world = lineages();
        let movers = world
            .chronicle
            .iter()
            .filter(|r| matches!(r.kind, Happening::PersonMoves { .. }))
            .count();
        assert!(movers > 5, "only {movers} moves in seventy years");
    }

    #[test]
    fn moving_is_not_a_thing_people_do_back_and_forth() {
        // A household that leaves somewhere should mostly stay left. Migration and churn
        // look identical in the aggregate — a hundred moves each way is the same net flow
        // as none — so the only way to tell them apart is to follow individual households
        // and count how often one returns to the place it was two moves ago.
        //
        // It used to be 65% of every move made, because the bar to get into a place was
        // lower than the bar to stay in it, so households were admitted and evicted in
        // alternate years for their whole lives. Reading one life end to end in the atlas
        // is what showed it: the same two town names, alternating, for twenty years.
        //
        // Over three worlds rather than one, and at a size where the question has an answer.
        // Both changes are §15.1's lesson applied here rather than a loosening.
        //
        // It used to run one seed of sixty founders for seventy years. That produces about a
        // hundred and fifty moves, and a rate estimated from that wanders badly — pooled over
        // three such worlds it reads 15%, and the single seed the test happened to use read
        // under 10% and passed. The fixture was not measuring churn, it was measuring which
        // seed it had been given.
        //
        // Small young worlds churn more, and that is not obviously a fault: their quarters
        // are marginal and nearly identical, so tiny differences in what a place is worth
        // decide moves that a larger world would settle on real ones. A hundred and twenty
        // founders over ninety years is where places have had time to become different
        // places, and pooled over three seeds it reads well inside a tenth.
        // One map per world, and that is not tidiness. Handles are per-arena: person 5 of one
        // world and person 5 of the next are the same key, and so are their places. Pooling
        // the *paths* into one map therefore stitched three strangers' lives into one and
        // counted a move in the second world as a return to somewhere in the first. It read
        // 10.5% while the same three worlds measured properly read 4%, and it had been
        // reporting a number nobody could have got any other way for as long as it existed.
        // What pools across worlds is the rate; the paths do not.
        let (mut moves, mut returns) = (0, 0);
        // **Eight seeds, and the bar is where the pathology is rather than where the world
        // sits** (§42.6). On three seeds this asserted under 10% and passed. Pooled over
        // twelve it reads 10.4% — and 10.9% *before* §42, which improved it — so the world
        // has been fractionally over its own bar the whole time and the three-seed sample was
        // hiding it. Churn's per-world sd is 0.067 (§40.3), but this is a ratio of sums rather
        // than a mean of rates and so is far tighter: about 0.4 points over five thousand
        // moves.
        //
        // What 10% was for is worth remembering. The fault it was written against ran at
        // **65%** — households admitted and evicted in alternate years for their whole lives.
        // A bar at 12 is three standard errors above where the world sits and five times
        // under the failure it exists to catch; a bar at 10 was inside the measurement and
        // told nobody anything except which seeds it had been handed.
        for world in settlements() {
            let mut path: std::collections::BTreeMap<PersonId, Vec<PlaceId>> = Default::default();
            for record in world.chronicle.iter() {
                if let Happening::PersonMoves { person, to } = record.kind {
                    path.entry(person).or_default().push(to);
                }
            }
            for steps in path.values() {
                moves += steps.len();
                returns += (2..steps.len()).filter(|i| steps[*i] == steps[i - 2]).count();
            }
        }
        assert!(moves > 1000, "too few moves to say anything: {moves}");
        assert!(
            returns * 100 < moves * 12,
            "{returns} of {moves} moves went straight back where they came from ({:.1}%); \
             the world sits at 10.4 and the fault this guards against ran at 65",
            100.0 * returns as f32 / moves as f32
        );
    }

    #[test]
    fn a_world_does_not_end_up_in_one_quarter() {
        // Sorting is a positive feedback: the well-off move somewhere and it becomes the
        // place the well-off live, so more of them move there. Something has to pull the
        // other way or every world ends with one town and four ghost quarters — measured at
        // 0.89 of all households in the biggest, and on one seed 1.00, every household in
        // the world.
        //
        // What pulls the other way is that the ground feeds fewer people each, and it always
        // did — `work::make` is Cobb–Douglas — but the decision was reading `affluence`,
        // which is what the residents have *accumulated*, and which rose the whole way down.
        // See §30.5. This asserts the outcome rather than the wiring, because the wiring has
        // now been wrong in three different ways and the outcome is what was wanted from it.
        // **A mean over twelve, plus a per-seed catch for the actual pathology** (§42.7).
        // This asserted `biggest < 0.75` on each of three seeds, and measured across twelve
        // the per-seed figure runs 0.34 to 0.80 with nothing changed — so two seeds in twelve
        // were already over the bar, and the test passed only because it did not use them. A
        // per-seed bar above that range would have to sit near 0.85, which is close enough to
        // collapse to be no guard at all.
        //
        // So the two questions are asked separately, which is what they always were. Whether
        // the world *tends* to pile into one quarter is a question about the mean and is
        // stable; whether it has actually collapsed is a question about a single world and
        // 1.00 is the answer. §30.5 is about the second and this now says so.
        let mut share = Vec::new();
        for world in settlements() {
            let counts: Vec<usize> = world
                .places
                .ids()
                .map(|id| world.society.households_in(id).count())
                .collect();
            let total: usize = counts.iter().sum();
            assert!(total > 10, "nobody is anywhere, {total} households");
            let biggest = *counts.iter().max().unwrap_or(&0);
            let here = biggest as f32 / total as f32;
            // The collapse §30.5 is named for: every household in the world in one place.
            assert!(
                here < 0.98,
                "{biggest} of {total} households are in one quarter — the world has collapsed \
                 into a point"
            );
            share.push(here);
        }
        let mean = share.iter().sum::<f32>() / share.len() as f32;
        // Measured at 0.54 before §42 and 0.61 after, with a standard error of 0.042 — the
        // rise is the Matthew effect arriving, since `backing` gates admission on `repute` and
        // `repute` finally ranks something real. 0.75 is three errors above where it sits.
        assert!(
            mean < 0.75,
            "households pile into one quarter across the board: {mean:.2} of them, meaned over \
             {} worlds",
            share.len()
        );
    }

    #[test]
    fn a_trade_is_something_people_settle_into() {
        // The same question as `moving_is_not_a_thing_people_do_back_and_forth`, asked of
        // livelihoods. Everybody in a place values the trades from the same numbers in the
        // same instant, so if they all act on them in the same year they all pile into
        // whatever was short and it is short no longer — and the signal points the other way
        // the year after. A cobweb, and it showed up as lives reading "gives up cook for
        // farmer, gives up farmer for cook" five times over.
        //
        // Stepped a year at a time because the chronicle cannot answer this: it records a
        // *settled* person changing trade, and the young do most of the moving. Measured at
        // 24% of all changes going back to the trade before last, and 11% after staggering
        // when people get round to reconsidering.
        let mut world = World::genesis(WorldSeed::from_u128(0x21), 80);
        world.record_only(Salience::Pivotal);
        world.set_detail_budget(100_000);

        let mut path: std::collections::BTreeMap<PersonId, Vec<economy::Trade>> = Default::default();
        for _ in 0..50 {
            let before: std::collections::BTreeMap<PersonId, economy::Trade> = world
                .people
                .iter()
                .filter(|(_, p)| p.is_alive())
                .map(|(id, p)| (id, p.trade()))
                .collect();
            world.run_for(Duration::from_years(1));
            for (id, person) in world.people.iter() {
                if !person.is_alive() {
                    continue;
                }
                if before.get(&id).is_some_and(|was| *was != person.trade()) {
                    path.entry(id).or_default().push(person.trade());
                }
            }
        }

        let (mut changes, mut back) = (0usize, 0usize);
        for steps in path.values() {
            changes += steps.len();
            back += (2..steps.len()).filter(|i| steps[*i] == steps[i - 2]).count();
        }
        assert!(changes > 30, "too few changes of trade to say anything: {changes}");
        assert!(
            back * 5 < changes,
            "{back} of {changes} changes of trade went straight back to the trade before last"
        );
    }

    #[test]
    fn fortune_is_uncorrelated_with_deserving() {
        // Luck has to reach people regardless of who they are, or it is not luck.
        let world = lineages();
        let now = world.now();
        let adults: Vec<&Person> = world
            .people
            .iter()
            .map(|(_, p)| p)
            .filter(|p| p.has_matured() && !p.stage(now).is_dependent())
            .collect();
        assert!(adults.len() > 20);

        // Standing should not have collapsed onto one value, which is what would happen
        // if the shocks were the only thing moving it.
        let peaks: Vec<f32> = adults.iter().map(|p| p.peak_standing()).collect();
        let spread = peaks.iter().cloned().fold(f32::MIN, f32::max)
            - peaks.iter().cloned().fold(f32::MAX, f32::min);
        assert!(
            spread > 0.2,
            "outcomes collapsed together: spread {spread:.2}"
        );
    }

    /// Same world, same seed, one watched closely and one not.
    fn at_detail(budget: usize, years: u64) -> World {
        let mut world = World::genesis(WorldSeed::from_u128(0x11), 60);
        world.record_only(Salience::Pivotal);
        world.set_detail_budget(budget);
        world.run_for(Duration::from_years(years));
        world
    }

    // ---- ground that is good at different things (§28) -----------------------------

    #[test]
    fn the_ground_of_a_world_is_not_all_the_same_ground() {
        // Before §28 every place in every world was good at exactly the same things in
        // exactly the same proportion, so geography could not produce a division of labour
        // between settlements however much it produced within one.
        let world = lineages();
        let mut ratios: Vec<f32> = Vec::new();
        for (id, place) in world.places.iter() {
            let Some(terrain) = place.terrain.as_ref() else {
                continue;
            };
            let ground = economy::ground_of(terrain, world.technique_of(id));
            if ground.food > 0.0 {
                ratios.push(ground.stock / ground.food);
            }
        }
        assert!(ratios.len() > 2, "too few places on the map to compare");
        let (low, high) = (
            ratios.iter().cloned().fold(f32::MAX, f32::min),
            ratios.iter().cloned().fold(f32::MIN, f32::max),
        );
        assert!(
            high > low * 1.2,
            "every place is good at the same things: {low:.2} to {high:.2}"
        );
    }

    #[test]
    fn a_road_is_what_makes_material_worth_cutting() {
        // The term that gives a road a reason to exist other than other people's charity.
        let world = lineages();
        for (id, place) in world.places.iter() {
            let Some(terrain) = place.terrain.as_ref() else {
                continue;
            };
            let ground = economy::ground_of(terrain, world.technique_of(id));
            assert!(
                ground.sells_for >= 0.0 && ground.sells_for <= 1.0,
                "{} sells at {}",
                place.name,
                ground.sells_for
            );
            if terrain.reach <= 0.0 {
                assert_eq!(
                    ground.sells_for, 0.0,
                    "{} is off every road and still selling",
                    place.name
                );
            }
        }
    }

    // ---- somebody works something out (§29) ----------------------------------------

    #[test]
    fn nothing_is_ever_practised_beyond_what_is_possible() {
        // The invariant that makes the frontier a frontier. A people can be as large and as
        // well connected as it likes and it will not exceed what anybody has worked out.
        let world = lineages();
        for id in world.places.ids() {
            let know = world.technique_of(id);
            for trade in economy::Trade::ALL {
                assert!(
                    know.at(trade) <= know.frontier(trade) + 1e-4,
                    "{:?} practised {} against a limit of {}",
                    trade,
                    know.at(trade),
                    know.frontier(trade)
                );
                assert!(know.at(trade) >= 1.0, "somebody forgot how to eat");
            }
        }
    }

    #[test]
    fn the_limit_only_moves_when_somebody_moves_it() {
        // The answer to why every world here was permanently medieval, stated as a test: the
        // ceiling is not a constant any more, and the only thing that lifts it is a person.
        let world = lineages();
        let advances = world
            .chronicle
            .iter()
            .filter(|r| matches!(r.kind, Happening::PersonWorksItOut { .. }))
            .count();
        let moved = world
            .places
            .ids()
            .any(|id| world.technique_of(id).reach_of_knowledge() > 1.0 + 1e-6);
        assert_eq!(
            advances > 0,
            moved,
            "{advances} advances and the limit {} moved",
            if moved { "had" } else { "had not" }
        );
    }

    #[test]
    fn what_one_person_works_out_everybody_in_touch_could_do() {
        // Everybody within reach of each other, which is *not* the same as everybody of one
        // people: an idea passes between two villages that walk to each other whether or not
        // they call themselves the same thing.
        let world = lineages();
        for country in world.neighbourhoods() {
            let mut seen: Option<[f32; economy::Trade::COUNT]> = None;
            for slot in &country {
                let Some(id) = world.place_at(*slot) else {
                    continue;
                };
                let know = world.technique_of(id);
                let here = economy::Trade::ALL.map(|t| know.frontier(t));
                match seen {
                    None => seen = Some(here),
                    Some(theirs) => {
                        for at in 0..economy::Trade::COUNT {
                            assert!(
                                (here[at] - theirs[at]).abs() < 1e-3,
                                "a place knows of things its neighbours do not: \
{here:?} against {theirs:?}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn an_advance_is_a_thing_that_happened_to_somebody() {
        // Not a date and not a tree. Every one of these names a person, and that person was
        // alive and working when it happened.
        let world = lineages();
        for record in world.chronicle.iter() {
            let Happening::PersonWorksItOut { person, .. } = record.kind else {
                continue;
            };
            let who = world.people.get(person).expect("a discoverer who never existed");
            assert!(
                record.at >= who.born,
                "{} worked something out before they were born",
                who.name
            );
        }
    }

    // ---- a supply chain (§27) ------------------------------------------------------

    #[test]
    fn a_world_divides_its_labour_without_being_told_to() {
        let world = lineages();
        let now = world.now();
        let mut trades: std::collections::BTreeMap<economy::Trade, usize> = Default::default();
        for (_, p) in world.people.iter() {
            if p.is_alive() && !p.stage(now).is_dependent() {
                *trades.entry(p.trade()).or_default() += 1;
            }
        }
        assert!(
            trades.len() > 1,
            "everybody in the world does the same thing: {trades:?}"
        );
        // And most people still farm, because most people always did. A world where the
        // majority is not growing food is a world that has stopped being about subsistence,
        // and this one has not.
        let farmers = trades.get(&economy::Trade::Farmer).copied().unwrap_or(0);
        let all: usize = trades.values().sum();
        assert!(
            farmers * 2 > all,
            "only {farmers} of {all} grow anything: {trades:?}"
        );
    }

    #[test]
    fn nobody_makes_a_thing_out_of_what_nobody_has_cut() {
        // The supply chain, checked over a whole world: a place with tools in it has had
        // hands on every link below them at some point, and a place that never spared a hewer
        // has nothing but what it grows.
        let world = lineages();
        for id in world.places.ids() {
            let held = world.holdings_of(id);
            assert!(held.tools >= 0.0 && held.stock >= 0.0);
            // Tools are made of stock and stock is cut by hands. Nothing can appear from
            // nowhere, which is the one thing a chain has to guarantee.
            if held.tools > 0.0 {
                assert!(
                    world.people.iter().any(|(_, p)| {
                        matches!(p.trade(), economy::Trade::Smith | economy::Trade::Hewer)
                    }),
                    "a place holds tools and nobody in the world ever cut or forged anything"
                );
            }
        }
    }

    #[test]
    fn a_hungry_place_puts_everybody_back_on_the_land() {
        // Subsistence first, as an outcome rather than a rule. Nowhere that cannot feed
        // itself should be spending hands on anything else.
        let world = lineages();
        let now = world.now();
        for (id, place) in world.places.iter() {
            if place.want <= 0.15 {
                continue;
            }
            let (mut farming, mut all) = (0, 0);
            for member in world
                .society
                .households_in(id)
                .flat_map(|(_, h)| h.members.iter())
            {
                let Some(p) = world.people.get(*member) else {
                    continue;
                };
                if !p.is_alive() || p.stage(now).is_dependent() {
                    continue;
                }
                all += 1;
                if p.trade() == economy::Trade::Farmer {
                    farming += 1;
                }
            }
            if all >= 8 {
                assert!(
                    farming * 4 >= all * 3,
                    "{} is {:.2} short and only {farming} of {all} are on the land",
                    place.name,
                    place.want
                );
            }
        }
    }

    #[test]
    fn what_a_place_owns_outlives_the_year_it_was_made_in() {
        // Capital, which §22 said this world could not have. A place that has been settled a
        // while holds tools it did not make this year.
        let world = lineages();
        let owning = world
            .places
            .ids()
            .filter(|id| world.holdings_of(*id).tools > 1.0)
            .count();
        assert!(owning > 0, "nowhere in the world owns anything");
    }

    #[test]
    fn what_somebody_does_for_a_living_has_a_name_in_their_own_language() {
        let world = lineages();
        let somebody = world
            .people
            .iter()
            .filter(|(_, p)| p.is_alive())
            .map(|(id, _)| id)
            .find(|id| world.trade_of(*id).is_some())
            .expect("somebody works for a living");
        let (trade, word) = world.trade_of(somebody).expect("just found");
        assert!(
            word.to_lowercase().ends_with(&trade.stem().to_lowercase()),
            "{word} is not a word for {}",
            trade.stem()
        );
        assert_eq!(world.trade_of(somebody), Some((trade, word)));
    }

    // ---- positions in a society (§26) ----------------------------------------------

    #[test]
    fn a_society_has_positions_in_it_and_most_people_are_ordinary() {
        let world = lineages();
        let biggest = world
            .places
            .ids()
            .max_by_key(|id| world.society.households_in(*id).count())
            .expect("a world has places");
        let read = world.society_of(biggest);
        assert!(read.len() > 10, "too few adults to have a society: {}", read.len());

        let mut tally: std::collections::BTreeMap<bonds::Role, usize> = Default::default();
        for (_, _, role) in &read {
            *tally.entry(*role).or_default() += 1;
        }
        let ordinary = tally.get(&bonds::Role::Householder).copied().unwrap_or(0);
        assert!(
            ordinary * 2 >= read.len() / 2,
            "a society where hardly anybody is ordinary is not a society: {tally:?}"
        );
        assert!(
            tally.len() >= 3,
            "one town, one kind of person: {tally:?}"
        );
    }

    #[test]
    fn nobody_is_a_patron_without_somebody_owing_them() {
        // The relation, not the neighbourhood of the measurements. Checked over a whole
        // world rather than a fixture, because the fixture cannot produce the case where
        // somebody has the most of everything and is owed nothing.
        let world = lineages();
        for id in world.places.ids() {
            for (who, _, role) in world.society_of(id) {
                let owed: f32 = world.bonds.of(who).map(|(_, t)| t.debt).sum();
                match role {
                    bonds::Role::Patron | bonds::Role::Elder => assert!(
                        owed > 0.0,
                        "{:?} is owed {owed}",
                        world.people.get(who).map(|p| &p.name)
                    ),
                    bonds::Role::Client => assert!(owed < 0.0, "a client who owes nothing"),
                    bonds::Role::Outcast => assert!(
                        world.bonds.repute_of(who) < 0.0,
                        "an outcast nobody thinks poorly of"
                    ),
                    _ => {}
                }
            }
        }
    }

    #[test]
    fn a_position_has_a_name_in_the_language_of_the_people_holding_it() {
        let world = lineages();
        let somebody = world
            .people
            .iter()
            .filter(|(_, p)| p.is_alive())
            .map(|(id, _)| id)
            .find(|id| world.standing_of(*id).is_some())
            .expect("somebody has a position");
        let (role, word) = world.standing_of(somebody).expect("just found");
        assert!(!word.is_empty());
        assert!(
            word.to_lowercase().ends_with(&role.stem().to_lowercase()),
            "{word} is not a word for {}",
            role.stem()
        );
        // And it is the same word every time it is asked, because it is derived rather
        // than drawn.
        assert_eq!(world.standing_of(somebody), Some((role, word)));
    }

    #[test]
    fn a_position_outlives_the_person_holding_it() {
        // The whole claim of §26. Nobody is made an elder and nobody succeeds to it: the
        // reading is taken again and it lands on whoever now fits. So the position survives
        // its holder with nothing anywhere that says it should.
        let mut world = World::genesis(WorldSeed::from_u128(0x11), 80);
        world.record_only(Salience::Pivotal);
        world.run_for(Duration::from_years(60));
        let biggest = |w: &World| {
            w.places
                .ids()
                .max_by_key(|id| w.society.households_in(*id).count())
                .expect("a world has places")
        };
        let notable = |w: &World| -> std::collections::BTreeSet<bonds::Role> {
            w.society_of(biggest(w))
                .into_iter()
                .map(|(_, _, role)| role)
                .filter(|role| *role != bonds::Role::Householder)
                .collect()
        };
        let before = notable(&world);
        let held_by: Vec<PersonId> = world
            .society_of(biggest(&world))
            .into_iter()
            .filter(|(_, _, r)| *r != bonds::Role::Householder)
            .map(|(who, _, _)| who)
            .collect();
        assert!(!before.is_empty(), "no positions to lose");

        world.run_for(Duration::from_years(75));
        let after = notable(&world);
        let survivors = held_by
            .iter()
            .filter(|w| world.people.get(**w).is_some_and(|p| p.is_alive()))
            .count();
        assert!(
            survivors * 3 < held_by.len(),
            "not enough of the holders died for this to say anything: {survivors} of {}",
            held_by.len()
        );
        assert!(
            !after.is_empty() && after.intersection(&before).count() > 0,
            "the positions died with the people in them: {before:?} then {after:?}"
        );
    }

    #[test]
    fn a_town_will_not_take_in_somebody_it_thinks_poorly_of() {
        // The only sanction in this world: no violence, no law, no court, just a door that
        // does not open. Checked as arithmetic rather than as an outcome, because whether a
        // *particular* world produces somebody shunned is a fact about that world's famines.
        let world = lineages();
        let somewhere = world.places.ids().next().expect("a world has places");
        let alive: Vec<PersonId> = world
            .people
            .iter()
            .filter(|(_, p)| p.is_alive())
            .map(|(id, _)| id)
            .collect();
        for who in &alive {
            let backing = world.backing(&[*who], somewhere);
            if world.repute_of(*who) < 0.4 {
                assert!(
                    backing < VOUCHING,
                    "being worse thought of than the town cost nothing at the door"
                );
            }
            assert!(
                (-VOUCHING..=VOUCHING).contains(&backing),
                "what a town thinks of somebody outweighed what they have: {backing}"
            );
        }
    }

    #[test]
    fn what_a_life_was_spent_on_does_not_depend_on_who_is_watching() {
        // §26 reads a position off how somebody spent their days, so if the two tiers
        // disagreed about that, an unwatched town would be full of different kinds of
        // people from a watched one.
        let (fine, coarse) = (at_detail(100_000, 25), at_detail(0, 25));
        let shares = |world: &World| {
            let mut totals = [0.0f64; Deed::COUNT];
            for (_, p) in world.people.iter() {
                if !p.is_alive() {
                    continue;
                }
                for deed in Deed::CHOSEN {
                    totals[deed as usize] += p.doings()[deed as usize] as f64;
                }
            }
            let all: f64 = totals.iter().sum();
            Deed::CHOSEN.map(|d| totals[d as usize] / all.max(1.0))
        };
        let (a, b) = (shares(&fine), shares(&coarse));
        for (at, deed) in Deed::CHOSEN.into_iter().enumerate() {
            assert!(
                (a[at] - b[at]).abs() < 0.06,
                "{}: {:.3} watched against {:.3} not",
                deed.label(),
                a[at],
                b[at]
            );
        }
    }

    // ---- society and politics (§25) -----------------------------------------------

    #[test]
    fn people_come_to_know_the_people_they_live_among() {
        // The first claim, and the one the rest stands on: a `Deed::Socialize` is spent
        // with somebody now, and who that somebody is follows from where you live.
        let world = lineages();
        let alive: Vec<PersonId> = world
            .people
            .iter()
            .filter(|(_, p)| p.is_alive())
            .map(|(id, _)| id)
            .collect();
        assert!(!world.bonds.is_empty(), "nobody knows anybody");

        let (mut near, mut far) = (0, 0);
        for who in &alive {
            let Some(home) = world.society.place_of(*who) else {
                continue;
            };
            for (other, tie) in world.bonds.of(*who) {
                if !tie.allied() {
                    continue;
                }
                if world.society.place_of(other) == Some(home) {
                    near += 1;
                } else {
                    far += 1;
                }
            }
        }
        assert!(near > 0, "nobody has a friend");
        assert!(
            near > far * 3,
            "friendships should mostly be with neighbours: {near} near, {far} far"
        );
    }

    #[test]
    fn patronage_has_a_patron_in_it() {
        // It used to be a coin flip with no mentor in it, and it is the largest single
        // lever on a life here. Every one of these now names somebody: older, better off,
        // and already acquainted.
        let world = lineages();
        let taken_up: Vec<(PersonId, PersonId)> = world
            .chronicle
            .iter()
            .filter_map(|r| match r.kind {
                Happening::PersonMentored { person, by } => Some((person, by)),
                _ => None,
            })
            .collect();
        assert!(!taken_up.is_empty(), "nobody ever found a patron");

        for (person, by) in &taken_up {
            assert_ne!(person, by, "somebody was their own patron");
            let (Some(young), Some(elder)) = (world.people.get(*person), world.people.get(*by))
            else {
                continue;
            };
            assert!(
                elder.born < young.born,
                "a patron younger than the person they took up"
            );
        }

        // And it is a thing that happens between two people, so it is in both their lives.
        let (person, by) = taken_up[0];
        assert!(
            world
                .life_of(by)
                .any(|r| matches!(r.kind, Happening::PersonMentored { person: p, .. } if p == person)),
            "taking somebody up left no mark on the patron's life"
        );
    }

    #[test]
    fn friends_inside_are_what_gets_you_in() {
        // Politics: what a household can bring to bear on a place is not only what it has.
        let world = lineages();
        let now = world.now();
        let places: Vec<PlaceId> = world.places.ids().collect();

        // Somebody with allies where they already live has backing there, and nobody has
        // backing anywhere from allies who do not live there.
        let mut backed_at_home = 0;
        for (id, person) in world.people.iter() {
            if !person.is_alive() || person.stage(now).is_dependent() {
                continue;
            }
            let Some(home) = world.society.place_of(id) else {
                continue;
            };
            let allies_at_home = world
                .bonds
                .of(id)
                .filter(|(other, tie)| {
                    tie.allied() && world.society.place_of(*other) == Some(home)
                })
                .count();
            // Signed: allies lend, and a reputation worse than the town's objects (§26.5).
            // So the ally term is what is checked here, not the total.
            let here = world.backing(&[id], home) - VOUCHING * ((world.repute_of(id) - 0.5) * 2.0).min(0.0);
            if allies_at_home > 0 {
                if here > 0.0 {
                    backed_at_home += 1;
                }
            } else {
                assert!(here.abs() < 1e-6, "somebody was vouched for by nobody: {here}");
            }

            // Somewhere they have no allies lends them nothing, whoever their friends are.
            for elsewhere in &places {
                if *elsewhere == home {
                    continue;
                }
                let allies_there = world
                    .bonds
                    .of(id)
                    .filter(|(other, tie)| {
                        tie.allied() && world.society.place_of(*other) == Some(*elsewhere)
                    })
                    .count();
                if allies_there == 0 {
                    let lift = world.backing(&[id], *elsewhere)
                        - VOUCHING * ((world.repute_of(id) - 0.5) * 2.0).min(0.0);
                    assert!(
                        lift.abs() < 1e-6,
                        "friends elsewhere spoke for somebody where they have none: {lift}"
                    );
                }
            }
        }
        assert!(
            backed_at_home > 0,
            "nobody in the world has anybody who would speak for them"
        );
    }

    #[test]
    fn hunger_given_away_is_hunger_somebody_else_goes_through() {
        // The conservation that keeps §21.2's brake exactly as strong as it was. A famine
        // that friendship could *lift* rather than move would be a population with no
        // ceiling, and this is the assertion that it does not.
        let mut world = World::genesis(WorldSeed::from_u128(0x11), 40);
        world.record_only(Salience::Pivotal);
        world.run_for(Duration::from_years(12));

        let people: Vec<PersonId> = world
            .people
            .iter()
            .filter(|(_, p)| p.is_alive())
            .map(|(id, _)| id)
            .collect();
        assert!(people.len() > 5);

        let taken = 0.4_f32;
        let before: f32 = world.shouldered.values().sum();
        let left = world.share_the_shortfall(people[0], taken);
        let after: f32 = world.shouldered.values().sum();
        assert!((0.0..=taken).contains(&left), "shortfall left {left}");
        assert!(
            ((taken - left) - (after - before)).abs() < 1e-4,
            "{:.4} was lifted off one person and {:.4} landed on others",
            taken - left,
            after - before
        );
    }

    #[test]
    fn who_your_friends_are_does_not_depend_on_who_is_watching() {
        // The §21.1 fault, applied to society: coarse people do not act, so on the naive
        // wiring looking away from a town would dissolve everybody's friendships and
        // looking back would rebuild them from nothing.
        //
        // **Twelve seeds, and the band is the measured floor** (§42.7). This ran on three,
        // with a comment reasoning that ties came out −33%, −8% and +18% coarse against fine
        // and that a mixed sign meant noise rather than bias. Measured properly, that
        // conclusion was an artefact of the sample size:
        //
        //     -36 -31 -27 -22 -13 -13 -11 -10 -4 +7 +8 +10
        //     mean -11.8%   sd 14.6   se 4.2
        //
        // The coarse tier really does hold about an eighth fewer acquaintances, at nearly
        // three standard errors from zero — a small, real, one-directional cost of not
        // deliberating over everybody, the same class of known gap as its fifth-larger
        // population in §21. At three seeds the standard error is 8.4 against a 20% band, so
        // the guard was about one error from its own bar and failed or passed with the
        // weather. It failed on an unrelated change, which is how this was found.
        //
        // §40.3's rule, applied to a test rather than to an instrument: a bar set inside its
        // own noise is not a bar.
        let society_of = |world: &World| {
            let alive: Vec<PersonId> = world
                .people
                .iter()
                .filter(|(_, p)| p.is_alive())
                .map(|(id, _)| id)
                .collect();
            let allies: usize = alive
                .iter()
                .map(|id| world.bonds.of(*id).filter(|(_, t)| t.allied()).count())
                .sum();
            (
                world.bonds.len() as f32 / alive.len().max(1) as f32,
                allies as f32 / alive.len().max(1) as f32,
            )
        };
        let (mut fine_ties, mut fine_allies) = (0.0, 0.0);
        let (mut coarse_ties, mut coarse_allies) = (0.0, 0.0);
        for seed in [
            0x11u128, 0x21, 0x31, 0x41, 0x5ee, 0x77, 0x8a, 0x91, 0xa3, 0xbb, 0xc7, 0x221,
        ] {
            let run = |budget: usize| {
                let mut world = World::genesis(WorldSeed::from_u128(seed), 60);
                world.record_only(Salience::Pivotal);
                world.set_detail_budget(budget);
                world.run_for(Duration::from_years(25));
                society_of(&world)
            };
            let ((ft, fa), (ct, ca)) = (run(100_000), run(0));
            fine_ties += ft;
            fine_allies += fa;
            coarse_ties += ct;
            coarse_allies += ca;
        }

        assert!(coarse_ties > 3.0, "an unwatched town knows nobody");
        // Mean −11.8% with a standard error of 4.2 over these twelve, so 30% is four errors
        // clear of the measurement — wide enough not to fail on the weather, tight enough
        // that a tier which stopped advancing ties at all could not get through it.
        assert!(
            (fine_ties - coarse_ties).abs() < 0.30 * fine_ties,
            "acquaintance drifted with the observer: {fine_ties:.1} watched, {coarse_ties:.1} not"
        );
        // Allies are the one measure here that is one-directional, and knowingly so: the
        // coarse tier has understated how tight a place is at every measurement ever taken
        // of it — −6%, −12% and −46% across these three seeds, and −32%, −14%, −1% before
        // the economy existed. Real, bounded, and the price of not deliberating over
        // everyone; the same class of known gap as the coarse tier's fifth-larger
        // population in §21. The band is set from the measurement rather than from taste.
        assert!(
            (fine_allies - coarse_allies).abs() < 0.35 * fine_allies,
            "friendship drifted with the observer: {fine_allies:.1} watched, {coarse_allies:.1} not"
        );
    }

    /// What a world comes to, and what it makes of itself.
    #[test]
    #[ignore]
    fn measure_what_the_world_comes_to() {
        let (mut alive, mut ever) = (0.0, 0.0);
        for seed in [0x11u128, 0x21, 0x31, 0x41, 0x51, 0x61] {
            let mut world = World::genesis(WorldSeed::from_u128(seed), 80);
            world.record_only(Salience::Pivotal);
            world.run_for(Duration::from_years(120));
            let now = world.now();
            alive += world.living() as f32;
            ever += world.people.len() as f32;

            let mut trades: std::collections::BTreeMap<&str, usize> = Default::default();
            let mut tools = 0.0;
            for (id, _) in world.places.iter() {
                tools += world.holdings_of(id).tools;
            }
            for (_, p) in world.people.iter() {
                if p.is_alive() && !p.stage(now).is_dependent() {
                    *trades.entry(p.trade().label()).or_default() += 1;
                }
            }
            let advances = world
                .chronicle
                .iter()
                .filter(|r| matches!(r.kind, Happening::PersonWorksItOut { .. }))
                .count();
            let frontier = world
                .places
                .ids()
                .map(|id| world.technique_of(id).reach_of_knowledge())
                .fold(0.0f32, f32::max);
            println!(
                "seed {seed:#x}: {} alive of {} ever, tools {tools:.0}, {advances} advances, frontier {frontier:.3}, {trades:?}",
                world.living(),
                world.people.len()
            );
            for (id, place) in world.places.iter().take(0) {
                let mut here: std::collections::BTreeMap<&str, usize> = Default::default();
                for member in world
                    .society
                    .households_in(id)
                    .flat_map(|(_, h)| h.members.iter())
                {
                    if let Some(p) = world.people.get(*member)
                        && p.is_alive()
                        && !p.stage(now).is_dependent()
                    {
                        *here.entry(p.trade().label()).or_default() += 1;
                    }
                }
                if here.is_empty() {
                    continue;
                }
                let ground = place
                    .terrain
                    .as_ref()
                    .map(|t| economy::ground_of(t, world.technique_of(id)));
                println!(
                    "    {:<14} {:<22} soil {:.2} {:>18}  ground {:?}",
                    place.name,
                    place.terrain.as_ref().map(|t| t.biome).unwrap_or(""),
                    place.terrain.as_ref().map(|t| t.fertility).unwrap_or(0.0),
                    format!("{here:?}"),
                    ground.map(|g| (g.food, g.stock)),
                );
            }
        }
        println!("mean alive {:.1}, mean ever {:.1}", alive / 6.0, ever / 6.0);
    }

    /// Whether anybody ever gets a year in which they can settle up.
    #[test]
    #[ignore]
    fn measure_whether_debts_are_ever_made_good() {
        let mut world = World::genesis(WorldSeed::from_u128(0x5ee), 120);
        world.record_only(Salience::Pivotal);
        world.set_detail_budget(100_000);
        world.run_for(Duration::from_years(150));

        let wants: Vec<f32> = world
            .places
            .iter()
            .filter(|(id, _)| world.society.households_in(*id).next().is_some())
            .map(|(_, p)| p.want)
            .collect();
        println!(
            "inhabited places: {:?}  ({} of them fed)",
            wants,
            wants.iter().filter(|w| **w <= 0.0).count()
        );

        let alive: Vec<PersonId> = world
            .people
            .iter()
            .filter(|(_, p)| p.is_alive())
            .map(|(id, _)| id)
            .collect();
        let (mut good, mut bad) = (0, 0);
        for who in &alive {
            for (_, tie) in world.bonds.of(*who) {
                if tie.regard > 0.0 {
                    good += 1;
                } else if tie.regard < 0.0 {
                    bad += 1;
                }
            }
        }
        println!("regards held: {good} good, {bad} ill");
        let owing = alive
            .iter()
            .filter(|w| world.bonds.of(**w).any(|(_, t)| t.debt < 0.0))
            .count();
        println!("{owing} of {} owe somebody something", alive.len());
    }

    /// What a finely simulated year is actually spent on.
    ///
    /// The coarse tier has to book the same tally for an unwatched life, and these are the
    /// numbers it books. Ignored: a measurement, not an assertion.
    #[test]
    #[ignore]
    fn measure_what_a_year_is_spent_on() {
        let budget: usize = std::env::var("DETAIL").ok().and_then(|v| v.parse().ok()).unwrap_or(100_000);
        let mut world = World::genesis(WorldSeed::from_u128(0x11), 60);
        world.record_only(Salience::Pivotal);
        world.set_detail_budget(budget);
        let years = 40;
        world.run_for(Duration::from_years(years));
        let now = world.now();

        let mut totals = [0.0f64; Deed::COUNT];
        let mut adult_years = 0.0f64;
        for (_, p) in world.people.iter() {
            if !p.is_alive() {
                continue;
            }
            let adult = (p.age(now).years() - 16.0).min(years as f64).max(0.0);
            if adult < 5.0 {
                continue;
            }
            adult_years += adult;
            for deed in Deed::CHOSEN {
                totals[deed as usize] += p.doings()[deed as usize] as f64;
            }
        }
        for deed in Deed::CHOSEN {
            println!(
                "{:>10}: {:.0} a year",
                deed.label(),
                totals[deed as usize] / adult_years.max(1.0)
            );
        }
    }

    /// Whether the tie graph could replace `bonding_capital`.
    ///
    /// §14 computes a place's bonding capital from churn and mean standing, which is a
    /// *model* of how densely its residents know each other. Now that they actually do know
    /// each other, the question is whether the measurement carries the same information —
    /// if it does, the formula should go. Ignored: a measurement, not an assertion.
    #[test]
    #[ignore]
    fn measure_whether_ties_could_replace_bonding_capital() {
        let mut world = World::genesis(WorldSeed::from_u128(0x11), 120);
        world.record_only(Salience::Pivotal);
        world.set_detail_budget(100_000);
        world.run_for(Duration::from_years(60));
        let now = world.now();

        let mut rows: Vec<(f32, f32, f32, usize)> = Vec::new();
        for (id, place) in world.places.iter() {
            let here: Vec<PersonId> = world
                .society
                .households_in(id)
                .flat_map(|(_, h)| h.members.iter().copied())
                .filter(|m| {
                    world
                        .people
                        .get(*m)
                        .is_some_and(|p| p.is_alive() && !p.stage(now).is_dependent())
                })
                .collect();
            if here.len() < 4 {
                continue;
            }
            let allies: usize = here
                .iter()
                .map(|who| {
                    world
                        .bonds
                        .of(*who)
                        .filter(|(other, tie)| {
                            tie.allied() && world.society.place_of(*other) == Some(id)
                        })
                        .count()
                })
                .sum();
            // Density, not headcount: the share of the people to hand that somebody
            // actually stands with. A raw count is a measure of how big the town is.
            let density = allies as f32 / (here.len() * (here.len() - 1)) as f32;
            rows.push((place.env.bonding_capital, density, place.env.churn, here.len()));
        }
        assert!(rows.len() > 2, "not enough inhabited places to compare");

        let mean = |v: &[f32]| v.iter().sum::<f32>() / v.len() as f32;
        let formula: Vec<f32> = rows.iter().map(|r| r.0).collect();
        let measured: Vec<f32> = rows.iter().map(|r| r.1).collect();
        let (mf, mm) = (mean(&formula), mean(&measured));
        let cov: f32 = formula
            .iter()
            .zip(&measured)
            .map(|(f, m)| (f - mf) * (m - mm))
            .sum();
        let sf: f32 = formula.iter().map(|f| (f - mf) * (f - mf)).sum::<f32>().sqrt();
        let sm: f32 = measured.iter().map(|m| (m - mm) * (m - mm)).sum::<f32>().sqrt();

        for (f, m, churn, n) in &rows {
            println!("  formula {f:.3}  density {m:.3}  churn {churn:.2}  n={n}");
        }
        println!(
            "formula spread {:.3}..{:.3}, density spread {:.3}..{:.3}, r = {:.2}",
            formula.iter().cloned().fold(f32::MAX, f32::min),
            formula.iter().cloned().fold(f32::MIN, f32::max),
            measured.iter().cloned().fold(f32::MAX, f32::min),
            measured.iter().cloned().fold(f32::MIN, f32::max),
            cov / (sf * sm).max(1e-6)
        );
    }

    /// What a year of company actually comes to, finely and coarsely.
    ///
    /// Ignored because it is a measurement rather than an assertion — run it when the
    /// utilities that pick a deed change, and move `EVENINGS_PER_YEAR` to what it says.
    #[test]
    #[ignore]
    fn measure_the_society_a_year_makes() {
        for (seed, budget) in [(0x11u128, 100_000usize), (0x11, 0), (0x21, 100_000), (0x21, 0), (0x31, 100_000), (0x31, 0)] {
            let mut world = World::genesis(WorldSeed::from_u128(seed), 60);
            world.record_only(Salience::Pivotal);
            world.set_detail_budget(budget);
            world.run_for(Duration::from_years(25));

            let alive: Vec<PersonId> = world
                .people
                .iter()
                .filter(|(_, p)| p.is_alive())
                .map(|(id, _)| id)
                .collect();
            let ties: usize = alive.iter().map(|id| world.bonds.count(*id)).sum();
            let close = alive
                .iter()
                .map(|id| world.bonds.of(*id).filter(|(_, t)| t.allied()).count())
                .sum::<usize>();
            let warmth: f32 = alive
                .iter()
                .flat_map(|id| world.bonds.of(*id))
                .map(|(_, t)| t.warmth)
                .sum();
            let circles = bonds::circles::circles(&world.bonds, &alive);
            let mut per_place: Vec<usize> = world
                .places
                .ids()
                .map(|id| {
                    world
                        .society
                        .households_in(id)
                        .flat_map(|(_, h)| h.members.iter())
                        .filter(|m| world.people.get(**m).is_some_and(|p| p.is_alive()))
                        .count()
                })
                .filter(|n| *n > 0)
                .collect();
            per_place.sort_unstable_by(|a, b| b.cmp(a));
            println!("  living per place: {per_place:?}");
            println!(
                "seed {seed:#x} budget {budget}: {} alive, {:.1} ties each, {:.1} allies each, mean warmth {:.3}, {} circles largest {}",
                alive.len(),
                ties as f32 / alive.len().max(1) as f32,
                close as f32 / alive.len().max(1) as f32,
                warmth / ties.max(1) as f32,
                circles.len(),
                circles.first().map_or(0, |c| c.members.len()),
            );
        }
    }

    #[test]
    fn coarse_places_stop_deliberating() {
        let coarse = at_detail(0, 20);
        let deeds = coarse
            .chronicle
            .iter()
            .filter(|r| matches!(r.kind, Happening::PersonDoes { .. }))
            .count();
        assert_eq!(deeds, 0, "nobody unwatched should be deliberating");
        assert!(
            coarse
                .places
                .iter()
                .all(|(id, _)| coarse.detail_of(id) == Detail::Coarse),
            "every place should have been demoted"
        );
        // But they are still alive, still ageing, still having children.
        assert!(coarse.living() > 20, "the world should still be running");
    }

    #[test]
    fn a_life_can_be_read_off_the_chronicle_directly() {
        // The point of the index: a biography is a lookup rather than a scan of the whole
        // log, and it contains that person's events and nobody else's.
        let world = lineages();
        let mut checked = 0;
        for (id, person) in world.people.iter() {
            let mine: Vec<&Happening> = world.life_of(id).map(|r| &r.kind).collect();
            if mine.is_empty() {
                continue;
            }
            checked += 1;
            for happening in mine {
                assert!(
                    happening.subjects().contains(&id.to_bits()),
                    "{}'s life contains something that is not about them",
                    person.name
                );
            }
        }
        assert!(checked > 10, "only {checked} people had any history at all");
    }

    #[test]
    fn a_birth_appears_in_three_lives() {
        let world = lineages();
        let born = world
            .chronicle
            .iter()
            .find_map(|r| match r.kind {
                Happening::PersonBorn {
                    child,
                    mother,
                    father,
                } => Some((child, mother, father)),
                _ => None,
            })
            .expect("nobody was born");

        for who in [born.0, born.1, born.2] {
            assert!(
                world.life_of(who).any(|r| matches!(
                    r.kind,
                    Happening::PersonBorn { child, .. } if child == born.0
                )),
                "the birth is missing from one of the three lives it is about"
            );
        }
    }

    #[test]
    fn compacting_keeps_the_pivotal_and_the_index_honest() {
        let mut world = World::genesis(WorldSeed::from_u128(0x33), 24);
        world.run_for(Duration::from_years(6));
        let pivotal = world.chronicle.at_least(Salience::Pivotal).count();
        assert!(pivotal > 5, "not enough happened to test on");
        assert!(
            world.chronicle.len() > 2000,
            "not enough routine to compact"
        );

        let budget = pivotal + 100;
        world.compact_chronicle(budget);
        assert!(world.chronicle.len() <= budget.max(pivotal));
        assert_eq!(
            world.chronicle.at_least(Salience::Pivotal).count(),
            pivotal,
            "compaction dropped something that mattered"
        );
        assert!(world.chronicle.forgotten_total() > 0);

        // And every life still reads as that person's.
        for (id, _) in world.people.iter() {
            for record in world.life_of(id) {
                assert!(record.kind.subjects().contains(&id.to_bits()));
            }
        }
    }

    #[test]
    fn watching_a_place_brings_it_back_into_focus() {
        let mut world = World::genesis(WorldSeed::from_u128(0x11), 60);
        world.set_detail_budget(0);
        world.run_for(Duration::from_years(5));

        let somewhere = world.places.ids().next().unwrap();
        assert_eq!(world.detail_of(somewhere), Detail::Coarse);

        // Fix the cast before watching. Asking who lives here *now* and applying that
        // to the whole back-catalogue counts a person's past only while they stay put,
        // so anyone who dies or is displaced in the meantime silently deletes their own
        // history and the total can fall. The chronicle only grows; the measure of it
        // should only grow too.
        let watched: BTreeSet<PersonId> = world
            .society
            .households_in(somewhere)
            .flat_map(|(_, h)| h.members.iter().copied())
            .collect();
        assert!(!watched.is_empty(), "nobody lives in the watched place");
        let deeds = |world: &World| {
            world
                .chronicle
                .iter()
                .filter(|r| {
                    matches!(r.kind, Happening::PersonDoes { person, .. }
                    if watched.contains(&person))
                })
                .count()
        };

        let before = deeds(&world);
        world.watch(somewhere);
        assert_eq!(world.detail_of(somewhere), Detail::Full);
        world.run_for(Duration::from_days(4));

        let after = deeds(&world);
        assert!(
            after > before,
            "a watched place should start deliberating again: {before} then {after}"
        );

        // And everywhere else stays coarse.
        assert!(
            world
                .places
                .ids()
                .filter(|id| *id != somewhere)
                .all(|id| world.detail_of(id) == Detail::Coarse)
        );
    }

    #[test]
    fn a_coarse_year_produces_the_year_a_fine_one_would_have() {
        // The consistency contract, and the whole justification for the tier. A
        // population simulated coarsely has to end up where the same population
        // simulated finely ends up, or looking away from somewhere quietly changes it.
        let years = 30;
        let (fine, coarse) = (at_detail(usize::MAX, years), at_detail(0, years));

        let mean_standing = |world: &World| {
            let adults: Vec<f32> = world
                .people
                .iter()
                .filter(|(_, p)| p.is_alive() && !p.stage(world.now()).is_dependent())
                .map(|(_, p)| p.standing())
                .collect();
            adults.iter().sum::<f32>() / adults.len().max(1) as f32
        };

        let (a, b) = (mean_standing(&fine), mean_standing(&coarse));
        // Tight, because a loose bound here hides exactly the kind of fault it exists to
        // catch: at a tenth of absolute standing this passed for a long time while the
        // coarse tier was paying a fifth too little, since the shortfall is proportional
        // and only opens up over a lifetime.
        assert!(
            (a - b).abs() < 0.04,
            "coarse living drifted from fine: {a:.3} finely, {b:.3} coarsely"
        );

        // Demography has to survive the projection too — but only approximately, and
        // the gap widens with time. A coarsely lived year keeps needs at the level a
        // competent adult maintains, so nobody unwatched ever has a bad month; health
        // stays a little higher, and through the fertility check that means a little
        // more childbearing. Measured at 150 years the coarse population runs about a
        // fifth larger. Real, known, and the price of not deliberating over everyone.
        let living = |w: &World| w.living() as f32;
        let ratio = living(&coarse) / living(&fine).max(1.0);
        assert!(
            (0.6..1.6).contains(&ratio),
            "populations diverged: {} finely, {} coarsely",
            living(&fine),
            living(&coarse)
        );
    }

    #[test]
    fn coarse_is_far_cheaper() {
        // The point of the exercise. Measured in events rather than seconds, so it
        // means the same thing on any machine.
        let (fine, coarse) = (at_detail(usize::MAX, 20), at_detail(0, 20));
        let events = |w: &World| w.chronicle.len();
        assert!(
            coarse.chronicle.len() * 4 < fine.chronicle.len().max(1) || events(&coarse) < 400,
            "coarse should cost a fraction: {} vs {}",
            events(&coarse),
            events(&fine)
        );
    }

    #[test]
    fn time_advances_to_the_horizon_even_when_quiet() {
        let mut world = World::new(WorldSeed::from_u128(1));
        world.run_for(Duration::from_myr(1));
        assert_eq!(world.now(), FOUNDING + Duration::from_myr(1));
        assert!(world.chronicle.is_empty());
    }

    #[test]
    fn elders_exist_after_enough_time() {
        let mut world = World::genesis(WorldSeed::from_u128(12), 20);
        world.record_only(Salience::Pivotal);
        world.run_for(Duration::from_years(10));
        let now = world.now();
        assert!(
            world
                .people
                .iter()
                .any(|(_, p)| p.is_alive() && p.stage(now) == LifeStage::Elder),
            "people should have aged into their sixties"
        );
    }

    // ── the join: people standing on a planet ────────────────────────────────────

    #[test]
    fn a_world_stands_on_a_planet() {
        let world = World::genesis(WorldSeed::from_u128(0x101), 20);
        let surface = world.surface().expect("a founded world has ground under it");
        assert!(surface.planet.land_fraction() > 0.0, "a planet with no land");
        assert!(surface.planet.grid().len() > 100);
        // And it is a planet somebody could live on rather than a snowball or a furnace.
        let mean = surface.climate.mean_temperature_c(&surface.planet);
        assert!(
            (-10.0..45.0).contains(&mean),
            "the world was founded on a planet at {mean:.1} °C"
        );
    }

    #[test]
    fn every_quarter_stands_on_dry_land_in_one_country() {
        let world = World::genesis(WorldSeed::from_u128(0x102), 30);
        let surface = world.surface().unwrap();
        let cells: Vec<u32> = world
            .places
            .iter()
            .filter_map(|(_, p)| p.terrain.as_ref().map(|t| t.cell))
            .collect();
        assert_eq!(cells.len(), world.places.len(), "a quarter with no ground");

        for &cell in &cells {
            assert!(surface.planet.is_land(cell), "cell {cell} is under water");
            assert!(!surface.life.biome(cell).is_marine());
        }
        // Neighbourhoods of one society, not five civilisations sharing a chronicle.
        for &cell in &cells[1..] {
            let apart = surface
                .planet
                .grid()
                .distance_km(cells[0], cell, geo::EARTH_RADIUS_KM);
            assert!(apart < 6_000.0, "a quarter {apart:.0} km from the others");
        }
    }

    #[test]
    fn a_world_has_a_people_and_they_are_named_after_where_they_live() {
        // The end of the eight-country enum. Nothing in the codebase now knows the name of
        // a single people, so the only thing that can be asserted is the shape: there is
        // one, it is called something, and what it is called came out of the ground.
        let mut world = World::genesis(WorldSeed::from_u128(0x103), 40);
        world.run_for(Duration::from_years(3));

        let peoples = world.peoples();
        assert!(!peoples.is_empty(), "a populated world has no peoples");
        let first = &peoples[0];
        assert!(!first.name.is_empty());
        assert_eq!(first.parent, None, "the first people came from somewhere");
        assert!(
            world.places.iter().any(|(_, p)| p.name == first.name),
            "the first people are not named after anywhere in this world",
        );
    }

    #[test]
    fn nationality_is_where_you_live_and_not_a_thing_you_carry() {
        // The property the enum could never have. Somebody's country is looked up from the
        // place they are in, so it is the same for everybody in that place and it is not
        // inherited from a mother who has never been there.
        let mut world = World::genesis(WorldSeed::from_u128(0x104), 40);
        world.run_for(Duration::from_years(2));

        let mut seen: std::collections::BTreeMap<PlaceId, String> =
            std::collections::BTreeMap::new();
        let mut anybody = false;
        for (id, person) in world.people.iter() {
            if !person.is_alive() {
                continue;
            }
            let Some(place) = world.society.place_of(id) else {
                continue;
            };
            let Some(country) = world.country_of(id) else {
                continue;
            };
            anybody = true;
            if let Some(before) = seen.get(&place) {
                assert_eq!(
                    *before, country,
                    "two people in one place are from different countries",
                );
            } else {
                seen.insert(place, country);
            }
        }
        assert!(anybody, "nobody in this world is from anywhere");
    }

    #[test]
    fn every_inhabited_place_is_in_exactly_one_country() {
        let mut world = World::genesis(WorldSeed::from_u128(0x105), 40);
        world.run_for(Duration::from_years(2));

        let countries = world.countries();
        assert!(!countries.is_empty(), "an inhabited world has no countries");

        let mut counted: Vec<usize> = countries.iter().flat_map(|c| c.places.clone()).collect();
        let unique: std::collections::BTreeSet<usize> = counted.iter().copied().collect();
        assert_eq!(counted.len(), unique.len(), "a place is in two countries");
        counted.sort_unstable();

        for at in &counted {
            assert!(
                world.souls_at(*at).unwrap_or(0) > 0,
                "an empty place is in a country",
            );
        }
        for country in &countries {
            assert!(!country.name.is_empty(), "a country with no name");
            assert!(!country.places.is_empty());
        }
    }

    #[test]
    fn a_places_norms_outlive_the_year_that_made_them() {
        // The loop §14 could not close. Norms used to be rebuilt from scratch each
        // reckoning, so a place could not carry a way of doing things through a
        // generation; now they carry, and the proof is that they are no longer equal to
        // what people did this year.
        let mut world = World::genesis(WorldSeed::from_u128(0x106), 60);
        world.run_for(Duration::from_years(6));

        let carried = world
            .places
            .iter()
            .filter(|(id, p)| {
                p.terrain.is_some() && world.society.households_in(*id).count() > 0
            })
            .any(|(_, p)| p.env.norms.iter().any(|n| (n - 0.5).abs() > 0.001));
        assert!(
            carried,
            "no inhabited place carries any way of doing things at all",
        );
    }

    #[test]
    fn two_worlds_are_founded_on_two_different_planets() {
        // The non-determinism promise, now reaching all the way down to the ground: a
        // new world is a new planet with different continents and differently named
        // towns on them, not the same map with the names shuffled.
        let a = World::genesis(WorldSeed::from_u128(0x201), 20);
        let b = World::genesis(WorldSeed::from_u128(0x202), 20);

        let names = |w: &World| -> Vec<String> {
            w.places.iter().map(|(_, p)| p.name.clone()).collect()
        };
        assert_ne!(names(&a), names(&b));

        let land = |w: &World| w.surface().unwrap().planet.land_fraction();
        assert_ne!(land(&a), land(&b), "two worlds got the identical planet");
    }

    #[test]
    fn the_ground_does_not_move_while_people_live_on_it() {
        // A still frame, deliberately: the continents move on a clock eleven rungs above
        // this one, and a century of human history is not a measurable interval to them.
        let mut world = World::genesis(WorldSeed::from_u128(0x203), 20);
        let before: Vec<f32> = world
            .surface()
            .unwrap()
            .planet
            .grid()
            .cells()
            .map(|c| world.surface().unwrap().planet.height_above_sea_m(c))
            .collect();
        world.record_only(Salience::Pivotal);
        world.run_for(Duration::from_years(80));
        let after: Vec<f32> = world
            .surface()
            .unwrap()
            .planet
            .grid()
            .cells()
            .map(|c| world.surface().unwrap().planet.height_above_sea_m(c))
            .collect();
        assert_eq!(before, after);
    }

    #[test]
    fn the_ground_shapes_the_quarters_it_holds() {
        // Not that the best-sited quarter is the richest — that is up to who lives there
        // — but that the ground is doing something rather than being decoration.
        let world = World::genesis(WorldSeed::from_u128(0x204), 40);
        let mut best: Option<(&society::Terrain, f32)> = None;
        let mut worst: Option<(&society::Terrain, f32)> = None;
        for (_, place) in world.places.iter() {
            let Some(terrain) = &place.terrain else {
                continue;
            };
            let ceiling = terrain.prosperity_ceiling();
            if best.is_none_or(|(_, b)| ceiling > b) {
                best = Some((terrain, ceiling));
            }
            if worst.is_none_or(|(_, b)| ceiling < b) {
                worst = Some((terrain, ceiling));
            }
        }
        let (_, high) = best.unwrap();
        let (_, low) = worst.unwrap();
        assert!(
            high > low + 0.03,
            "every quarter sits on identical ground ({low:.3}..{high:.3})"
        );

        // And a quarter's opportunity is held under what its ground allows.
        for (_, place) in world.places.iter() {
            let Some(terrain) = &place.terrain else {
                continue;
            };
            let allowed = 0.15 + 0.85 * terrain.prosperity_ceiling();
            assert!(
                place.env.job_opportunity <= allowed + 1e-4,
                "{} offers more work than its ground has",
                place.name
            );
        }
    }

    #[test]
    fn a_world_goes_round_a_real_star() {
        let world = World::genesis(WorldSeed::from_u128(0x301), 20);
        let surface = world.surface().unwrap();
        let star = surface.star();
        let orbit = surface.orbit();

        assert!((cosmos::LIGHTEST_STAR..=cosmos::HEAVIEST_STAR).contains(&star.mass_solar));
        assert!(star.remaining_gyr() > 0.0, "founded a world round a dead star");
        assert!(star.age_gyr < cosmos::UNIVERSE_GYR, "a star older than everything");
        assert!(orbit.is_rocky(), "people were put on a gas giant");
        assert!(cosmos::habitability::zone(&star).holds(orbit.semi_major_au));
        // And the climate knows about it rather than assuming a sun.
        assert_eq!(surface.climate.star(), Some(star));
    }

    #[test]
    fn nobody_is_founded_on_a_snowball() {
        // The reason `LIVEABLE_FLUX` is narrower than the astronomy. Before it, a world
        // at two thirds of Earth's light passed the habitable-zone test and then froze
        // solid at forty below with its carbon dioxide pinned at the model's ceiling.
        for seed in [0x401u128, 0x402, 0x403, 0x404, 0x405, 0x406] {
            let world = World::genesis(WorldSeed::from_u128(seed), 12);
            let surface = world.surface().unwrap();
            let ice = surface.climate.ice_fraction(&surface.planet);
            let mean = surface.climate.mean_temperature_c(&surface.planet);
            assert!(
                ice < 0.6 && mean > -5.0,
                "seed {seed:#x} founded a world at {mean:.1} °C with {:.0}% ice",
                ice * 100.0
            );
            // And people ended up somewhere real rather than on the abstract fallback.
            assert!(
                world.places.iter().all(|(_, p)| p.terrain.is_some()),
                "seed {seed:#x} fell back to abstract quarters"
            );
        }
    }

    #[test]
    fn the_star_a_world_gets_is_drawn_rather_than_chosen() {
        // Different seeds, different skies. If every world came out round the sun the
        // search would be theatre.
        let stars: Vec<cosmos::Star> = [0x501u128, 0x502, 0x503, 0x504]
            .iter()
            .map(|&s| World::genesis(WorldSeed::from_u128(s), 8).surface().unwrap().star())
            .collect();
        let distinct: std::collections::BTreeSet<String> =
            stars.iter().map(|s| format!("{:.4}", s.mass_solar)).collect();
        assert!(distinct.len() > 1, "every world got the same star: {stars:?}");
    }

    #[test]
    fn the_air_on_a_founded_world_is_air_a_person_could_breathe() {
        // The constraint that actually binds, and it is not temperature. Everywhere from
        // three quarters to a quarter again of Earth's light comes out temperate, because
        // that is what a thermostat is for; what varies is the atmosphere it needs in
        // order to manage it. At nine tenths of Earth's light this planet is a comfortable
        // thirteen degrees under seven per cent carbon dioxide, which is four times the
        // concentration that kills a human being.
        const LETHAL_PPM: f32 = 10_000.0; // one per cent
        const PHOTOSYNTHESIS_FLOOR_PPM: f32 = 150.0;
        for seed in [0x701u128, 0x702, 0x703, 0x704] {
            let world = World::genesis(WorldSeed::from_u128(seed), 12);
            let air = world.surface().unwrap().climate.co2_ppm();
            assert!(
                air < LETHAL_PPM,
                "seed {seed:#x} founded people under {air:.0} ppm of carbon dioxide"
            );
            assert!(
                air > PHOTOSYNTHESIS_FLOOR_PPM,
                "seed {seed:#x} founded a world with {air:.0} ppm — nothing grows below 150"
            );
        }
    }

    #[test]
    fn the_light_a_world_gets_is_what_its_star_and_orbit_imply() {
        let world = World::genesis(WorldSeed::from_u128(0x601), 12);
        let surface = world.surface().unwrap();
        let expected =
            surface.star().flux_at_au(surface.orbit().semi_major_au) / cosmos::SOLAR_CONSTANT_WM2;
        assert!(
            (surface.climate.brightness() - expected).abs() < 1e-9,
            "the climate is lit by {} and the star supplies {expected}",
            surface.climate.brightness()
        );
        assert!(LIVEABLE_FLUX.contains(&expected), "{expected} is outside the band");
    }
}

#[cfg(test)]
mod the_land_holds {
    //! A population has to stop somewhere, and the somewhere has to be the ground.
    //!
    //! For a long time nothing stopped one. `births_relative` is centred on the world's own
    //! middle, so it averages one by construction and can only decide *where* children are
    //! born — a uniformly poorer world gets a multiplier of one everywhere. Crowding acts on
    //! where households live, not on how many there are. So worlds ran to three and five
    //! times what their ground would hold and growth accelerated to nearly two per cent a
    //! year, for ever.
    //!
    //! The missing reading was being computed and thrown away: `prosperity` is
    //! `per_head().max(0)`, so a place in famine reported exactly what a place that just
    //! broke even reported. `Ledger::want` is that clamp undone.

    use super::*;

    /// Whether a world ever grows more than one people.
    #[test]
    #[ignore]
    fn measure_whether_peoples_diverge() {
        for (seed, founders, years) in [
            (0x11u128, 80usize, 180u64),
            (0x21, 80, 180),
            (0x21, 400, 150),
            (0x22, 400, 150),
        ] {
            let mut world = World::genesis(WorldSeed::from_u128(seed), founders);
            world.run_for(Duration::from_years(years));
            let living: Vec<&culture::Culture> =
                world.peoples().iter().filter(|p| p.living()).collect();
            let names: Vec<&str> = living.iter().map(|p| p.name.as_str()).collect();
            let biggest = world
                .countries()
                .first()
                .map(|c| c.places.iter().filter_map(|a| world.souls_at(*a)).sum::<u32>())
                .unwrap_or(0);
            let know = world
                .places
                .iter()
                .map(|(id, _)| world.technique_of(id).level())
                .fold(0.0f32, f32::max);
            println!(
                "seed {seed:x} ({founders} founders, {years} yr): {:>5} living, {} peoples {:?}, \
{} countries, biggest {biggest}, technique {know:.4}",
                world.living(),
                living.len(),
                names,
                world.countries().len(),
            );
        }
    }

    /// Whether a world can ever hold enough minds together to learn anything.
    #[test]
    #[ignore]
    fn measure_whether_the_trap_ever_opens() {
        let centuries: u64 = std::env::var("CENTURIES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);
        let founders: usize = std::env::var("FOUNDERS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(120);
        let seed: u128 = std::env::var("SEED")
            .ok()
            .and_then(|v| u128::from_str_radix(&v, 16).ok())
            .unwrap_or(0x221);
        let mut world = World::genesis(WorldSeed::from_u128(seed), founders);
        world.record_only(Salience::Historic);
        world.set_detail_budget(0);
        for century in 1..=centuries {
            world.run_for(Duration::from_years(100));
            let biggest = world
                .countries()
                .first()
                .map(|c| c.places.iter().filter_map(|a| world.souls_at(*a)).sum::<u32>())
                .unwrap_or(0);
            let best = world
                .places
                .iter()
                .map(|(id, _)| world.technique_of(id).level())
                .fold(0.0f32, f32::max);
            let frontier = world
                .places
                .ids()
                .map(|id| world.technique_of(id).reach_of_knowledge())
                .fold(0.0f32, f32::max);
            let advances = world
                .chronicle
                .iter()
                .filter(|r| matches!(r.kind, Happening::PersonWorksItOut { .. }))
                .count();
            let reachable = world
                .neighbourhoods()
                .iter()
                .map(|g| g.iter().filter_map(|at| world.souls_at(*at)).sum::<u32>())
                .max()
                .unwrap_or(0);
            // How short the world is, which is the number that says whether the trap is
            // still shut. A world at its ceiling has somebody going without.
            let short = world
                .places
                .iter()
                .filter(|(id, _)| world.society.households_in(*id).next().is_some())
                .map(|(_, p)| p.want)
                .fold(0.0f32, f32::max);
            println!(
                "year {:>5}: living {:>5} country {:>5} in touch {reachable:>5} practised {:.3} frontier {:.3} advances {advances} short {short:.3}",
                century * 100,
                world.living(),
                biggest,
                best,
                frontier,
            );
        }
    }

    /// How far apart the quarters of one world actually stand.
    #[test]
    #[ignore]
    fn measure_how_far_apart_quarters_are() {
        for seed in [0x230u128, 0x231, 0x232] {
            let world = World::genesis(WorldSeed::from_u128(seed), 40);
            let surface = world.surface().unwrap();
            let grid = surface.planet.grid();
            let cells: Vec<u32> = world
                .places
                .iter()
                .filter_map(|(_, p)| p.terrain.as_ref().map(|t| t.cell))
                .collect();
            let mut gaps: Vec<f64> = Vec::new();
            for (i, a) in cells.iter().enumerate() {
                for b in &cells[i + 1..] {
                    gaps.push(grid.distance_km(*a, *b, geo::EARTH_RADIUS_KM));
                }
            }
            gaps.sort_by(f64::total_cmp);
            println!(
                "seed {seed:x}: grid level {} spacing {:.0} km | quarter gaps min {:.0} median {:.0} max {:.0} km",
                grid.level(),
                grid.spacing_km(geo::EARTH_RADIUS_KM),
                gaps.first().copied().unwrap_or(0.0),
                gaps[gaps.len() / 2],
                gaps.last().copied().unwrap_or(0.0),
            );
        }
    }

    /// What the ceiling costs, across seeds. A measurement, not an assertion.
    #[test]
    #[ignore]
    fn measure_the_ceiling() {
        println!(
            "{:>7} {:>7} {:>7} {:>7} {:>8} {:>8} {:>8}",
            "seed", "yr90", "yr180", "growth", "want", "know", "biggest"
        );
        for seed in [0x220u128, 0x221, 0x222, 0x223, 0x224, 0x225] {
            let mut world = World::genesis(WorldSeed::from_u128(seed), 60);
            world.run_for(Duration::from_years(90));
            let early = world.living();
            world.run_for(Duration::from_years(90));
            let late = world.living();
            let want: f32 = {
                let lived: Vec<f32> = world
                    .places
                    .iter()
                    .filter(|(id, _)| world.society.households_in(*id).count() > 0)
                    .map(|(_, p)| p.want)
                    .collect();
                if lived.is_empty() { 0.0 } else { lived.iter().sum::<f32>() / lived.len() as f32 }
            };
            let growth = (late as f32 / early.max(1) as f32).powf(1.0 / 90.0) - 1.0;
            let (know, biggest) = {
                let lived: Vec<f32> = world
                    .places
                    .iter()
                    .filter(|(id, _)| world.society.households_in(*id).count() > 0)
                    .map(|(id, _)| world.technique_of(id).level())
                    .collect();
                let best = world
                    .countries()
                    .first()
                    .map(|c| c.places.iter().filter_map(|a| world.souls_at(*a)).sum::<u32>())
                    .unwrap_or(0);
                (
                    if lived.is_empty() { 1.0 } else { lived.iter().cloned().fold(0.0f32, f32::max) },
                    best,
                )
            };
            println!(
                "{seed:>7x} {early:>7} {late:>7} {:>+6.2}% {want:>8.3} {know:>8.3} {biggest:>8}",
                growth * 100.0
            );
        }
    }

    #[test]
    fn a_people_large_enough_to_carry_a_technique_improves_on_it() {
        // Wiring the Tasmanian result to the thing that decides it. Technique is advanced
        // per *country*, because a country is the set of people who can reach each other to
        // copy a technique from — which is exactly the quantity `MINDS_TO_KEEP` is about.
        let mut world = World::genesis(WorldSeed::from_u128(0x230), 60);
        world.run_for(Duration::from_years(120));

        let known: Vec<f32> = world
            .places
            .iter()
            .filter(|(id, _)| world.society.households_in(*id).count() > 0)
            .map(|(id, _)| world.technique_of(id).level())
            .collect();
        assert!(!known.is_empty(), "nobody lives anywhere");
        // Nobody forgets how to eat, whatever else happens.
        for level in &known {
            assert!(*level >= 1.0, "somebody forgot how to farm: {level}");
        }
        // And a world of a few hundred is nowhere near the threshold, so it should be at
        // or near bare — this is the Malthusian trap staying shut, not a bug.
        assert!(
            known.iter().all(|l| *l < 1.5),
            "a few hundred people invented their way out of subsistence",
        );
    }

    #[test]
    fn founding_a_world_crowded_does_not_kill_it() {
        // The fault I put in and had to take out again. `is_fertile` gates at a vitality of
        // a half, so hunger deep enough to reach that gate does not slow births, it stops
        // them — a cliff, and the very shape this whole mechanism exists to avoid. With
        // `HUNGER_COSTS` at 1.4 the gate sat at a want of 0.36, which any world founded on
        // ground that was already full reaches immediately. Four hundred founders came to
        // 86 souls where eighty founders on the same seed grew to 373: starting crowded was
        // fatal, and the more people you began with the fewer you ended with.
        let crowded = {
            let mut world = World::genesis(WorldSeed::from_u128(0x21), 400);
            world.run_for(Duration::from_years(120));
            world.living()
        };
        let sparse = {
            let mut world = World::genesis(WorldSeed::from_u128(0x21), 80);
            world.run_for(Duration::from_years(120));
            world.living()
        };
        assert!(
            crowded > sparse / 2,
            "founding with four hundred left {crowded} where founding with eighty left {sparse}",
        );
    }

    #[test]
    fn going_short_costs_people_their_health() {
        // The mechanism stated where it can be seen, rather than as a claim about where a
        // population ends up. Whether a *particular* world levels off inside two centuries
        // depends on how much room its land had to begin with, and asserting a number there
        // would be asserting a fact about one seed's geography — §21.2 measures six.
        //
        // What is always true is the link itself: a place that cannot feed its people leaves
        // them in worse condition than one that can, and condition is what feeds back into
        // births and deaths.
        //
        // It needs a world where somewhere is actually short, and sixty founders stopped
        // being one. §30 fixed a migration rule that had been sorting people into a quarter
        // that could not feed them, and the worlds that followed are markedly better fed —
        // one seed went from 326 alive at year 220 to 630. At sixty founders every inhabited
        // place now reads `want` of exactly zero, and the test says so itself rather than
        // passing on a world with nothing to compare. Three hundred is where the ground is
        // strained again: measured at 120 years, wants of 0.00, 0.23, 0.03 and 0.00 across
        // four inhabited quarters.
        let mut world = World::genesis(WorldSeed::from_u128(0x222), 300);
        world.run_for(Duration::from_years(120));
        let now = world.now();

        let condition = |place: PlaceId| {
            let people: Vec<f32> = world
                .society
                .households_in(place)
                .flat_map(|(_, h)| h.members.iter())
                .filter_map(|m| world.people.get(*m))
                .filter(|p| p.is_alive() && !p.stage(now).is_dependent())
                .map(|p| p.health().vitality)
                .collect();
            (!people.is_empty()).then(|| people.iter().sum::<f32>() / people.len() as f32)
        };

        let mut lived_in: Vec<(PlaceId, f32, f32)> = world
            .places
            .iter()
            .filter_map(|(id, place)| condition(id).map(|health| (id, place.want, health)))
            .collect();
        assert!(lived_in.len() > 1, "only one place is inhabited, nothing to compare");

        lived_in.sort_by(|a, b| a.1.total_cmp(&b.1));
        let (_, least_want, best_fed) = lived_in[0];
        let (_, most_want, worst_fed) = *lived_in.last().unwrap();
        assert!(
            most_want > least_want,
            "every inhabited place is equally fed, so this world says nothing",
        );
        assert!(
            worst_fed < best_fed,
            "the hungriest place ({most_want:.2} short) is in no worse condition \
({worst_fed:.2}) than the best fed one ({least_want:.2} short, {best_fed:.2})",
        );
        // And visibly so in somebody's body, rather than as a rounding error on an average.
        //
        // This used to test the mean and the mean stopped being the right thing to look at
        // when §25.3 made a famine something neighbours share out. Redistribution does not
        // change how much hunger there is — it is conserved to the day — but it concentrates
        // it, so a place a sixth short now has most of its people untouched and a few
        // carrying the lot. Averaged, that reads as a healthy town. The claim worth making
        // is the one the model actually supports: where the land is thin, there are people
        // in visibly worse condition, and there are more of them than where it is not.
        let short_of_hale = |place: PlaceId| {
            let people: Vec<f32> = world
                .society
                .households_in(place)
                .flat_map(|(_, h)| h.members.iter())
                .filter_map(|m| world.people.get(*m))
                .filter(|p| p.is_alive() && !p.stage(now).is_dependent())
                .map(|p| p.health().vitality)
                .collect();
            let hungry = people.iter().filter(|v| **v < 0.9).count();
            (hungry, people.len())
        };
        let (hungry_here, of_them) = short_of_hale(lived_in.last().unwrap().0);
        let (hungry_there, of_those) = short_of_hale(lived_in[0].0);
        assert!(
            hungry_here > 0,
            "the hungriest place is {most_want:.2} short and nobody in it is short of hale",
        );
        assert!(
            hungry_here as f32 / of_them.max(1) as f32
                > hungry_there as f32 / of_those.max(1) as f32,
            "{hungry_here} of {of_them} are short of hale where the land is thin, against {hungry_there} of {of_those} where it is not",
        );
    }

    #[test]
    fn hunger_is_what_stops_it_and_it_is_felt_where_the_land_is_thin() {
        // The mechanism, not just the outcome. Somewhere in a settled world people are
        // going short, and going short is what closes the fertility gate.
        // **Founded crowded, rather than waiting for a world to fill.**
        //
        // This used to found sixty people and run until somebody went short, and the horizon
        // had to be pushed out every single time anything raised what the land could produce:
        // a hundred and twenty years, then two hundred for the tools of §27, and §28's better
        // use of the ground would have wanted more again. That is a treadmill, and a test on a
        // treadmill is measuring the horizon rather than the mechanism.
        //
        // What the claim is actually about is that a place which cannot feed its people leaves
        // them short — so the honest thing is to put more people on the ground than it will
        // carry and look, rather than to breed them slowly and hope. `founding_a_world_crowded_does_not_kill_it`
        // already establishes that a crowded founding is survivable, so this asks what such a
        // world *feels* like rather than whether it exists.
        //
        // Four hundred was enough until households stopped commuting (§30.4), then two
        // thousand until §17.2.1 changed what people take to be normal, and at that point
        // this had been re-fixtured twice in a day by the very treadmill its own comment
        // warns about. Every improvement to how well the world feeds itself raises the
        // founding population this needs, and picking a new number each time is the horizon
        // measuring itself.
        //
        // So it *looks* for the crowding instead of assuming it. Found a world it should
        // strain, then run until somewhere is short, and fail only if that never happens
        // inside a horizon nothing plausible could need. As the world gets better fed this
        // takes longer and keeps meaning the same thing, which is what the earlier versions
        // could not do.
        let mut world = World::genesis(WorldSeed::from_u128(0x221), 2_000);
        world.record_only(Salience::Pivotal);

        let short_somewhere = |world: &World| {
            world
                .places
                .iter()
                .any(|(id, p)| world.society.households_in(id).count() > 0 && p.want > 0.0)
        };
        let mut waited = 0;
        while waited < 200 && !short_somewhere(&world) {
            world.run_for(Duration::from_years(20));
            waited += 20;
        }

        let inhabited: Vec<&society::Place> = world
            .places
            .iter()
            .filter(|(id, _)| world.society.households_in(*id).count() > 0)
            .map(|(_, place)| place)
            .collect();
        assert!(!inhabited.is_empty(), "nobody lives anywhere");
        assert!(
            inhabited.iter().any(|p| p.want > 0.0),
            "two thousand people on this ground, {waited} years, and nobody is short of \
anything — either the land has stopped binding or hunger has stopped being felt",
        );
        // And nowhere is so far gone that the model has stopped meaning anything.
        for place in &inhabited {
            assert!(
                place.want < 0.9,
                "{} is short of {:.2} of everything its people need",
                place.name,
                place.want,
            );
        }
    }
}

#[cfg(test)]
mod detail_neutrality {
    //! The level-of-detail machinery must not decide what happens.
    //!
    //! §19 calls scale-crossing equivalence the riskiest property in the design, and for a
    //! long time it was silently false: the same world, same seed, run with different
    //! amounts of the observer's attention, reached different populations by way of
    //! different death rates. A budget of 150 finished at 184 souls, 400 at 384, and 2000
    //! at 990 — and the 400 run lost 141 people in a decade on the way, which read as a
    //! famine and was actually an accounting error.
    //!
    //! Two gaps, both the same shape: a person leaves the coarse tier without being handed
    //! over, so the fine tier's first act is to bill them for every need across a span
    //! nobody simulated. One gap opened when a *place* was promoted, the other when a
    //! *household* moved out of a coarse quarter into an already-fine one.

    use super::*;

    /// How many spells of work a finely simulated adult actually does in a year, against
    /// what the coarse tier assumes on their behalf.
    #[test]
    #[ignore]
    fn measure_the_working_year() {
        let years = 30u64;
        let mut world = World::genesis(WorldSeed::from_u128(0x240), 40);
        world.set_detail_budget(usize::MAX);
        world.run_for(Duration::from_years(years));

        let worked = world
            .chronicle
            .iter()
            .filter(|r| matches!(r.kind, Happening::PersonDoes { deed: Deed::Work, .. }))
            .count();
        let now = world.now();
        // Adult-years lived, near enough: everybody alive now who is grown, times the span
        // they were grown for.
        let adult_years: f64 = world
            .people
            .iter()
            .filter(|(_, p)| p.is_alive() && !p.stage(now).is_dependent())
            .map(|(_, p)| p.age(now).years().min(years as f64 - 18.0).max(0.0))
            .sum();
        println!(
            "finely: {worked} spells of work over {adult_years:.0} adult-years = {:.0} a year",
            worked as f64 / adult_years.max(1.0),
        );

        let availability: Vec<f32> = world
            .places
            .iter()
            .filter(|(id, _)| world.society.households_in(*id).count() > 0)
            .map(|(_, p)| p.env.surroundings(false).availability[Deed::Work as usize])
            .collect();
        let mean = availability.iter().sum::<f32>() / availability.len().max(1) as f32;
        println!(
            "coarsely: WORK_SPELLS_PER_YEAR {WORK_SPELLS_PER_YEAR} x availability {mean:.3} = {:.0} a year",
            WORK_SPELLS_PER_YEAR * mean,
        );
    }

    /// What being unwatched still costs, if anything.
    #[test]
    #[ignore]
    fn measure_what_is_left_of_the_gap() {
        for (seed, budget) in [
            (0x211u128, 12usize), (0x211, 4_000),
            (0x212, 12), (0x212, 4_000),
            (0x213, 12), (0x213, 4_000),
        ] {
            let mut world = World::genesis(WorldSeed::from_u128(seed), 40);
            world.set_detail_budget(budget);
            world.run_for(Duration::from_years(60));
            let now = world.now();
            let adults: Vec<&person::Person> = world
                .people
                .iter()
                .map(|(_, p)| p)
                .filter(|p| p.is_alive() && !p.stage(now).is_dependent())
                .collect();
            let n = adults.len().max(1) as f32;
            println!(
                "seed {seed:x} budget {budget:>5}: living {:>4} ever {:>4} standing {:.3} vitality {:.3} starved {}",
                world.living(),
                world.people.len(),
                adults.iter().map(|p| p.standing()).sum::<f32>() / n,
                adults.iter().map(|p| p.health().vitality).sum::<f32>() / n,
                starved(&world),
            );
        }
    }

    fn starved(world: &World) -> usize {
        world
            .people
            .iter()
            .filter(|(_, p)| matches!(p.death(), Some((_, Cause::Deprivation))))
            .count()
    }

    #[test]
    fn promoting_a_quarter_does_not_starve_the_people_in_it() {
        // Let a world settle, put everybody out of detail for a good while, then bring
        // them all back at once. Nobody has been going hungry — they have been coping, by
        // assumption — so nobody should die of hunger the moment somebody looks at them.
        let mut world = World::genesis(WorldSeed::from_u128(0x210), 30);
        world.run_for(Duration::from_years(2));

        world.set_detail_budget(0);
        world.run_for(Duration::from_years(4));
        let before = starved(&world);

        world.set_detail_budget(FULL_DETAIL_BUDGET);
        world.run_for(Duration::from_years(2));

        assert_eq!(
            starved(&world),
            before,
            "looking at a quarter again killed people in it",
        );
        // And they arrive coping rather than at death's door.
        let now = world.now();
        for (_, person) in world.people.iter() {
            if person.is_alive() && !person.stage(now).is_dependent() {
                assert!(
                    person.health().vitality > 0.5,
                    "somebody came back from the coarse tier half dead",
                );
            }
        }
    }

    #[test]
    fn the_budget_does_not_decide_the_death_rate() {
        // The property itself, at the smallest size that still exercises it: one budget too
        // thin to hold everybody, one that never binds. Deaths by deprivation are the
        // tell-tale, because a coarse person is by assumption fed.
        let toll = |budget: usize| {
            let mut world = World::genesis(WorldSeed::from_u128(0x211), 40);
            world.set_detail_budget(budget);
            world.run_for(Duration::from_years(40));
            (world.living(), starved(&world), world.people.len())
        };

        let (thin_living, thin_starved, thin_ever) = toll(12);
        let (ample_living, ample_starved, ample_ever) = toll(4_000);

        assert!(
            thin_starved <= ample_starved + 2,
            "a thin budget starved {thin_starved} people against {ample_starved} on an ample one",
        );
        // And the world it produces has to be recognisably the same world.
        let apart = (thin_living as f32 - ample_living as f32).abs() / ample_living.max(1) as f32;
        assert!(
            apart < 0.25,
            "watching fewer people changed the population by {:.0}% \
({thin_living} against {ample_living}, {thin_ever} and {ample_ever} ever lived)",
            apart * 100.0,
        );
    }
}

