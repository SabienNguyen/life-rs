# life-rs — Universe Simulation Design

> Big-picture architecture. Nothing here is implemented yet; this document is the
> plan we implement against. Phase 0 is the only part that touches existing code.

## 1. The goal, stated precisely

Today the simulation runs one planet and one person, each driving a hand-written
state machine, printed to stdout. The target is different in kind, not just in size:

> At any moment, pick a random person out of the whole living population and see
> **everything** about them — who they are, what they want, what they are doing right
> now and why, who their parents and children and friends are, and every notable thing
> that has happened to them since birth.

And, having seen them, understand *why they are that way* — which of their traits came
from their parents, which from the street they grew up on, and what they'd have been
had they grown up somewhere else.

That is the whole design driver. Four consequences fall out of it
immediately, and they shape every decision below:

1. **Anyone must be addressable.** Not "the person we happen to hold a reference to."
   Any of them, by handle, at any time. Borrowed references (`&Planet` passed into
   `choose_action`) cannot express this — a family is a cycle, and a cycle of borrows
   does not compile.
2. **Everyone must have a past.** A dossier with a blank history is not omniscience.
   But storing a full life history for every person in a universe is not affordable,
   so history has to be *reconstructible*, not merely *recorded*.
3. **Not everyone can be simulated at full detail.** A universe of individually
   ticked agents does not fit in memory or in a frame budget. Detail has to be a
   dial, and the dial has to be turned by where the observer is looking.
4. **No trait may be a bare random number.** If personality is rolled directly, "why
   is she like that" has no answer beyond *the RNG said so*. Every trait has to be a
   computed output of causes that are themselves inspectable — genes from specific
   parents (§6), and a specific place at a specific age (§7).

## 2. Current state, and what specifically has to change

| What exists | Why it stops here |
| --- | --- |
| `Person`, `Planet` as owned structs in `main` | No collection, no identity — there is no "person #4,182,905" to ask about |
| `person.choose_action(&earth)` | Person borrows Planet. Person↔Person (family) is a cycle; won't borrow-check |
| `enum State { Idle, Eat, Sleep, .. }` + `match` | Behavior is hardcoded per-variant. Personality can't influence a `match` arm without combinatorial explosion |
| `generate()` returns a fresh `Person` | Independent random draws. Siblings can't resemble parents; no lineage |
| `Ethnicity`, `HairColor`, `Height` as independent enums | Uncorrelated draws — a person's features have no common cause, and none of them descend from anyone. These become *expressions of a genome* (§6) |
| Nothing represents where a person lives, beyond `Country` | `Country` is a label with no properties. Behavior can't respond to a place that has no attributes (§7) |
| `loop { .. thread::sleep(3s) }` in `main` | Wall-clock pacing, one entity per tick, print-only. No way to run 10,000 years or to query mid-run |
| `rand::random()` (thread-local RNG) | Not reproducible. Two runs of "the same" universe differ, so a person has no stable identity across runs |

None of this is wrong for where the project is — it's the natural first draft. The
pivot is from *object graph with references* to **data-oriented world with handles**.

## 3. Architecture at a glance

```
                      ┌──────────────────────────────┐
                      │        observer / TUI        │  "show me a random person"
                      └───────────────┬──────────────┘
                                      │ queries (read-only)
                      ┌───────────────▼──────────────┐
                      │            World             │  arenas + indices + chronicle
                      └───────────────┬──────────────┘
                                      │ systems mutate
   ┌──────────┬───────────┬───────────┼───────────┬────────────┬──────────┐
   │  needs   │ behavior  │  social   │  lifecycle│  economy   │ environment │
   └──────────┴───────────┴───────────┴───────────┴────────────┴──────────┘
                                      │
                      ┌───────────────▼──────────────┐
                      │      scheduler (ticks)       │  multi-rate, event-driven
                      └──────────────────────────────┘
```

### Crate layout

Keep the workspace, grow it:

