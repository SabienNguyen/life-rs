//! People.
//!
//! A person is a handle-addressable bundle of who they are (fixed at birth), how they
//! are doing (needs, health), and what they are currently up to (intent). Deciding is
//! delegated to [`deeds`]; being alive is delegated to [`life`].
//!
//! Nothing here is updated on a tick. Needs and health are brought forward from the last
//! time anyone looked, which is what lets a large population sit dormant for free.

pub mod deeds;
pub mod psyche;

use faker_rand::en_us::names::FullName;
use genetics::{Ancestry, Architecture, FounderPool, Genome};
use life::{Age, Health, LifeStage, Mortality, Need, Needs};
use planet::PlanetId;
use sim_core::{Duration, Id, Rng, Time};

pub use deeds::{Choice, Deed, Mind, Situation, Surroundings};
pub use psyche::{Origins, Outlook, Personality, Values};

pub type PersonId = Id<Person>;

/// Which gamete someone contributes. Reproduction needs the distinction; nothing else
/// in the simulation reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sex {
    Female,
    Male,
}

impl Sex {
    pub fn sample(rng: &mut Rng) -> Sex {
        // Slightly more boys are born than girls, and slightly more of them die young.
        if rng.chance(0.512) {
            Sex::Male
        } else {
            Sex::Female
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Sex::Female => "female",
            Sex::Male => "male",
        }
    }
}

/// The window in which someone can bear children. A crude stand-in for fertility that
/// declines with age and health.
pub const FERTILE_FROM: f64 = 18.0;
pub const FERTILE_UNTIL: f64 = 42.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Height {
    Short,
    Average,
    Tall,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ethnicity {
    Hispanic,
    African,
    Asian,
    White,
    PacificIslander,
    Indigenous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HairColor {
    Black,
    White,
    Brown,
    Blonde,
    Silver,
    Red,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Weight {
    Underweight,
    Normal,
    Overweight,
}


#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysicalAttrs {
    pub weight: Weight,
    pub height: Height,
}

impl PhysicalAttrs {
    pub fn new(weight: Weight, height: Height) -> PhysicalAttrs {
        PhysicalAttrs { weight, height }
    }
}

/// Something being done, and when it will be finished.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Intent {
    pub deed: Deed,
    pub started: Time,
    pub until: Time,
}

impl Intent {
    pub fn remaining(&self, now: Time) -> Duration {
        self.until.since(now)
    }
}

/// Why someone died. Derived from their state at the end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    Deprivation,
    Illness,
    OldAge,
    Misadventure,
}

