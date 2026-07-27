# life-rs — Universe Simulation Design

> Big-picture architecture — the plan we implement against.
>
> **Phases 0–9, 11, and 10 are implemented**, along with the §15 balance harness — foundations
> (`sim-core`, `sim`), person depth (`life`, `person`), genetics with families
> (`genetics`, `society`), neighbourhoods with the four behaviour channels, the solid
> planet (`geo`: geodesic grid, plates, isostasy, erosion, eustatic sea level), the
> climate on top of it (`climate`: insolation, energy balance, moisture, and the
> carbon–silicate thermostat), the biosphere read off it (`biome`: Whittaker
> classification and the Miami productivity model), the animals on it (`ecology`:
> demes, tolerances, the trophic pyramid), evolution over it (`evolution`: adaptation,
> allopatric speciation, phylogeny), a chronicle that indexes and forgets, the omniscient
> view (`observer`: dossiers, biographies, why, counterfactuals), and level-of-detail (§6, pulled forward from phase 10 because everyone acting whether
> watched or not was making every later phase more expensive to build and test).
> §20 marks progress. Everything beyond that is still a plan.
>
> The two are not yet joined: people live in abstract places and the planet has no
> people on it. Giving a neighbourhood a grid cell is Phase 5's business, because a
> place wants a climate before it wants a coordinate.

## 1. The goal

Today the simulation runs one planet and one person, each driving a hand-written
state machine, printed to stdout. The target is different in kind, not just in size.

**Watch a world from its formation to its far future** — continents drifting and
colliding, oceans opening, ice ages arriving and retreating, life radiating and going
extinct and radiating again — over millions of years.

**Then stop anywhere in that history and zoom all the way in.** Pick a random person
out of the living population and see everything about them: who they are, what they
want, what they are doing right now and why, their family and friends, everything
notable that has happened to them since birth — and *why they turned out that way*,
which of their traits came from their parents and which from the street they grew up
on.

Those two demands pull hard against each other. Reconciling them is what most of this
document is about.

## 2. Where the code is now

| What exists | Why it stops here |
| --- | --- |
| `Person`, `Planet` as owned structs in `main` | No collection, no identity — there is no "person #4,182,905" to ask about |
| `person.choose_action(&earth)` | Person borrows Planet. Person↔Person (family) is a cycle; won't borrow-check |
| `enum State { Idle, Eat, Sleep, .. }` + `match` | Behavior hardcoded per-variant. Personality can't influence a `match` arm without combinatorial explosion |
| `generate()` returns a fresh `Person` | Independent random draws. Siblings can't resemble parents; no lineage |
| `Ethnicity`, `HairColor`, `Height` as independent enums | Uncorrelated draws — features have no common cause and descend from no one |
| `Country` as the only notion of place | A label with no properties. Behavior can't respond to a place that has no attributes |
| Planet is a name, a size, and a time-of-day FSM | No surface, no climate, no ocean. Nothing for a biosphere to live on |
| `loop { .. thread::sleep(3s) }` | Wall-clock pacing, one entity per tick, print-only. Cannot run a year, let alone an eon |
| `rand::random()` (thread-local RNG) | Not reproducible, so nothing can be re-derived, saved compactly, or revisited |

None of this is wrong for a first draft. But every row is load-bearing for what
follows, so Phase 0 rebuilds the foundation before anything is added on top.

## 3. Design principles

Five rules that recur throughout. Where a later section seems arbitrary, it's usually
one of these.

**1. Mechanisms, never scripts.** Nothing is placed by fiat. No "spawn a mass
extinction at year 3,000,000", no "this neighborhood is poor", no "this species is
adapted to cold". Extinctions happen because a volcanic province raised CO₂ and the
oceans went anoxic. A neighborhood is poor because of what happened to its economy. A
species is cold-adapted because the ones that weren't left fewer offspring. Scripted
outcomes are what make a simulation feel fake, and they're also what make it stop
surprising you.

**2. Store vectors, derive labels.** `Outlook::Pessimistic` is not stored — it's a
reading of a personality vector. A biome is not authored — it's a reading of
temperature and precipitation. A neighborhood archetype is a reading of an
environment vector. A species is a reading of a gene pool. Labels are for humans;
vectors are what the simulation runs on. This keeps categories from becoming
straitjackets, and it's what lets things *change into* other things.

**3. Store the spine, compute the detail.** The full state of a world across deep time
is unaffordable to store and unnecessary to keep. Store a coarse authoritative
timeline plus keyframes, and reconstruct fine detail on demand, deterministically.
This is the single mechanism that makes megayears and individual people coexist.

**4. The same machinery at every scale.** A family tree and a phylogeny are the same
data structure. A person's Tuesday and a mass extinction are the same event type at
different salience. Population genetics and individual inheritance are the same allele
frequencies viewed at different resolutions. Every place this holds is a place we
don't write a second system.

**5. Grounded models, honestly coarse.** Every subsystem borrows a real scientific
approach — energy-balance climate models, Whittaker biome classification, dynamic
global vegetation models, Wright–Fisher drift, Airy isostasy. None of them are
research-grade, and §19 says exactly where the approximations are. "Realistic" here
means *the mechanism is real and the resolution is coarse*, which is very different
from a plausible-looking fake.

## 4. Architecture

```
                        ┌─────────────────────────────┐
                        │       observer / UI         │  read-only, any scale
                        └──────────────┬──────────────┘
                        ┌──────────────▼──────────────┐
                        │            World            │  arenas + fields + chronicle
                        └──────────────┬──────────────┘
   ┌───────────┬────────────┬──────────┴──┬────────────┬───────────┬──────────┐
   │ tectonics │  climate   │  vegetation │  ecology   │ evolution │  society │
   │ 1 Myr     │  1–10 kyr  │  1 yr       │  1 yr      │  1–10 kyr │  varies  │
   └───────────┴────────────┴─────────────┴────────────┴───────────┴──────────┘
                        ┌─────────────────────────────┐
                        │   scheduler: scale ladder   │  §5.3
                        └─────────────────────────────┘
```

### Crates

```
sim-core/    Ids, arenas, TimeScale ladder, seeded RNG, event bus, LOD policy
geo/         Geodesic grid, plates, tectonics, isostasy, erosion, bathymetry
climate/     Energy balance, insolation & orbital forcing, hydrology, ice, carbon cycle
ocean/       Wind belts, Ekman transport, upwelling, basins, nutrient supply
biome/       Derived terrestrial + marine classification from climate & substrate
life/        Shared substrate: Organism, Phenotype, Needs, aging, death
genetics/    Loci, trait specs, meiosis, phenotype expression, allele-frequency pools
ecology/     Vegetation fields, animal demes, trophic web, dispersal, disturbance
evolution/   Selection, drift, mutation, gene flow, speciation, extinction, phylogeny
person/      Humans: identity, personality, skills, memory, intent
society/     Households, kinship, places, environment vectors, settlements
economy/     Land and labour, subsistence, surplus, trade — the outside of the loop
settlement/  The join: habitability, where people found towns, and what the ground does
chronicle/   Append-only event log, indices, compaction, biography & history assembly
observer/    Read-only query API at every scale
sim/         Systems + scheduler; owns `World`
cosmos/      Stars from the mass-luminosity relation, orbits, habitable zones
main/        Frontend
```

Dependencies point strictly downward; `observer` never mutates. Keeping the omniscient
view read-only is what stops it becoming a god-mode editor by accident.

## 5. Foundations

### 5.1 Handles, not references

```rust
pub struct Id<T> { index: u32, generation: u32, _marker: PhantomData<T> }
pub type PersonId = Id<Person>;   pub type DemeId = Id<Deme>;
```

Generational indices make death safe: a reused slot fails an old id's lookup instead
of silently resolving to a stranger. `Person` stores `home: PlaceId`, not `&'a Planet`.
Systems take `&mut World`. Cycles — spouses, food webs, plate boundaries — become
trivial.

> **Why not bevy_ecs/hecs?** Recommendation: hand-rolled arenas. Much of the world is
> *fields over a grid*, not entities, and an ECS's archetype churn suits neither that
> nor entities whose component set changes on life events. Revisit if profiling says
> otherwise.

### 5.2 Seeds: reproducible, never repetitive

This needs stating carefully, because the two properties sound contradictory and are
not.

**Every new world is genuinely different.** A fresh world draws a 128-bit seed from OS
entropy. Different seed → different continents, different ocean circulation, different
lineages, different people, different history. You will not see the same world twice,
and worlds are not variations on a theme: the dynamics are chaotic, so a one-bit seed
change reroutes a current, moves a mountain range, and gives you an unrecognizable
planet 50 Myr later. Non-determinism *from the user's point of view* is total.

**Any given world is reproducible.** Internally, all randomness comes from streams
derived from that world's seed — never `thread_rng()`, never OS entropy after
creation:

```rust
fn stream(world_seed: u128, domain: Domain, id: u64, epoch: u64) -> Rng
```

This is not a constraint on variety; it is the mechanism that makes the whole
architecture possible:

- The deep past can be *recomputed* instead of stored (§6.3) — the only way megayears
  and individual biographies fit in one program.
- A save file is a seed plus a divergence log, not a memory dump.
- You can rewind, replay, and re-inspect. Without it, looking away from a region and
  looking back would show you a different place.
- Bugs are reportable: "seed `0x8f3a…`, 4.2 Myr, deme 8811."

Randomness that *should* be random still is. Mutation, drift, weather, chance
encounters, and accidents all draw from these streams, and the sim has fat tails on
purpose (§19.2) — rare catastrophes and outliers, not smooth averages. Determinism is
a property of the engine, not of the outcomes.

### 5.3 The time-scale ladder

The heart of the deep-time requirement. **You cannot reach a million years by ticking
faster** — at 15-minute ticks that's 3.5×10¹⁰ steps, and no amount of optimization
closes a gap of that size. So different scales run *different models*, exactly as
spatial LOD swaps individuals for cohorts:

| Scale | Step | Integrator | State it evolves |
| --- | --- | --- | --- |
| **Moment** | 15 min | Agent utility AI, needs, interactions | Individual people and animals |
| **Day** | 1 day | Households, foraging, weather realization | Households, herds, local resources |
| **Season** | 1 season | Vegetation growth, migration, harvest, disturbance | PFT cover, herd ranges |
| **Generation** | 1 yr | Demography, economy, settlement, succession | Populations, settlements, biomass |
| **Ecological** | 100 yr | Community turnover, range shifts, soil | Species ranges, biome boundaries |
| **Evolutionary** | 1–10 kyr | Selection, drift, mutation, gene flow, speciation | Allele frequencies, phylogeny |
| **Orbital** | 10 kyr | Milankovitch forcing, glacial cycles, sea level | Ice sheets, climate state |
| **Geological** | 1 Myr | Plate motion, orogeny, erosion, outgassing | Continents, ocean basins, CO₂ |

Two rules make this affordable, and they're the same rule from opposite ends:

- **Coarse scales are cheap because they're coarse.** A megayear of geology is ~1,000
  steps over ~40k cells. Seconds, not hours.
- **Fine scales are cheap because they're brief.** You never run agent-level
  simulation for a millennium. You run it for the *window* you're watching — an
  afternoon, a year — then drop back up the ladder.