```
sim-core/    Ids, arenas, Tick/Calendar, seeded splittable RNG, event bus, LOD policy
cosmos/      Galaxy, System, Star, Orbit  (thin for a long time — one system is fine)
planet/      Geography, biome, climate, day/night, calendar  (exists, gets extended)
life/        Traits shared by anything alive: needs, aging, health, death
genetics/    Loci, trait specs, meiosis/recombination, phenotype expression
person/      Identity, personality, skills, memory, intent  (exists, gets rebuilt)
society/     Households, kinship graph, relationships, settlements, culture, places
             and their environment vectors
chronicle/   Append-only event log + indices + biography assembly
sim/         Systems + scheduler; owns `World`; the actual simulation loop
observer/    Read-only query API: sample, dossier, follow, lineage
main/        Frontend (TUI first)
```

Dependency direction is strictly downward — `observer` never mutates, `sim-core`
depends on nothing in the workspace. That's what keeps the omniscient view from
accidentally becoming a god-mode *editor*.

## 4. Foundations

### 4.1 Handles, not references

Every entity lives in a generational arena and is named by a typed id:

```rust
pub struct Id<T> { index: u32, generation: u32, _marker: PhantomData<T> }

pub type PersonId    = Id<Person>;
pub type HouseholdId = Id<Household>;
pub type PlanetId    = Id<Planet>;
```

The generation counter is what makes death safe: when a person dies their slot is
eventually reused, but an old `PersonId` pointing at it fails lookup instead of
silently resolving to a stranger. `Person` stores `home: PlanetId`, not `&'a Planet`.
Systems take `&mut World` and resolve ids as needed. Cycles (spouse points at spouse)
become trivial.

> **Why not an off-the-shelf ECS (bevy_ecs, hecs)?** Recommendation: hand-rolled
> arenas for now. The entity count that matters is bounded by LOD (§6), the component
> set is small and stable, and an ECS's archetype churn is a poor fit for entities
> whose component set changes on life events. Revisit at Phase 5 if profiling says so.

### 4.2 Determinism and the seed hierarchy

This is load-bearing, not a nicety. The rule: **the same seed produces the same
universe, forever.**

```rust
// Derive, never share. Each id gets its own reproducible stream.
fn stream(world_seed: u64, domain: Domain, id: u64) -> Rng   // e.g. splitmix/ChaCha
```

`rand::random()` and `thread_rng()` are banned outside of seed creation. A person's
appearance, their parents' courtship, and the weather on the day they were born are
all derivable from `(world_seed, their id, tick)`. That buys three things at once:

- Reproducible bug reports ("seed 42, tick 91,203, person 8811").
- Save files that are a seed plus a divergence log, not a memory dump.
- **Backfill** (§8.3) — the ability to *invent history that was never simulated*, on
  demand, consistently, the first time anyone looks.

### 4.3 Time

```rust
pub struct Tick(u64);          // monotonic, 1 tick = 15 simulated minutes (proposed)
pub struct Date { year, day_of_year, tick_of_day }
```

Wall-clock `thread::sleep` moves to the frontend only. The sim runs as fast as it
can, or as fast as a requested time-scale allows (`1×`, `1000×`, `pause`, `run 100
years`). Decoupling sim time from real time is what makes a person's whole life
inspectable in a session.

### 4.4 Multi-rate, event-driven scheduling

Do **not** update every entity every tick. Two mechanisms:

- **Rate tiers.** Individual actions ~ticks; households ~daily; settlements ~monthly;
  demographics/climate ~yearly; cosmic ~millennia.
- **A future-event queue.** A sleeping person schedules "wake at tick N" rather than
  being polled 32 times. Most of the population, most of the time, costs nothing.

```rust
struct Scheduler { queue: BinaryHeap<Scheduled> }   // (Tick, EntityId, EventKind)
```

## 5. People

### 5.1 Identity vs. state

Split what never changes from what changes constantly — it makes both cheaper:

```rust
struct PersonCore {          // written once at birth, then immutable
    name: Name, born: Date, sex: Sex, ancestry: Ancestry,
    birthplace: PlaceId, parents: [Option<PersonId>; 2],
    personality: Personality, innate: InnateTraits,   // baseline temperament, constitution
}

struct PersonState {         // mutated by systems
    needs: Needs, health: Health, age_stage: LifeStage,
    location: PlaceId, household: Option<HouseholdId>,
    intent: Option<Intent>, mood: Mood, occupation: Option<Occupation>,
}
```

### 5.2 Personality — from enums to a vector

`Outlook` + `confident: bool` gives 6 distinct people. Move to a continuous model:

```rust
struct Personality {                 // OCEAN, each roughly N(0,1), clamped
    openness: f32, conscientiousness: f32, extraversion: f32,
    agreeableness: f32, neuroticism: f32,
}
struct Values { security: f32, achievement: f32, benevolence: f32,
                hedonism: f32, tradition: f32, power: f32 }
```

Keep the existing enums as a **presentation layer** — `Outlook::Pessimistic` becomes a
label derived from high neuroticism plus low openness, so the prose stays readable
while the mechanics stay continuous.

Crucially, `Personality` here is a **phenotype** — an output, not an input. It is
computed from genes (§6) and environment (§7), never rolled directly. That's the
difference between people who merely differ and people who differ *for reasons you
can inspect*.

### 5.3 Behavior — from FSM to utility scoring

The current `match self.state` cannot answer "why did *this* person do that?" Replace
the branch with scored options:

```rust
trait Action {
    fn score(&self, p: &PersonView, w: &WorldView) -> f32;   // 0.0 ..= 1.0
    fn duration(&self, p: &PersonView) -> Ticks;
    fn apply(&self, p: PersonId, w: &mut World);
}
```

Score = need pressure × personality weight × opportunity (is food nearby?) × social
context. Pick by softmax rather than argmax, so people are varied but not random. The
FSM doesn't disappear — it survives as `Intent`, the multi-tick action currently in
progress ("walking to the market, 6 ticks remaining"), which is exactly what the
omniscient view needs to display.

The payoff: the observer can show the *scoring table* — "ate because hunger 0.81 ×
conscientiousness 0.3 beat socialize 0.44." That is the difference between watching a
simulation and understanding one.

### 5.4 Needs

Replace booleans with decaying scalars: hunger, thirst, energy, hygiene, social,
safety, purpose. Decay rates modulated by age, health, and occupation. Unmet needs
raise stress, stress affects health, health affects mortality — that's the causal
chain that turns "properties" into "a life."

### 5.5 Memory

Bounded and salience-weighted, not a full log:

```rust
struct Memory { episodic: RingBuffer<MemoryTrace>, // ~64 recent/vivid events
                impressions: HashMap<PersonId, Impression> }  // how they feel about others
```

Salience = emotional intensity × recency × personal involvement. Old memories decay
into `impressions` (aggregate feelings) rather than being kept verbatim. People
misremember; that's a feature, and it's also how the sim stays in memory budget.
Distinguish clearly from the chronicle (§10): memory is *subjective and lossy*, the
chronicle is *objective and authoritative*.

## 6. Genetics

Behavior should have a *source*. Rolling `personality: rand::random()` produces
variety; inheritance produces families, resemblance, and regression to the mean —
and it produces them without anyone scripting it.

### 6.1 Model: polygenic, not base pairs

Simulating literal DNA is the wrong altitude — it costs a great deal and changes
nothing observable. Simulate the layer that actually produces trait variation: a
fixed set of loci, each with additive effects on several traits.

```rust
const N_LOCI: usize = 256;

struct Haplotype { alleles: [u8; N_LOCI] }        // one inherited copy
struct Genome    { maternal: Haplotype, paternal: Haplotype }   // diploid, 512 B
```

Trait architecture is a static table, authored once, not per-person data:

```rust
struct TraitSpec {
    trait_id: TraitId,
    loci: &'static [(LocusIdx, f32)],   // which loci, and each one's weight
    h2: f32,                             // heritability: genetic share of variance
    c2: f32,                             // shared (household + neighborhood) share
    // unique/idiosyncratic share is the remainder: 1 - h2 - c2
}
```

Two properties matter and both come free from this shape:

