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
const CROWDING_AVERSION: f32 = 0.5;

/// How much better a neighbourhood has to be before a household will move to it.
///
/// Without a threshold, households shuffle endlessly between places that differ in the
/// third decimal, and churn — which erodes community — becomes an artefact of the
/// sorting loop rather than a fact about the world.
const MOVE_THRESHOLD: f32 = 0.05;

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

/// Evenings of company an unwatched person keeps in a year, at ordinary appetite.
///
/// The same calibration as `WORK_SPELLS_PER_YEAR` and for the same reason: an unwatched
/// person still has neighbours, and if their ties stood still while a watched person's
/// grew, then who your friends are would depend on who the observer happened to be looking
/// at. That is the exact bug class that once had the observer setting the death rate.
///
/// A finely simulated person does not use this — their count is what they actually chose.
/// Measured against that: see `measure_the_society_a_year_makes`.
const EVENINGS_PER_YEAR: f32 = 640.0;

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
    PlaceChanges {
        place: PlaceId,
        into: society::Archetype,
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
            | Happening::PersonDies { person, .. } => one(person.to_bits()),
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
            | Happening::PersonMentored { person, .. } => Some(*person),
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
    /// Shortfall somebody has taken on for somebody else's sake, not yet gone through.
    ///
    /// Hunger given away is not hunger destroyed — see `share_the_shortfall`. It waits here
    /// until the giver's own birthday comes round and they go without instead.
    shouldered: std::collections::BTreeMap<PersonId, f32>,
    /// Scratch: who somebody is weighing up this evening. Kept on the world purely so that
    /// an evening in company costs no allocation, of which there are some hundreds of
    /// millions in a run.
    company: Vec<PersonId>,
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
            evenings: std::collections::BTreeMap::new(),
            shouldered: std::collections::BTreeMap::new(),
            company: Vec::new(),
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
            let diligence = (0.6 + 0.5 * subject.personality.conscientiousness).clamp(0.2, 2.0);
            let taught = 0.5 + schooling;
            subject.earn(WORK_GAIN * job_opportunity * diligence * taught * subject.patronage());
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
        self.release(id);
        self.remember(
            at,
            Salience::Pivotal,
            Happening::PersonDies { person: id, cause },
        );
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
        let diligence = (0.6 + 0.5 * person.personality.conscientiousness).clamp(0.2, 2.0);
        let taught = 0.5 + env.education_access;
        let gain = WORK_GAIN * env.job_opportunity * diligence * taught * person.patronage();
        person.earn_repeatedly(gain, spells);

        // And a year of company. Unwatched people still have neighbours: if their ties
        // stood still while a watched person's grew, then who your friends are would depend
        // on who the observer happened to be looking at — the same fault that once had the
        // observer setting the death rate.
        //
        // How much company is the one thing the coarse tier has to guess at, since nobody
        // deliberated. Guessed with the fine tier's own expression for how much somebody
        // wants company, rather than a new one — an extravert is more sociable unwatched for
        // exactly the reason they are more sociable watched.
        let appetite = Deed::Socialize.appeal(&person.personality, &person.values)
            * surroundings.payoff[Deed::Socialize as usize];
        let evenings = (EVENINGS_PER_YEAR * appetite).round().max(0.0) as u32;
        *self.evenings.entry(id).or_insert(0) += evenings;
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
    fn spend_an_evening(&mut self, id: PersonId, rng: &mut Rng, evenings: u32) {
        let Some(place) = self.society.place_of(id) else {
            return;
        };

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

        self.bonds.meet_repeatedly(id, other, suits, evenings);
        // What each takes away about everybody else. Both directions, because both of them
        // were there — and this is the only channel in the simulation by which a fact about
        // one person reaches somebody who has never met them.
        self.bonds.hearsay_repeatedly(id, other, evenings);
        self.bonds.hearsay_repeatedly(other, id, evenings);
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

        // Taken out first: asking each ally in turn edits the ties while the walk is still
        // holding them. Once a year per person, so the small allocation is nothing.
        let allies: Vec<(PersonId, f32)> = self
            .bonds
            .of(id)
            .filter(|(_, tie)| tie.allied())
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
        self.reckon_cultures(at);
        self.assign_detail();
        self.absorb_upbringings(at);
        self.sort_households(at);
        self.reckon_bonds();
        self.scheduler
            .schedule_at(at + Duration::from_years(1), Task::Reckoning);
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
            for _ in 0..COMPANY_A_YEAR {
                self.spend_an_evening(who, &mut rng, each);
            }
        }
    }

    /// What every place produced this year, before anybody is asked about anything.
    ///
    /// Computed first and separately from the census that reads places off their
    /// residents, because the whole point is that it does not depend on them: land,
    /// position and headcount, and none of those is an opinion. It is the one term in a
    /// neighbourhood's character that comes from outside the loop.
    fn economies(&self) -> std::collections::BTreeMap<PlaceId, economy::Ledger> {
        let mut on_the_map: Vec<(PlaceId, society::Terrain, f32)> = Vec::new();
        for (id, place) in self.places.iter() {
            let Some(terrain) = place.terrain.clone() else {
                continue;
            };
            let hands = self
                .society
                .households_in(id)
                .flat_map(|(_, h)| h.members.iter())
                .filter(|m| self.people.get(**m).is_some_and(|p| p.is_alive()))
                .count() as f32;
            on_the_map.push((id, terrain, hands));
        }
        let inputs: Vec<(society::Terrain, f32, economy::Technique)> = on_the_map
            .iter()
            .map(|(id, t, w)| {
                (
                    t.clone(),
                    *w,
                    self.technique.get(id).copied().unwrap_or_default(),
                )
            })
            .collect();
        let ledgers = economy::year_knowing(&inputs);
        on_the_map
            .into_iter()
            .map(|(id, _, _)| id)
            .zip(ledgers)
            .collect()
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
        for country in self.countries() {
            let minds: u32 = country
                .places
                .iter()
                .filter_map(|at| self.souls_at(*at))
                .sum();
            for at in &country.places {
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
    fn within_reach(&self, a: usize, b: usize) -> bool {
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
    pub fn place_named(&self, at: usize) -> Option<&str> {
        let id = *self.roster.get(at)?;
        self.places.get(id).map(|p| p.name.as_str())
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

    /// Children take on the character of wherever they are living.
    fn absorb_upbringings(&mut self, at: Time) {
        let ids: Vec<PersonId> = self.people.ids().collect();
        for id in ids {
            let quality = self
                .society
                .place_of(id)
                .and_then(|p| self.places.get(p))
                .map(|p| p.env.upbringing())
                .unwrap_or(0.0);

            let opportunity = self
                .society
                .place_of(id)
                .and_then(|p| self.places.get(p))
                .map(|p| p.env.job_opportunity)
                .unwrap_or(0.0);

            let Some(person) = self.people.get_mut(id) else {
                continue;
            };
            if !person.is_alive() {
                continue;
            }
            let age = person.age(at).years();

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
        most.min(VOUCHING)
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

            let standing = {
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
                offered - CROWDING_AVERSION * (occupancy_of(world, id) - 1.0).max(0.0)
            };

            // Can they still afford where they are? Falling well behind the local
            // average means leaving, whether or not anywhere better will have them.
            let priced_out = current.is_some_and(|c| {
                self.places.get(c).is_some_and(|place| {
                    !place.admits(standing + DISPLACEMENT_MARGIN, occupancy_of(self, c))
                })
            });

            let best = self
                .places
                .ids()
                .filter(|id| {
                    // Backing counts only towards somewhere they do not already live.
                    // Ties are overwhelmingly local — company is drawn from neighbours — so
                    // a term that also applied to where you already are would be a bonus for
                    // staying put dressed up as a bonus for having friends, and it was: it
                    // stopped displacement dead and left every world with one inhabited
                    // quarter out of five. Pointed outward it is chain migration, which is
                    // the thing it should have been all along — your friends who left are
                    // what makes it possible to follow them.
                    let means = if current == Some(*id) {
                        standing
                    } else {
                        backing(self, *id)
                            + if restless {
                                standing + YOUNG_MOVER_SLACK
                            } else {
                                standing
                            }
                    };
                    // Staying put needs no admitting — unless they have been priced
                    // out of it, in which case it is no longer an option either.
                    (current == Some(*id) && !priced_out)
                        || self
                            .places
                            .get(*id)
                            .is_some_and(|p| p.admits(means, occupancy_of(self, *id)))
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
            let here = world.backing(&[id], home);
            if allies_at_home > 0 {
                if here > 0.0 {
                    backed_at_home += 1;
                }
            } else {
                assert_eq!(here, 0.0, "somebody was vouched for by nobody");
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
                    assert_eq!(
                        world.backing(&[id], *elsewhere),
                        0.0,
                        "friends elsewhere spoke for somebody where they have none"
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
        let (fine, coarse) = (at_detail(100_000, 25), at_detail(0, 25));
        let society = |world: &World| {
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
        let ((fine_ties, fine_allies), (coarse_ties, coarse_allies)) =
            (society(&fine), society(&coarse));

        assert!(coarse_ties > 1.0, "an unwatched town knows nobody");
        assert!(
            (fine_ties - coarse_ties).abs() < 0.25 * fine_ties,
            "acquaintance drifted with the observer: {fine_ties:.1} watched, {coarse_ties:.1} not"
        );
        // Looser, and knowingly so: past saturation more evenings buy familiarity that was
        // already at its ceiling, so the coarse tier understates how tight a place is by
        // about a seventh and more contact does not close it.
        assert!(
            (fine_allies - coarse_allies).abs() < 0.35 * fine_allies,
            "friendship drifted with the observer: {fine_allies:.1} watched, {coarse_allies:.1} not"
        );
    }

    /// What a year of company actually comes to, finely and coarsely.
    ///
    /// Ignored because it is a measurement rather than an assertion — run it when the
    /// utilities that pick a deed change, and move `EVENINGS_PER_YEAR` to what it says.
    #[test]
    #[ignore]
    fn measure_the_society_a_year_makes() {
        for budget in [100_000, 0] {
            let mut world = World::genesis(WorldSeed::from_u128(0x11), 60);
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
            println!(
                "budget {budget}: {} alive, {:.1} ties each, {:.1} allies each, mean warmth {:.3}, {} circles largest {}",
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
        let mut world = World::genesis(WorldSeed::from_u128(0x221), 120);
        for century in 1..=5 {
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
            println!(
                "year {:>4}: living {:>5} biggest country {:>5} best technique {:.4}",
                century * 100,
                world.living(),
                biggest,
                best,
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
        let mut world = World::genesis(WorldSeed::from_u128(0x222), 60);
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
        // And visibly short of hale, rather than a rounding error away from it. The exact
        // arithmetic of the ceiling is `person`'s to test; here it is reached through a
        // year's lag, since a body carries the ceiling set at its last birthday.
        assert!(
            worst_fed < 0.9,
            "the hungriest place is {most_want:.2} short and its people are at {worst_fed:.2}",
        );
    }

    #[test]
    fn hunger_is_what_stops_it_and_it_is_felt_where_the_land_is_thin() {
        // The mechanism, not just the outcome. Somewhere in a settled world people are
        // going short, and going short is what closes the fertility gate.
        let mut world = World::genesis(WorldSeed::from_u128(0x221), 60);
        world.run_for(Duration::from_years(120));

        let inhabited: Vec<&society::Place> = world
            .places
            .iter()
            .filter(|(id, _)| world.society.households_in(*id).count() > 0)
            .map(|(_, place)| place)
            .collect();
        assert!(!inhabited.is_empty(), "nobody lives anywhere");
        assert!(
            inhabited.iter().any(|p| p.want > 0.0),
            "a world at its ceiling has nobody short of anything",
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