impl Cause {
    pub const fn label(self) -> &'static str {
        match self {
            Cause::Deprivation => "deprivation",
            Cause::Illness => "illness",
            Cause::OldAge => "old age",
            Cause::Misadventure => "misadventure",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Person {
    pub name: String,
    pub sex: Sex,
    pub physical: PhysicalAttrs,
    /// The totals. Kept alongside `origins` because behaviour reads them constantly.
    pub personality: Personality,
    /// The same five factors, still split into genes, household, and chance.
    pub origins: Origins,
    pub values: Values,
    pub genome: Genome,
    /// Where the genome came from — enough to recompute it without keeping it.
    pub ancestry: Ancestry,
    pub parents: Option<(PersonId, PersonId)>,
    /// Which world they live on — a handle, so no lifetime ties a person to a planet.
    pub home: PlanetId,
    pub born: Time,

    /// Accumulated means and position, 0 to 1.
    ///
    /// Not currency — a relative standing, grown by work where there is work to be had,
    /// partly passed on to children, and averaged into the affluence of wherever they
    /// live. It is the quantity that closes the loop between people and places.
    standing: f32,
    /// The highest standing reached since maturity.
    ///
    /// A lifetime measure rather than a snapshot: current standing depends on when you
    /// look, since it decays in old age, so comparing a forty-year-old with their dead
    /// grandmother needs something that stops moving.
    ///
    /// Reset at maturity on purpose. A child begins with a share of what their parents
    /// had, so a peak tracked from birth is `max(inheritance, attainment)` — which made
    /// measured intergenerational elasticity 0.93, not because the world was a caste
    /// system but because the outcome had the predictor baked into it.
    peak_standing: f32,
    /// A lasting multiplier on what work returns, from having been taken up by someone.
    ///
    /// The mechanism by which a tight community produces people who get out. It is not
    /// a reward for merit and it is not inherited — it is who happened to notice you.
    patronage: f32,
    /// The opportunity of the places lived in as an adult: weighted sum and weight.
    ///
    /// Childhood is not the whole of someone's circumstances. Once people can move for
    /// work, where they end up matters as much as where they started — and an account of
    /// environment that stops at twenty attributes all of that to nothing.
    opportunity: (f32, f32),
    /// Childhood exposure, accumulating until maturity: weighted sum and total weight.
    upbringing: (f32, f32),
    matured: bool,

    needs: Needs,
    health: Health,
    intent: Option<Intent>,
    died: Option<(Time, Cause)>,
    /// When needs and health were last brought forward.
    updated: Time,
    met: bool,
}

impl Person {
    /// Assemble a person from a genome and the household that raised them.
    ///
    /// `shared` is the household's contribution to personality. Everything
    /// idiosyncratic is drawn from `rng` and inherited by nobody.
    #[allow(clippy::too_many_arguments)]
    pub fn express(
        architecture: &Architecture,
        name: impl Into<String>,
        sex: Sex,
        genome: Genome,
        ancestry: Ancestry,
        parents: Option<(PersonId, PersonId)>,
        home: PlanetId,
        born: Time,
        shared: f32,
        rng: &mut Rng,
    ) -> Person {
        let origins = Origins::express(architecture, &genome, shared, rng);
        let personality = origins.personality();
        let values = Values::sample(rng, &personality);

        Person {
            name: name.into(),
            sex,
            physical: PhysicalAttrs::new(
                pick(
                    rng,
                    &[Weight::Underweight, Weight::Normal, Weight::Overweight],
                ),
                stature_of(architecture, &genome),
            ),
            personality,
            origins,
            values,
            genome,
            ancestry,
            parents,
            home,
            born,
            standing: 0.0,
            peak_standing: 0.0,
            patronage: 1.0,
            opportunity: (0.0, 0.0),
            upbringing: (0.0, 0.0),
            matured: false,
            needs: Needs::rested(),
            health: Health::hale(),
            intent: None,
            died: None,
            updated: born,
            met: false,
        }
    }

    pub fn standing(&self) -> f32 {
        self.standing
    }

    pub fn set_standing(&mut self, standing: f32) {
        self.standing = standing.clamp(0.0, 1.0);
        self.peak_standing = self.peak_standing.max(self.standing);
    }

    pub fn patronage(&self) -> f32 {
        self.patronage
    }

    pub fn is_mentored(&self) -> bool {
        self.patronage > 1.0
    }

    /// Someone has taken an interest. Once only — a second patron is not twice the help.
    pub fn take_patron(&mut self, worth: f32) -> bool {
        if self.is_mentored() {
            return false;
        }
        self.patronage = worth.max(1.0);
        true
    }

    /// The highest standing reached in adult life — their attainment.
    pub fn peak_standing(&self) -> f32 {
        self.peak_standing
    }

    /// Gain from a spell of work. Saturating, so standing is a position rather than a
    /// pile that grows without limit.
    pub fn earn(&mut self, gain: f32) {
        self.set_standing(self.standing + gain * (1.0 - self.standing));
    }

    /// Apply many spells of work at once.
    ///
    /// Closed form, not an approximation. Each spell gains `gain * (1 - standing)`, so
    /// `n` of them compound to `1 - (1 - s)(1 - gain)^n` exactly. That matters: this is
    /// how a coarsely simulated year has to agree with a finely simulated one, and an
    /// approximation here would show up as a population quietly diverging from the
    /// version of itself that was being watched.
    pub fn earn_repeatedly(&mut self, gain: f32, times: f32) {
        if times <= 0.0 || gain <= 0.0 {
            return;
        }
        let remaining = (1.0 - gain.min(1.0)).powf(times);
        self.set_standing(1.0 - (1.0 - self.standing) * remaining);
    }

    /// Lose ground — the slow drift back that keeps standing from ratcheting.
    pub fn slip(&mut self, loss: f32) {
        self.set_standing(self.standing - loss * self.standing);
    }

    pub fn has_matured(&self) -> bool {
        self.matured
    }

    /// Absorb `years` of living somewhere of this quality.
    ///
    /// Weighted by age: infancy and adolescence count for far more than adulthood, so
    /// *where someone grew up* stays legible in them for life while a move at forty
    /// barely registers. Nothing accumulates after maturity.
    pub fn absorb(&mut self, quality: f32, age_years: f64, years: f32) {
        if self.matured || years <= 0.0 {
            return;
        }
        let weight = developmental_weight(age_years) * years;
        if weight > 0.0 {
            self.upbringing.0 += quality * weight;
            self.upbringing.1 += weight;
        }
    }

    /// Freeze the upbringing and re-express personality with what was actually absorbed.
    ///
    /// Genes and luck are untouched — only the shared term is replaced, with the
    /// weighted average of everywhere this person spent a childhood.
    pub fn mature(&mut self) {
        if self.matured {
            return;
        }
        self.matured = true;
        // Attainment is measured from here: what they made of their position, not the
        // position they were handed.
        self.peak_standing = self.standing;
        if self.upbringing.1 <= 0.0 {
            return;
        }
        let absorbed = self.upbringing.0 / self.upbringing.1;
        self.origins = self.origins.reshared(absorbed);
        self.personality = self.origins.personality();
    }

    /// Record `years` of adult life somewhere with this much opportunity.
    pub fn work_amid(&mut self, opportunity: f32, years: f32) {
        if years > 0.0 {
            self.opportunity.0 += opportunity * years;
            self.opportunity.1 += years;
        }
    }

    /// The opportunity this person has had access to across their working life.
    pub fn mean_opportunity(&self) -> f32 {
        if self.opportunity.1 <= 0.0 {
            0.0
        } else {
            self.opportunity.0 / self.opportunity.1
        }
    }

    /// The upbringing absorbed so far, whether or not it has been applied.
    pub fn absorbed_upbringing(&self) -> f32 {
        if self.upbringing.1 <= 0.0 {
            0.0
        } else {
            self.upbringing.0 / self.upbringing.1
        }
    }

    /// Whether this person could bear a child right now.
    pub fn is_fertile(&self, now: Time) -> bool {
        let years = self.age(now).years();
        self.is_alive()
            && self.sex == Sex::Female
            && (FERTILE_FROM..FERTILE_UNTIL).contains(&years)
            && self.health.vitality > 0.5
    }

    /// Whether this person is old enough to pair off.
    pub fn is_marriageable(&self, now: Time) -> bool {
        self.is_alive() && self.age(now).years() >= FERTILE_FROM
    }

    /// How well two temperaments suit each other, 0 to 1.
    ///
    /// Similarity, mostly. Real pairing is assortative, and it matters downstream:
    /// partners who resemble each other produce children whose traits are more
    /// spread out than random pairing would give.
    pub fn compatibility(&self, other: &Person) -> f32 {
        let a = self.personality;
        let b = other.personality;
        let distance = ((a.openness - b.openness).powi(2)
            + (a.conscientiousness - b.conscientiousness).powi(2)
            + (a.extraversion - b.extraversion).powi(2)
            + (a.agreeableness - b.agreeableness).powi(2)
            + (a.neuroticism - b.neuroticism).powi(2))
        .sqrt();
        (1.0 - distance / 6.0).clamp(0.0, 1.0)
    }

    pub fn age(&self, now: Time) -> Age {
        Age::from_birth(self.born, self.effective_now(now))
    }

    pub fn stage(&self, now: Time) -> LifeStage {
        self.age(now).stage()
    }

    pub fn needs(&self) -> &Needs {
        &self.needs
    }

    pub fn health(&self) -> Health {
        self.health
    }

    pub fn intent(&self) -> Option<Intent> {
        self.intent
    }

    pub fn is_alive(&self) -> bool {
        self.died.is_none()
    }

    pub fn death(&self) -> Option<(Time, Cause)> {
        self.died
    }

    /// True the first time anyone observes this person, false afterwards. Drives the
    /// one-off introduction.
    pub fn first_sighting(&mut self) -> bool {
        let first = !self.met;
        self.met = true;
        first
    }

    /// A dead person's clock stops, so their age and needs stay as they were.
    fn effective_now(&self, now: Time) -> Time {
        match self.died {
            Some((when, _)) => when.min(now),
            None => now,
        }
    }

    /// Treat this person as up to date and in ordinary condition as of `now`.
    ///
    /// A founding population has decades of life behind it that were never simulated.
    /// Without this, catching them up would charge them for every hour of that at once
    /// and kill the whole world on its first morning. Summarising an unsimulated past
    /// as "they are fine" is the crudest possible version of the backfill the design
    /// calls for — and the same seam a real reconstruction will slot into.
    pub fn assume_settled(&mut self, now: Time) {
        self.updated = now;
        self.needs = Needs::rested();
        self.health = Health::hale();
    }

    /// Bring needs and health forward to `now`. Idempotent, and cheap to call.
    pub fn catch_up(&mut self, now: Time) {
        if !self.is_alive() || now <= self.updated {
            return;
        }
        let elapsed = now.since(self.updated);
        let scale = self.stage(now).metabolic_scale();
        self.needs.accrue(elapsed, scale);
        self.health.respond_to(self.needs.vital_pressure(), elapsed);
        self.updated = now;

        if self.health.is_dead() {
            self.die(now, Cause::Deprivation);
        }
    }

    /// Apply a finished intent's effects, if it has finished.
    pub fn settle_intent(&mut self, now: Time) -> Option<Deed> {
        let intent = self.intent?;
        if intent.until > now {
            return None;
        }
        for (need, delta) in intent.deed.effects() {
            self.needs.adjust(*need, *delta);
        }
        self.intent = None;
        Some(intent.deed)
    }

    /// Decide what to do next and commit to it.
    /// Price every option without choosing one, or changing anything.
    ///
    /// What the observer asks when somebody wants to know *why*. Deliberately `&self`:
    /// looking at a person must not alter them, and a `why` that ran the decision again
    /// would consume randomness and change what they went on to do.
    pub fn weigh(&self, now: Time, situation: &Situation) -> [f32; Deed::COUNT] {
        deeds::score_all(
            &Mind {
                personality: &self.personality,
                values: &self.values,
                needs: &self.needs,
                age_years: self.age(now).years(),
            },
            situation,
        )
    }

    pub fn decide(&mut self, now: Time, situation: &Situation, rng: &mut Rng) -> Choice {
        let choice = deeds::choose(
            &Mind {
                personality: &self.personality,
                values: &self.values,
                needs: &self.needs,
                age_years: self.age(now).years(),
            },
            situation,
            rng,
        );
        self.intent = Some(Intent {
            deed: choice.deed,
            started: now,
            until: now + choice.deed.duration(),
        });
        choice
    }

    /// Catch up and settle whatever finished, without deciding anything new.
    ///
    /// Split out from [`Person::step`] so a caller can see *what* was completed — work
    /// has to be paid for, and only the world knows what the pay is.
    pub fn settle_intent_only(&mut self, now: Time) -> Option<Deed> {
        self.catch_up(now);
        if !self.is_alive() {
            return None;
        }
        self.settle_intent(now)
    }

    /// Catch up, finish whatever was in progress, and pick the next thing. The whole
    /// per-person step, in the order it has to happen.
    pub fn step(&mut self, now: Time, situation: &Situation, rng: &mut Rng) -> Option<Choice> {
        self.catch_up(now);
        if !self.is_alive() {
            return None;
        }
        self.settle_intent(now);
        Some(self.decide(now, situation, rng))
    }

    /// Roll against the mortality schedule for a span of time.
    pub fn survive(&mut self, now: Time, over: Duration, rng: &mut Rng) -> Option<Cause> {
        if !self.is_alive() {
            return None;
        }
        let age = self.age(now);
        if Mortality::HUMAN.rolls_death(age, over, self.health.frailty(), rng) {
            let cause = self.likely_cause(age);
            self.die(now, cause);
            return Some(cause);
        }
        None
    }

    fn likely_cause(&self, age: Age) -> Cause {
        if self.needs.vital_pressure() > 0.5 {
            Cause::Deprivation
        } else if self.health.vitality < 0.6 {
            Cause::Illness
        } else if age.stage() == LifeStage::Elder {
            Cause::OldAge
        } else {
            Cause::Misadventure
        }
    }

    fn die(&mut self, when: Time, cause: Cause) {
        if self.is_alive() {
            self.died = Some((when, cause));
            self.intent = None;
        }
    }

    /// Assume a year of coping: someone competent, unwatched, meeting their own needs.
    ///
    /// The claim a coarse tier makes. Rather than accruing a year of hunger nobody was
    /// simulated to relieve, needs are returned to where a person who looks after
    /// themselves actually sits — which is what the fine simulation shows them doing.
    ///
    /// The span matters and passing `Duration::ZERO` here was a serious bug. It made the
    /// call a no-op: needs were set to coping and then health was asked to respond to them
    /// over no time at all, so a coarse person's vitality **froze** at whatever it happened
    /// to be the moment their neighbourhood fell out of the detail budget. It could never
    /// recover, because `catch_up` also sees no elapsed time once `updated` has been moved
    /// forward. Anyone demoted below the fertility gate of 0.5 was sterile for life, and
    /// everybody else carried their frailty for ever.
    ///
    /// What it cost: the same world, same seed, differing only in how many people the
    /// observer could afford to watch, finished at 184 souls with a budget of 150, 384 with
    /// 400, and 990 with 2000. The level-of-detail machinery was not observation-neutral —
    /// it decided the population. That is the one property §19 calls the riskiest in the
    /// design, and it was failing silently.
    ///
    /// A year at coping pressure recovers to full, which is not a generous assumption but a
    /// measured one: finely simulated adults sit at vitality 1.000 — mean, median and tenth
    /// percentile alike — because a fine person acts on a need long before it does them any
    /// harm. Coping and being watched have to arrive at the same place, or watching is a
    /// treatment.
    pub fn get_by(&mut self, now: Time) {
        let coped = if now > self.updated {
            now.since(self.updated)
        } else {
            Duration::ZERO
        };
        self.updated = now;
        self.needs = Needs::rested();
        for need in Need::ALL {
            self.needs.set(need, COPING);
        }
        self.health.respond_to(self.needs.vital_pressure(), coped);
    }

    /// A year of not having enough, because the place could not grow it.
    ///
    /// The positive check, and the only thing in the model that stops a population. Every
    /// other limit was on where people are born or how crowded a place feels; none of them
    /// bounds the total, because a world that is uniformly poorer still has an ordinary
    /// place to be compared against. What bounds it is that land does not yield to wanting
    /// more of it — past what the ground carries, another pair of hands makes the shortfall
    /// worse, and people go hungry no matter what anybody decides.
    ///
    /// `want` is how far short of feeding one person for a year the place fell, per head,
    /// after trade. It raises hunger and thirst above what coping can reach — that is the
    /// whole of the mechanism, and everything downstream is already built: pressure above
    /// what the body tolerates costs vitality, lost vitality raises frailty and so the
    /// hazard of dying, and drops a woman below the fertility gate. Malthus's two checks
    /// out of one number.
    ///
    /// Applied once a year to everybody, watched or not, which is deliberate: a hunger that
    /// only reached the people somebody was looking at would be exactly the bug this
    /// project has just spent a long time removing from the other direction.
    pub fn go_hungry(&mut self, want: f32, now: Time) {
        if !self.is_alive() || want <= 0.0 {
            return;
        }
        // A ceiling on condition, not a year of the needs cycle run at once.
        //
        // Routing chronic hunger through `Needs` and `Health::respond_to` does not work,
        // and the reason is worth writing down because it is the shape that defeated four
        // earlier attempts at this. That machinery is built for the fine tier, where needs
        // swing over hours and health answers per *day*. Raising hunger and thirst by a
        // want of 0.4 puts vital pressure at 0.30 against a tolerance of 0.45 — so the body
        // *recovers*, and a place forty per cent short of feeding itself is no worse off
        // than one with food to spare. Push want a little higher and pressure crosses the
        // tolerance, where a year at three tenths a day of decline kills everybody outright.
        // Nothing, and then a massacre, with no useful ground between.
        //
        // What is true instead is simply that a body cannot be in better condition than its
        // food allows. `want` is measured in what one person needs for a year, so it maps
        // straight onto how far below hale that ceiling sits — and everything downstream
        // already responds: frailty rises with the square of the deficit, conception scales
        // with vitality, and `is_fertile` stops at a half.
        //
        // That last gate is what actually holds a population, and it should be: Malthus's
        // preventive check is the one that operates in ordinary times. The positive check
        // is here too, through frailty, but it is weak for the young because the mortality
        // schedule's baseline hazard is small — which is also true of real famine, where
        // the collapse in births outruns the rise in deaths.
        // A standing ceiling rather than a yearly knock. Applied as a one-off it lasted
        // about a fortnight — the fine tier's recovery runs at five hundredths a day — so
        // chronic hunger only bit because the mortality and conception rolls happened to
        // follow it in the same call. That is not a mechanism, it is a coincidence of
        // ordering, and it would have broken the moment anything moved.
        // Total failure is fatal on its own terms, and not as a consequence of whatever
        // `HUNGER_COSTS` happens to be. A want of one means the shortfall is a whole
        // person's food for a whole year — the ground grew nothing and no neighbour sold
        // them anything — and nobody survives that at any coefficient. Below it the ceiling
        // is the gradual thing the coefficient describes.
        let ceiling = if want >= 1.0 {
            0.0
        } else {
            (1.0 - want * HUNGER_COSTS).max(0.0)
        };
        self.health.feed(ceiling);
        if self.health.is_dead() {
            self.die(now, Cause::Deprivation);
        }
    }

    /// A place that feeds its people takes the ceiling off again.
    pub fn eat_well(&mut self) {
        self.health.feed(1.0);
    }

    /// Force a need, for tests and for events that act on a person from outside.
    pub fn set_need(&mut self, need: Need, level: f32) {
        self.needs.set(need, level);
    }
}

/// A founder: someone with no simulated parents, whose genome comes from a
/// population's allele frequencies.
pub fn found(
    architecture: &Architecture,
    pool: &FounderPool,
    rng: &mut Rng,
    home: PlanetId,
    born: Time,
    shared: f32,
) -> Person {
    let genome = pool.draw(rng);
    let seed = rng.next_u64();
    Person::express(
        architecture,
        random_name(rng),
        Sex::sample(rng),
        genome,
        Ancestry::founder(seed),
        None,
        home,
        born,
        shared,
        rng,
    )
}

/// A child, from two parents.
///
/// The genome is conceived from the parents' and one recombination seed, so it can be
/// recomputed from the pedigree later rather than stored.
pub fn born_to(
    architecture: &Architecture,
    mother: (PersonId, &Person),
    father: (PersonId, &Person),
    rng: &mut Rng,
    born: Time,
    shared: f32,
) -> Person {
    let (mother_id, mother) = mother;
    let (father_id, father) = father;

    let recomb_seed = rng.next_u64();
    let genome = genetics::conceive(&mother.genome, &father.genome, recomb_seed);
    let ancestry = Ancestry::of(mother_id.to_bits(), father_id.to_bits(), recomb_seed);

    // Family names travel with the father here, which is a convention rather than a
    // finding; naming belongs with culture once culture exists.
    let surname = family_name(&father.name);
    let full = random_name(rng);
    let given = given_name(&full);
    let name = if surname.is_empty() || given.is_empty() {
        full
    } else {
        format!("{given} {surname}")
    };

    Person::express(
        architecture,
        name,
        Sex::sample(rng),
        genome,
        ancestry,
        Some((mother_id, father_id)),
        mother.home,
        born,
        shared,
        rng,
    )
}

/// Height is read off the genome rather than drawn — the first visible feature that
/// actually descends from someone.
fn stature_of(architecture: &Architecture, genome: &Genome) -> Height {
    match architecture.genetic_value(genome, genetics::Trait::Stature) {
        z if z < -0.6 => Height::Short,
        z if z > 0.6 => Height::Tall,
        _ => Height::Average,
    }
}

/// Where needs sit for someone quietly getting on with looking after themselves.
///
/// Measured from finely simulated people rather than chosen: their needs oscillate as
/// they eat and sleep, and this is roughly where that cycle averages out.
const COPING: f32 = 0.25;

/// How far below hale a year of going short holds a body, per unit of shortfall.
///
/// `want` is in units of what one person needs for a year, so a want of a quarter means a
/// quarter of a diet missing. At this rate that caps vitality at 0.78; the fertility gate at
/// `is_fertile` closes at a want of about 0.56, and nobody survives a want past 1.0.
///
/// Where that gate sits is the whole of the calibration, because it is a **cliff**: below it
/// hunger only slows births, above it they stop dead. It was 1.4, which put the gate at a
/// want of 0.36 — reachable by any world founded on ground that was already full. Such a
/// world did not level off, it fell over: four hundred founders came to 86 souls in a
/// hundred and fifty years, and 65 on another seed, while the same worlds founded with
/// eighty grew to 373. Starting crowded was fatal, which is the same "nothing, then a
/// massacre" shape this mechanism exists to avoid, reintroduced by me one layer up.
///
/// At 0.9 the gate needs a want of 0.56 — real famine rather than a lean generation — and
/// those worlds come to 521 and 260 instead. Growth is still braked by about a quarter
/// against no hunger at all. §21.2 has the sweep.
const HUNGER_COSTS: f32 = 0.9;

/// How much a year at this age shapes someone.
///
/// The developmental windows: in utero and the first years count most, adolescence
/// nearly as much, and adulthood not at all. Without the weighting, a person is simply
/// a reading of wherever they happen to live now, and moving house would rewrite them.
fn developmental_weight(age_years: f64) -> f32 {
    match age_years {
        a if a < 5.0 => 1.5,
        a if a < 13.0 => 1.0,
        a if a < 20.0 => 1.2,
        _ => 0.0,
    }
}

/// The given name in a full name, ignoring any leading title.
///
/// Without this a child inherits "Mrs." as a first name from whichever generated name
/// happened to carry a title.
fn given_name(full: &str) -> String {
    const TITLES: [&str; 8] = ["mr", "mrs", "ms", "miss", "dr", "prof", "sir", "dame"];
    full.split_whitespace()
        .find(|word| {
            let bare = word.trim_end_matches('.').to_ascii_lowercase();
            !TITLES.contains(&bare.as_str())
        })
        .unwrap_or("")
        .to_string()
}

/// The family name in a full name, ignoring titles and suffixes.
///
/// Taking the last word gets "III" out of "Marcus Strosin III" — and once that is
/// inherited, the next generation is "Coby III", and a few generations later everyone
/// is a numeral.
fn family_name(full: &str) -> String {
    const SUFFIXES: [&str; 12] = [
        "jr", "sr", "i", "ii", "iii", "iv", "v", "md", "dds", "phd", "dvm", "esq",
    ];
    full.split_whitespace()
        .rfind(|word| {
            let bare = word.trim_end_matches('.').to_ascii_lowercase();
            !SUFFIXES.contains(&bare.as_str())
        })
        .unwrap_or("")
        .to_string()
}

fn random_name(rng: &mut Rng) -> String {
    // Names come from `faker_rand`, which draws from our stream but picks from its own
    // word lists — stable for a pinned version rather than forever. Cosmetic.
    use rand::Rng as _;
    let name: FullName = rng.r#gen();
    name.to_string()
}

fn pick<T: Copy>(rng: &mut Rng, options: &[T]) -> T {
    *rng.pick(options).expect("cannot choose from an empty set")
}

#[cfg(test)]
mod tests {
    use super::*;
    use planet::DayPhase;
    use sim_core::{Arena, Domain, WorldSeed};

    fn a_home() -> PlanetId {
        let mut arena: Arena<planet::Planet> = Arena::new();
        arena.insert(planet::Planet::earth())
    }

    fn rng(n: u64) -> Rng {
        WorldSeed::from_u128(0xb0_0c).stream(Domain::Behavior, n, 0)
    }

    fn arch() -> &'static Architecture {
        genetics::standard_architecture()
    }

    fn somebody() -> Person {
        found(
            arch(),
            &FounderPool::uniform(),
            &mut rng(1),
            a_home(),
            Time::ORIGIN,
            0.0,
        )
    }

    fn adult() -> Person {
        let mut p = somebody();
        p.born = Time::ORIGIN;
        p.updated = Time::ORIGIN + Duration::from_years(30);
        p
    }

    #[test]
    fn needs_accrue_between_visits() {
        let mut p = somebody();
        assert_eq!(p.needs().total_pressure(), 0.0);
        p.catch_up(Time::ORIGIN + Duration::from_hours(12));
        assert!(p.needs().get(Need::Thirst) > 0.0);
    }

    #[test]
    fn catching_up_is_idempotent() {
        let mut p = somebody();
        let noon = Time::ORIGIN + Duration::from_hours(12);
        p.catch_up(noon);
        let once = *p.needs();
        p.catch_up(noon);
        assert_eq!(*p.needs(), once, "catching up twice must not double-charge");
    }

    #[test]
    fn one_long_gap_equals_many_short_ones() {
        let mut straight = somebody();
        straight.catch_up(Time::ORIGIN + Duration::from_hours(10));

        let mut piecemeal = somebody();
        for h in 1..=10 {
            piecemeal.catch_up(Time::ORIGIN + Duration::from_hours(h));
        }
        for need in Need::ALL {
            let (a, b) = (straight.needs().get(need), piecemeal.needs().get(need));
            assert!((a - b).abs() < 1e-4, "{need}: {a} vs {b}");
        }
    }

    #[test]
    fn a_step_settles_the_old_intent_and_starts_a_new_one() {
        let mut p = adult();
        p.set_need(Need::Thirst, 0.95);
        let now = p.updated;

        let first = p
            .step(now, &Situation::plain(DayPhase::Morning), &mut rng(1))
            .unwrap();
        assert_eq!(first.deed, Deed::Drink);
        assert!(p.intent().is_some());

        // Nothing settles before the intent is due.
        assert_eq!(p.settle_intent(now), None);

        let later = now + Deed::Drink.duration();
        p.catch_up(later);
        assert_eq!(p.settle_intent(later), Some(Deed::Drink));
        assert!(
            p.needs().get(Need::Thirst) < 0.5,
            "drinking should have helped"
        );
    }

    #[test]
    fn a_week_of_living_keeps_needs_under_control() {
        // The end-to-end check that the loop closes: someone left to run their own life
        // should feed, water, and rest themselves indefinitely. The planet's clock
        // drives their sense of time of day, exactly as the simulation drives it.
        let calendar = planet::Calendar::EARTH;
        let mut p = adult();
        let mut now = p.updated;
        let end = now + Duration::from_days(7);

        while now < end {
            let situation = Situation::plain(calendar.phase_at(now));
            p.step(now, &situation, &mut rng(now.as_secs()));
            now = p.intent().map(|i| i.until).unwrap_or(end);
        }
        p.catch_up(end);

        assert!(p.is_alive(), "should not have starved");
        assert!(
            p.needs().vital_pressure() < 0.5,
            "vital needs ran away: {:?}",
            p.needs()
        );
        assert!(
            p.health().vitality > 0.9,
            "health should hold up: {:?}",
            p.health()
        );
    }

    #[test]
    fn a_lived_week_shows_a_daily_rhythm() {
        // Sleep should land at night without anyone scheduling it. The circadian term
        // and the planet's derived day phase are the only things pushing it there.
        let calendar = planet::Calendar::EARTH;
        let mut p = adult();
        let mut now = p.updated;
        let end = now + Duration::from_days(7);
        let (mut at_night, mut by_day) = (0, 0);

        while now < end {
            let phase = calendar.phase_at(now);
            let choice = p
                .step(now, &Situation::plain(phase), &mut rng(now.as_secs()))
                .expect("still alive");
            if choice.deed == Deed::Sleep {
                match phase {
                    DayPhase::Night | DayPhase::Evening => at_night += 1,
                    _ => by_day += 1,
                }
            }
            now = p.intent().map(|i| i.until).unwrap_or(end);
        }

        assert!(at_night > 0, "should have slept at all");
        assert!(
            at_night > by_day,
            "sleep should prefer the dark: {at_night} night vs {by_day} day"
        );
    }

    #[test]
    fn neglect_kills() {
        let mut p = adult();
        p.catch_up(p.updated + Duration::from_days(60));
        assert!(!p.is_alive());
        assert_eq!(p.death().unwrap().1, Cause::Deprivation);
    }

    #[test]
    fn the_dead_stop_changing() {
        let mut p = adult();
        p.catch_up(p.updated + Duration::from_days(60));
        let at_death = *p.needs();
        let age_at_death = p.age(Time::ORIGIN + Duration::from_years(200)).years();

        p.catch_up(Time::ORIGIN + Duration::from_years(200));
        assert_eq!(*p.needs(), at_death, "a corpse does not get thirstier");
        assert!(
            (p.age(Time::ORIGIN + Duration::from_years(500)).years() - age_at_death).abs() < 0.01,
            "nor older"
        );
        assert!(
            p.step(
                Time::ORIGIN + Duration::from_years(200),
                &Situation::plain(DayPhase::Morning),
                &mut rng(1)
            )
            .is_none()
        );
    }

    #[test]
    fn age_and_stage_are_derived() {
        let p = somebody();
        assert_eq!(p.stage(Time::ORIGIN), LifeStage::Infant);
        assert_eq!(
            p.stage(Time::ORIGIN + Duration::from_years(30)),
            LifeStage::Adult
        );
        assert_eq!(
            p.stage(Time::ORIGIN + Duration::from_years(80)),
            LifeStage::Elder
        );
    }

    #[test]
    fn mortality_eventually_catches_everyone() {
        let mut deaths = 0;
        for i in 0..200u64 {
            let mut p = somebody();
            let mut year = 0u64;
            while p.is_alive() && year < 200 {
                let now = Time::ORIGIN + Duration::from_years(year);
                p.survive(now, Duration::from_years(1), &mut rng(i * 1000 + year));
                year += 1;
            }
            if !p.is_alive() {
                deaths += 1;
            }
        }
        assert_eq!(deaths, 200, "everyone dies within two centuries");
    }

    #[test]
    fn given_names_ignore_titles() {
        assert_eq!(given_name("Mrs. Marjory O'Kon"), "Marjory");
        assert_eq!(given_name("Dr. Savion Bode"), "Savion");
        assert_eq!(given_name("Kyle Anderson MD"), "Kyle");
        assert_eq!(given_name(""), "");
    }

    #[test]
    fn family_names_ignore_titles_and_suffixes() {
        assert_eq!(family_name("Marcus Strosin III"), "Strosin");
        assert_eq!(family_name("Dr. Savion Bode"), "Bode");
        assert_eq!(family_name("Kyle Anderson MD"), "Anderson");
        assert_eq!(family_name("Mrs. Marjory O'Kon"), "O'Kon");
        assert_eq!(family_name("Velva Corwin Jr."), "Corwin");
        assert_eq!(family_name(""), "");
    }

    #[test]
    fn a_child_takes_a_real_family_name() {
        let home = a_home();
        let pool = FounderPool::uniform();
        let mut people: sim_core::Arena<Person> = sim_core::Arena::new();
        let mother = people.insert(found(arch(), &pool, &mut rng(3), home, Time::ORIGIN, 0.0));
        let father = people.insert(found(arch(), &pool, &mut rng(4), home, Time::ORIGIN, 0.0));

        let child = born_to(
            arch(),
            (mother, people.get(mother).unwrap()),
            (father, people.get(father).unwrap()),
            &mut rng(5),
            Time::ORIGIN,
            0.0,
        );

        let words: Vec<&str> = child.name.split(' ').collect();
        assert_eq!(
            words.len(),
            2,
            "a child's name is given plus family: {}",
            child.name
        );
        assert!(
            !["I", "II", "III", "IV", "V", "Jr.", "MD"].contains(&words[1]),
            "child inherited a suffix as a surname: {}",
            child.name
        );
        assert!(
            !["Mr.", "Mrs.", "Ms.", "Miss", "Dr."].contains(&words[0]),
            "child inherited a title as a given name: {}",
            child.name
        );
        assert_eq!(child.parents, Some((mother, father)));
        assert!(!child.ancestry.is_founder());
    }

    #[test]
    fn a_child_inherits_from_both_parents() {
        let home = a_home();
        let pool = FounderPool::uniform();
        let mut people: sim_core::Arena<Person> = sim_core::Arena::new();
        let mother = people.insert(found(arch(), &pool, &mut rng(6), home, Time::ORIGIN, 0.0));
        let father = people.insert(found(arch(), &pool, &mut rng(7), home, Time::ORIGIN, 0.0));

        let child = born_to(
            arch(),
            (mother, people.get(mother).unwrap()),
            (father, people.get(father).unwrap()),
            &mut rng(8),
            Time::ORIGIN,
            0.0,
        );

        // Closer to its parents than to an unrelated person, at the genome level.
        let stranger = found(arch(), &pool, &mut rng(9), home, Time::ORIGIN, 0.0);
        let to_mother = child.genome.distance(&people.get(mother).unwrap().genome);
        let to_stranger = child.genome.distance(&stranger.genome);
        assert!(
            to_mother < to_stranger,
            "child {to_mother:.3} from its mother, {to_stranger:.3} from a stranger"
        );
    }

    #[test]
    fn upbringing_shifts_a_personality_without_deciding_it() {
        let home = a_home();
        let pool = FounderPool::uniform();
        let bleak = found(arch(), &pool, &mut rng(10), home, Time::ORIGIN, -2.0);
        let kind = found(arch(), &pool, &mut rng(10), home, Time::ORIGIN, 2.0);

        // Same seed, so the same genome and the same idiosyncratic draws: the only
        // difference between these two people is where they grew up.
        assert_eq!(bleak.genome, kind.genome);
        assert_ne!(bleak.personality, kind.personality);
        assert_eq!(
            bleak.origins.openness.genetic, kind.origins.openness.genetic,
            "genes are untouched by upbringing"
        );
        assert!(
            (kind.origins.openness.shared - bleak.origins.openness.shared).abs() > 1.0,
            "upbringing should move the shared term substantially"
        );
    }

    #[test]
    fn a_first_sighting_happens_once() {
        let mut p = somebody();
        assert!(p.first_sighting());
        assert!(!p.first_sighting());
    }

    #[test]
    fn the_same_seed_produces_the_same_person() {
        let home = a_home();
        let seed = WorldSeed::from_u128(0xfeed);
        let pool = FounderPool::uniform();
        let one = found(
            arch(),
            &pool,
            &mut seed.stream(Domain::Genetics, 0, 0),
            home,
            Time::ORIGIN,
            0.0,
        );
        let two = found(
            arch(),
            &pool,
            &mut seed.stream(Domain::Genetics, 0, 0),
            home,
            Time::ORIGIN,
            0.0,
        );
        assert_eq!(one, two);
    }

    #[test]
    fn a_different_world_produces_different_people() {
        let home = a_home();
        let pool = FounderPool::uniform();
        let of = |seed: u128| {
            found(
                arch(),
                &pool,
                &mut WorldSeed::from_u128(seed).stream(Domain::Genetics, 0, 0),
                home,
                Time::ORIGIN,
                0.0,
            )
        };
        assert_ne!(of(1), of(2));
    }

    #[test]
    fn generated_people_are_well_formed_and_varied() {
        let home = a_home();
        let seed = WorldSeed::from_u128(77);
        let people: Vec<Person> = (0..200)
            .map(|i| {
                found(
                    arch(),
                    &FounderPool::uniform(),
                    &mut seed.stream(Domain::Genetics, i, 0),
                    home,
                    Time::ORIGIN,
                    0.0,
                )
            })
            .collect();

        for p in &people {
            assert!(!p.name.trim().is_empty());
            assert!(p.is_alive());
            assert_eq!(p.home, home);
        }

        let outlooks: std::collections::HashSet<Outlook> =
            people.iter().map(|p| p.personality.outlook()).collect();
        assert!(
            outlooks.len() >= 2,
            "a population should not share one outlook"
        );
    }
}

#[cfg(test)]
mod hunger {
    use super::*;