- **Pleiotropy.** One locus feeds several traits, so trait correlations emerge from
  the genetic architecture instead of being hand-tuned. This is why real people who
  are impulsive also tend to be sociable.
- **Regression to the mean.** Two exceptional parents mostly produce a less
  exceptional child, because they pass on half their alleles, not their phenotype.
  Ad-hoc `midparent + noise` gets this roughly right by accident; a genome gets it
  right by construction, and gets siblings right too.

### 6.2 Inheritance

```rust
fn meiosis(parent: &Genome, rng: &mut Rng) -> Haplotype   // crossover between the
                                                          // parent's two haplotypes
fn conceive(mother: &Genome, father: &Genome, rng: &mut Rng) -> Genome
```

Crossover at a few random points per gamete, plus a low per-locus mutation rate. The
consequence worth naming: **siblings share ~50% of variable alleles, but which 50% is
random.** Two siblings raised in the same household diverge in exactly the way real
siblings do — same expected value, different draw — and that is a far better source
of narrative texture than any amount of noise injection.

Dominance is a per-locus deviation on top of the additive mean, which lets recessive
traits skip generations and reappear — a grandparent's trait surfacing in a
grandchild is one of the most legible things a family simulation can show.

### 6.3 Storage: genomes are derived, not stored

512 B/person is affordable at Full tier (10⁵ people ≈ 50 MB) and not below it. But a
genome is a pure function of its parents' genomes plus one recombination seed:

```rust
struct GenomeRef { parents: [Option<PersonId>; 2], recomb_seed: u64 }   // 24 B
```

Store the *reference*; reconstruct the genome by walking up the pedigree to founders,
whose genomes are `f(world_seed, founder_id)`. Cache reconstructed genomes in an LRU
keyed by `PersonId`. This is exactly the backfill principle from §8.3 applied to
biology, and it means an arbitrarily deep ancestry costs 24 bytes per ancestor.

### 6.4 Founder populations — and an explicit guardrail

Founders draw alleles from population-specific frequency distributions, which
produces genuine population structure: visible family and regional resemblance,
inherited appearance, and ancestry that is *derived from the genome* rather than
stored as today's `Ethnicity` enum.

**Design rule, deliberate and non-negotiable: founder-population allele frequencies
differ only at appearance and physiology loci. Behavioral loci draw from a single
shared pool with identical frequencies across all founder populations.**

Two reasons, and they point the same way. It is what the science supports —
between-population variance in behavioral traits is not what the genetics shows. And
architecturally, the alternative would hardcode a racial determinism into the engine,
which would be both false and repellent. When the sim later shows outcome differences
across groups (and it will, via §7), those differences will trace to environment,
opportunity, and history — which is the interesting simulation anyway, because those
are the parts that can *change*.

## 7. Environment — how place shapes behavior

The other half of the answer. A person is genotype × circumstance, and circumstance
is not a mood modifier — it is mostly a set of doors that are open or shut.

### 7.1 Places carry an environment vector

```rust
struct EnvironmentVector {
    affluence: f32,          // local resource level
    density: f32,
    safety: f32,             // inverse exposure to violence/instability
    bonding_capital: f32,    // dense ties *within* the neighborhood
    bridging_capital: f32,   // ties *out* to distant opportunity
    education_access: f32,
    job_opportunity: f32,
    services: f32,           // healthcare, transit, food access
    pollution: f32,
    churn: f32,              // residential turnover; erodes bonding capital
    norms: NormProfile,      // locally prevailing behavior distribution
}
```

Neighborhood archetypes are **labels derived from the vector**, never assigned —
the same trick as `Outlook` over OCEAN:

| Archetype | affluence | density | safety | bonding | bridging | opportunity | churn |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Distressed urban | low | high | low | **high** | **low** | low | high |
| Working-class | low–mid | mid | mid | high | mid | mid | low |
| Suburb | mid–high | low | high | mid | mid | mid | low |
| Metropolitan core | high variance | very high | mid | low | **high** | high | high |
| Affluent enclave | high | low | high | mid | **very high** | high | very low |
| Rural | low–mid | very low | mid–high | high | low | low | very low |