Crossing a boundary is a **projection**. Downward: sample individuals from
distributions (a genome from a deme's allele frequencies; a person's childhood from
their settlement's history). Upward: aggregate individuals back into distributions.
Both directions must preserve the aggregates (§6.2).

```rust
trait ScaleIntegrator {
    fn step(&mut self, world: &mut World, dt: Duration, rng: &mut Rng);
    fn project_down(&self, world: &mut World, target: TimeScale);
    fn project_up(&self, world: &mut World);
}
```

**Adaptive stepping.** Step size is not fixed: the climate integrator takes 10 kyr
steps while the system is stable and drops to 1 kyr through a deglaciation or an
anoxic event. Cheap where nothing happens, careful where it does — which is also
where the interesting events are.

### 5.4 Scheduling within a scale

Never poll every entity every step. A future-event queue (`BinaryHeap<(Tick, Id,
EventKind)>`) means a sleeping person or a dormant seed bank costs nothing until it's
due. Most of the world, most of the time, is free.

## 6. Scale in space, and the backfill contract

Spatial LOD mirrors the time ladder:

| Tier | What runs | Typical count |
| --- | --- | --- |
| **Full** | Individual agents, per-tick AI, memory | 10³–10⁴ individuals, where you're looking |
| **Coarse** | Households, herds, settlements as units | 10⁶–10⁷ individuals as records |
| **Statistical** | Cohorts and demes; no individuals stored | Unbounded — the rest of the planet |

### 6.1 Promotion and demotion

Looking somewhere promotes it; looking away demotes it after a grace period.
Promotion must be cheap and seamless — which is what backfill is for.

### 6.2 The consistency contract

Aggregates are preserved across every tier and scale transition. A coarse region that
owed 4 births and 2 deaths this year produces exactly 4 births and 2 deaths once
promoted. A deme whose allele frequency was 0.3 yields individuals averaging 0.3.
**Statistics are the contract; individuals are the implementation.**

### 6.3 Backfill — detail computed on first inspection

When something never simulated is first observed, generate it deterministically from
`(world_seed, id)`, constrained by everything already known: this person's parents
exist and have ages, their valley had a drought in 1,204,331 BP, their species' allele
frequencies at that time are on record. Cache it as though it had always been there.

**The past is computed lazily, but only once, and always the same way.** A person you
inspect for the first time at 4.2 Myr has a childhood — it just didn't exist five
milliseconds ago. This applies at every scale: an individual's biography, a genome
from a pedigree, a decade of weather from a climate keyframe.

## 7. The physical world

### 7.1 The grid

An icosahedral geodesic sphere — near-equal-area cells, no polar singularity, no
lat/lon distortion. Subdivision level 6 gives 40,962 cells (~112 km spacing on an
Earth-sized planet); level 7 gives 163,842 for a closer look.

```rust
struct Cell {
    neighbors: [CellId; 6],          // 5 at the twelve pentagon vertices
    plate: PlateId,
    crust: CrustType,                // Continental | Oceanic
    crust_thickness_km: f32, crust_age_myr: f32,
    elevation_m: f32, sediment_m: f32,
    // climate fields live in parallel arrays, not here — see §7.3
}
```

Fields are stored as **struct-of-arrays over cells**, not per-cell structs: climate
integrators sweep one field at a time, and that layout is what makes them fast.

### 7.2 Tectonics — where the deep-time drama comes from

Plates are cell sets with an Euler rotation pole and angular velocity. Each megayear
step rotates plates, reassigns boundary cells, and resolves what happens where they
meet:

| Boundary | Result |
| --- | --- |
| Divergent | New oceanic crust at age 0; ridge subsides as √age (real, and cheap) |
| Ocean–continent convergent | Subduction: trench, volcanic arc, CO₂ outgassing |
| Continent–continent convergent | Orogeny: crust thickens, mountains rise by isostasy |
| Transform | Strike-slip; little vertical motion |

Elevation follows **Airy isostasy** from crust thickness and density, minus erosion.
Erosion is stream-power (river incision, ∝ slope × discharge) plus hillslope
diffusion, with the sediment deposited downstream — so mountains wear down, basins
fill, and deltas form, all on their own.

Plate reorganizations occur every ~50–100 Myr. The supercontinent cycle isn't
scripted: continents collide because they drift, aggregate because collision welds
them, then rift because a large continent traps heat beneath itself. It falls out of
the mechanism, which is the whole point of principle 1.

### 7.3 Climate

An **energy-balance model** per cell, iterated to equilibrium at each step:

```
absorbed shortwave  =  insolation(lat, orbit, luminosity) × (1 − albedo)
outgoing longwave   =  A(CO₂) + B·T                  [linearized]
transport           =  ∇·(D ∇T)                      [ocean D > land D]
```

Not a GCM — a GCM is out of reach and unnecessary. This captures what actually matters
over deep time, including the feedbacks that produce the big events:

- **Ice–albedo feedback.** Ice raises albedo, which cools, which grows ice. Runaway is
  possible; a snowball planet is an outcome this model can genuinely reach.
- **Carbon–silicate thermostat.** Volcanic outgassing (from §7.2's subduction) adds
  CO₂; silicate weathering removes it at a rate rising with temperature and runoff.
  This is the real reason Earth stayed habitable for four billion years, and it links
  tectonics → climate → life in one loop.
- **Orbital forcing.** Eccentricity (~100 kyr), obliquity (~41 kyr), and precession
  (~23 kyr) modulate insolation by latitude and season. Glacial cycles emerge; so does
  the Sahara alternating between desert and grassland every ~20 kyr, which is a
  spectacular thing to watch on a map.
- **Solar brightening.** ~1% per 100 Myr — the faint young sun, and eventually the end
  of the biosphere.

Precipitation comes from a moisture-budget scheme: evaporation (temperature- and
surface-dependent), advection along prevailing winds (Hadley/Ferrel/polar cells by
latitude, deflected by rotation), orographic lift, and rain shadow. Coarse, but it
puts deserts in the right places — subtropical highs, continental interiors, and the
lee of mountains — for the right reasons.

### 7.4 Oceans

Not scenery: the ocean is half the climate system and most of the biosphere.

```rust
struct OceanCell { depth_m: f32, temp_c: f32, salinity: f32,
                   current: Vec2, upwelling: f32, nutrients: f32, o2: f32 }
```

- **Circulation.** Wind-driven gyres from basin geometry and rotation, plus a bulk
  thermohaline overturning term driven by high-latitude density. Continental
  configuration controls it — open a seaway or close an isthmus and the heat transport
  reorganizes, which is exactly the sort of deep-time consequence worth being able to
  watch.
- **Upwelling** at eastern boundaries and divergence zones brings nutrients up, which
  drives marine productivity. Fisheries, seabird colonies, and whale populations end
  up where the physics puts them.
- **Sea level** from ice volume plus ocean-basin volume (young ridges are buoyant and
  displace water). Continental shelves flood and drain over glacial cycles, opening
  and closing land bridges — which drives migration, isolation, and speciation.
- **Oxygen and anoxia.** Warm oceans hold less O₂ and stratify. Sustained warming can
  produce anoxic events, which are one of the main ways this world will kill most of
  its marine life without anyone scripting it.

## 8. Biomes — derived, never authored

Terrestrial biome per cell from the **Whittaker** scheme: mean annual temperature ×
annual precipitation, adjusted for seasonality, elevation, and soil. Tundra, taiga,
temperate forest, grassland, savanna, desert, rainforest, and the rest are *readings*
of the climate field.

Marine biome from depth, temperature, light, and productivity: shelf, reef, kelp
forest, pelagic, upwelling zone, abyssal, polar ice edge.

Because they're derived (principle 2), biomes **move**. Precession dries the Sahara;
a glacial period pushes taiga a thousand kilometres south and drops sea level enough
to bridge continents; an orogeny casts a rain shadow that turns forest into steppe
over a few hundred thousand years. Nobody edits a biome map — the map is a view.

## 9. Life: the shared substrate

People, wolves, oak trees, and plankton share one foundation. Only humans get §14–15
on top.

```rust
trait Organism { fn genome(&self) -> GenomeRef; fn phenotype(&self) -> &Phenotype;
                 fn needs(&self) -> &Needs;     fn age(&self) -> Age; }

struct Species { lineage: LineageId, clade: CladeId, kingdom: Kingdom,
                 traits: TraitSpecSet, niche: NicheVector,
                 trophic: TrophicRole, demes: Vec<DemeId> }
```

`NicheVector` — tolerated temperature range, moisture, salinity, depth, light, diet
breadth — is what gets compared against a cell's environment to yield habitat
suitability. Selection (§11) then pushes the niche around. A species is cold-adapted
because its ancestors that weren't left fewer descendants.

## 10. Genetics

### 10.1 Polygenic, not base pairs

Literal DNA is the wrong altitude — expensive, and it changes nothing observable.
Simulate the layer that produces trait variation:

```rust
const N_LOCI: usize = 256;
struct Haplotype { alleles: [u8; N_LOCI] }
struct Genome    { maternal: Haplotype, paternal: Haplotype }   // 512 B

struct TraitSpec {
    loci: &'static [(LocusIdx, f32)],   // which loci, and each one's weight
    h2: f32,                             // heritable share of variance
    c2: f32,                             // shared-environment share
}                                        // remainder = unique/idiosyncratic
```

Two properties come free from this shape and are worth the whole design:

- **Pleiotropy.** One locus feeds several traits, so trait correlations emerge from
  the architecture rather than being hand-tuned.
- **Regression to the mean.** Two exceptional parents mostly produce a less
  exceptional child, because they pass on half their alleles, not their phenotype. A
  genome gets this right by construction — and gets siblings right too.

### 10.2 Inheritance

```rust
fn meiosis(parent: &Genome, rng: &mut Rng) -> Haplotype;
fn conceive(a: &Genome, b: &Genome, rng: &mut Rng) -> Genome;
```

Crossover at a few points per gamete, plus a low per-locus mutation rate. **Siblings
share ~50% of variable alleles, but which 50% is random** — so siblings raised in one
household diverge exactly as real siblings do: same expected value, different draw.
Per-locus dominance deviations let recessive traits skip generations and resurface,
which is one of the most legible things a family simulation can show.

### 10.3 Genomes are derived, not stored

512 B/person is fine at Full tier and impossible below it. But a genome is a pure
function of its parents' genomes and one recombination seed:

```rust
struct GenomeRef { parents: [Option<PersonId>; 2], recomb_seed: u64 }   // 24 B
```

Reconstruct by walking the pedigree to founders, whose genomes come from their deme's
allele frequencies. LRU-cache the results. Principle 3, applied to biology: arbitrary
ancestral depth for 24 bytes a head.

### 10.4 Individuals and populations are the same model

The unification that makes evolution nearly free: a **population is an allele
frequency vector**, an **individual is a draw from it**, and evolution is those
frequencies changing.

```rust
struct Deme { species: SpeciesId, cells: CellRange, population: f64,
              allele_freqs: Box<[f32; N_LOCI]>, age_structure: [f32; N_AGE] }
```

Project down: sample a genome from the frequencies. Project up: recompute frequencies
from realized genomes. The same `TraitSpec` table computes a wolf's phenotype and a
person's. One genetics system serves the whole biosphere.

### 10.5 Founder populations — an explicit guardrail

Founders draw from population-specific allele frequencies, producing real population
structure and family resemblance — and letting ancestry be *derived from the genome*
rather than stored as today's `Ethnicity` enum.

**Design rule, deliberate and non-negotiable: for humans, founder-population
frequencies differ only at appearance and physiology loci. Behavioral loci draw from a
single shared pool with identical frequencies across all founder populations.**

It's what the science supports — between-population variance in behavioral traits is
not what the genetics shows. And architecturally, the alternative would hardcode
racial determinism into the engine, which would be both false and repellent. Outcome
differences across human groups will still appear, via §13's environment and history —
which is the better simulation anyway, because those are the parts that can change.
(Non-human species have no such restriction; that's what adaptation *is*.)

## 11. Ecology

### 11.1 Plants are fields, not entities

Individual trees exist only at Full LOD. Everywhere else, vegetation is a field of
**plant functional types** per cell — the standard dynamic-global-vegetation-model
approach:

```rust
struct Vegetation {          // per cell, fractional cover summing to ≤ 1
    cover:   [f32; N_PFT],   // grass, shrub, broadleaf decid/evergreen, needleleaf,
    biomass: [f32; N_PFT],   // succulent, macrophyte, phytoplankton, ...
    lai: f32, soil_carbon: f32, soil_nutrients: f32,
}
```

Net primary productivity from temperature, moisture, light, CO₂, and nutrients. PFTs
compete for light, water, and nutrients; the winner is whichever the local climate
favors, which is why biome boundaries move on their own. Disturbance — fire from fuel
load × dryness × ignition, storms, herbivory — resets patches and drives succession.
Marine primary production runs the same way with phytoplankton, limited by light and
upwelled nutrients.

### 11.2 Animals are demes, not entities

Same rule: individual animals exist only at Full LOD (the wolf you're watching), and
everywhere else it's `Deme`s from §10.4.

Population dynamics per year: births and deaths from resource availability and
predation, with a **Holling type II functional response** rather than raw
Lotka–Volterra — LV oscillates unstably and will blow up a long run. Resource-limited
logistic growth with saturating predation, clamped, stays bounded over megayears,
which matters more than elegance here.

Dispersal follows habitat-suitability gradients across the grid — `NicheVector` vs.
cell conditions. That single mechanism gives range expansion, contraction, refugia
during glaciations, and geographic isolation, which is where speciation comes from.

### 11.3 The trophic web

Producers → herbivores → carnivores → apex, plus detritivores closing the loop.
Because production is grounded in real climate and nutrients, the food web has real
constraints: upwelling zones support enormous biomass, deserts almost none, and a
productivity collapse propagates up the web as a cascade rather than as a scripted
die-off.

## 12. Evolution and deep time

Every 1–10 kyr, per deme, four operators — the standard population-genetics set:

- **Selection.** Fitness from phenotype vs. local environment (the §10 machinery, the
  §7 climate). Allele frequencies shift toward what survives locally.
- **Drift.** Wright–Fisher sampling scaled by effective population size. Small
  isolated demes drift fast — which is why islands and refugia are where novelty
  appears.
- **Mutation.** Low per-locus rate; the source of new variation.
- **Gene flow.** Migration between adjacent demes, pulling them back together.

**Speciation** is the balance of the last two: when demes are isolated long enough
that drift and divergent selection outrun gene flow, genetic distance crosses a
reproductive-incompatibility threshold and the lineage splits. Sea level closing a
land bridge, a mountain range rising, a desert widening — §7's geology *causes*
speciation, without a speciation event ever being scheduled.

**Extinction** is a deme reaching zero. **Mass extinction** is emergent and this is
where the subsystems pay off together: a large igneous province (§7.2) spikes CO₂
(§7.3), which warms and stratifies the ocean (§7.4), which goes anoxic, which collapses
marine productivity (§11.1), which cascades through the food web (§11.3). Nothing in
that chain is scripted. Afterwards, empty niches drive an adaptive radiation, because
survivors face weak competition and diversify fast.

The **phylogeny** is a tree of lineages with branch times, node causes, and extinction
dates — structurally identical to a family tree (principle 4), and rendered by the
same code.

## 13. People

### 13.1 Identity vs. state

```rust
struct PersonCore {          // written once at birth, then immutable
    name: Name, born: Date, sex: Sex, genome: GenomeRef,
    birthplace: PlaceId, parents: [Option<PersonId>; 2],
    personality: Personality, developmental: Developmental,
}
struct PersonState {         // mutated by systems
    needs: Needs, health: Health, stress: f32, life_stage: LifeStage,
    location: PlaceId, household: Option<HouseholdId>,
    intent: Option<Intent>, mood: Mood, occupation: Option<Occupation>,
}
```

### 13.2 Personality is an output

```rust
struct Personality {   // OCEAN, ~N(0,1) each
    openness: f32, conscientiousness: f32, extraversion: f32,
    agreeableness: f32, neuroticism: f32,
}
struct Values { security: f32, achievement: f32, benevolence: f32,
                hedonism: f32, tradition: f32, power: f32 }
```

Keep the existing enums as a presentation layer — `Outlook::Pessimistic` reads out of
high neuroticism plus low openness. But `Personality` is a **phenotype**: computed
from genes (§10) and environment (§14), never rolled. That's principle 1 applied to
people, and it's what makes "why is she like that" answerable.

### 13.3 Behavior: utility, not a state machine

`match self.state` can't answer "why did *this* person do that". Replace it with
scored options:

```rust
trait Action {
    fn score(&self, p: &PersonView, w: &WorldView) -> f32;
    fn duration(&self, p: &PersonView) -> Ticks;
    fn apply(&self, p: PersonId, w: &mut World);
}
```

Selection by softmax, not argmax — varied without being random. The FSM survives as
`Intent`, the multi-tick action in progress ("walking to the market, 6 ticks left"),
which is exactly what the observer displays. And the *scoring table itself* becomes
inspectable: "ate because hunger 0.81 × conscientiousness 0.3 beat socialize 0.44."

### 13.4 Needs and memory

Needs are decaying scalars — hunger, thirst, energy, hygiene, social, safety, purpose
— with rates modulated by age, health, and occupation. Unmet needs raise stress;
stress degrades health; health drives mortality. That chain is what turns properties
into a life.

Memory is bounded and salience-weighted: a ring buffer of ~64 vivid episodes plus
`impressions` (aggregate feelings about specific people). Old memories decay into
impressions rather than persisting verbatim. People misremember — a feature, and also
the memory budget. Distinct from the chronicle (§16): memory is *subjective and
lossy*, the chronicle is *objective*.

## 14. Environment — how place shapes behavior

### 14.1 Places carry an environment vector

```rust
struct EnvironmentVector {
    affluence: f32, density: f32, safety: f32,
    bonding_capital: f32,     // dense ties *within* the neighborhood
    bridging_capital: f32,    // ties *out* to opportunity
    education_access: f32, job_opportunity: f32, services: f32,
    pollution: f32, churn: f32, norms: NormProfile,
}
```

Archetypes are derived labels (principle 2), never assigned:

| Archetype | affluence | density | safety | bonding | bridging | opportunity | churn |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Distressed urban | low | high | low | **high** | **low** | low | high |
| Working-class | low–mid | mid | mid | high | mid | mid | low |
| Suburb | mid–high | low | high | mid | mid | mid | low |
| Metropolitan core | high variance | very high | mid | low | **high** | high | high |
| Affluent enclave | high | low | high | mid | **very high** | high | very low |
| Rural | low–mid | very low | mid–high | high | low | low | very low |

Splitting social capital into bonding and bridging is the most important column there.
A distressed neighborhood is typically *not* short on community — it's short on ties
that reach opportunity. One combined "social capital" number would collapse that and
produce the lazy version of this simulation.

### 14.2 Four channels

Distinct because they have different signatures, and the observer should tell them
apart:

1. **Opportunity — which actions exist at all.** Schooling and capital-intensive work
   are gated by `education_access` and `job_opportunity`. Structural, and the largest
   effect. Someone who never attended school didn't lack conscientiousness; the action
   wasn't on the menu.
2. **Payoff — same action, different expected value.** Job search where opportunity is
   0.1 has a low return, so a *correctly* reasoning agent invests less. Looks like low
   motivation; isn't.
3. **Stress — scarcity changes how you decide.** Danger, churn, and unmet needs
   accumulate into a load that raises the **discount rate** and amplifies neuroticism
   expression. Short time horizons explain under-investment in slow payoffs far better
   than any trait does.
4. **Norms — local behavior is contagious.** Weighted by conformity (agreeableness ×
   age, peaking in adolescence). How neighborhoods reproduce themselves.

```rust
score = need_pressure(person)
      × trait_weight(phenotype, env)                  // G×E
      × opportunity(place, person)                    // channel 1 — a hard gate
      × payoff(place, action, discount_rate(stress))  // channels 2 & 3
      × norm_bias(place.norms, action, conformity)    // channel 4
```

### 14.3 Genes × environment

```
phenotype = √h²·genetic + √c²·shared_env + √(1−h²−c²)·unique
```

Three interaction mechanisms, implemented separately because they behave differently:

- **G×E interaction.** Same genotype, different outcome by context: high
  sensation-seeking becomes entrepreneurship where opportunity is high and risk-taking
  where it isn't. The trait is constant; its expression is not. This is what the
  ghetto/suburb question is really about — the answer isn't "environment applies a
  penalty."
- **rGE.** *Passive*: parents supply both genes and neighborhood, so the two are
  correlated from birth and genuinely hard to disentangle — the sim should be honest
  about that rather than faking clean identification. *Evocative*: temperament shapes
  how others treat you. *Active*: adults choose environments matching their traits,
  which produces residential sorting endogenously.
- **Developmental windows.** Exposure is age-weighted — in utero, 0–5, and adolescence
  count far more than adulthood — and accumulates into `Developmental`, which largely
  freezes at maturity. So where someone grew up stays legible in them for life. Active
  rGE plus this weighting reproduces the Wilson effect for free: measured heritability
  rises with age as adults increasingly select environments matching their genes.

### 14.4 Feedback loops

Emergent from §14.1–14.3, and the reason to build it this way:

- **Sorting.** Households move toward places that fit → neighborhoods diverge →
  divergence strengthens sorting. Schelling dynamics.
- **Intergenerational persistence.** Low-opportunity childhood → less human capital →
  lower adult resources → same neighborhood → children inherit both the genes and the
  place.
- **Escape routes.** A model with only the loop above is deterministic doom — wrong,
  and boring. Bridging ties, mentors, schooling shocks, migration, and luck need real
  probability mass. See §15.

## 15. Balance: nature, nurture, and luck

You asked for both, and for a balance. Getting there needs one non-obvious move.

**Balance can't be defined as the variance split.** Heritability is a statement about
trait variance, but outcomes are dominated by channel 1 — the opportunity gate — which
isn't a variance component at all. You can set h² to a modest 0.4 and still build a
world where birth decides everything, simply by gating hard. So balance has to be
defined and measured **in outcomes**.

**Trait-level defaults** (behavior-genetics consensus, roughly):

| | h² (genes) | c² (shared env) | unique |
| --- | --- | --- | --- |
| Personality | 0.40 | 0.20 | 0.40 |
| Cognitive | 0.50 rising with age | 0.25 falling | 0.25 |
| Height/physiology | 0.80 | 0.10 | 0.10 |

**Outcome-level targets** — these are the actual definition of balance, asserted in
tests (§21) and tuned against:

| Measure | Target | Why |
| --- | --- | --- |
| Variance in adult outcome: genes / environment / luck | ~30% / ~40% / ~30% | Neither cause dominates; chance is a real third share |
| Intergenerational elasticity | 0.30–0.40 | 0.0 is a meritocracy fantasy; 1.0 is a caste system |
| Sibling correlation in adult outcome | 0.40–0.50 | Same genes, same home, different lives |
| Cross at least one outcome quintile from childhood | ≥ 40% | Mobility must be common, not miraculous |
| Same genome, distressed vs. affluent upbringing | 0.5–0.8σ shift | Place matters enormously — and isn't destiny |

The third row of the first column is the one most simulations get wrong. **Luck gets
an explicit ~30% share.** Real outcomes have a large stochastic component, and
modeling it prevents the sim from becoming a morality tale in either direction — the
"anyone can make it" version and the "nobody ever escapes" version are both wrong, and
both are what you get when chance is squeezed out.

A **balance harness** (`observer::balance`) runs a world for generations and reports the
whole table against its target bands, so constants are tuned against measurements rather
than vibes.

Two things it found immediately, neither visible by reading the code. Genes and
upbringing came out *exactly* balanced — 0.39 each once the entangled share is counted
on both sides — which is the design goal met. And intergenerational elasticity sits at
0.6, above the target band, but moving the transfer-at-birth constant across a 2.75×
range shifts it only from 0.55 to 0.62: advantage travels through the **neighbourhood a
child is raised in**, not through what they are handed. Which means the lever for
mobility is §14.4's escape routes, not the inheritance dial.

A caution the harness reports rather than hides: because parents supply both genes and
neighbourhood, a fifth of the variance is explained by either and separable by neither.
That **entangled** share is shown as its own quantity instead of being assigned to
whichever cause a regression happened to enter first.

## 16. Families, society, and the chronicle

```rust
struct Household { members: Vec<PersonId>, head: PersonId, dwelling: PlaceId, .. }
struct Relationships {
    kin:  Graph<PersonId, Kinship>,   // parent edges only — structural, permanent
    ties: Graph<PersonId, Tie>,       // friend/rival/partner/colleague — dynamic
}
```

Store kinship structurally (parent edges only) and derive siblings, cousins, and
ancestors by traversal; anything else explodes in edge count and drifts out of sync.
Life events drive the graph: pairing, birth, death, inheritance, migration,
estrangement — each emitting a chronicle event, which is where life stories come from
for free.

**The chronicle is one log for the whole world**, at every scale (principle 4):

```rust
struct Event { at: Time, place: Locus, kind: EventKind,
               participants: SmallVec<[EntityId; 4]>, salience: u8 }
```

A continental collision, a mass extinction, a speciation, a famine, a marriage, and an
argument at dinner are all `Event`s differing in salience and scale. A biography is
the log filtered by participant; a geological history is the same log filtered by
salience. Storage is tiered — recent verbatim, older compacted into summaries, only
high-salience kept forever — which is what makes megayears affordable.

## 17. Social intelligence

People in this simulation need to model *each other*, not just react to a world. The
agent-research field has converged on a useful decomposition of what that involves, and
it is worth borrowing — while being clear that almost none of its machinery transfers.

### 17.1 What the field decomposes social intelligence into

Humalike, which sells this as behavioural infrastructure for LLM agents, splits it into
roughly seven primitives: **turn-taking**, **theory of mind**, **norms**, **social
memory**, **social learning**, **persona**, and **proactiveness**. The generative-agents
line of work (Stanford's Smallville and its successors) arrives at a similar list from a
different direction, built on a memory stream with retrieval, reflection, and planning.

The *taxonomy* is the valuable part. The mechanism is not: all of it runs on LLM calls
per agent per utterance, which is incompatible with a world of millions of people
running for millions of years, offline and reproducibly.

### 17.2 Two things the design is missing

Checking that list against §14–16 turns up two real gaps, both cheap:

**Theory of mind — beliefs about people, not just feelings about them.** The design has
`Tie { affinity, trust }` and `impressions`: how someone *feels* about another person.
It has nothing for what someone *believes about* another person's traits, wants, and
intentions — and, crucially, nothing that can be **wrong**.

```rust
struct Belief {                    // A's model of B, stored on the tie
    traits: Personality,           // what A thinks B is like
    disposition: f32,              // whether A thinks B likes them
    intent: Option<Intent>,        // what A thinks B is up to
    confidence: f32,               // and how sure A is
    last_revised: Time,
}
```

A belief that diverges from the truth is the source of most of what makes social life
narratively interesting: misunderstanding, mistaken reputation, gossip that mutates as
it spreads, betrayal, and reconciliation when a belief is finally corrected. It composes
with the memory model already specified — memory is lossy and salience-weighted (§13.4),
so beliefs *should* drift from reality, and stale beliefs about people rarely seen are a
consequence rather than a special case. Cost is one small struct per tie.

**Norms as learned, not ambient.** §14.2's fourth channel reads `norms` off the place, as
though everyone were equally steeped in them. Humalike's framing — agents pick up hidden
rules and tone *from a group* — is the better model: each person carries their own
estimate of local norms, learned by observation, weighted by the developmental windows of
§14.3. That single change earns three things the ambient version cannot: migrants who
carry the old country's norms and only partly assimilate, adolescence as the period when
norm learning runs fastest, and cultural change that is *transmitted* rather than
imposed by editing a field.

Both belong in Phase 3 (environment) and Phase 4 (chronicle and memory) respectively, not
in Phase 1 — they need relationships and places to exist first.

### 17.3 Where an LLM belongs, and where it does not

Two independent findings from the scaling literature, pointing the same way.

The first is architectural convergence. Light Society reaches a billion agents with an
**event queue** plus a **mixture of full models and distilled surrogates** — full fidelity
for the agents that matter, cheap approximation for everyone else. Hybrid frameworks pair
LLM agents for a core subset with a cheap generative model for the rest. That is
precisely the level-of-detail design in §6 and §8, arrived at independently by people who
had to make planetary scale work. Encouraging, and worth taking as confirmation that
the LOD tiering is not a compromise but the known solution.

The second is more surprising, and it argues *for* the mechanistic core rather than
merely excusing it: **LLM agents show 20–300× lower behavioural variability than real
people**, collapsing toward high-probability responses. Population-scale diversity is
exactly what this project needs, and it is exactly what an LLM population fails to
produce. The genes + environment + luck model of §15 generates heterogeneity by
construction, with the variance split as a tunable. For simulating a *population*, the
mechanistic model is not the budget option — it is the more accurate one.

So the rule:

- **Never in the tick loop.** Behaviour is utility-scored from traits, needs, and
  environment (§13.3). Deterministic, offline, microseconds, and more varied than the
  alternative.
- **At the observation boundary, optionally.** When you zoom to one person, an LLM may
  narrate their dossier — turning a scoring table and an event list into prose in their
  voice. That is §8.3's backfill applied to language: expensive work done once, for the
  one entity being looked at, cached and keyed by seed so that re-inspecting a person
  gives the same words. The simulation stays reproducible because nothing the narrator
  produces feeds back into it.

The line to hold: **language is a view, never a cause.** The moment an LLM's output
changes what happens, worlds stop being reproducible, deep time stops being computable,
and the whole architecture in §5–8 unwinds.

## 18. The omniscient view

Read-only, at every scale:

```rust
impl Observer<'_> {
    // deep time
    fn timeline(&self, span: TimeSpan) -> Vec<Event>;      // salience-filtered by zoom
    fn globe_at(&self, t: Time) -> GlobeSnapshot;          // plates, biomes, ice, sea level
    fn phylogeny(&self, t: Time) -> Tree<LineageId>;
    fn climate_series(&self, t: TimeSpan) -> Series;       // CO₂, temp, sea level, ice

    // zoomed in
    fn random_person(&self) -> PersonId;
    fn dossier(&self, id: PersonId) -> Dossier;
    fn family_tree(&self, id: PersonId, depth: u8) -> Tree<PersonId>;
    fn why(&self, id: PersonId) -> ActionScores;
    fn nature_nurture(&self, id: PersonId) -> VarianceBreakdown;
    fn counterfactual(&self, id: PersonId, raised_in: PlaceId) -> Personality;
    fn follow(&mut self, id: PersonId);
}
```

The defining interaction is **continuous zoom**: scrub a timeline from 500 Myr to a
single afternoon, and from a globe view to one person's kitchen, with the LOD system
promoting and backfilling underneath. Watch continents assemble; zoom to a coastline;
zoom to a village on it; zoom to a person in the village; read her mind; zoom back
out and watch her descendants' lineage persist or vanish over the next hundred
thousand years.

A `Dossier` gathers identity and appearance; personality decomposed into genetic /
household / neighborhood / unique; needs, mood, stress, and current intent; a location
breadcrumb from galaxy to street with the local environment vector; family tree with
traits traced to the parent they came from; relationships; life timeline; and the
scoring table showing which options were *gated off* rather than merely outscored.

**Frontend.** Phase 5 is a TUI (`ratatui`) — clock, dossier, event feed, `r` to reroll,
`f` to follow. Terminal first because the data model is the hard part and a TUI can't
hide a thin one. The globe and phylogeny views need real rendering (`wgpu`) and come
with deep time in M4; both read the same `Observer` API.

### 18.1 The atlas

One page, four scenes, and the same gesture at every level: point at a thing and go
inside it. A globe you turn with the mouse; click it and you are in a region three
thousand kilometres across; click a settlement and you are in it; click somebody and you
are reading a life. The rail across the top is the way back up, and every step of it names
what you actually chose rather than "level 2".

It is drawn **pixelated on purpose**, and the reason is honesty rather than nostalgia. The
map is a quarter-degree per pixel because that is genuinely all the ground the simulation
solved; magnifying it into smooth coastlines would be inventing detail nobody computed. A
hard-edged pixel says *this is the resolution of what is known*, and a limited palette with
banded shading and a dither says the same thing about the shading. Where the atlas draws
something that is not in the data — a figure standing in a settlement — it is plainly a
token, built from the person's own numbers: their name picks the palette, their age picks
the height, and how they have done picks the collar.

It is operable without a mouse. The globe takes focus, arrows turn it, Enter descends at
whatever faces you — the same promise the pointer makes, kept without one. That was missing
from the first version, which put the whole point of the page behind a gesture some people
cannot make. On touch, a tap that descends is followed half a second later by a synthesised
click at the same coordinates, which lands on whatever the descent just put under the
finger; anything arriving within that breath is a ghost and is ignored, or one tap carries
you down two levels.

The person scene shows the §15 decomposition rather than hiding it: every one of the five
factors is three bars — what came down the genome, what the household put there, and what
was nobody's doing — because "why is she like that" having an answer is the point of
carrying the three separately.

## 19. Fidelity: what's real and what's approximated

Being explicit about this is what separates a coarse simulation from a fake one.

### 19.1 The approximations

| Subsystem | Real approach borrowed | What's given up |
| --- | --- | --- |
| Climate | Budyko-type energy balance, diffusive transport, smoothed ice albedo | No GCM: no storms, jets, eddies, or seasons — climate, not weather |
| Insolation | Daily-mean integrated over the year, any obliquity | Circular orbit: no eccentricity or precession, so no sub-Myr Milankovitch |
| Carbon | GEOCARB weathering against boundary-length outgassing | No organic carbon, no methane, no sea-floor weathering |
| Moisture | Budget advected on prescribed Hadley/Ferrel winds | Winds are parameterized by latitude, not solved |
| Ocean | Enhanced heat transport across water | No circulation as such: no gyres, upwelling, nutrients, or anoxia yet |
| Tectonics | Euler-pole plate motion, Airy isostasy, √age ridge subsidence | No mantle convection; plate reorganizations and rifting are stochastic |
| Erosion | Threshold stream power, sediment routed downstream | No hillslope diffusion — see below; coarse at 112 km cells; no river networks below cell size |
| Vegetation | Whittaker classification + Miami productivity | Read instantly from climate: no lag, no competition, no fire, no plant populations |
| Populations | Energy-limited demes, Kleiber and Damuth scaling, tenth passed up | Not individual-based outside Full LOD; no age structure, no migration |
| Genetics | 256 polygenic loci, Wright–Fisher | Not molecular; no gene regulation or real recombination maps |
| Behavior | Utility AI over needs and traits | Not a cognitive model; no language or planning depth |

Each row is a real method with real feedbacks, run coarsely. That's the honest claim:
mechanistically grounded, low resolution. Anywhere the resolution stops mattering, we
can refine that row without touching the others — which is the reason for the crate
boundaries in §4.

**Three things Phase 4 deviates from this plan on, and why.**

*Hillslope diffusion is absent.* Measured hillslope creep has a diffusivity around
10⁻² m²/yr. At 112 km cells the term it contributes is under a millimetre per megayear —
sub-grid by four orders of magnitude. Putting it in would have meant inventing a
coefficient thousands of times the measured one and calling the result physics.

*Erosion carries a threshold.* `E = K(√A·S − ω)` rather than `E = K·√A·S`, which is the
standard threshold form. Without it a continental interior at a gradient of one part in
a thousand still erodes at tens of metres per megayear, and every continent is planed to
the waterline within a few hundred megayears. With it a craton is below threshold and
effectively permanent, which is what cratons are.

*Erosion rates are resolution-dependent, and under-resolved.* An orogenic gradient is a
few percent; a grid cannot represent a gradient steeper than its own spacing allows, so
at 450 km cells a mountain range is a gentle ramp and erodes at cratonic rates. The
planet compensates in part — collision piles crust high enough that neighbouring cells
do differ by kilometres — but an isolated range decays several times slower than a real
one. This improves with grid level and does not go away.

**What Phase 5 gets wrong, stated plainly.** Below about three quarters of today's
sunlight the planet goes to a snowball and no achievable carbon dioxide recovers it. The
infrared law is linearised around present conditions and its greenhouse saturates
logarithmically, so the forcing available at half a bar is not enough against an albedo
that has doubled. The real Earth of that era is thought to have needed methane as well,
which is not modelled. There is a test that pins this so it stays a known limit.

**And one thing the model gets visibly wrong.** A planet settles at around a seventh to a
quarter of its surface above water where Earth manages a little under a third. The
continental crust itself is stable — arc magmatism replaces what collision and erosion
take — but more of it ends up submerged than should. The likeliest reason is that
erosion here is blind to climate: every square kilometre of land is rained on
identically, where a third of the real one is arid and barely erodes. Precipitation
arrives with Phase 5, and waiting for it is better than bending a measured constant
until the land fraction comes out right.

### 19.2 Keeping it from feeling fake

Coarse models tend to produce smooth, averaged, boring worlds. Four countermeasures:

- **Fat tails, not means.** Volcanism, impacts, droughts, epidemics, and windfalls
  draw from heavy-tailed distributions. Most centuries are dull; some are catastrophic.
  Averaging that away is the single fastest route to a lifeless simulation.
- **Hysteresis and thresholds.** Ice sheets, anoxia, and biome boundaries have
  tipping points and don't retrace their path on the way back. Systems that only
  respond smoothly and reversibly feel synthetic because they are.
- **No global knobs.** Every quantity is local and computed. There is no "civilization
  level" or "world danger" dial — those are the tell of a fake.
- **Validated against reality.** §21 checks the world against known Earth statistics:
  land fraction, biome area distribution, species-area relationships, extinction rate
  distributions, demographic pyramids. If a simulated Earth-like planet produces 90%
  desert, the model is wrong regardless of how convincing the mechanism sounded.

## 20. Roadmap

Four milestones, each shippable, each independently interesting.

### M1 — A world that lives (foundations + people)
| Phase | Deliverable |
| --- | --- |
| **0** ✅ | `sim-core`: handles, arenas, seeded RNG, **the full time-scale ladder and scheduler**, event bus. Port existing Person/Planet behavior unchanged onto it |
| **1** ✅ | Person depth: OCEAN, values, continuous needs, utility AI (with §14.2 hooks present but constant), aging, health, mortality |
| **2** ✅ | Genetics + families: loci, meiosis, pedigree-derived genomes, households, kinship, birth/pairing/death, a population that sustains itself |

The scale ladder lands in Phase 0 even though nothing uses it yet. Retrofitting it
later means rewriting every system, and it's cheap to build before there are systems.

### M2 — A world that has places (neighbourhoods, then the planet under them)
| Phase | Deliverable |
| --- | --- |
| **3** ✅ | Environment & neighbourhoods: environment vectors on places, archetypes derived from them, the four channels live, developmental windows, standing, residential sorting, the §15 balance harness |
| **4** ✓ | `geo`: geodesic grid, plates, elevation, isostasy, erosion, bathymetry |
| **5** ◑ | `climate`: energy balance, insolation, moisture, ice, carbon cycle. Ocean circulation — wind belts, Ekman transport, upwelling, overturning, nutrients — now in `ocean`, and read by `biome` |
| **6** ◑ | `biome`: Whittaker classification, NPP. Plant functional types with populations of their own — and therefore lag, and fire — deferred to Phase 7 |
| **7** ✓ | `ecology`: animal demes, trophic web, dispersal, habitat suitability |

Neighbourhoods come before geography, which is the reverse of the obvious order and
deliberate. The four channels are already wired into behaviour and sitting neutral, so
filling them needs households and places with properties — not tectonics. Places start
abstract and acquire a grid cell in Phase 4; nothing about §14 depends on where a
neighbourhood physically sits. Doing it the other way round would mean a planet with a
climate and no one whose life it changes.

**Phases 9 and 11, in the event.** The observer is the thing the project was started for
and it turned out to need one change elsewhere: `why` has to be answered in the situation
the person is *actually* in, which only the world knows. Asked in a neutral situation it
reports that a child ranked work poorly, when the truth is that work was never on offer —
and those are different facts about a life. So the world hands over the situation and the
observer reads it. `why` is `&self` throughout, and there is a test that a run with every
person interrogated at every step comes out identical to one where nobody was asked.

Evolution needed three goes at balance. Speciation on a planet with real geography is a
fountain — ranges are dotted with islands and mountain valleys, and treating each as a
founding population split every species several times within a hundred megayears. Raising
the founder threshold and the isolation time helped; making extinction risk depend on
*range size* rather than only on abundance helped more, since range size is the best
single predictor there is. What actually balanced it was diversity-dependent
diversification: the fuller the world, the harder it is for a new lineage to establish,
because there is less unoccupied opportunity to establish into. With that the count
settles near its niche ceiling and turnover goes on underneath — seven hundred and
eighty-six originations against two hundred and ninety-nine extinctions over a gigayear,
with lineages four deep.

**Phase 7, in the event.** The trophic pyramid, the latitudinal diversity gradient, and
ranges that track their climate all came out of the arithmetic rather than being aimed
at. Two things had to be got right for them to. Capacity has to be shared among
competitors *in proportion to how well each is suited*, not equally — equal shares makes
every species of a class the same size, so none is ever rare and nothing ever dies. And
"present" has to be an absolute density rather than a share of a species' own best cell,
which quietly inverted the diversity map: a species with an enormous tropical peak counted
as absent everywhere else, so the richest places came out looking the poorest.

The phase also closed the loop Phase 4 left open. Erosion now cuts in proportion to how
much rain actually falls, which needed the climate to exist first. Without it a desert
wore down as fast as a rainforest, land fell from a third of the surface to three percent
over a gigayear, and the carbon thermostat then lost the rock it regulates with and let
carbon dioxide climb to six percent of the atmosphere. With it the same planet holds a
tenth of its surface dry and stays temperate throughout.

**`cosmos`, in the event.** The project is called a simulation of the universe and its
universe was one sun, hardcoded, with a brightness curve fitted to it. What was missing was
not machinery — everything above already took a solar constant as an argument — but a
*source*: something to say where that number comes from, and therefore what a world that is
not Earth would be like.

A main-sequence star's mass fixes everything else about it, which is as close to a free
lunch as astrophysics offers. Luminosity from the mass–luminosity relation (piecewise: one
exponent across the whole range puts red dwarfs out by a factor of several), lifetime from
`M/L`, brightening across that lifetime calibrated on the one star anybody has measured
properly. Habitable zones on the standard bounds. Systems with their orbits spaced
geometrically and their masses drawn log-uniformly, giants beyond the snow line.

Founding a world is now a **search**, and that is the honest shape: most stars have nowhere
worth living, and a world with people on it is by construction one of the lucky ones. The
anthropic principle written as a loop.

Four things had to be got right and three of them were caught by tests written against
measured stars. The brightening curve was normalised against lifetime fraction rather than
the sun's present age, so the sun came out at 0.82 of its own luminosity. Planet masses
drawn uniformly put almost every inner world above the mass that holds an atmosphere, and
four systems in five then came out with a comfortable Earth in them — against a measured
eta-Earth nearer a third; log-uniform masses and a hard floor at 0.3 Earth masses fixed it,
which is also why Mars is in the sun's habitable zone and dead anyway. And tidal locking
had to be modelled, because most stars are red dwarfs and a red dwarf keeps its habitable
zone inside its own locking radius.

The fourth was a genuine disagreement between two models rather than a bug, and chasing it
down produced the most interesting result in the phase. A world at two thirds of Earth's
sunlight passes the habitable-zone test and then freezes solid here at forty below with its
carbon dioxide pinned at the model's ceiling — because the standard outer bound assumes a
planet can accumulate several bars of it and this energy balance caps at half a bar.

Narrowing the band to what the climate can hold fixed the freezing and revealed the real
constraint, which is **not temperature at all**. Everywhere from three quarters to a
quarter again of Earth's light comes out temperate, because that is what a thermostat is
*for*. What varies is the atmosphere it needs in order to manage it. At nine tenths of
Earth's light this planet settles at a comfortable thirteen degrees under **seven per cent
carbon dioxide**, which is four times the concentration that kills a person. Past about one
and an eighth, the thermostat has drawn the air below a hundred and fifty parts per million
and ordinary photosynthesis stops: the planet is warm, blue, and starving.

So a world's habitability here is decided by its air, and the band where that air is
breathable *and* green is 0.97 to 1.12 of Earth's light — startlingly narrow, and it holds
to within a couple of per cent across every planet tested. Even that is not sufficient on
its own, because how much carbon dioxide a thermostat needs also depends on how much
weatherable rock the planet has, which varies by a factor of two between seeds. So the last
step is to *ask* the climate rather than predict it: `sim` solves up to four candidate
worlds and takes the first whose air people could breathe.

One further consequence had to be untangled. `cosmos` scores the middle of the habitable
zone highest, which is right as astronomy and wrong here, since the breathable band sits
against the zone's *inner* edge — so the two criteria pulled in opposite directions and
between them admitted almost nothing. Placement is now separable: `promise` is the
astronomy, `body_and_time` is everything about a world except where it sits, and `sim`
supplies its own placement term.

**The sea, in the event.** Marine productivity was a placeholder and said so: a flat
multiplier — one on the shelf, a bit over a quarter everywhere else — standing in for a
nutrient budget that did not exist. It got the pattern roughly right by accident, because
shelves really are better fed, and it got the reason wrong, which meant it could produce
none of the consequences.

The reason is that **everything which grows in the sunlit layer sinks when it dies**. The
surface is stripped and the depths are rich, so the productive sea is wherever deep water
is being brought back up — which makes the whole crate really about one quantity,
upwelling, with the circulation existing to produce it. Wind belts are a function of
latitude (the three-cell circulation, which is not a choice), Ekman transport is the wind
turned ninety degrees, and coastal upwelling is wherever that rotation points away from a
shore. Plus equatorial divergence, winter mixing where the water is cold enough to turn
over, and river supply on shelves below wet land.

Nothing in it knows the word "eastern", and the eastern-boundary fisheries come out
anyway — Peru, California, Benguela, the Canaries. That is the test the crate exists to
pass, and drawing it settles the matter: the nutrient map has blue subtropical gyres in
two bands, a bright line along the equator, green at the mixing latitudes and bright
fringes on the upwelling coasts. It reads like a satellite chlorophyll image because it
was arrived at the same way the real pattern is.

Two corrections came out of wiring it in, and both were mine. The temperature term still
peaked in the mid-latitudes, which had been standing in for stratification; moving
stratification to the nutrient supply and leaving Eppley's exponential in its place made a
starved tropical gyre out-produce well-fed temperate water. The residual temperature
effect on *annual* production is weak, and Eppley bounds a maximum growth rate rather than
a realised one. And a test asserting that a frozen planet's biosphere collapses turned out
to have been passing for the wrong reason — the planet in it is a cold world rather than a
snowball, and a cold ocean is *better* fed than a warm one, because cold surface water is
dense and the column turns over. What collapses is the land. That is the Cryogenian
pattern and the test says so now.

**Phase 6, in the event.** Almost nothing to report, which is the point: a biome turned
out to be sixty lines of `if` and the whole crate has no state at all. That is principle
two paying off — store the vectors, derive the labels — and the payoff is that biomes
*move* with no machinery to move them. Continents drift into the subtropics and grow
deserts down their middles; an orogeny casts a rain shadow and the forest behind it
becomes steppe; the thermostat draws carbon down and the taiga retreats.

Two corrections to the textbook diagram were needed, both because it was drawn from field
sites on one planet. Dryness had to become the **aridity index** — rainfall over potential
evaporation — rather than a depth of rain, because six hundred millimetres is a forest in
Siberia and scrub in the Sahel. And potential evaporation had to be driven by the *warm
season* rather than the annual mean, which meant the seasonless climate needed a stand-in
for seasons: latitude and distance from the sea. That second term is what separates
Yakutsk from Reykjavík — the same annual mean, taiga at one and tundra at the other,
because only one of them gets a summer.

**Phase 5, in the event.** The thermostat works, and it is the most satisfying thing in
the project so far: give the planet a sun that brightens by a third and its mean
temperature moves by fifteen degrees rather than thirty-eight, because weathering
accelerates and draws carbon dioxide from thirty thousand parts per million down to
sixty. Nothing in the code aims at a temperature. The faint young sun comes out right
too — a planet under a sun four fifths of today's holds about a tenth of a bar of carbon
dioxide, which is what geochemists read out of Archean rocks and is not a number this
model was fitted to.

Two things were wrong and both were arithmetic rather than physics. The heat-transport
term was written as a conductance between neighbours without dividing by the square of
the distance between them, which is a factor of a couple of hundred at level four and
produced a planet with a hundred degrees between its equator and its poles. And the
ice albedo was a step function, which makes the ice–albedo feedback far more violent
than it is: the hysteresis loop it opened was so wide that a frozen planet could not
escape under *any* amount of carbon dioxide. A third problem was subtler and worth
recording — solving the temperature all the way to equilibrium before letting the carbon
answer freezes a planet under a faint sun and then strands it, because by then its
albedo has doubled. The two have to move together, which is what the real system does.

**Phase 4, in the event.** The supercontinent cycle does fall out of the mechanism, which
was the thing most at risk: plates weld where continents collide, a plate holding too
much of the world's continental crust rifts, and the largest landmass accordingly runs
up towards one and back down over hundreds of megayears without anything scheduling it.
Three things had to be added that §7.2 does not mention, each because the planet visibly
misbehaved without it — orogenic collapse (thick crust spreads sideways, or collisions
convert continental area to thickness one way only and the continents shrink away), arc
magmatism tied to convergent boundaries and calibrated to the measured rate of crustal
growth (or erosion and collision between them retire the continents), and a **gather**
rather than a scatter when resolving plate motion onto the grid (or the rounding is a
random walk, and continents dissolve into archipelagos of single cells within a few
hundred megayears). Only the last of those was visible in any summary statistic, and
only after the fact; all three were found by drawing the planet and looking at it.

### M3 — A world you can watch
| Phase | Deliverable |
| --- | --- |
| **8** ◑ | `chronicle`: unified event log, indices, salience, compaction. Per-person *memory* — a bounded set of remembered events that feeds back into behaviour — is left with the observer phase that reads it |
| **9** ◑ | `observer`: random person, dossier, family tree, why, nature/nurture, counterfactual — all present and tested. The TUI is not: the HTML viewer turned out to be a better instrument than a terminal one and took its place |
| **10** ◑ | Spatial LOD: tiers, promotion/demotion, aggregate invariants — **done early**. Backfill of never-simulated history is the remaining part |

### M4 — A world with history (deep time)
| Phase | Deliverable |
| --- | --- |
| **11** ◑ | `evolution`: adaptation with a tracking speed limit, allopatric speciation, diversity-dependent diversification, extinction, phylogeny. Gene flow and molecular drift are not modelled — species traits move by a rule rather than by inheritance from individuals |
| **12** ◑ | Deep-time integration: adaptive stepping ✓ (the lithosphere subdivides its own step so no plate ever jumps a cell), supercontinent cycle ✓, orbital forcing ✓ as machinery — obliquity varies on its 41 kyr cycle, which megayear steps cannot resolve and honestly return the mean of. **The join ✓** — `settlement` puts neighbourhoods on real grid cells and the ground shapes them; the planet under a populated world is a still frame, and people at deep-time resolution is what remains. Keyframing and a named mass-extinction mechanism are not built; extinction pulses do emerge from climate shocks |
| **13** ✗ | Globe + phylogeny rendering (`wgpu`), timeline scrubbing, continuous zoom. **Superseded.** The self-contained HTML viewer does the job better in every way that matters here — five layers, a time scrubber, hover readout, and a file anybody can open — and it is what actually found four of the modelling bugs in Phase 4. A `wgpu` client would be a second renderer for the same data |
| **14** ◑ | `cosmos` ✓ — stars, orbits, habitable zones, and an anthropic search for a world worth founding on. Save/load ✓ — as a *derivation* rather than a format: a world is a pure function of five numbers, so the save file is those numbers and loading re-runs. Exact, no schema to rot, and it costs the time being opened, which is the whole trade and is written down. Economy ✓ — `economy`: Cobb–Douglas on land and labour, subsistence taken out first, and trade weighted by reach, with hunger as the check that stops a population (§21.2). Culture ✓ — `culture`: peoples and countries emerge from transmission, drift and descent, and nobody writes either down (§24). Technology ✓ as machinery and inert in practice — technique is carried per country and decays below its Tasmanian threshold, which no world yet reaches (§21.3). Determinism goldens exist in the form that survives: every subsystem tests that the same seed produces the same result, self-comparing rather than pinned to a constant, so a legitimate change does not require re-blessing a hash |

**Sequencing note.** Deep time is what you most want and it lands last, for a real
reason: there's nothing to evolve until there's a planet and a biosphere to evolve on,
and watching biomes shift requires biomes. The mitigation is that the *architecture*
for it — the scale ladder, backfill, the unified chronicle, deme-based genetics — is
in from Phase 0, so M4 is integration rather than invention.

**Where it stands.** The stack runs end to end: plates → climate → biomes → animals →
evolution, a gigayear at a time, and each layer reads the one below it and nothing else.
The one wire that runs *back down* is rainfall into erosion, and it had to exist —
without it the continents wear away, the carbon thermostat loses the rock it regulates
with, and the planet cooks.

**The join, in the event.** People stand somewhere now. A world's neighbourhoods are no
longer five authored names with a capacity divided out of the founding population; they
are cells of a real planet's grid, chosen because somebody could live there, named after
what the ground is, and shaped by it. `settlement` is where the two halves meet, and it
is the only crate that can see both.

The projection down is **four numbers wide**, and that is the decision worth recording.
The planet knows elevation, crustal thickness, sediment depth, sea level, temperature,
rainfall, ice cover, net primary production and which of fifteen biomes each cell is. A
human life turns on almost none of it. It turns on whether the land feeds you, whether
anyone can reach you, how hard the year is, and how many of you the place will hold — so
those four cross and the rest stays on the planet's side. Handing `society` a grid and a
climate instead would have made every rule about neighbourhoods also a rule about
geophysics.

Habitability is a **product** rather than a weighted sum: a place has to be survivable,
and fed, and reachable, all three at once. A sum lets a spectacular score on one term
carry a zero on another, which is how a capital city ends up on an ice cap because the
fishing offshore is excellent.

Three things were wrong and the map found two of them.

*The five quarters came out on three continents* — 128° east, 75° west, 165° east. They
are neighbourhoods of one society, not five civilisations sharing a chronicle. Sites are
now chosen within one country around the single best cell, so what varies between them is
the difference between good ground at the centre and poorer ground at the edges, which is
what a region is.

*Fertility saturated.* A hard ceiling at what a temperate forest produces meant every
decent site scored exactly one and the term distinguished nothing. It saturates now
instead, which keeps the shape that is true — bare rock to thin pasture is worth far more
than good land to better — without running out of range.

*Terrain capped affluence, and the balance harness caught it.* Affluence is what the
residents have; it is what their children's upbringing is read off, and what decides where
those children can afford to live. Capping it puts the ground inside that loop: a poor site
drove its residents poorer, which drove their children poorer, and three of five quarters
fell to an affluence of one part in twenty-five with the heritable share of outcomes down
to 0.03. Land does not confiscate wages. It limits **what work there is**, and income
follows from that through people — which the loop already models. With that corrected the
shared-environment and luck shares both came back inside their targets, and more of the
world is lived in than before.

A fourth thing looked wrong and was not. These worlds carry several thousand parts per
million of carbon dioxide, which reads as a thermostat that was never given time. It was
measured rather than assumed: six hundred megayears of plates and climate leaves the same
planet *higher*, at fifty-eight hundred, because what the thermostat regulates against is
weatherable land and six hundred megayears of erosion cuts the land from a third of the
surface to a sixth. The carbon dioxide is high because there is little rock to draw it
down. That is the carbonate–silicate cycle working. The cost of finding out was thirteen
seconds per world at the grid the plates need, on every world any test founds, and it is
written into the code so it is not discovered twice.

**People at deep time, in the event.** The other half of the join, and the question it
turned on — what a person *is* when the clock strides a megayear — has a short answer once
written down: **they are not anybody**. A megayear is thirty thousand lifetimes. There is
no individual at that resolution, and what survives the projection upward is a *folk*: a
number of people, in a place, with a memory of where they came from. That is the §6
level-of-detail move taken one rung further, and `sim::deep` is it.

The order is the whole design. The planet steps *first*, with no knowledge that anybody is
on it; the people are then told what their ground is now. Causation runs one way, so
everything that happens to them is a consequence rather than a rule — and it reaches them
through exactly one channel, the habitability of their cell, recomputed each step.

Half a gigayear of one world, at level three: **827 settlements founded and 790 lost.**
Six hundred and fifty-three drowned, a hundred and six dried out, twenty-two froze, nine
were thrown up by an orogeny. Population tracks habitable area — fifty-seven million people
across a quarter of the surface early on, twenty-five million across a tenth after a
glaciation at 432 Myr took the temperate belt. Not one settlement standing at the end was
founded before 416 Myr; the world outlived every one of the original thirty-seven.

That drowning is five sixths of all losses is the nicest thing to fall out of it, and
nothing aimed at it. The best ground to live on is coastal — the sea is a road, so reach is
a third of what makes a site good — and coastal is exactly what the sea takes back.

**Recommended tactic:** after M1, build one deliberately crude vertical slice through
every scale — a blobby planet, three PFTs, two animal species, a million years — before
deepening any of them. It will be ugly and it will de-risk the entire project, because
the scale-crossing projections (§5.3) are where this design is most likely to be
wrong, and that's much cheaper to discover early.

## 21. Validation

Coarse models must be checked against reality or they drift into plausible nonsense.

**Engine**
- Determinism goldens: seed → state hash at time T. Catches stray `thread_rng()` and
  iteration-order nondeterminism, which are otherwise invisible until they corrupt a
  save format.
- Scale-crossing equivalence: a region run fine for 50 years vs. coarse-then-promoted
  agrees on aggregates within tolerance. The riskiest property in the design, so it's
  tested hardest. **It was false for a long time — see §21.1.**
- Graph invariants: kinship acyclic, no parent born after a child, every id resolves.

**Physical**
- Earth-like inputs → Earth-like outputs: land fraction, latitudinal temperature
  gradient, biome area distribution, precipitation in the right places.
- Conservation: mass, energy, water, and carbon balance to tolerance over 1 Myr.
- Milankovitch response: glacial cycles at the right periods under orbital forcing.
- Stability: 100 Myr with no NaN, no runaway, no dead planet from numerical drift.

### 21.1 The observer was setting the death rate

The riskiest property in the design was silently false, and it stayed false because
everything it broke looked like something else. The same world, same seed, differing only
in how many people the observer could afford to simulate finely, ran like this:

| Detail budget | Living at year 220 | Worst 10-year fall |
|---|---|---|
| 150 | 184 | 29 |
| 400 | 384 | **141** |
| 2000 | 990 | 11 |

The trajectories are *identical* for the first century and separate exactly where the budget
starts to bind. The 141-soul collapse in the middle run read as a famine — a population
overshooting its land and being cut back, which is precisely what the economy was built to
produce, so it was the most plausible possible disguise.

It was an accounting error. A coarse person's clock is stamped forward once a year, at
their birthday, by `get_by`. The fine tier's first act is `catch_up`, which accrues every
need across the whole span since that stamp. So anybody crossing from coarse to fine at any
*other* moment was billed for up to a year of unrelieved hunger and thirst in a single
step, then charged a year of health decline against it. They died of deprivation, and
children died fastest.

There are three ways to cross, and each had to be found separately:

1. a **place** is promoted back into the budget;
2. a **household moves** out of a coarse quarter into an already-fine one — invisible to
   promotion, because that place never changed tier;
3. two people **pair**, and the new household takes the *other* partner's quarter.

All three now go through one handover that hands a person to the fine tier in the state the
coarse tier claims for them. Afterwards, the same three runs finish at 1030, 893 and 1045,
the crash is gone at every budget, and the populations agree to within 1% out to year 160
and 15% at year 200 — with the residual no longer ordered by budget, which makes it chaotic
divergence rather than bias.

Three things are worth keeping from how this was found. **The first fix was wrong**: the
frozen-vitality bug next door (`get_by` passing `Duration::ZERO`, so a coarse person's
health could never recover) is real and is fixed, but repairing it moved the population by
four souls. **The second fix made things far worse before it made them better** — handing
*every* mover to the fine tier, rather than only those arriving from coarse ground, wipes a
life mid-stride and collapsed every world to a dozen people at every budget alike. **And
the whole thing was invisible to the test suite**, which had 614 passing tests at the time,
because every one of them ran small enough worlds that the budget never bound.

### 21.2 What stops a population, and what does not

With §21.1 fixed, nothing bounded population at all. Places ran to three and five times what
their ground would hold, and growth *accelerated* — 0.98%/yr over years 40–80, 1.76%/yr over
160–200 — with no sign of levelling. The bug had been the only brake, and it worked by
killing children in demoted quarters.

**Fertility was the wrong lever, and this is the fifth time of asking.** `births_relative`
was switched on against the world's own household-weighted mean — the most defensible centre
available — on the explicit expectation that the four earlier verdicts against it were
confounded, since every one had been measured while the detail budget was culling people.
They were not confounded. Six seeds, sixty founders, and the check on its own:

| Seed | yr 90 | yr 180 | growth | yr 90 with check | yr 180 with check |
|---|---|---|---|---|---|
| 220 | 65 | 187 | +1.18% | 35 | 64 |
| 221 | 130 | 489 | +1.48% | 105 | 388 |
| 222 | 141 | 485 | +1.38% | 118 | 471 |
| 224 | 150 | 590 | +1.53% | 92 | 339 |
| 225 | 157 | 629 | +1.55% | 46 | 15 |

It halves worlds before hunger ever enters, and it kills the marginal ones. Prosperity does
not vary enough between places for a multiplier of this strength to be anything but noise
with a downward bias. `economy::births_relative` stays written, tested, and uncalled, and
the original judgement stands.

**What does work is that the land will not feed them.** The reading already existed and was
being discarded: `prosperity` is `per_head().max(0)`, so a place in famine reported exactly
what a place breaking even reported. `Ledger::want` is that clamp undone — how far short of
feeding its people a place falls, per head, after trade.

Routing it through `Needs` and `Health::respond_to` does **not** work, and the shape is worth
recording because it is what defeated the earlier attempts. That machinery answers per *day*.
A want of 0.4 puts vital pressure at 0.30 against a tolerance of 0.45, so the body recovers
and famine is free; a little higher and a year at three tenths a day kills everyone outright.
Nothing, then a massacre, with no useful ground between.

What is true instead is that a body cannot be in better condition than its food allows, so
`want` sets a **standing ceiling** on vitality (`Health::fed`), and everything downstream
already responds — frailty rises with the square of the deficit, conception scales with
vitality, and `is_fertile` stops at a half. It has to be standing rather than applied yearly:
recovery runs at five hundredths a day, so a one-off ceiling is gone inside a fortnight and
only bites because the mortality roll happens to follow it in the same call.

| Seed | growth, no hunger | growth, with hunger | want at yr 180 |
|---|---|---|---|
| 220 | +1.18% | +1.00% | 0.074 |
| 221 | +1.48% | +1.14% | 0.104 |
| 222 | +1.38% | +0.61% | 0.146 |
| 224 | +1.53% | +0.80% | 0.091 |
| 225 | +1.55% | +0.44% | 0.105 |
| 223 | −0.87% | −0.87% | 0.000 |

A brake proportional to how tight the land is, biting hardest exactly where want is highest,
and culling nobody — the year-90 populations barely move. Seed 223 declines identically in
every configuration, including with everything switched off: a world that was always going to
fail, not something the check did.

**`HUNGER_COSTS` is set by where a cliff sits, not by how hard the brake pulls.** `is_fertile`
gates at a vitality of a half, so hunger deep enough to reach that gate does not slow births,
it stops them — the same "nothing, then a massacre" shape this mechanism exists to avoid,
reintroduced one layer up. At 1.4 the gate sat at a want of 0.36, which any world founded on
ground that was already full reaches at once, and such worlds did not level off, they fell
over:

| `HUNGER_COSTS` | 80 founders, 180 yr | 80 founders, 180 yr | 400 founders, 150 yr | 400 founders, 150 yr |
|---|---|---|---|---|
| 0.6 | 461 | 630 | 602 | 383 |
| **0.9** | **350** | **452** | **521** | **260** |
| 1.4 | 308 | 373 | **86** | **65** |

Four hundred founders came to 86 souls where eighty founders on the same seed grew to 373 —
the more people a world began with, the fewer it ended with. At 0.9 the gate needs a want of
0.56, real famine rather than a lean generation, and the braking is still about a quarter
against no hunger at all. Higher values were also measured (2.2 and 3.0) and only make the
collapse worse for no extra brake.

This is a brake, not a ceiling — a world with room in its land still takes more than two
centuries to fill it, and that is the honest description.

**And it is what lets a second people exist.** Peoples are gated on `ENOUGH_TO_BE_A_PEOPLE`
(Dunbar's 150), so a world has to grow a place past that before it can go its own way. Under
the collapsing calibration no world ever did, and every run reported one people for ever.
With hunger set where it belongs, three worlds in four grow a second: `Norhaven` and
`Clearolu`, `Norhaven` and `Unquietyr`, `Bramwick` and `Untoilyr` — the ones who wash, the
ones who barely sleep, the ones who barely work. Nobody wrote a word of that down.

### 21.3 Technique is wired, and the trap does not open

`economy::Technique` — the Tasmanian mechanism, where technique lives in people rather than
in writing and decays below `MINDS_TO_KEEP` carriers — was built, tested, and never called:
`economies()` produced with `Technique::BARE` every year, so no world could accumulate or
lose anything. It is now advanced each reckoning, per **country**, because a country is
precisely the set of people who can reach each other to copy a technique from. Tasmania is
then not a special case but the ordinary case with a sea in it.

It does not fire, and the reason is scale rather than modelling. Carriers are
`minds x (0.5 + 1.5 x reach)`, so about five hundred well-connected souls are needed to hold
technique steady. Measured over five centuries on a world founded with a hundred and twenty
people: at year 200 it held 332 living, its largest country 192, and technique at exactly
1.0000. Nothing had been forgotten — nobody forgets how to eat — and nothing had been gained.

That is the right answer for populations of a few hundred, and it is what the Malthusian
trap staying shut looks like. It does mean the mechanism is inert in practice until worlds
are an order of magnitude larger, which is a scale limitation to state rather than a
calibration to fiddle with: `MINDS_TO_KEEP` is anchored on Tasmania's real four thousand.

**A country cannot be small here, and that is the grid's doing.** The link between places
was written as six hundred kilometres — a fortnight on foot, the classical radius of a state
held together by walking. It could not stay that way. A populated world runs at grid level
three, where a cell is 961 km across, wider than France, so settlements land one to four
thousand kilometres apart because that is the finest the ground can distinguish. An absolute
six hundred kilometres did not describe a small country; it guaranteed that no two places
were ever in one, every quarter was its own country, and the technique pool was a single
settlement for ever. The link is now expressed in grid spacing and tightens automatically if
the level is ever raised — but at level three a "country" is a handful of adjacent regions,
not anywhere anybody walked across. §23's first open question is load-bearing for more than
coastline fidelity.

### 21.4 The last of the observer's thumb on the scale

With §21.1 fixed, being unwatched no longer changed whether you lived. It still changed how
well you did. A coarse year approximates work as `WORK_SPELLS_PER_YEAR x availability`, and
that constant was 300 against a fine tier that works closer to five hundred times a year, so
an unwatched adult reached mean standing 0.374 where a watched one reached 0.476 — a fifth of
a lifetime's advantage, lost to nobody looking.

The equivalence test did not catch it because it allowed a tenth of *absolute* standing at
thirty years, while the shortfall is proportional and only opens over a lifetime. Standing
feeds affluence, a quarter's character, who is admitted where, and every §15 measurement, so
this was a systematic bias in the balance harness with the observer's budget as its cause.

At 380 the same comparison gives 0.472 against 0.476, and across three seeds the residual
gaps are +0.004, −0.020 and −0.007 — no longer one-directional, so what remains is noise.
Demography matches too: 42/42, 45/45 and 51/54 living. The test's tolerance is now 0.04,
which would have caught the original.

**Where the model misses its targets, measured.** §15's bands come from the
behaviour-genetics literature and the model does not meet two of them. Across four seeds at
about two hundred lives each:

| Quantity | Target | Measured | Verdict |
| --- | --- | --- | --- |
| Heritable share of outcome | 0.15–0.45 | 0.08, 0.10, 0.13, 0.27 | **low** |
| Chance's share | 0.15–0.45 | 0.41, 0.52, 0.57, 0.66 | **high** |
| Shared environment | 0.20–0.55 | 0.10, 0.15, 0.20, 0.36 | borderline |
| Intergenerational elasticity | 0.20–0.50 | 0.45–0.63 | slightly high |
| Mobility | 0.40–0.90 | 0.63–0.73 | met |
| Upbringing gap | 0.30–1.20 | 0.53–1.38 | mostly met |

**The measurement was most of it.** The bands come from twin and adoption studies, and the
heritability such a study reports is the *whole* genetic contribution — gene–environment
correlation included, because the design cannot separate it either. A conscientious child
raised by conscientious parents shows up inside the A component. This harness *can*
separate it, into an `entangled` bucket its own report describes as "inseparable — parents
supply both", and it was then comparing only the separated remainder against a figure that
never was separated. That marks the model wrong for being more careful than the measurement
it is checked against. Counting entangled towards both causes, which is what the studies
do, the same worlds read:

| Quantity | Target | Measured, eight seeds | Verdict |
| --- | --- | --- | --- |
| Heritable share of outcome | 0.15–0.45 | 0.31 | **met** |
| Shared environment | 0.20–0.55 | 0.41 | **met** |
| Chance's share | 0.15–0.45 | 0.46 | marginally high |
| Intergenerational elasticity | 0.20–0.50 | 0.63 | high |
| Mobility | 0.40–0.90 | 0.63–0.73 | met |

**Re-measured after §21.1 and §21.4.** Every figure above was taken while the detail budget
was culling people (§21.1) and while unwatched adults were earning a fifth too little
(§21.4) — both of which act directly on attainment, so the harness was measuring a world
with a thumb on it. Five seeds, 150 years, 150–382 lives each, on the repaired model:

| Quantity | Target | Five seeds | Mean | Verdict |
| --- | --- | --- | --- | --- |
| Heritable share | 0.15–0.45 | 0.35, 0.49, 0.27, 0.36, 0.25 | 0.34 | **met** |
| Shared environment | 0.20–0.55 | 0.31, 0.40, 0.37, 0.37, 0.12 | 0.31 | **met** |
| Chance's share | 0.15–0.45 | 0.54, 0.44, 0.51, 0.51, 0.68 | 0.54 | **high**, and worse |
| Intergenerational elasticity | 0.20–0.50 | 0.39, 0.58, 0.46, 0.57, 0.22 | 0.44 | **met** — was 0.63 |
| Sibling correlation | 0.25–0.65 | 0.17, 0.37, 0.07, 0.23, 0.10 | 0.19 | **low** |
| Mobility | 0.40–0.90 | 0.74, 0.70, 0.70, 0.69, 0.74 | 0.71 | met |
| Upbringing gap | 0.30–1.20 | 0.95, 1.17, 1.07, 1.01, 0.34 | 0.91 | met |

Elasticity was the clearest remaining miss and it is now inside the band — a real gain, and
not one that was aimed at: nothing in either fix touches inheritance, and both simply stopped
the observer's budget from adding noise and bias to attainment. Chance's share moved the
other way, from marginally high to clearly high, and sibling correlation is low, which is the
same defect seen twice: siblings share a household and a neighbourhood, so anything that
makes outcomes more idiosyncratic pulls their correlation down while pushing chance up. The
patronage coin flip below remains the prime suspect and remains unfixed.

**Four real defects behind what remains, each measured and none yet fixed.** An attempt at
all four together is recorded in this section rather than in the code, because every
configuration tried either traded one target for another or halved the population, and a
half-tuned core mechanism is worse than a documented one.

*Patronage is the largest single fact about a life.* A mentor multiplies the rate work pays
by 2.1, for life, on a coin flip that about half the population wins. Regressing attainment
on it: **R² = 0.61**, more than every other input combined. The chance of it scales with
local bonding capital so it looks like circumstance, but the draw is a coin flip, so its
variance lands in *luck*. Cutting it to 1.3 drops its R² to 0.02 and lifts the heritable
share from 0.13 to 0.26.

*Pay and availability disagree about the same fact.* The channel deciding whether work is
worth doing floors at 0.35 — "subsistence work exists nearly everywhere" — while the channel
deciding what it pays uses the raw figure and runs to nothing. Unfloored, the best place to
work pays five times the worst, for life.

*Schooling is read from the wrong place.* The earn path multiplies by the education
available where somebody lives *now*. Schooling is a childhood investment that pays out for
the rest of a life; read from the present, childhood circumstance can only reach an outcome
through the shared term of a personality, which is why it explains seven per cent of
attainment while temperament explains seventy.

*An emptying quarter can never recover.* Appeal is what a place offers less how packed it
is, so a quarter whose character freezes as its last residents leave — poor, because poor is
why they left — is unattractive forever. Nothing in the model makes anywhere attractive for
being *cheap*. Worlds end with two of five quarters occupied and the others standing empty
with room to spare: forty-four households in a place holding forty-seven, beside three
holding nobody between them.

**The economy, in the event.** Design principle one says a neighbourhood is poor "because
of what happened to its economy", and until now that was simply false: a place's prosperity
was read off the standing of its residents and nothing else, a loop with no outside. It
sustained whatever level it reached and had nothing to say about why that level.

`economy` is the outside. Output is `land^0.35 · labour^0.65` — neither factor substitutes
for the other, and returns to labour on fixed land diminish, which is the Malthusian core
and was entirely absent. Subsistence comes out before there is a surplus, so a poor place
with many mouths has *nothing* spare rather than a little less. Trade carries a third of a
neighbour's surplus, weighted at both ends, so a road is worth having and cannot abolish
geography.

Wiring it into *opportunity* was tried at five strengths and is not shipped. At the strong
end the economy dominated, and because per-head surplus equalises across places — people
move to where it is, and the check would have them bear more children there, both of which
level it — opportunity stopped varying between neighbourhoods at all: the poorest quarter in
a world came out with **more** work than the richest, being thinly settled on decent land,
and a test encoding §14's second channel failed on it. At the weak end the whole level fell
and populations with it, to 123. Real economies do not equalise like that, because of
capital, agglomeration and institutions, and this one has none of the three.

So the economy is computed, stored on every place and shown in the neighbourhood readout —
a place now says what it *makes* as well as what its residents have — and channel two stays
what §14 says it is. That is less than was hoped for and it is what the measurements
support.

**Technique, and the Malthusian trap.** What a people know how to do is a multiplier on what
their land yields, and it is a *population* variable rather than a clock. Technique is not
written down; it lives in people who know it, each an imperfect copy of whoever taught them.
A large group has enough learners that the best copy each generation is nearly as good as
the original; a small one loses a little every generation until the technique is gone. That
is Tasmania — cut off at about four thousand people and over eight thousand years it lost
bone tools, cold-weather clothing, fishing and hafted implements, through arithmetic rather
than catastrophe. Connection multiplies the pool you can learn from, which is why isolation
is the thing that impoverishes rather than poverty itself.

The trap closes on its own, and it is the point of putting technique next to the economy
rather than in a crate of its own. Better technique raises what the land yields; the extra
food feeds more people; more people on the same land drive the surplus per head back down.
Living standards return to where they were and what grew was the *population*. The most
robust finding about the ten thousand years before 1800, here as arithmetic rather than as
a claim — and there is a test that says so.

**The demographic check: four attempts, four culls.** Phase 2 set fertility to a constant
and said the feedback "arrives with resources and an economy". The economy arrived and the
feedback did not, and the four failures are worth recording because each is the same mistake
in a different costume. Every one of them is a *cull* — a reduction in births dressed as a
redistribution — and each was caught by measuring the surviving population across eight
worlds against a baseline of 208.

| Centring | Mean surviving population |
| --- | --- |
| None: `1 − 0.72·(1 − opportunity)` | 82 |
| On a constant living standard | 88 |
| On the world's unweighted mean over places | 46 |
| On the mean weighted by where people live | 138 |

The third is the instructive one. Centred on the mean over *places*, the multiplier averages
exactly one across places — and still culls, because places are not where the arithmetic
happens. Crowded places are poor places and crowded places hold most of the people, so most
of the population sits on the below-one side and the births lost there outnumber those
gained on the thinly settled good ground. Weighting by households fixes that and still lands
at 138, which means there is a fourth interaction — most likely with sorting, which moves
people towards the places the check is already favouring.

`economy::births_relative` is written, tested and deliberately not called. A demographic
response is the right mechanism; it needs a pass of its own with the sorting loop in view,
not a coefficient bolted to the end of an economy one.

**And the tension that makes this hard.** The last two are the same knob. Fixing emptiness —
by letting a household prefer somewhere it can afford — keeps all five quarters occupied and
raises the shared-environment share from 0.19 to 0.53, because people finally have different
childhoods to be shaped by. It also sorts households by means almost exactly, which pushes
the intergenerational elasticity from 0.37 to 0.75 and, at the strengths that fix the
environment share, collapses the population in three worlds out of eight. Differentiated
neighbourhoods and weak inheritance pull against each other, and closing the gap means
finding what decouples them — most likely something that mixes people across places for
reasons unrelated to means. That is the work, and it is not a coefficient.

These are *reported* rather than asserted. A unit test that fails on a known, documented
gap makes the suite permanently red and useless as a regression signal; the `--balance`
output names every target and says which are missed, and the tests assert the structural
claims — that the shares sum to one, that neither cause decides everything, that the sample
is large enough to measure at all.

One measurement error worth recording, because it nearly became a wrong conclusion. The
elasticity fixture ran sixty people for ninety years and produced sixty-six lives. An
intergenerational elasticity is a regression over parent–child pairs, and at that size the
estimate is noise: the same seed read 0.89 at sixty-six lives, 0.66 at two hundred and five,
and 0.53 at three hundred and sixty-four. The first of those looked like a caste system and
was a sample size. The fixture now runs to two hundred lives and a separate test fails
loudly if it ever drops below a hundred and fifty.

**Biological**
- Heritability recovery: after 10 generations, sibling ≈ 0.5·h², parent–child ≈
  0.5·h², unrelated ≈ 0. If configured `h2` doesn't come back out, the architecture is
  wrong however plausible the individuals look.
- Regression to the mean: children of +2σ parents average ≈ +2σ·h². A sim that breeds
  ever-more-extreme dynasties has a bug.
- No behavioral population structure: with environment held constant, mean behavioral
  traits don't differ across human founder populations beyond sampling noise. The §10.5
  guardrail as a test, so it can't rot.
- Ecological plausibility: species-area relationship, trophic pyramid ratios,
  extinction rates power-law distributed, no perpetual predator–prey blowup.

**Social**
- The §15 balance table, in CI with tolerance bands.
- Channel attribution: the four §14.2 channels sum to the observed score shift, so the
  dossier's explanations stay honest.
- Demography: stable age pyramid, no orphaned households, population neither exploding
  nor collapsing over 500 years.

## 22. Performance budget

Rough estimates to be replaced by benchmarks — stated so they can be falsified.

| Workload | Estimate |
| --- | --- |
| Grid state (40,962 cells × ~24 f32) | ~4 MB per snapshot |
| Climate solve (one equilibrium) | ~1–10 ms |
| One 1 kyr evolutionary step (10³ demes × 256 loci) | ~1 ms |
| 1 Myr at adaptive 1–10 kyr steps | ~1–10 s |
| 100 Myr | minutes, with adaptive stepping doing the work |
| Full-LOD agents at 15-min ticks | 10³–10⁴ agents in real time, event-driven |
| Dossier assembly, including backfill | < 1 ms (target) |

Deep-time storage: keyframe the grid every ~1 Myr and reconstruct intermediate states
by deterministic replay — 100 Myr of keyframes is ~400 MB uncompressed, and delta
encoding should cut that hard. Principle 3 again.

The honest risk: Full-LOD agent throughput and the scale-crossing projections are the
two places these numbers could be wrong by an order of magnitude. Both are exercised
by the M1 vertical slice for exactly that reason.

### 22.1 Measured, and what was actually slow

A profile rather than an estimate. Eighty founders, sixty years, two hundred and twenty-three
lives — 13.8 s before, 10.5 s after, producing the identical world down to the event count.
Where it went, by instruction count:

| Cost | Share | What it was |
|---|---|---|
| `expf` | 10.5% | discounting delayed payoffs, and the softmax |
| `score_all` | 12% | pricing seven options per decision |
| scheduler | 11% | a heap pop and push per deed |
| `malloc`/`free` | 4.8% | **one heap allocation per event recorded** |
| chronicle + map lookups | 4% | filing every act under whoever it concerned |

Four things were wrong rather than merely expensive, and all four are fixed with no change
to any result:

1. **Every recorded event allocated.** `Happening::subjects` returned a `Vec` — twenty-six
   million allocations and frees in a sixty-year world, almost all holding a single
   identifier. Nothing concerns more than three parties, so it is a fixed array now.
2. **Four sevenths of one `exp` caller computed `exp(0)`.** Only three deeds pay off later
   than immediately, and two of those at the same remove, so seven calls per decision were
   really two.
3. **The softmax allocated too**, collecting seven weights into a `Vec` per decision, and
   spent an `exp` computing the best option's weight — which is exactly one by construction.
4. **The chronicle index was a `BTreeMap`** on the hottest path in the simulation. Nothing
   ever iterates it in order, so it is a hash map with a deterministic integer hasher —
   deliberately not `std`'s default, which is randomly seeded and would break the promise
   that a seed reproduces a world.

Two more findings are recorded because they are *not* wins. **Compaction cannot pay for
itself**: it rebuilds every surviving record and the whole index, so its cost falls on the
total ever recorded rather than on the budget — trimming to one million cost 18% of the
running time and so did trimming to eight million. It is wired as a safety valve at twenty
million records, where an ordinary run never reaches it and a run heading for gigabytes is
still bounded. And **pre-filtering the relief table gained nothing**, because the compiler
was already folding the filter over a static slice; it was reverted rather than kept as
duplication that could drift.

**A run with no reader now records nothing small.** The salience floor already existed to
say what a run does not care about; `--quiet` with no dossier and no file has said it
already, and honouring that rather than asking again takes a sixty-year world from 10.5 s to
8.8 s. Anything that reads the chronicle — a dossier, a JSON or HTML export, or simply
printing events — keeps every record it always did.

**The test suite was six times slower than it needed to be**, and not for any subtle reason:
`cargo test` builds unoptimised, and this suite founds planets, solves climates and lives out
centuries, so it is bound by how fast the simulation runs and not by how fast it compiles.
The same test takes 3.34 s at `opt-level = 0` and 0.53 s at 2 — inside a quarter of release,
for about thirty seconds of extra compilation across the whole workspace. The full suite runs
in five and a half minutes.

## 23. Open questions

1. **Planet fidelity vs. speed.** Grid level 6 (40k cells, ~112 km) or level 7 (164k,
   ~56 km)? Level 7 is 4× the cost for visibly better coastlines and mountains. Start
   at 6, make it a parameter.
2. **Does human history run to the present, or stop early?** Agriculture, cities,
   industry, and technology each multiply the design surface. M4 phase 14 assumes a
   simple economy; a full tech tree is a project of its own.
3. **How mobile should society be?** §15 proposes IGE 0.30–0.40. Worth choosing
   deliberately, since it decides whether this is a story about inheritance or
   circumstance.
4. **Is the observer ever allowed to intervene?** Read-only is planned. Intervention
   is a much larger surface (causality, undo, counterfactual branches) but the
   read-only API leaves the door open.
5. **One planet or many?** `cosmos` is sketched but thin. Other worlds start
   Statistical and cost almost nothing — but "a second inhabited planet" means a second
   biosphere's worth of tuning.
6. **How much culture?** *Partly answered — see §24.* Transmission, drift and descent
   over ideas are built, and peoples and countries emerge from them with no list of
   either written down anywhere. Distinct **languages** remain out of scope: they need
   phonology and sound change to be worth having, and a name that describes a practice
   carries more real information than a fake word would. Religion and kinship norms are
   still open.

## 24. Peoples and countries

For most of this project's life there was an enum called `Country` with eight variants —
`Usa`, `Gbr`, `Deu`, `Can`, `Fra`, `Chn`, `Jpn`, `Vnm` — carried on every person and
inherited from their mother. On a planet drawn from a random seed, orbiting a star of nine
tenths a solar mass, a woman living on savanna at thirty degrees north would introduce
herself as being from the United States. It was the plainest violation of design principle
one in the codebase: nothing is placed by fiat, and there they were, eight of them, placed
by fiat.

It is gone. Nothing now writes down a people or a country. What is written down is the
mechanism, and the peoples are what the mechanism produces.

### 24.1 Culture is §14's norms with a memory

§14 already gave every place a vector of `norms` — how prevalent each way of spending a day
is, read off what its residents actually did — and `Deed::choose` already weighted every
choice by how far it departs from them. So behaviour followed norms and norms followed
behaviour: the loop was already there. What was missing is that norms were **rebuilt from
scratch every reckoning**, which breaks it. A place could not carry a way of doing things
through a generation, so nothing accumulated, and nowhere was anywhere in particular for
longer than a census.

`culture` closes the loop by adding the three things that make genetics genetics:

- **Transmission** — a place's ways move toward its neighbours', at a rate set by its
  *exposure*: how much of the company its people keep is from somewhere else. A hamlet at
  the gates of a city and the city have the same roads, but almost everybody the hamlet
  meets is a stranger and almost nobody the city meets is.
- **Drift** — ways wander at random, faster in small populations and *only where a practice
  is contested*. This is `p(1-p)/N`, the same Wright–Fisher variance the genetics runs on,
  because it is the same phenomenon: a way ninety-nine in a hundred follow is what
  ninety-nine children in a hundred see, and it does not move.
- **Descent** — a place far enough from the rest of its people, for whom it is a minority,
  *is* a different people, with a name, a date and a parent. The allopatric rule `evolution`
  uses on species, applied to ideas.

A **country** is derived and never stored: a maximal set of inhabited places that share a
people and can walk to one another (`A_FORTNIGHT_WALK`, 600 km, transitively chained). Open
a sea between two halves of one and you get two countries of one people; leave them
connected and it is one country however long the ribbon.

Names are built from what a people actually does — the way furthest from the unremarkable
middle, with `Un-` for a notable *lack* — and a country takes the name of its largest place,
which is already derived from the terrain it stands on. So the chain runs ground → place
name → country name with nothing authored at any link.

### 24.2 Four bugs worth keeping a record of

Every one of these produced plausible output and none showed up as a failing assertion until
the mechanism was asked to state its own claim.

| Bug | Symptom | Cause |
|---|---|---|
| Practice pull at 0.20/yr | 600 years of total isolation moved a hamlet 0.03 | A three-year half-life is not a culture, it is last decade's behaviour. Drift can never accumulate against it. Now `ADOPTION = 0.02`, a 35-year half-life |
| Frozen culture snapshot | One village produced 15 peoples in 2000 years | A people measured against the record of the day it was named keeps crossing the threshold. A culture is what its members do *now* |
| Merging on resemblance | 8 peoples from one sealed hamlet | A place nobody can reach cannot rejoin anybody, however much it comes to resemble them. Resemblance across a gap nobody crosses is convergence, not kinship |
| Distance is mutual | A city of 900 announced it had broken away from a village of 30 | The smaller part is the part that leaves. A people is where most of it lives |

A fifth was structural rather than a bug: a place was measured against a mean it was itself
inside, so two equal halves of one culture each sat at *half* their true separation and
neither could ever leave, however differently they lived. Divergence is now measured against
**the rest of** a people, which also makes the threshold mean the same thing whether a
culture has two places or twenty.

### 24.3 Calibration

One place held in total isolation beside a mainland of 900, over 3000 years, twelve seeds
each:

| Souls | Diverged | Seeds that did | Peoples ever |
|---|---|---|---|
| 20 | never — below the floor | 0/12 | 1.0 |
| 150 | 324 yr | 12/12 | 2.0 |
| 200 | 1296 yr | 12/12 | 2.0 |
| 300 | 910 yr | 1/12 | 1.1 |
| 400+ | never | 0/12 | 1.0 |

One divergence that sticks wherever drift can do it at all, and no churn at any size. Drift
alone runs out of road at about 300 souls, and it should: neutral drift in a vacuum is slow,
and the isolated populations that really diverged also differed in how they lived.

Two floors bracket the useful range, and they are set from different arguments:

- **`ENOUGH_TO_BE_A_PEOPLE` = 150** (Dunbar). Below it a place is a band. Its ways still
  rattle across the whole space in a decade — sampling error at twenty carriers is enormous
  and the model keeps that — but a band that drifts is a band, not a nation. Without this
  floor a world of a hundred souls in five neighbourhoods named **three peoples inside three
  years**, one per quarter.
- Above ~300 souls, **circumstance** is the only road left. An upland of 400 who rove and
  work and barely sleep become a distinct people from the lowland below them in six
  centuries, on thin mountain-road contact, purely because they are not spending a day the
  same way. In a real world that is the path that will produce nearly all the peoples, since
  terrain, food and work already differ from place to place.

`DRIFT` = 0.045/yr for a contested practice in a village of 100 is at the top of what is
defensible, and it is load-bearing — it is exactly the rate at which small populations can
escape the pull of their own practice, so lowering it much switches drift-driven divergence
off entirely. It also means the `norms` of a twenty-soul quarter now swing by ±0.19 around
what its people actually do, and those norms feed conformity in `Deed::choose`. Since that
is a real behavioural change to every small place in every world, it was measured rather
than assumed — six seeds, eighty founders, a hundred and twenty years, on both sides of the
change:

| | mean alive | mean ever lived |
|---|---|---|
| Norms rebuilt each census | 191.5 | 366.2 |
| Norms carried by culture | 192.2 | 368.5 |

No effect. The spread narrowed (sd 76 → 46 on the living) but at six seeds that is an F of
2.7 against a critical 7.2, so it is not a finding — only a reason not to worry. Note the
comparison is between distributions and not paired: once norms carry, the trajectories
diverge from the first census, so the same seed is not the same world on both sides.

### 24.4 What is deliberately not here

No language, religion, kinship rule, law, or state — no government, no taxation, no army, no
border anybody could be stopped at. A country here is an extent of shared practice, which is
the older and broader meaning of the word and the one that does not require inventing an
institution. **Conquest in particular is absent**: countries merge by converging, never by
one taking another. That is a real gap rather than a modelling choice made confidently, and
it is the obvious next thing to argue about.

## 25. Society: what people are to each other

Before this, a person could be paired with somebody, born to somebody, and live in a
household with somebody, and that was the whole of it. Everything else was aggregate: you
did not interact with *people*, you interacted with a statistic of your neighbours.

Two things made that plain, and both were in the code for a long time before anybody looked
at them squarely.

- `Deed::Socialize` relieved the social need and **named nobody**. People in this world
  socialised alone. It cost two hours, it moved a number, and no second person was involved
  at any point.
- `seek_patron` — by §15's own measurements the single largest fact about a life here, worth
  more of the variance in attainment than genes, upbringing and luck combined — was a coin
  flip against local `bonding_capital` with **no patron in it**. There was no mentor. There
  was a multiplier.

`bonds` is the repair. A tie runs *from* somebody *to* somebody and carries four numbers:
**warmth** (do I like you), **regard** (do I rate you), **debt** (signed, in days of help
owed), and **known** (how familiar we are, which gates the rest). Directed, because
unrequited regard is the ordinary case and a model where liking is always mutual cannot
express a hanger-on, a patron, or a grudge the other party has forgotten.

### 25.1 Complexity without language

There is no language here, no lies, no promises, and no violence. What there is:

- **An evening is spent with somebody.** `choose_company` weights the people to hand by how
  well you know them, how much you like them, and how many friends you have in common. That
  last term is triadic closure, and it is what makes groups close into circles rather than
  everybody knowing everybody equally.
- **Opinions travel.** After time together, each person's regard for every third party
  drifts toward the other's, in proportion to how warmly they hold them. Gossip, with no
  words in it — and the only channel in the simulation by which a fact about one person
  reaches somebody who has never met them. It stops dead at a cold tie, which is what makes
  it reputation rather than broadcast.
- **Help is owed.** Being fed through a bad year is a debt, and a debt that goes unpaid
  sours the *creditor* — nothing tells them to resent anybody, it falls out of being out of
  pocket for long enough.
- **Patronage has a person in it.** A young adult is taken up by a specific older neighbour
  who is better off and already thinks well of them. Which means it can now only reach people
  who made the acquaintance, and the largest single lever on a life became something you can
  fail to have.

### 25.2 Politics is admission

Scarcity was already here: there is only so much good land and only so much room on it, and
`Place::admits` decided who got in by what they had. That made the model a market. It is now
a society, because the people already living somewhere count: an ally inside vouches for
you, in proportion to their own standing and how warmly they hold you (`standing_with_allies`,
lent at a discount — backing somebody is not the same as being them). A poor household with
friends in a good quarter gets in over a richer one with none.

Only ties *into the place being sought* count, which is what stops this from being a second
wealth term. Your friends elsewhere cannot speak for you here.

No violence is modelled and none is needed. The whole of it is that there is not enough good
land and some people have friends.

### 25.2.1 What it took to not wreck §15

Wiring real ties into the two largest levers on a life — patronage and admission — broke the
variance decomposition twice, in two different directions, and both breakages were
informative enough to be worth recording.

**Uncapped vouching flattened the world.** `standing_with_allies` sums over every ally, so
four allies of middling standing were worth more than a lifetime of work. Admission stopped
depending on means: every household got into every quarter, the quarters stopped differing
from each other, and §15's upbringing gap fell below the floor — where a child grew up no
longer showed up in their outcome, because everywhere had become the same place. Backing is
now capped at `VOUCHING` = 0.15, the same size as the other two thumbs on this scale
(`DISPLACEMENT_MARGIN`, `YOUNG_MOVER_SLACK`). No amount of vouching makes a pauper a
landowner.

**Backing pointed the wrong way.** Ties are overwhelmingly local — company is drawn from
neighbours — so "allies who live in the place I am considering" almost always meant "allies
where I already live". Applied to the place a household was already in, the term was a bonus
for staying put wearing the costume of a bonus for having friends, and it stopped
displacement dead: every world ended with one inhabited quarter out of five. It now counts
only towards somewhere the household does not already live, which turns it into **chain
migration** — your friends who left are what makes it possible to follow them — and is the
thing it should have been from the start.

**A flat patronage multiplier moved the variance into chance.** With a real patron but a
fixed 2.1× payoff, patronage became a large term that correlated with *whom you happened to
befriend* and with nowhere at all: the shared-environment share fell from a fifth to one part
in a hundred thousand and chance took three quarters. The fix is the thing that should have
been true from the start — a patron is a person, so what patronage is worth depends on who
your patron is (`1.0 + PATRONAGE × their standing`). Where you grew up now reaches your
outcome through the quality of the people you could get to know there, which is what
`bonding_capital` was a crude stand-in for and is now the thing itself.

Both faults share a shape: a mechanism that is *right* can still be sized wrong, and sizing
it wrong does not show up as a wrong-looking mechanism. It shows up three layers away, in a
regression over two hundred lives.

### 25.3 Famine picks, and it stopped picking at random

`want` — how far a place fell short of feeding its people — is per head, so on its own a
famine kills at random within a place. `share_the_shortfall` is what makes it not random: an
ally with more than you, in proportion to how warmly they hold you, takes a share of your
shortfall onto themselves, and goes without in your stead when their own birthday comes.

**Nothing is created there.** Every day of hunger lifted off one person is a day put onto
another, so the Malthusian brake of §21.2 is exactly as strong as it was. What changed is
*who* it takes, which stopped being a lottery and became a question of who has friends. This
is also the only place ordinary reciprocity is generated: company deliberately does **not**
put people in each other's debt, because an early version where every evening with an
unequal booked a favour had everybody resenting everybody inside a decade.

### 25.4 A circle is a reading, and the first reading was wrong

A faction is never stored. It is walked out of the tie graph on demand, the same rule
`culture` applies to countries and §14 applies to a place's character.

The first implementation made a circle a **connected component** of mutual warmth, on the
argument that a chain of friendships is one faction even where the ends have never met.
Measuring it settled the question. At a mean of three or four allies apiece the ally graph
sits far above the percolation threshold, so the flood fill returned one blob holding two
thirds of the town — every time, in every world, at every threshold that left anybody with
allies at all. A faction with most of the population in it is not a faction.

So a circle is now a set in which *every* pair stands together, which cannot percolate, and
which is what the description had claimed all along. In a village of fifty-nine that gives
about twenty overlapping circles of three to six people, which is a social structure.

The finding underneath is worth keeping in view: **mutual affection alone does not divide a
society into camps**, because liking is not transitive but reachability is. Real factions
form around something to be against, and there is nothing here for anybody to be against.
Structural balance — a group that is warm within *and cold toward another group* — is what
that would take, and it is the obvious next thing to argue about.

### 25.5 Ties have to survive being unwatched

Coarse-tier people do not act, so on the naive wiring they would form no ties at all, and
looking away from a town would quietly dissolve everybody's friendships while looking back
rebuilt them from nothing. That is precisely the bug class that once had the observer setting
the death rate (§21.1), and it was designed against from the outset rather than found later.

The answer is not two social models. **Company is settled once a year, for everybody, through
one code path.** Evenings are *counted* as they are chosen — by four thousand separate
decisions for a watched person, by one estimate for an unwatched one — and at the reckoning
each person makes `COMPANY_A_YEAR` draws of company, each carrying its share of the year.

What that preserves is the part that has to come from the person: an extravert chooses
`Deed::Socialize` more often, so an extravert ends the year with more friends, and that is an
outcome of their temperament rather than a rule about temperaments. The coarse tier keeps it
by asking `Deed::Socialize.appeal` — the fine tier's own expression — instead of writing a
second one, because a second expression for the same question is exactly how two tiers drift
apart.

The tiers now differ only in how the count was arrived at, which is the smallest difference
they can differ by and still be two tiers:

| | ties each | allies each | circles | largest |
|---|---|---|---|---|
| Finely simulated | 21.4 | 3.6 | 21 | 6 |
| Coarsely simulated | 22.0 | 3.3 | 20 | 5 |

Sixty founders, twenty-five years, one seed.

### 25.6 What this cost, and what it cost to make it not cost that

The first wiring settled company **per evening**: every `Deed::Socialize` chose a partner, met
them, and ran gossip in both directions — some hundred map edits. At six hundred evenings a
year each for four hundred people over a century, that is billions of operations, and it made
the observer's own regression fixture take minutes where it had taken seconds. All to model
the difference between seeing a friend on Tuesday and on Wednesday.

Batching it into sixteen draws a year cost nothing anybody can measure and cut it by a factor
of forty. The two tiers agree *better* after the change than before it, which is what you
would expect once they stopped being two mechanisms.

What is left costs **12%**. The four longest-running world tests — four hundred founders
over a century and a couple of sixty-founder worlds over the same — take 218 seconds without
any of this and 245 seconds with it, on the same machine, same seeds. That is the price of
every person in every world having friends, creditors, a reputation and a faction.

Gossip is the other expensive part, because it touches everything the speaker knows rather
than one other person. The first version had it carry *everything*, including what the
speaker had merely heard, and in a village of fifty-nine that made every resident hold a tie
to every other within a few years — a cost that grows with the square of the population
rather than with Dunbar. It now carries only what the speaker knows first hand
(`known > HEARD_OF`), which bounds it by the sympathy group whatever the town's size, and
hearing of somebody can never make them familiar. You can learn who a person is by
reputation; you cannot become close to them that way.

### 25.7 The tie graph cannot yet replace `bonding_capital`

§14 computes a place's bonding capital from its churn and its residents' means — a *model*
of how densely the people there know each other. Now that they actually do know each other,
the obvious next move is to delete the formula and measure the thing directly. That would
remove one of the last authored expressions in the neighbourhood vector, so it was worth
checking rather than assuming.

It does not work, and the reason is worth more than the change would have been. One world,
a hundred and twenty founders, sixty years, every inhabited quarter:

| formula | ally count | tie density | churn | adults |
|---|---|---|---|---|
| 0.831 | 4.4 | 0.335 | 0.13 | 14 |
| 0.723 | 17.1 | 0.219 | 0.07 | 79 |
| 0.481 | 2.2 | 0.550 | 0.53 | 5 |
| 0.850 | 1.3 | 0.127 | 0.10 | 11 |

Against the formula, the raw ally count correlates at **r = 0.04** and the tie density at
**r = −0.85**. Neither measures cohesion. The count is a measure of how many people live
there; the density is the same measure upside down, because in a quarter of five adults you
can be allied with half the town by arithmetic and in a quarter of eighty you cannot.

Underneath that is the same result the technique model ran into: **Dunbar is not binding at
these population scales.** Where a quarter holds five to eighty adults, everybody can know
everybody, so who you know is not yet a choice and the graph has no room to say anything the
headcount does not already say. Turnover ought to show up — ties decay at 22% a year without
contact — but a household that moves starts saturating its new neighbourhood within two
years, so even the quarter with half its households arriving each year is the densest one on
the list.

The formula stays, and it stays for a stated reason rather than because nobody looked: it
carries information (that turnover erodes community, that hardship builds it) which the tie
graph genuinely does not contain, because nothing in the graph yet responds to either. Making
it respond is a real piece of work and not a tidy-up — hardship would have to raise how much
company people keep, and the number of *distinct* people a year of company reaches would have
to grow with it, since at present `COMPANY_A_YEAR` fixes that at sixteen and more evenings buy
depth rather than breadth. The measurement that says so is kept as
`measure_whether_ties_could_replace_bonding_capital`, so the answer can be rechecked rather
than remembered.