    fn somebody() -> Person {
        let mut arena: sim_core::Arena<planet::Planet> = sim_core::Arena::new();
        let home = arena.insert(planet::Planet::earth());
        let mut rng =
            sim_core::WorldSeed::from_u128(0x40).stream(sim_core::Domain::Genetics, 0, 0);
        found(
            genetics::standard_architecture(),
            &FounderPool::uniform(),
            &mut rng,
            home,
            Time::ORIGIN,
            0.0,
        )
    }

    #[test]
    fn a_body_cannot_be_in_better_condition_than_its_food_allows() {
        let mut person = somebody();
        assert_eq!(person.health().vitality, 1.0);

        person.go_hungry(0.25, Time::ORIGIN);
        assert!(
            (person.health().vitality - (1.0 - 0.25 * HUNGER_COSTS)).abs() < 1e-6,
            "a quarter short left them at {}",
            person.health().vitality,
        );
    }

    #[test]
    fn the_ceiling_holds_against_recovery() {
        // The bug this shape exists to prevent. Recovery runs at five hundredths a day, so
        // a ceiling applied once and then forgotten is gone inside a fortnight — chronic
        // hunger would only bite in the instant it was applied.
        let mut person = somebody();
        person.go_hungry(0.3, Time::ORIGIN);
        let hungry = person.health().vitality;

        person.get_by(Time::ORIGIN + Duration::from_years(1));
        assert!(
            person.health().vitality <= hungry + 1e-6,
            "a year of coping mended somebody nobody was feeding: {hungry} then {}",
            person.health().vitality,
        );
    }

    #[test]
    fn a_famine_that_ends_lets_people_mend() {
        let mut person = somebody();
        person.go_hungry(0.4, Time::ORIGIN);
        let starved = person.health().vitality;

        person.eat_well();
        person.get_by(Time::ORIGIN + Duration::from_years(1));
        assert!(
            person.health().vitality > starved,
            "the land fed them again and they stayed at {starved}",
        );
    }

    #[test]
    fn going_short_of_everything_is_fatal() {
        let mut person = somebody();
        person.go_hungry(1.0, Time::ORIGIN);
        assert!(!person.is_alive());
        assert_eq!(person.death().map(|(_, c)| c), Some(Cause::Deprivation));
    }

    #[test]
    fn a_place_that_feeds_its_people_costs_them_nothing() {
        let mut person = somebody();
        person.go_hungry(0.0, Time::ORIGIN);
        assert_eq!(person.health().vitality, 1.0);
    }
}