Splitting social capital into bonding and bridging is the single most important
column in that table. A distressed neighborhood is typically **not** short on
community — it is short on *ties that reach opportunity*. Modeling one number called
"social capital" would collapse that distinction and produce the lazy version of this
simulation; two numbers produce the real mechanism behind limited mobility, and they
also explain why a dense affluent enclave and a dense poor one behave differently.

### 7.2 Four channels by which place changes behavior

The channels are distinct because they have different signatures in the data, and the
observer should be able to tell them apart:

1. **Opportunity — which actions exist at all.** Apprenticeship, schooling, and
   capital-intensive work are gated by `education_access` and `job_opportunity`. This
   is structural, and it is the largest effect. A person who never attends school did
   not lack conscientiousness; the action was not on the menu.
2. **Payoff — same action, different expected value.** Job search where
   `job_opportunity` is 0.1 has a low return, so a *correctly* reasoning agent
   allocates less effort to it. This produces what looks like low motivation and is
   nothing of the kind — an important thing for the sim to be able to demonstrate.
3. **Stress load — scarcity and danger change how you decide.** Low safety, high
   churn, and unmet needs accumulate into an allostatic load that raises the
   **discount rate** (future rewards weigh less) and amplifies neuroticism
   expression. One number, enormous behavioral reach: short time horizons explain
   under-investment in slow payoffs far better than any personality trait does.
4. **Norms and peers — local behavior is contagious.** `norms` biases action scores,
   weighted by the person's conformity (a function of agreeableness and age, peaking
   in adolescence). This is how neighborhoods reproduce themselves culturally.

The scoring function from §5.3 becomes:

```rust
score = need_pressure(person)
      × trait_weight(phenotype, env)                  // G×E: env modulates expression
      × opportunity(place, person)                    // channel 1 — a hard gate, often 0
      × payoff(place, action, discount_rate(stress))  // channels 2 & 3
      × norm_bias(place.norms, action, conformity)    // channel 4
```

### 7.3 Combining genes and environment

Phenotype for each trait, with variance components that sum to 1:

```
phenotype = √h²·genetic + √c²·shared_env + √(1−h²−c²)·unique
```

`shared_env` is the household and neighborhood contribution; `unique` is everything
idiosyncratic — a particular teacher, an illness, a friendship. Making heritability
an explicit per-trait parameter turns the entire nature/nurture balance into a config
file, which is the honest way to handle a genuinely contested empirical question.
Starting values: height ~0.8, personality ~0.4, cognitive ~0.5.

Three interaction mechanisms, worth implementing separately because they produce
different dynamics:

- **G×E interaction.** The same genotype yields different outcomes by context. High
  sensation-seeking becomes entrepreneurship where opportunity is high and
  risk-taking where it isn't — the trait is constant, its expression is not. This is
  the mechanism the user's ghetto/suburb question is really about, and it's why the
  answer isn't "environment adds a penalty."
- **rGE (gene–environment correlation).** Passive: parents supply both genes and
  neighborhood, so the two are correlated from birth and their effects are genuinely
  hard to separate — the sim should be honest about that rather than pretending to
  clean identification. Evocative: a child's temperament shapes how others treat
  them. Active: adults select environments matching their traits, which produces
  residential sorting endogenously.
- **Developmental windows.** Exposure is age-weighted — in utero, ages 0–5, and
  adolescence count for far more than adult exposure. Accumulate into a
  `Developmental` record that largely freezes at maturity, so *where someone grew up*
  stays legible in them for life. Active rGE plus this weighting reproduces the
  Wilson effect for free: measured heritability rises with age as adults increasingly
  choose environments that match their genes.

### 7.4 Feedback loops — where the interesting behavior comes from

None of these are scripted; they fall out of §7.1–7.3 and are the reason to build it
this way:

- **Sorting.** Households move toward places their resources and traits fit →
  neighborhoods diverge → divergence strengthens sorting. Schelling dynamics, emergent.
- **Intergenerational persistence.** Low-opportunity childhood → less accumulated
  human capital → lower adult resources → same neighborhood → children inherit both
  the genes and the place. This is passive rGE operating across generations, and it
  is the loop that makes the whole model worth simulating.
- **Escape routes must exist and be visible.** A model with only the loop above is
  deterministic doom, which is both wrong and boring. Bridging ties, mentors,
  schooling shocks, migration, and plain luck need real probability mass. Intergenerational
  elasticity should land well under 1.0 — and it's a testable assertion (§13), not a
  vibe.

### 7.5 Making it inspectable

Since the variance components are separable, the dossier can decompose any trait:

```
Conscientiousness   +1.2σ
  ├─ genetic         +0.7σ   (both parents above average)
  ├─ household       +0.5σ   (stable, two-earner)
  ├─ neighborhood    −0.3σ   (high churn, low bridging capital)
  └─ unique          +0.3σ
```

And, because the components are separable, run the counterfactual: *what would this
person be if raised in a different place?* Swap the environment term, recompute.
Nearly free, and it turns the omniscient view from a character sheet into an
instrument for actually understanding the model.

## 8. Scale — the level-of-detail system

The central scaling idea. Three tiers:

| Tier | What runs | Cost | Population |
| --- | --- | --- | --- |
| **Full** | Per-person needs, utility AI, memory, per-tick | ~10⁴–10⁵ people | Where the observer is looking, plus pinned regions |
| **Coarse** | Households and settlements as units; individuals exist as records but don't act | ~10⁶–10⁷ | Rest of the inhabited planet |
| **Statistical** | Cohort math only: birth/death/migration rates, no individuals stored | unbounded | Everywhere else, other worlds |

### 8.1 Promotion and demotion

Looking at someone **promotes** their region to Full. Looking away demotes it after a
grace period. Promotion must be cheap and must not produce a discontinuity — which is
what backfill is for.

### 8.2 The consistency contract

Aggregate invariants are preserved across tier changes: a coarse region that owed 4
births and 2 deaths this year still produces exactly 4 births and 2 deaths once
promoted. Statistics are the contract; individuals are the implementation.

### 8.3 Backfill — history invented on first inspection

When a never-simulated person is first observed, generate their biography
deterministically from `(world_seed, person_id)`, constrained by what is already
known (their parents exist and have ages; their settlement had a famine in year 41;
their occupation implies where they were at 20). The result is cached as if it had
always been recorded.

This is the trick that makes universe scale meet omniscience: **the past is computed
lazily, but only once, and always the same way.** A person you inspect for the first
time at tick 900,000 has a childhood — it just didn't exist five milliseconds ago.

## 9. Families and society

```rust
struct Household { members: Vec<PersonId>, head: PersonId, dwelling: PlaceId, .. }

struct Relationships {                        // world-level graph, not per-person
    kin:  Graph<PersonId, Kinship>,           // parent/child/sibling — structural, permanent
    ties: Graph<PersonId, Tie>,               // friend/rival/partner/colleague — dynamic
}
struct Tie { kind: TieKind, affinity: f32, trust: f32, last_contact: Tick }
```

Store kinship **once, structurally** (parent edges only) and derive siblings,
cousins, and ancestors by traversal — otherwise the edge count explodes and gets
inconsistent. Cache descendant/ancestor sets per query, invalidate on birth/death.

Life events drive the graph: pairing (by proximity + compatibility + culture), birth
(inherits personality and ancestry, joins a household), death (dissolves ties, moves
dependents, triggers inheritance), migration, estrangement. Each emits a chronicle
event — which is where life stories come from, for free.

## 10. The chronicle — how a life becomes a story

Append-only, indexed by participant:

```rust
struct Event { at: Tick, place: PlaceId, kind: EventKind,
               participants: SmallVec<[PersonId; 4]>, salience: u8 }
```

A biography is `chronicle.by_person(id)` filtered by salience, grouped into chapters
by life stage. Storage is tiered: recent events verbatim; older events compacted into
summaries ("worked the fields, 12 uneventful years"); only high-salience events kept
forever. Compaction is what makes millennia affordable.

## 11. The omniscient view

The headline feature, and deliberately a **read-only** API:

```rust
impl Observer<'_> {
    fn random_person(&self) -> PersonId;                    // uniform, or weighted by "interestingness"
    fn dossier(&self, id: PersonId) -> Dossier;             // everything, assembled
    fn family_tree(&self, id: PersonId, depth: u8) -> Tree;
    fn timeline(&self, id: PersonId) -> Vec<Event>;
    fn why(&self, id: PersonId) -> ActionScores;            // the deliberation, exposed
    fn nature_nurture(&self, id: PersonId) -> VarianceBreakdown;   // §7.5
    fn counterfactual(&self, id: PersonId, raised_in: PlaceId) -> Personality;
    fn follow(&mut self, id: PersonId);                     // pin to Full LOD, stream events
}
```

A `Dossier` gathers: identity and appearance; personality (numbers *and* prose), each
trait decomposed into genetic / household / neighborhood / unique contributions;
current needs, mood, stress load, and intent with time remaining; location breadcrumb
(`Universe → Sol → Earth → Vietnam → Da Nang → 14 Market St.`) with the neighborhood's
environment vector and archetype; household and family tree, with inherited traits
traced to the parent they came from; significant relationships with affinity; life
timeline; and the current scoring table showing which options were *gated off* rather
than merely outscored. That is the entire feature in one struct — the rest of the
architecture exists to make it fillable in under a millisecond, for anyone, at any
scale.

**Frontend, Phase 4:** a TUI (`ratatui`) — world clock and speed controls, dossier
pane, live event feed, `r` to reroll a random person, `f` to follow, `/` to search.
Terminal first because the data model is the hard part and a TUI can't hide a thin
one. A web/graphical layer later reads the same `Observer` API.

## 12. Roadmap

Each phase is independently useful and leaves the tree green.

| Phase | Deliverable | Why here |
| --- | --- | --- |
| **0. Foundations** | `sim-core` (ids, arenas, Tick, seeded RNG, event bus), `World` owning all entities, scheduler; port existing Person/Planet behavior unchanged onto it | Everything else is blocked on removing borrowed references. Behavior-neutral, so it's verifiable: same seed, same output |
| **1. Depth of person** | OCEAN + values, continuous needs, utility-based action selection with the §7.2 hooks stubbed, aging and life stages, health/mortality | Makes people distinguishable, which is the point of viewing one. The scoring function must take `env` from day one even while it's constant — retrofitting it later means rewriting every action |
| **2. Genetics & families** | `genetics` crate: loci, trait specs, meiosis, pedigree-derived genomes. Households, kinship graph, pairing/birth/death/inheritance. Population that sustains itself | Genetics needs families to inherit through, and families need genetics to be worth having. Doing them together avoids building placeholder inheritance twice |
| **3. Environment & neighborhoods** | `EnvironmentVector` on places, archetype derivation, the four behavior channels, developmental windows, residential sorting | Needs Phase 2's households to sort and Phase 1's scoring to modulate. First point where the sorting and persistence loops can actually run |
| **4. Chronicle & memory** | Event log + indices, salience, compaction, episodic memory and impressions | "Lives" from the brief — a person acquires a past |
| **5. Omniscient view** | `observer` crate + TUI: random person, dossier, family tree, timeline, follow, why, nature/nurture breakdown, counterfactual | The headline feature; first moment the whole thing is *fun*. Also the first time phases 2–3 become visible rather than statistical |
| **6. Scale** | LOD tiers, promotion/demotion, statistical background, backfill, aggregate invariants | Defer until there's something worth scaling. Needs 1–5 to define what "detail" means |
| **7. World** | Geography, biomes, climate, seasons, resources, settlements, real economy feeding `affluence`/`job_opportunity`; reintroduce `animal` on the shared `life` traits | Turns Phase 3's environment vector from authored input into simulated output — the point where neighborhoods start *changing* rather than just existing |
| **8. Cosmos** | `cosmos` crate: systems, stars, orbits, multiple inhabited worlds | Cheap once LOD exists — other worlds start Statistical |
| **9. Durability** | Save/load (seed + divergence log), replay-to-tick, determinism golden tests, profiling | Formalizes what §4.2 already made possible |

Phases 0–5 are the spine. If the project stopped at 5 it would already deliver the
brief on one planet; 6–9 are what make the word "universe" honest.

## 13. Testing

- **Determinism goldens** — seed → hash of world state at tick N. Catches accidental
  `thread_rng()` and iteration-order nondeterminism, which are otherwise invisible
  until they've corrupted a save format.
- **Demographic property tests** — run 500 years headless, assert population doesn't
  explode or collapse, age pyramid stays plausible, no orphaned households, no person
  with a parent born after them.
- **Graph invariants** — kinship is acyclic, ties are symmetric where they should be,
  every `PersonId` in a household resolves.
- **LOD equivalence** — a region run Full for 50 years vs. Coarse-then-promoted lands
  within tolerance on aggregate statistics.
- **Benchmarks** — tick cost vs. population, dossier assembly latency (target: <1 ms).

The genetics and environment models get their own validation suite, because both are
easy to write and easy to get quietly wrong:

- **Heritability recovery.** Run 10 generations, then measure realized correlations:
  siblings ≈ 0.5·h², parent–child ≈ 0.5·h², unrelated ≈ 0. If the configured `h2`
  doesn't come back out of the simulated population, the genetic architecture is
  wrong regardless of how plausible the individuals look.
- **Regression to the mean.** Children of +2σ parents should average around +2σ·h²,
  not +2σ. A simulation that breeds ever-more-extreme dynasties has a bug.
- **No behavioral population structure.** Assert the §6.4 guardrail directly: with
  environment held constant, mean behavioral traits do not differ across founder
  populations beyond sampling noise. This is a test, not a comment, so it can't rot.
- **Environment does something, and not everything.** Twin-style test — the same
  genome developed in a distressed vs. affluent place must differ measurably in
  outcomes; and intergenerational elasticity across the population must land well
  below 1.0, so mobility exists.
- **Channel attribution.** For a sample of decisions, the four §7.2 channels must sum
  to the observed score shift. Keeps the dossier's explanations honest.

## 14. Open questions

Answers change the shape of the work, so they're worth settling before Phase 1:

1. **Target scale.** One planet with ~10⁶ people, or genuinely many worlds? The plan
   supports both, but if it's one planet, Phase 5 gets much simpler and can be
   deferred indefinitely.
2. **Realism vs. legibility.** Grounded demographic modelling, or fantastical and
   readable? This decides whether mortality comes from life tables or from vibes.
3. **Where do people *live*, spatially?** Continuous coordinates, a grid, or an
   abstract graph of places? Recommendation: abstract place graph — cheapest, and
   spatial detail buys little for a dossier-centric view.
4. **How far does the observer go?** Read-only witness (planned here), or eventually
   able to intervene? Intervention is a much larger design surface; keeping the
   `Observer` read-only leaves the door open without paying for it now.
5. **Language and culture.** Do settlements have distinct cultures affecting naming,
   pairing norms, and occupations? Adds a lot of texture per unit of code, but it
   belongs in Phase 7, not earlier.
6. **Where to set heritability.** The `h2`/`c2` split per trait is the single most
   consequential set of constants in the model — it decides whether this simulation
   is a story about inheritance or about circumstance. Proposed defaults are
   deliberately middling (personality 0.4); worth setting on purpose rather than by
   drift, and worth exposing as config so it can be experimented with.
7. **How mobile should the world be?** Related, and just as consequential: how much
   probability mass do escape routes get (§7.4)? Too little and every dynasty is
   sealed at birth; too much and neighborhoods stop meaning anything. This wants a
   target intergenerational elasticity chosen up front, then tuned against.
8. **Do neighborhoods change, or only people?** Phase 3 treats the environment vector
   as authored; Phase 7 makes it emerge from the economy. Worth deciding early
   whether gentrification, decline, and investment are in scope, since it affects
   whether places need their own history and event stream.
