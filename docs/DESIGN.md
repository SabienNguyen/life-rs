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

### 15.1 The sheet was validated on one world, and it is a statistic

`observer::balance::targets` states seven bands the design commits to, with a comment saying a
measurement outside one *"is a finding, not necessarily a fault — but it should be looked at
rather than shrugged off"*. Two of the seven were looked at by a test. Elasticity, sibling
correlation, mobility and the upbringing gap were computed, printed by `--balance`, and
asserted nowhere, so five bands the design meets could have quietly stopped being met between
one change and the next. §30 came close: the shared-environment share ran 0.33 before that work
and 0.21 after — still inside `ENVIRONMENT`, a third of the way to the floor, and nothing would
have said so.

Measured properly, over three seeds at 160 founders and 120 years:

| seed | genes | upbringing | luck | elasticity | siblings | mobility | gap |
|---|---|---|---|---|---|---|---|
| 0x11 | 0.50 | 0.42 | 0.40 | 0.69 | 0.47 | 0.69 | 1.13 |
| 0x21 | 0.45 | 0.23 | 0.51 | 0.56 | 0.36 | 0.70 | 0.72 |
| 0x221 | 0.33 | 0.20 | 0.59 | 0.41 | 0.19 | 0.61 | 0.61 |

**Four of the seven leave their band on some seed, and each of those four is comfortably inside
it on another.** A few hundred lives is a small sample and its statistics wander, so a single
seed reading "within target" — which is what the `--balance` sheet has always shown, and what
this document has quoted from — is close to no evidence. Averaged over the three, exactly two
are out: luck and intergenerational elasticity, which is the pair the existing test already
names from its own four-seed measurement. Those two are quarantined with bounds rather than
having their bands widened.

It depends on the size of the world as well. The same seeds at 110 founders and 110 years put
the upbringing share at 0.17, *below* its floor, and bring elasticity back inside its band —
because a world that small and that young has barely had time for its neighbourhoods to become
different places, so there is less shared environment to find and less of it to hand down.
Which means §15's validation is contingent on the fixture, and was not said to be.

Both sheets are in `balance_tests.rs` so the dependence is on the record rather than waiting to
be rediscovered.

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

**Theory of mind — beliefs about people, not just feelings about them.** *(Built — see
§17.2.2.)* The design had `Tie { affinity, trust }` and `impressions`: how someone *feels*
about another person.
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

**Norms as learned, not ambient.** *(Built — see §17.2.1.)* §14.2's fourth channel read
`norms` off the place, as though everyone were equally steeped in them. Humalike's framing — agents pick up hidden
rules and tone *from a group* — is the better model: each person carries their own
estimate of local norms, learned by observation, weighted by the developmental windows of
§14.3. That single change earns three things the ambient version cannot: migrants who
carry the old country's norms and only partly assimilate, adolescence as the period when
norm learning runs fastest, and cultural change that is *transmitted* rather than
imposed by editing a field.

Both belong in Phase 3 (environment) and Phase 4 (chronicle and memory) respectively, not
in Phase 1 — they need relationships and places to exist first.

### 17.2.2 The one number on a tie that can be wrong

Built, and much smaller than the sketch above. `Tie` gains `welcome`: **what I think you make
of me**, from certain you loathe me to certain you are glad of me.

Everything else on a tie is a fact about its holder — how I feel, what I have been told, what
I am owed — and none of it can be mistaken. This is a belief about somebody else, and it is
the only thing in the whole social model that can diverge from the truth.

The sketch wanted traits, intent, confidence and a timestamp per tie. That is twenty bytes
apiece and it changes nothing anybody does. One number costs four and changes **who knocks on
whose door**, which is the test of whether a mechanism is load-bearing or decoration.

Two rates do the work and the gap between them *is* the misunderstanding:

- `WARMING` (0.14) — how fast your own feelings move. They are yours; you have them at once.
- `READING` (0.06) — how fast your read of somebody else's does. It has to be inferred from
  how they are with you, and people are not good at it.

Set the two equal and belief tracks truth exactly and nothing is ever mistaken about anything.

Staleness is free rather than a rule: `welcome` is revised only when two people actually meet,
so it goes wrong exactly as fast as they drift apart. Somebody who soured on you during a bad
year is somebody you do not know has soured.

And it feeds back. `choose_company` now weighs how you feel about somebody *and* whether you
think they will be glad to see you — so somebody who has decided they are unwelcome stops
going, stops finding out, and keeps the mistake. That self-sustaining loop is what §17.2 was
after and is exactly what a number that always agreed with reality could not give.

One implementation note worth keeping. Both directions of a meeting are stepped together
rather than one after the other, because what each person comes to believe depends on how the
other actually feels *at that meeting* — and the two sides are not symmetric, since a debt
sours one direction of a tie without touching the other. Written the obvious way, with the
other's warmth read once before a batch of meetings, `welcome` chases a stranger's zero
through the whole batch and never moves at all.

### 17.2.1 What is normal, learned by watching

Built. `Person` carries its own `norms` — its estimate of §14.2's fourth channel rather than
the channel itself — and `score_all` reads that instead of the place's. Everybody learns, at
every age, by moving towards what they see: fast in the first years and in adolescence, and
at a quarter of that rate for the rest of a life.

The last clause is the one that matters and it is deliberately *not* `developmental_weight`,
which falls to nothing at twenty. That function exists so where you were raised cannot be
rewritten by where you live, which is right for temperament and wrong for manners. A rate
that stopped at twenty would make every migrant a permanent foreigner; a flat one would make
nobody a migrant at all. A quarter is what makes partial assimilation the ordinary case.

Three things fall out that the ambient version could not have, and
`what_is_normal_is_learned_and_not_breathed_in` asserts all three, because any one of them
alone could be had by accident:

- **A childhood among these people leaves their habits.** Eighteen years of watching puts
  somebody above 0.8 against a local 0.9.
- **Adolescence is when it happens fastest** — the same three years at fifteen move somebody
  more than 0.15 further than at forty-five.
- **A migrant brings the old country and only partly assimilates.** Twenty years somewhere
  new moves them, and does not finish the job.

Those three are properties of `learn_norms` measured on its own, which is not the same as the
mechanism mattering in a world — the belief on a tie passed its unit tests too and turned out to
be inert (§17.2.3). So the same question was asked of a running world, three seeds at ninety
years: **how far is anybody's picture of local practice from local practice?**

    everybody          0.139
    has moved house    0.212
    has not            0.066

Not inert, and not merely non-zero: the split is threefold and it is the claim itself rather
than a side effect of one. Somebody who has moved carries where they came from, and somebody
who has stayed put has very nearly the ambient number the old model gave everybody. `vitals`
reports it, so it cannot quietly stop being true.

The third property is the point. Two people standing in the same room, one raised among these
neighbours and one who arrived from somewhere that did the opposite, no longer face the same
decision — and cultural change is now something *transmitted between people* rather than
imposed by editing a field. §24's peoples and countries were already built on `norms`
drifting; this is what makes the drift travel through anybody.

### 17.2.3 And measured, nobody is wrong

§17.2.2 claimed the belief on a tie would produce misunderstanding, avoidance that feeds
itself, and reconciliation when somebody is finally corrected. It was written the day the
mechanism was built and before anything looked at it.

Displayed, over 183 living people at ninety years: **mean misreading 0.002, worst case 0.04,
and not one person more than 0.1 out.** The number that can be wrong is right.

The cause is structural and was there to be seen in `meet_repeatedly` the whole time. Warmth is
driven by `suits`, which is symmetric — the same target for both directions of a meeting — so
what A comes to feel about B and what B comes to feel about A move together. A's belief about
B's feelings is therefore chasing a quantity that is nearly identical to A's *own* feelings, and
`READING` being slower than `WARMING` only makes it lag: a smaller number in the same direction
is not a mistake. The only genuine asymmetry in a tie is debt, and grievance is rare enough that
it does not shift the average.

So the mechanism is correct, cheap, and produces no misunderstanding, because there is almost
nothing in this world for it to misunderstand.

Sharpened by ablation afterwards (§31.2), because the first way of asking was wrong. Setting
`READING` to zero changes a great deal — but that freezes the belief at nothing rather than
making it accurate, and since the belief tracks warmth closely, it removes four tenths of the
warmth signal from `choose_company`. The experiment that asks the actual question is
`READING = 1.0`: belief exactly equal to truth, no lag, no possibility of error. That is
indistinguishable from the baseline. **The term is load-bearing as a carrier of warmth; the
divergence it exists for is inert.**

What it would need is an asymmetry worth being wrong about. Warmth that depends on what each
person *brings* to a meeting rather than on how well the pair suits each other; or a reading
that is noisy per person, so that somebody with low social intelligence systematically
misjudges. Both are real changes to how two people come to like each other, and §31.1's rules
apply to them — particularly the third, since a symmetric warmth is exactly the kind of
resting-state equivalence that hides everything about how a system moves.

Recorded rather than fixed, and the display kept, because the display is what found it. Ten
minutes after being shown, against a write-up that had gone unchallenged for six hours. That is
§30.6's argument arriving for the fourth time in one day.

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

   *Partly answered, and not by building a tree.* §29 makes the technique ceiling a
   **frontier** that a particular person moves by working something out, so a world can leave
   the age it started in without anybody writing down what it discovers. There are still no
   named technologies and no prerequisites, and there are not going to be: what a world has is
   a limit on how far each trade can be taken, and people who occasionally push it. Whether
   that is enough to produce anything recognisable as an industrial revolution is now a
   measurement rather than a design question — see §29.8.
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
| …and again once places could own tools (§27) | 244.5 | 429.0 |

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

## 26. Roles and titles: what somebody *is*

§25 gave the world ties, debts, reputation and factions, and it was still not a society. It
had inequality in it, and inequality is not structure. What was missing is that a society has
**positions** — the one everybody consults, the one everybody owes, the one nobody will stand
with — and those positions **outlive whoever is holding them**.

### 26.1 A role is a reading, and that is what makes it an institution

Nothing here is stored, assigned, conferred or inherited. A role is walked out of what can
already be measured about a life, exactly as `Archetype` is walked out of a place's vector, a
`Country` out of who can reach whom, and a `Circle` out of who stands with whom.

Nobody is *made* an elder. Somebody is *read as* an elder because they are old, well off,
widely stood with, and owed by half the town — and on the day that stops being true of them
it is true of somebody else. That is the whole succession mechanism: there isn't one. The
reading is taken again and it lands where it lands.

This is the cheapest honest institution. No office, no title deed, no rule for who inherits
what, and nothing that can fall out of step with reality, because it *is* reality, re-read.
It is also the only kind this project's first principle permits: an office written down would
be an office placed by fiat.

### 26.2 Measured against the neighbours, never against a number

Every quantity is a **rank within the people to hand**. A rich man in a poor village is the
patron; the same man among richer neighbours is nobody in particular, and no threshold
written anywhere could say so. Rank also makes the reading scale-free — a hamlet of nine and
a town of two hundred are both readable — and immune to the drift in absolute standing that a
long run produces.

Eight quantities: seniority, means, credit (net days of help owed), how many stand with them,
what others hold about them, and the share of their life given to work, to company, and to
wandering. That last group is `Person::doings`, a running tally of what four thousand
decisions a year actually came to — so *what somebody does* is read off what they did, and
temperament is what makes two people with the same options spend their lives differently.

### 26.3 A position is a relation, not a location

Nearest-prototype alone gave a **patron with nobody owing her anything**. She had more of
everything than anybody, and that put her in the corner of the measured space where patrons
live even though the relation that makes somebody a patron did not exist.

So the sign of the thing is checked first — you cannot be a patron unless somebody owes you,
a client unless you owe somebody, an outcast unless people think worse of you than of the
rest — and only then is proximity used to choose among the positions somebody could actually
occupy. `Householder` qualifies always, which is what makes it a fallback rather than a
prototype that has to win on distance. It is also, correctly, the commonest reading by a
wide margin.

A world with no famine has no creditors and reads nobody as a patron. That is the model
working, not a gap in it.

### 26.4 Every people has its own word for it

The *meaning* comes from `bonds::roles` and the *sound* from `culture::naming`, which is the
same voice that names their children. So the elders of two peoples who diverged in opposite
directions are called two different things, and the elders of a people and its daughter are
called nearly the same thing, with nobody writing a word list. Derived, not stored, and
deterministic: `Nistelder`, `Vaeskkeeper`, `Nildhand`, `Nilshunned`.

It is not a language and it is not trying to be — §24.4 puts those out of scope. It is a
naming habit that differs between peoples and descends with them, which is the cheapest thing
that carries real information.

### 26.5 What a position is *for*: the door

A role that only described would be decoration. The architecture is that **roles describe and
the quantities underneath them act** — so `Outcast` is not a state anybody is put into, it is
what being widely thought poorly of comes to, and the thing that acts is the regard itself.

Being thought worse of than the rest costs you at the door. `World::backing` is now signed:
allies inside a place vouch for a household, and a bad reputation objects, within the same
`VOUCHING` budget. That is the only sanction in this world — no violence, no law, no court,
just a door that does not open — and it is what finally makes `regard`, the one quantity that
travels between people who have never met, decide something.

### 26.6 Half a mechanism reads exactly like a whole one

`Bonds::repaid` existed, was tested, and was **called by nothing**. Reciprocity was a one-way
ratchet: you could be carried through a famine and you could never settle up, so every
debtor's regard fell for the rest of their life and every person in every world was
eventually thought poorly of. The tests passed. The unit was correct. Nothing used it.

Fixing it took three goes, and each failure was a different way of being wrong about *scale*:

1. **Resentment was linear in days owed.** Famine relief books debts of hundreds of days, so
   any real debt drove warmth and regard to the floor and pinned them there. Being owed a
   great deal is worse than being owed a little, and then it stops getting worse — so
   grievance now saturates: `days / (days + PATIENCE)`.
2. **Repayment was capped at a quarter of each debt a year.** That bound long before
   affordability did, so people with ample means paid nine days against a debt of thirty-five
   and were resented for the twenty-six they had not touched, every year, for life. Somebody
   who can settle up settles up; the only cap left is what a year can spare.
3. **Credit for paying was of the same order as the yearly cost of not paying.** Ill regard
   accrues every year a debt stands and credit arrives once, so a rate merely equal to it left
   somebody who borrowed and repaid in full worse thought of than somebody who never needed
   help.

What survives is the trap, and it is the right one: repaying costs standing, so somebody with
little never clears what they owe, stays resented, and is who a town shuts its door on.

### 26.7 Everybody is slightly ill thought of

Measuring reputation against zero turned out to say nothing. At the Malthusian edge nearly
everybody owes somebody something they cannot repay, so nearly everybody's raw regard sits a
little below zero — which makes "thought poorly of" true of the whole population and
therefore a statement about nobody.

So reputation, like everything else in §26, is a **rank**: `World::repute_of` is where
somebody stands in the world's regard from 0 to 1, and the sanction is scaled by how far
below the middle they are. What a town can act on is not whether you are liked in the
abstract; it is whether you are worse thought of than the rest.

### 26.8 Positions survive their holders — measured

`a_position_outlives_the_person_holding_it` runs a world sixty years, records which positions
exist and who holds them, then runs it forty-five years more — long enough that two thirds of
the holders are dead. The positions are still there, held by other people, with nothing
anywhere that says they should be.

### 26.8.1 One seed measures the divergence, not the tiers

`who_your_friends_are_does_not_depend_on_who_is_watching` compares a watched world with an
unwatched one, and it failed on the seed it had always used. Three seeds settled it: ties come
out −33%, −8% and +18% coarse against fine, and allies −32%, −14% and −1%. Not
one-directional, so what is left is divergence rather than bias — two worlds from one seed
differ in their families, their famines and their migrations within a decade, and a tie count
is a sensitive enough aggregate to show it.

The test now averages three seeds, which is the same standard `WORK_SPELLS_PER_YEAR` is held
to and the reason that calibration is trustworthy. Chasing the single-seed gap first led to
recalibrating the coarse tier's evenings by a third — which brought the evening counts into
exact agreement and moved the tie counts by 1%, proving the gap had never been about evenings.

### 26.9 What is still missing

This fills the gap between "people with different numbers" and "a society with positions in
it". It does not fill the gap to a *real* society, and the honest list of what is still
absent is longer than what was added:

- ~~**Occupation.**~~ *Closed by §27.* This said the largest single thing missing was an
  `economy` carrying more than one product, so that nobody could be a smith because there was
  nothing to smith. There are now five goods and five trades, people take them up and give
  them up, and §27.9 records what is thin about it. Left here struck through rather than
  deleted, because a list of gaps that quietly loses its entries stops being evidence of
  anything.
- **Law.** The only sanction is a door that does not open. There is no rule anybody states,
  no judgement, no penalty anybody imposes on behalf of anybody else.
- **Language, religion, ritual, kinship rules.** §24.4 puts these out of scope and they stay
  there. What exists is a naming habit, which is not a language.
- **Conquest and the state.** Countries merge by converging, never by one taking another.
  There is no taxation, no army, no border anybody could be stopped at.
- **Households as political units.** *Still open — see §26.10.* A head and a succession are
  built and tested, and nothing in the simulation reads them: two attempts at giving a head
  something to decide were both measured worse and reverted. Property that outlives its members
  is untouched, and deferred on a measurement rather than for want of time.
- **Roles that are chosen.** *Still open, and attempted — see §26.11.* Nobody here decides to
  become anything. A position is read off a life, never aimed at, and a society in which people
  pursue standing deliberately would need `Deed` to include acts whose whole point is what
  others will make of them. One was built and reverted.

### 26.10 A head that is well-formed and decides nothing

`Household::head` exists and is a **reading**, the same discipline §26.1 applies to a village's
elders and for the same reason: a stored head can be dead, or absent while a grown adult stands
in the room, or two at once, and none of those can happen to a question answered afresh. It is
tested, and succession falls out of it for free — nothing schedules an inheritance and nothing
records one, the head dies, somebody asks again, and the answer is somebody else.

**And nothing in the simulation reads it.** Two things were tried and both measured worse, so
this gap is recorded as still open rather than closed, on the principle that a mechanism
nothing consults is decoration and this document has spent a lot of words saying so.

**Attempt one: a household's means.** The argument was that a door opens on the strength of who
is asking, so admission should read the head's standing rather than a mean over every working
adult that describes no individual. The head is by construction the *strongest* member, so this
is judging every household by its best: admission stops being selective, the quarters stop
differing, and §15's shared-environment share falls through its floor to **0.19**. That is the
failure `backing`'s cap comment already describes, written down before any of this. The band
test from §15.1 caught it, which is exactly what it was added for. A household's ability to keep
a roof over itself is its collective means, and the mean is right for that even though it
describes nobody.

**Attempt two: whether the household is a young one.** Better on its face — being ready to
uproot for work is a fact about whoever leads a household, and the old rule needed *every*
working adult under `RESTLESS_UNTIL`, so a household stopped being young the moment one member
aged past it. But the head is read from standing, and standing moves year to year, so which
member is the head flips between a younger and an older one. With it flips whether the
household ranks places on what they offer in work or on what they are like to live in — and a
household that changes which question it is asking changes where it wants to be. **107 of 1,018
moves went straight back where they came from**, against 6% before.

The common cause is worth naming, because it is the fourth instance of it today: **a decision
that flickers**. The revolving door of §30.4, the raw-prosperity migration cobweb of §30.5, the
trade cobweb of §30.5.1, and this. Anything read afresh each year from a quantity that moves
will oscillate unless something damps it, and the damping has to be part of the design rather
than discovered afterwards.

So what a head is *for* remains open. The reading is kept because it is correct and cheap and
the next thing that wants a household's representative will want exactly this — but the honest
status of §26.9's third bullet is that one third of it is built and unused, and the other two
thirds are untouched.

**Property that outlives its members is now built** — see §26.12 — and the argument that
deferred it turned out to be wrong in a way worth keeping.

### 26.12 An estate, and a prediction that was backwards

`Person::estate` is the one thing in this world that survives a death. `standing` is what
somebody *can do* — built by working, and it slips when they stop, so it dies with them and
every generation starts again from what its own hands are worth. An estate is a claim that
outlives the claimant: a tenth of anything a year leaves over above keeping a household going,
never decaying, and at death divided among the children.

It measures as real rather than notional — **93% of adults own something**, mean 0.42 against a
mean standing of 0.59 — and what it does is shape a childhood: what a household owns per adult
adds to the quality of upbringing the children in it absorb.

#### It buys no admission, and that took two tries

The first version put an estate into what a household could put behind a claim at a door, which
seemed obvious: means are means. It was wrong, and wrong in a way §31.1 has a rule for.

An estate **steps discontinuously when a parent dies**. A household's means jump the year of a
funeral, admission jumps with them, and somebody moves house. Churn went from 9% of moves going
straight back to **21%**, and one seed in three stopped fitting in its quarters at all. §31.1's
first rule is about decisions read from quantities that *move*; this is the same rule for
quantities that **jump**, and admission has now been the path by which five separate mechanisms
broke this world.

What wealth does to a life is not which door opens. It is how you are raised — §14 makes the
quarter almost all of what shapes a child, which was always a little too clean, since two
families on the same street do not raise children identically and what they have is a large
part of why. Moved there, the estate enters *smoothly*: an inheritance changes what a child
absorbs from that year onwards rather than moving anybody's house the same afternoon.

The world with it is better than the world without on every line that was measured:

| | no estates | estate at a door | estate in a childhood |
|---|---|---|---|
| churn | 9% | **21%** | **4%** |
| biggest quarter | 0.55 | — | **0.47** |
| empty quarters | 0.33 | — | **0.27** |
| upbringing share | 0.397 | 0.217 | 0.360 |
| luck share | 0.401 | 0.499 `<<` | 0.449 |
| elasticity | 0.655 `<<` | 0.542 `<<` | 0.601 `<<` |

#### The deferral was wrong, and the direction is the interesting part

§26.10 held this back on a measurement: intergenerational elasticity already ran 0.55 against a
ceiling of 0.50, and inherited wealth obviously pushes an out-of-band number further out.
Obviously, and wrongly. Measured on the seed the argument was made about:

| | before | with estates |
|---|---|---|
| elasticity | 0.655 `<<` | **0.542** `<<` |
| genes | 0.505 `<<` | 0.468 `<<` |
| upbringing | 0.397 | 0.217 |
| luck | 0.401 | 0.499 |

**Inherited property lowered the elasticity it was expected to raise.** Partible division —
every child takes an equal share — disperses an estate every single generation, so a large one
becomes several middling ones in thirty years and nothing compounds. Set against a genome, which
is transmitted whole and undiluted to every child, wealth of that shape is a *decorrelating*
channel. The intuition that inheritance concentrates advantage is an intuition about
primogeniture and about capital that earns, and this world has neither.

Which is the finding worth having: **partible inheritance with linear saving cannot make a
dynasty.** No estate here grows faster than it is divided. That is the same answer §31.3 gives
about why nobody is a Caesar, arrived at from the other side, and it says what the next piece
would have to be — an estate that *earns*, so that having something is a reason to get more.

The three-seed band test passes with all of this in, so nothing was bought by breaking anything.

#### The channel that is genuinely too strong, found on the way

The ablation that was meant to clear the way for this found something separate and real.
Elasticity is high because **temperament decides too much of how well anybody does**:
conscientiousness is heritable, and `diligence` scaled it at 0.5, so most of an outcome came
down the genome alone — genes at 0.51 of outcome variance against a ceiling of 0.45. Nothing
else came close. Taking away a patron's lift moved elasticity by 0.04; removing mentoring
entirely moved it by nothing.

Dropping the coefficient to 0.15 brings genes to 0.280 and elasticity to 0.276, both inside
their bands — and pushes upbringing to 0.122 and luck to 0.680, both outside. The variance does
not vanish, it moves to whatever else explains an outcome, and there is not enough else.

The estate was supposed to be what takes it up, and it cannot, for a reason that only shows up
when both are tried together: **loosening temperament shrinks the estate channel too.** Less
spread in what people earn is less spread in what they have left over, which is less spread in
what they leave. The two levers push the same way rather than opposite ways, which is why the
sheet at temperament 0.15 *with* estates is worse on every line than at 0.5 with them.

So `TEMPERAMENT_AT_WORK` exists, and is named, and is left at the 0.5 it always was. The
finding is recorded because it is the strongest lead anybody has on §15's standing excursion,
and the thing that would use it — a channel that explains outcomes without coming from either
the genome or the year's work — is not the estate.

### 26.11 An eighth deed, and why it was reverted

`Deed::Host` — keeping open house. Built in full, measured, and taken out again.

The design was right and is worth keeping on the record. It is the one act whose return is not
a need: it relieves almost nothing, costs energy and a meal, and is done entirely for what the
neighbours make of it afterwards. It pays into `regard` rather than `warmth`, because warmth is
whether I like you and stays with me while regard is what I would say about you to somebody
else — so a reputation bought at one table travels, along the gossip §25 already has, to people
who were never at it. Both detail tiers fed the same tally, so what the neighbours make of a
generous man does not depend on whether anybody was watching him.

It even had the damper designed in rather than discovered afterwards, which was the whole
lesson of the four failures above it. Standing is positional — a feast distinguishes you only
insofar as the neighbours are not holding one — so `payoff[Host]` *falls* as the local practice
of hosting rises. It is the one deed where doing what everybody does is worth less, pushing
against the conformity pull that applies to every other.

And it still failed, in a way none of that addressed. Churn went from 6% of moves going
straight back to **16%**, and the number of moves rose 64%, from 1,018 to 1,671.

Three things were ruled out by measurement rather than argument:

- **The regard rate.** Cutting `THOUGHT_WELL_OF` tenfold left churn at 13.7% and moves at
  1,659 — and cost the deed its point, since hosts were no longer better thought of than
  anybody else. There is no value that buys both.
- **Membership of `CHOSEN`.** Taking `Host` out of the set that positions are read from
  changed the outcome not at all: 266 of 1,671, byte for byte.
- Which leaves **the existence of an eighth option**. `score_all` chooses by softmax over
  every deed, so adding one changes every choice anybody makes, whether or not they ever host.
  The world is simply a different world, and 64% more moving is what that difference came to.

That last one is the real finding and it is about the model rather than about hospitality: **a
new deed is not an addition, it is a re-normalisation.** Anything added to `Deed::ALL` reprices
all seven of the others, so the cost of a new act has to be paid by everything already
calibrated against the old set — §15's shared-environment share, §21's ceiling, §30's churn.
Whatever the next deed is, it needs those re-measured as part of building it, not afterwards.

The branch is kept in `git stash` rather than deleted, since none of the above says the deed is
wrong — only that landing it is a recalibration and not an afternoon.

## 27. A supply chain, and the trades that fill it

Until now this world had **one good**. Everybody did the same undifferentiated work, produced
the same undifferentiated output, and the only thing distinguishing two workers was how much
of it they made. §26 could read a position off a life, but not a *living*: it could tell you
who was owed and who was shunned, and not what anybody did all day.

That is not an economy. An economy is people doing different things **because other people
are doing the other things**, and the whole of it turns on one fact: you cannot make tools
until somebody else is growing enough food to feed you while you make them.

### 27.1 The chain

```
  land + hands              → stock     (timber, stone, clay, ore)
  stock + hands             → tools     ── and tools multiply everything above
  land + hands  (× tools)   → food      ── what everybody must have
  food + hands              → meals     ── frees everybody else's time
  hands                     → upkeep    ── what keeps tools from wearing out
```

Four links deep, with a loop in it: tools make it easier to get the stock that tools are made
of, and easier to grow the food that feeds the people making them. One trade per good —
**farmer, hewer, smith, cook, keeper** — because a trade is exactly "the people who make
this", and everybody starts a farmer because that is what everybody was before there was
anything else to be.

The goods are authored and the jobs are not. What a thing is made of is a physical fact, like
the seven `Deed`s or the five factors of a temperament — those are the primitives the model is
built out of. What is **not** authored is who makes what, how many of each a place has,
whether it has any at all, what its people call them, or whether the chain gets past its first
link. A hungry village has no smiths because it cannot spare the hands, and nothing anywhere
says so.

### 27.2 Capital, at last

§22 said plainly that without capital nothing here could compound: a rich place was rich
because of its land and its road, never because it was rich last century. **Tools are the
correction** and they are deliberately the smallest possible one — a stock that is made by
people, wears out at a tenth a year, is held together by keepers, and multiplies what
everybody else's hands get off the land.

That is the first thing in this world that outlives the year it was made in, and it is what
makes an economy able to *build*. It shows up immediately in the population: six worlds of
eighty founders over a hundred and twenty years averaged **192 alive** before there was
anything to own and **245 after**. The Malthusian ceiling did not stop working — it moved,
which is what capital does to it.

### 27.3 It reduces to what came before

With everybody farming and nothing owned, food is exactly the Cobb–Douglas output of the
one-good model, to the last decimal. That is deliberate and it is what protects every number
§21 and §22 calibrated: a world that never specialises **is** the world that existed before
this chapter. Only the hands actually on the land count towards crowding, because that is
where the diminishing return comes from — a smith does not make the fields smaller — and
everybody eats regardless, which is what makes every trade above the land a claim on somebody
else's surplus.

### 27.4 "What happens if I do this instead" — not a price

There is no currency here and none was invented. What somebody choosing a trade looks at is
the only question they could actually answer: one more hand is put into each trade in turn,
the year is run again, and what the place ends up with is compared. The supply chain then
enforces itself with nothing written down — a smith where there is no stock adds nothing, so
smithing is worth nothing, so nobody smiths.

This replaced a table of target quantities per good, and the table was wrong in a way worth
recording: it priced meals by how far short of a target the place was, so cooking stayed worth
doing however many cooks there were. Six worlds ran to **fifty-one cooks against forty-seven
farmers**, and the population fell by a third. *A want that does not fall as it is met is not
a want.*

### 27.5 Three ways the chain refused to start

Getting a four-link chain to bootstrap from a village of farmers took three corrections, and
each was a different way of being wrong about **valuing a thing that is not finished yet**.

1. **Unworked stock was worth nothing**, so a hewer with no smith added nothing, so hewing was
   never worth taking up, so no smith ever had anything to work with. Stock had to persist and
   be worth something before anybody had made anything of it.
2. **Then it was worth exactly the finished tool less the labour still to go** — which is the
   textbook answer and makes a smith add *precisely* what a farmer adds. Nobody ever finished
   anything: the place cut timber for ever and owned nothing. A pile of material is worth
   *less* than the thing it will become, because it is not the thing yet.
3. **Then it was worth its size.** A hundred and eighty units of timber in a village of forty
   were valued at twenty years of its own harvest, and finishing a tool *revalued the heap
   downwards* by more than the tool was worth. The pile has to saturate the same way tools do:
   a hundred and eighty units are worth about what one is.

With all three fixed a village of forty settles at thirty-four farmers, four smiths, a hewer
and a keeper, holding sixty-four tools — an occupational structure nobody wrote down.

### 27.6 A cook serves a dozen

Cooking is the only trade here whose whole product is **time**. It makes no food; it hands
back the hours everybody else would have spent grinding, fetching water and tending a fire —
about a sixth of a working year, done one household at a time.

The first version had a cook serve 0.9 of a person, which meant they saved slightly less time
than they spent and no place ever had one. A cook serves *twelve*, and doing for twelve
households what each would otherwise do for itself is the oldest economy of scale there is.
It is also why cooking is the first trade a place takes up once it is **dense** rather than
merely rich: twelve mouths have to be within reach of one kitchen.

Across six worlds it now runs at fifteen to twenty-four cooks against a hundred and fifty to
two hundred farmers — cooking being the largest non-farming trade is not a surprise, it is
what the record shows for every settled agrarian society that got past subsistence.

### 27.7 What a trade is worth reaches the person

The economy has to reach somebody's own outcome or the division of labour is decoration. A
hand in a trade its place badly wants earns more than a hand in one it does not, clamped,
because that multiplies a figure §15 and §21 already calibrated and an unbounded term there
would be a second `WORK_GAIN` in disguise.

Changing trade is slow on purpose: a fifth of the whole range of worth before anybody
reconsiders, and an eight per cent chance a year of acting on it. Without inertia the whole
town moves into whatever paid best last year, makes far too much of it, and moves out again —
a four-year cycle that never settles. **People are slow, and the slowness is what lets an
occupational structure exist at all.**

### 27.8 Where the famine stays

Making food a produced good exposed something §25.3 had wrong. Famine relief between allies
was reaching across places, so a place a fifth short of feeding itself had every one of its
people at full health — their friends two valleys away had quietly absorbed the whole famine.

That double-counts. **Food moves between places by trade, which `economy` already models and
which `want` is measured after; within a place it moves by obligation.** The two do not
overlap, and relief is now neighbours only.

The same change made an older test wrong rather than failing: it measured the *mean* health of
a hungry place, and redistribution does not change how much hunger there is — it concentrates
it. A place a sixth short now has most of its people untouched and a few carrying the lot,
which averages out to a healthy town. What the model supports is the claim about the
distribution: where the land is thin there are people in visibly worse condition, and more of
them than where it is not.

### 27.9 What is still not an economy

- **No money, no prices, no credit, no ownership.** "Worth" here is a comparison somebody
  makes, not a number anybody quotes, and the tools belong to the place rather than to a
  person. Property is the obvious next argument.
- **No firms, no employment, no contracts.** Nobody works for anybody. A trade is a thing you
  do, not a thing you are hired into.
- **Five goods.** No cloth, no shelter, no drink, no ornament, no weapons — and no way for a
  place to specialise in what its *land* is good for, because stock is stock everywhere.
  Regional specialisation and long-distance trade in particular goods is the largest missing
  piece, and it is what would make `TRADE_REACH` mean something.
- **No waiters, no janitors as such.** A restaurant here is what cooking looks like when a
  place is dense enough to do it for strangers, and it reads out of density rather than out of
  a new good; the distinction between a cook and the people who serve beside them needs a
  model of the *firm*, which is the item above.
- **Tools are one thing.** A plough and a loom are the same object, so a place cannot be
  well-equipped for one trade and not another.
- **And at the sizes this project runs, it is thin — but it is no longer notional.** Counted at
  ninety years over four seeds, it used to read farmers 165–249 with hewers and smiths in the
  low single figures or *zero* in two of them. §27.4 says a trade exists when there is food
  enough to spare a hand for it, and a world sorted into one starving quarter has none to
  spare. With §30.5's fix every seed now carries hewers, smiths, cooks and keepers — still few,
  because these worlds are genuinely poor, but the chain is being climbed rather than sitting
  unused. The remaining thinness is the population and the ground, not the mechanism.

### 27.10 Tools per trade: attempted, and the argument that was wrong

Tools are one number, so a place that has spent a century farming is, on the day it turns to
hewing, exactly as well equipped for hewing as it was for farming. Capital that transfers
perfectly between trades is not capital, it is a bonus attached to a place. That is still true
and still worth fixing.

`Holdings::tools` was made per trade, and the mechanism worked. Measured: thirty farmers with
thirty-six tools grow a third more food than thirty with none, and the same village turned to
quarrying overnight hews **exactly what it would with nothing at all** — under 1e-4 apart,
because none of what it owns is the right thing. Equip it for quarrying and it quarries as well
as it farmed. Who new tools are made for was deliberately not a decision: a smith makes what
the people around them are asking for, in proportion to how many are asking, so there was
nothing for §31.1's oscillation to get hold of.

**Churn went from 6% of moves going straight back to 12%, and it was reverted.**

The interesting part is why the safety argument failed, because it was checked and it was true.
`at_rest_it_is_the_pool_it_replaced` runs a mixed village forty years and finds every trade
equipped within 5% of the same tools-per-hand — which is exactly what one pooled figure meant.
So a place that goes on doing what it has been doing sees no difference at all.

That is a proof about **rest**, and it was used as an argument about **safety**. The failure is
entirely in motion: tools-per-hand is now divided by the hands *in one trade* rather than by
everybody, and a trade's hands are a smaller and far more volatile number than a place's. A few
farmers leaving now moves farmer-tools-per-farmer sharply, which moves what the ground gives a
head, which moves what draws anybody there — so the emptying-and-refilling cycle that §30.5's
twenty-five-year memory was calibrated to damp comes back through a denominator that got
smaller.

Which is worth adding to §31.1 as a third rule: **equivalence at rest is not equivalence.** A
change can be provably identical in equilibrium and still change everything about how a system
moves, because what oscillates is decided by the *derivatives* — and a smaller denominator is a
larger derivative. Ask what the change does to the sensitivity of the quantities decisions read,
not only to their resting values.

The reverted commit is kept in history rather than deleted. Nothing above says per-trade tools
are wrong — the fix is a denominator that does not move as fast, and that is a piece of design
rather than a correction.

## 28. Ground that is good at different things

§27.9 named the largest thing missing from the supply chain: **stock was stock everywhere.**
Every place in every world was good at exactly the same things in exactly the same proportion,
so geography could produce a division of labour *within* a settlement and never one *between*
settlements — and `TRADE_REACH`, the term that decides what a road is worth, had almost nothing
to move along it.

### 28.1 The biome finally does arithmetic

`Terrain` has carried a biome label since it was written, and said in as many words that it was
"for reading rather than for arithmetic". That was honest and it is no longer true. What grows
on the ground decides what can be got off it that is not food:

- **timber**, from the biome. A temperate or seasonal forest is best — not a rainforest, where
  standing timber is thickest and getting it out is worst, and where pre-industrial Europe
  emphatically did not cut its wood.
- **stone and ore**, from elevation and harshness. Height is a good proxy: mountains are where
  rock is at the surface, and the same uplift that put it there put the ore with it. A river
  plain has a hundred metres of its own silt over anything worth digging.

So `Ground` carries two numbers where it carried one, and a river plain and a wooded hillside
stop being the same place with different dials. Measured across six worlds, a tundra settlement
with poor soil and high ground came out with three smiths and two farmers, and a grassland with
good soil came out with none — nobody wrote either of those.

### 28.2 Trade that is an exchange rather than an access

`trade` pools *access*: everybody reachable draws a share of everybody else's spare food,
whatever they have to offer. That is right for what it models — a road means the harvest two
valleys over is not irrelevant to you — but it is not an exchange, and without one a place whose
ground gives timber and no wheat simply starves next to a place with the opposite problem.

`barter` is the other half. One unit of material for one unit of food, which needs no currency
and no price, because the two are **already in the same unit**: the unit is what one person's
year of work produces. Unlike the access pool it is conserved — what one place hands over
another receives — and both ends are weighted by reach, because a road needs two ends.

It fires only between places that differ. Two identical valleys trade nothing, which is correct,
and is why this does nothing whatever in a world whose ground is uniform.

### 28.3 A road is worth having only if the timber is worth cutting

Adding the exchange made hewing *less* attractive at first, which was the wrong sign and a good
clue. The valuation only ever counted material at the tools it could become locally, so a place
that could sell its timber saw no reason to cut any. `Ground::sells_for` is the repair: what a
unit of material fetches from the neighbours, one for one with food, scaled by how much actually
moves and by whether anybody can get there.

A place off every road sells nothing, whatever it is sitting on. That is the first time in this
model that a road has been worth having for a reason other than other people's charity.

### 28.4 Sixty per cent of a coarse world was one line

Long-horizon worlds are coarse worlds — nobody deliberates, and what is left is demography,
economy and society. Profiling one showed **`choose_company` at about sixty per cent of the
whole run**, nearly all of it `BTreeMap` search.

The cause was one line written the obvious way. For each candidate it asked "how many of my
friends stand with this person", which walks my ties and does a tree lookup per ally —
candidates × allies × a search each, sixteen times a year for everybody alive. Walking my
friends' ties *once* and counting who turns up gives the identical answer and is quadratic in
my own friendships, which Dunbar bounds.

`hearsay_repeatedly` was second at eight per cent, iterating a season of talk one year at a
time. Both of its updates are geometric approaches to a target, so a season of them is the same
arithmetic done once; it is now two `powi` calls.

**14.9 seconds to 4.5 on a sixty-year coarse world of two hundred — 3.3×.** That is the
difference between being able to run a world for two thousand years and not, which is exactly
what §29 needs to answer whether the trap ever opens.

The general shape recurs: the expensive thing was not a wrong algorithm but a right one asked
the same question repeatedly. It is worth profiling after every layer that touches everybody,
because the layers compose and each one looks cheap on its own.

**And then a second round that gained nothing, which is worth recording because it looked at
least as convincing as the first.** The obvious next step was to hoist the friend-of-friend
table out to once a year — it is nearly the same table all year, and rebuilding it sixteen
times *had* to be costing something. It measured 29.2 billion instructions before and 29.7
after: no faster, and now a year stale. Reverting it and keeping only the change that fetches
what a chooser holds once instead of once per candidate took the same world from 4.57 seconds
to **4.15**, which is faster than either.

So the honest ledger is 14.9 seconds to 4.15 — **3.6×** — of which the first change is all of
it. The second profile is flat: `choose_company` at fourteen per cent, `Bonds::edit` at eleven,
the reckoning's own map work at ten, and nothing above that. A flat profile is the signal that
the micro-optimisations are done and anything further needs a structural change.

The lesson is the one this project keeps relearning in different clothes: **a plausible
optimisation is a hypothesis, and a hypothesis that is not measured is a change of behaviour
bought for nothing.**

## 29. Somebody works something out

Every world this simulation has ever run has been permanently medieval, and §23 said why in as
many words: `Technique` is "deliberately *not* a tech tree — there are no discoveries, no
prerequisites and no names". It had a **hard ceiling of three**. A people could get better at
what it already did, up to three times bare subsistence, and could never come to do anything
else. Ten thousand simulated years and the last year looks like the first.

That boundary is now crossed, and crossing it needed exactly one idea: **the ceiling is not a
constant, it is a frontier, and the only thing that moves it is a person.**

### 29.1 Two numbers per trade, not one

- **known** is what is actually practised. It rises by ordinary copying where there are enough
  people to copy well and falls where there are not — the Tasmanian result, unchanged, and
  still the reason technique is a *population* variable rather than a clock.
- **frontier** is the most that could be practised. Nothing moves it except somebody working
  something out.

Per trade, because knowing how to farm better is not knowing how to smith better. A people
with no smiths never improves smithing, so two worlds that specialised differently end up good
at different things — which is a thing civilisations do and this model could not previously
express at all.

### 29.2 What decides whether anybody ever does

Four things, and not one of them is a date.

- **Slack.** Somebody has to have had a year they did not spend staying alive. A place with no
  surplus produces no advances however clever anybody in it is — and *that is what makes the
  Malthusian trap a trap*: the surplus that would buy thinking is the same surplus that buys
  the children who eat it.
- **Openness.** The trait for novelty, and the only place in the model where it decides
  something that outlives the person who has it.
- **Roads.** How easily a country's people reach each other. Ideas need somebody to have them
  at; a hamlet at the end of a track and a town on a road get different numbers of good ideas
  out of the same number of heads.
- **What they do all day.** An advance is in the discoverer's **own trade**. Nobody works out a
  better forge who has never stood at one.

The advance belongs to the **country**, because a country is exactly the set of people who can
reach each other to copy something — the same unit `learn_and_forget` uses, and for the same
reason.

### 29.3 A proportion, not a step

`BREAKTHROUGH` moves the frontier by one per cent **of what it already is**. That is the whole
of why this can ever escape anything.

An absolute step makes knowledge arithmetic, and arithmetic knowledge always loses to a
population that grows geometrically — the extra food feeds extra people and the standard of
living returns to where it was, for ever. A proportion compounds, and `dA/dt ∝ P · A` with `P`
bounded by `A` is the shape that Kremer's account of the very long run turns on. Whether a
given world ever gets there is then an **outcome** of how much surplus it managed to hold on
to, rather than a date somebody wrote into the model.

### 29.4 The one number that had to be chosen

`WORKING_IT_OUT` is the yearly chance that somebody with a whole year of slack, in a
well-connected place, works something out. It is the only figure in §29 that is not derived,
and it is worth being plain about what it is choosing.

A real village of three hundred produced, over a century, essentially no attributable lasting
improvement. What reached it came from populations a thousand times larger. Calibrated to
*that*, the mechanism would be correct and permanently invisible in any world this machine can
run. It is set instead to about **one lasting improvement per comfortable country per human
lifetime** — generous by a wide margin — and the reason for the generosity is written down
here rather than hidden inside the number.

### 29.5 The whole of technique had been inert, and nobody had looked

Building the frontier turned up something worse than the ceiling. `after_a_year` was a
**cliff**: above `MINDS_TO_KEEP` carriers a people climbed towards three, and below it
everything decayed to bare subsistence. Every country in every world this simulation runs is
smaller than that threshold. So no world had ever practised anything above bare subsistence at
all — the technique model had been switched off for its entire existence, and the only symptom
was a number that read 1.000 in a diagnostic nobody was reading closely.

It is also stronger than the evidence it comes from. Tasmania is a claim about four thousand
people *losing* a complex toolkit over eight thousand years. It is not a claim that two hundred
people know nothing.

So what a people can hold is now a capacity that grows with its numbers:

```
holdable  = 1 + (FIRST_CEILING − 1) × carriers / MINDS_TO_KEEP
practised → min(frontier, holdable)
```

`MINDS_TO_KEEP` carriers hold exactly the old ceiling of three, which is where that number came
from and where it stays. Fewer hold proportionally less; more hold more, bounded by what anybody
has worked out. The first attempt made it a *share of the frontier* instead, and that was wrong
in the opposite direction — it let forty people carry a quarter of an industrial civilisation,
which is precisely what Tasmania says cannot happen.

What survives is the shape that matters: **a people loses what it can no longer carry**. Cut a
large one off, shrink it, and it slides back down to what its remaining numbers can hold. That
is the dark age, and it is the same arithmetic as the discovery that made the light one.

### 29.5.1 Technique travels by contact, not by identity

`learn_and_forget` counted a technique's carriers as the people of one **country** — places
that can reach each other *and share a people*. That unit was chosen because a country is "the
set of people who can reach each other to copy something", and the second half of that
description was quietly doing work the first half did not support.

Culture fragments a world faster than anything else in it. Nine hundred people spread over five
quarters came out as countries of **eighty**, because drift splits a people long before
distance does. So the population that had to carry a body of technique was always about a tenth
of the population that could actually have carried it, and no world ever held anything at all.

Tasmania is an argument about **contact**. Two villages that walk to each other copy each
other's tools whether or not they call themselves the same thing — and they emphatically do not
stop when one of them starts calling itself something else. So technique now travels in
`neighbourhoods`: places connected by reach, whatever they think of each other. Countries keep
doing what countries are for, which is naming and practice, and stop deciding what anybody is
able to know.

### 29.6 A test on a treadmill measures the horizon, not the mechanism

`hunger_is_what_stops_it_and_it_is_felt_where_the_land_is_thin` founded sixty people and ran
until somebody went short. Its horizon had to be pushed out **every single time anything raised
what the land could produce** — a hundred and twenty years, then two hundred once places could
own tools, and §28's better use of the ground would have wanted more again. Each extension
looked like a small maintenance edit and each one was hiding the same thing: the test was
measuring how long a world takes to fill, not whether hunger works.

It now founds four hundred people on ground that will not carry them and looks after forty
years. The claim it makes is unchanged and the mechanism it exercises is the same; what has gone
is the dependence on a horizon that every future improvement would move again.

The general lesson is worth keeping: **a test that has to be re-tuned whenever the model gets
better is measuring the tuning.** Where the condition under test can be *constructed*, construct
it.

### 29.7 What actually happened: one world, five hundred years

Seed 0x221, a hundred and twenty founders, nobody watching closely.

| year | living | in touch | practised | frontier | advances | short |
|---|---|---|---|---|---|---|
| 100 | 210 | — | 1.013 | 1.006 | 5 | — |
| 200 | 696 | 357 | 1.060 | 1.010 | 7 | 0.235 |
| 300 | 1691 | 887 | 1.261 | 1.023 | 13 | 0.365 |
| 400 | 1888 | 988 | 1.553 | 1.045 | 25 | 0.321 |
| 500 | 1870 | 1019 | 1.749 | 1.098 | 49 | 0.267 |

Three things happen in that table and they happen in order.

**The world fills.** Two centuries of fast growth — the population more than doubles twice —
until at year 300 it is a third short of feeding itself and stops. From 400 onwards it is
pinned: 1888, then 1870. That is the Malthusian ceiling, working exactly as §21 says it should.

**The threshold is crossed.** Around year 300 the number of people in touch with each other
passes `MINDS_TO_KEEP`, so the world can finally *hold* everything anybody has worked out.
Before that the binding constraint was carriers; after it, the frontier. That is the moment
the discovery model starts to matter at all.

**And then knowledge compounds while the population does not.** Advances run 5, 7, 13, 25, 49 —
doubling every century — against a flat population. Hunger falls through it: 0.365, 0.321,
0.267. What is being produced is going into knowing things rather than into more mouths.

(That doubling is one seed and it does not replicate; two more and one more century are in
§29.7.1, and they change the claim.)

That is the escape, in the only form it ever takes. Nothing in the model schedules it, and
nothing in the model rules it out; what decides it is whether the compounding in
`BREAKTHROUGH` outruns the population that eats the surplus paying for it, and here it does.

It should be said plainly what has *not* been shown. Five centuries is not an industrial
revolution, `practised` at 1.75 is a better plough and not a steam engine, and one seed is one
seed. What the table establishes is that the mechanism has the right *shape*: a long
Malthusian flat, a threshold, and then a curve that bends. Whether it keeps bending is a longer
run and it is now affordable — see §28.4.

### 29.7.1 It does not replicate, and the claim above was one seed

§29.7 said advances **double every century**. Two more seeds and two more centuries say that is
not a law of this world, it is what seed 0x221 did.

| century | 0x221 | 0x11 | 0x31 |
|---|---|---|---|
| 100 | 5 | — | — |
| 200 | 7 | 7 | 10 |
| 300 | 13 | 11 | 15 |
| 400 | 25 | — | 15 |
| 500 | 49 | — | — |
| 600 | 80 | — | — |

0x221 runs 7 → 13 → 25 → 49, which is a doubling, and then 80, which is not — the sixth
century grows by 63%. 0x11 grows by 57% in its third century. 0x31 grows by half in its third
and **not at all in its fourth**: fifteen advances, then fifteen.

What survives is the shape and not the rate. Every seed measured grows its rate of discovery
rather than holding it, none of them stalls at the medieval ceiling the model had before §29,
and none of them runs away either. What does not survive is the number: a century's growth
ranges from 0% to 100% across three worlds, so "doubling" was a description of one trajectory
that happened to be the first one run long enough to see.

One more thing the sixth century says, against §29.7's other claim. That world's population was
supposed to be pinned at its Malthusian ceiling — 1888, then 1870 — and between years 500 and
600 it went to 4,787. So knowledge did not only fail to be eaten by more mouths; past some
point it *bought* more mouths, and the ceiling moved. That is the mechanism working (a better
plough feeds more people) and it means the phrase "against a flat population" in §29.7 holds
for centuries four and five and stops holding in century six. Whether hunger comes back with
the extra people is the next thing to measure: it read 0.267 at 500 and 0.243 at 600, so not
yet.

**Both tables predate §30 and neither has been re-measured.** Every number above comes from a
world whose households spent half their moves going straight back where they came from, and
which therefore never finished sorting itself into one quarter. §30.4 changed that, and §30.5
records that the world it leaves behind is more concentrated and poorer for it, so these levels
describe a world that no longer exists. What was being claimed is the *shape* — a long flat, a
threshold, then a curve that bends — and nothing in §30 touches the mechanism that produces it.
The levels are left standing and labelled rather than quietly restated, because re-running six
centuries is an afternoon and pretending the old numbers still apply is worse than saying they
do not.

### 29.7.2 Thinking capacity is conserved while the population is still growing

Re-measuring after §30 turned up something that looked like a regression and is not. The
repaired worlds are larger, richer, better connected and less hungry — seed 0x221 at year 300
reads 2,957 alive against 1,691, with 2,104 people in touch against 887 and hunger 0.264
against 0.365 — and they produce *fewer* advances: 9 against 13, with the frontier at 1.010
against 1.023.

The reason is visible in one probe. §29 makes an advance need `prosperity - want`, a place's
spare per head, and the thing that rolls for one is a head standing in it. So what a world can
think with is the product: heads times how idle each of them is. Over two hundred years, seed
0x221, that product reads

    84, 124, 81, 94, 134, 148, 124, 119, 135, 130

while the population it is drawn from goes from 120 to about 800. **It is flat.** Spare per
head falls exactly as fast as heads accumulate, which is what a Malthusian world *is* — every
gain in what the ground yields is spent on more people to yield it to, including the gain in
slack. A world still filling has a fixed budget for having ideas no matter how many people it
puts in the field.

Which means the compounding in §29.7 was never a property of the growth phase. It began when
that world's population stopped at its ceiling — 1,888 then 1,870 — and technique kept climbing
into a population that was no longer eating the difference. §30's world is bigger and better
fed and has therefore *not finished filling* by year 300, so it is still in the flat part. The
trajectory is not worse; the threshold is later, because the ceiling is higher.

Stated as a prediction so it can be wrong: the compounding should appear in these worlds too,
one or two centuries further out than it did before, and it should appear at the point where
`living` stops rising rather than at any particular date. Runs to six centuries are what will
say.

### 29.8 An advance is a thing that happened to somebody

`Happening::PersonWorksItOut` is the rarest thing in the chronicle and the only one that
changes what is *possible* rather than what happened. It names a person, a year, a place and a
trade, and it reads out like any other event in a life: *"Vasta Laen works out a better way to
make tools."* There is no tree, no prerequisite and no name for the thing discovered — what is
recorded is that the people who do this trade can now go further than they could, and how far
they actually get is still a matter of there being enough of them to carry it.

## 30. One life, end to end

Everything in §18's omniscient view reads the world *across* people: a town's roll-call, a
people's descent, a plot of what the whole world knew in each year. The chronicle has recorded
who each event was about since §16, and `Happening::subjects` exists precisely so that "a
biography is the log filtered by participant" — and nothing had ever displayed one.

The person scene now does. It is four lines of javascript and no new simulation: take the
events the world panel already reads, keep the ones this person appears in, and print them by
year. Births show up in three lives, being taken up shows in two, and each side reads it in the
same words. Nothing is written for the subject; a life is an angle on the record, not a record
of its own.

### 30.1 The handles in the chronicle are not typed, and that mattered

`Id<T>::to_bits` packs a generation and an index and throws the type away — which is correct
for what it is for, seeding an RNG stream and writing a save file, and correct for the
chronicle's own index, which files everything under one map. It is wrong as a way to ask "which
*people* is this about". Place 3 generation 0 and person 3 generation 0 are the same `u64`, and
`Happening::PersonMoves` carries a place. Filtering the chronicle by bare bits would have filed
somebody's move into the life of whichever person happened to share a slot number with the town
they moved to, silently, for the first forty people in every world.

So the export matches on the happening and returns typed `PersonId`s. It duplicates a match arm
list that `subjects` already has, which looked like waste until it was the thing that made the
bug impossible.

### 30.2 The first life read out had moved house twenty-two times

Thoumiaste Sern, a cook, sixty-seven years old, of Twycrag. Her record:

> 156 moves to Stanhythe · 157 moves to Stanquay · 158 moves to Twycrag · 159 moves to Stanquay
> · 160 moves to Twycrag · 162 moves to Stanquay · 163 moves to Stanhythe · 164 moves to
> Stanquay · 165 moves to Stanhythe · 166 moves to Twycrag · …

Two towns, alternating, for twenty years. Counted across the whole world: **65% of every move
ever made was a return to the place that household had left two moves before**, a mean of six
moves per mover and one household with twenty-eight.

The aggregate could never have shown this. A hundred households moving A→B and a hundred moving
B→A is the same net flow as nobody moving at all, so every measurement the project had — where
people live, how sorted the quarters are, how far displacement gets — read exactly as it should
have. Churn is invisible to any statistic that does not follow an individual through time, and
until the life panel existed there was no such statistic.

### 30.3 A first guess that was wrong, and worth keeping

The obvious suspect was the crowding term. `sort_households` scores a place as what it offers
less how packed it is, and *how packed* counted the households living there — including, when
scoring the place you already lived, you. So a household weighed a town it was crowding
against a town with a vacancy it would fill the instant it arrived; the discount is real when
the move is decided and gone by the time it is made.

That is a genuine defect and it was fixed — every candidate is now charged the crowding the
household would itself add, which makes the gain from A to B the exact negation of the gain
from B to A, so a positive `MOVE_THRESHOLD` cannot be met in both directions. It is also the
true statement of what a place is worth to you: what it is worth *once you are in it*.

It changed the churn rate by nothing at all. 64%, before and after.

Worth recording as its own thing. A plausible mechanism, correctly identified as broken and
correctly repaired, that was not the mechanism — the second time in this project that a
convincing story about a number survived until it was measured (§22.1). The fix was kept
because it is right, not because it helped.

### 30.4 The cause: the bar to get in was lower than the bar to stay

What did it was two constants that had never been compared with each other.

A household is admitted to a place if `standing + backing + slack >= affluence`, and a young
household — one whose working members are all under `RESTLESS_UNTIL` — is given
`YOUNG_MOVER_SLACK`, 0.30, because it is renting a room rather than buying a house. A
household is *priced out* of where it lives when `standing + DISPLACEMENT_MARGIN < affluence`,
where the margin is 0.18.

0.30 to arrive and 0.18 to stay. And `backing` — what your allies inside a place will lend you
— counted for getting in and not at all for staying. So a young household with friends
somewhere was admitted on Monday's terms, failed Tuesday's, was turned out, and was admitted
again on Wednesday's. Neither number is wrong on its own. Together they are a revolving door,
and it turned for people's whole lives.

The fix is one sentence: **nobody can be turned out of somewhere that would admit them
today.** The eviction test is now the admission test plus a grace, built from the same
`means_at` the door uses, so the two cannot drift apart again. Backing counts for both or
neither; the slack the young get to arrive is the slack they get to stay.

Churn went from 51% of all moves to **1%**, and the number of moves in a ninety-year world from
3,298 to 909 — the difference being, exactly, the moves that were never a decision about
anywhere. Sorting is untouched: the spread of affluence across inhabited quarters reads 0.199
against 0.208 before.

`moving_is_not_a_thing_people_do_back_and_forth` pins it, written the only way that detects the
failure — group every move by household, walk each path, count the steps that land where the
one before last did.

### 30.5 One dead mechanism underneath, and three tries at replacing it

Following the households through year by year turned up something the churn had been hiding:
four of five quarters emptying to nobody at all, permanently, while the fifth took everything.
Fixing the churn made it worse rather than better — with the sorting loop no longer too busy
shuffling households to sort them, it now runs to completion. Across four seeds the biggest
quarter went from 0.65 of all households to 0.89, and on seed 0x221 it went to **1.00**: every
household in the world, in one quarter, with the other four empty.

It costs people. The same world read 326 alive at year 220 before any of this and 203 after —
one quarter's ground feeding everybody is less ground than five quarters' was. That was the
price of the churn fix as first shipped; what follows is three attempts at not paying it, two
of which failed.

`CROWDING_AVERSION` exists to prevent precisely that, and its comment says so — it was added
after a world where *"all 1,260 survivors lived in one place at twenty-five times its capacity
while the other four stood empty"*. But it was written as `(occupancy - 1).max(0.0)`: crowding
is felt only *past* capacity. Housing is built out to meet demand, so a quarter absorbing its
neighbours absorbs them while staying inside its own walls. Measured across four seeds, **no
place in any world this project runs has ever exceeded its capacity**, so the term is
identically zero and always has been. A documented negative feedback that has never once
fired — §29.5 again, in a different room.

What that leaves is an absorbing state, which is what makes it a bug rather than just
concentration. A quarter emptied to nobody has no residents to make it worth anything, so its
`env` freezes at whatever it fell to, and nothing can ever draw anyone back.

#### The obvious fix, and why it was reverted

Crowding felt continuously and convexly — `aversion · occupancy²`, since the hundredth
household costs more than the tenth. Swept over four seeds at ninety years, against the two
failures it sits between, and it does what it says:

| aversion | biggest quarter | quarters empty | spread of affluence |
|---|---|---|---|
| gate (never fires) | 0.89 | 0.40 | 0.140 |
| 0.05 | 0.77 | 0.45 | 0.054 |
| 0.20 | 0.61 | 0.20 | 0.111 |
| 0.40 | 0.53 | 0.20 | 0.123 |

Then it was checked against §21.1, and it fails:

| crowding | starved on a thin detail budget | on an ample one |
|---|---|---|
| gate (never fires) | 2 | 0 |
| 0.05 | 13 | 0 |
| 0.20 | 25 | 0 |

Six seeds, never once the other way. Crowding computed from where households actually are
makes migration sensitive to the population's distribution, and the population's distribution
is exactly where the two detail tiers differ — so an unwatched household is pushed somewhere a
watched one is not, and some of them starve for it. That is the observer setting the death
rate, which is the one fault this project treats as disqualifying. It is not worth a
better-looking map.

So the term stays inert and is now *labelled* inert, with the measurement in its doc comment.
It is still inert after everything below — what fixed the concentration was not crowding.

#### The second try: hunger, which is at least honest

What is wanted is a counterforce that does not read the population's distribution. There is one
already computed and not consulted: `Place::want`, how far short of feeding its people a place
fell, per head. It needs no coefficient — `want` and `quality` are both a fraction of what a
person needs — and it comes from the economy rather than from a household count, so both detail
tiers run the same code to get it. It is also the honest reason to leave somewhere: crowding
pushes people out of a packed quarter for a preference, hunger pushes them out of one that
cannot feed them.

It is nearly free — tier neutrality is untouched (3 starved against 2), the quarters diverge
*more* rather than less (spread 0.140 → 0.175), and one fewer stands empty (0.40 → 0.35). And
it barely touches the thing it was added for: concentration goes 0.89 → 0.87.

Reverted, for two hundredths. It also breaks §21's ceiling in a way worth noticing — with
hunger a reason to leave, a crowded founding relieves its own hunger by scattering, so the
population at which *anywhere* is short goes up again. That is the mechanism working, and it is
not worth having a Malthusian check that migration can dodge in exchange for 0.02.

#### The diagnosis that was wrong: "nothing local is scarce"

At this point the conclusion written here was that sorting is a positive feedback and the world
has nothing to balance it with — that housing builds out to meet demand, land does not run out,
a grid cell is most of a country, and so a quarter absorbing the entire world is not worse off
for it in any term the model computes. The fix, it said, had to be a scarcity that is really
there, which meant a finer grid, which is §6's business and not anybody's afternoon.

Every clause of that is wrong, and it was reasoned from `Place::build_for`'s comment rather
than measured. Measuring takes one probe. Seed 0x221, the quarter that wins, year by year:

| year | households | output per head | affluence |
|---|---|---|---|
| 0 | 11 | 0.482 | 0.493 |
| 24 | 27 | 0.438 | 0.517 |
| 48 | 41 | 0.320 | 0.605 |
| 84 | 55 | **0.013** | **0.630** |

Crowding is not free. It is *ruinous* — thirty-seven times poorer per head — and the model
computes it every single year, because `work::make` is Cobb–Douglas in land and labour and the
land does not grow. Meanwhile a neighbour with one household in it sat at 0.862 and nobody
went.

The right-hand column is the bug. `appeal` reads `env.quality()`, three tenths of which is
`affluence`, and affluence is built from what the residents have *accumulated*. It is a
description of the social environment and not a measure of whether you can eat, and it rose the
whole way down. People were moving towards the place that was starving them because it looked
rich.

#### Wiring it in raw makes everything worse

`Place::prosperity` is the missing number, per head, tier-consistent — `year_working` runs the
same for both. Added to `appeal` directly:

| | biggest quarter | quarters empty | moves | straight back | starved thin / ample |
|---|---|---|---|---|---|
| before | 0.89 | 0.40 | 570 | 1% | 2 / 0 |
| raw prosperity | 0.89 | 0.55 | **30,697** | **68%** | **171 / 0** |

The trade cobweb of §30.5.1 exactly, in migration, and for the same reason: **a signal that
answers the action taken on it**. One more household arrives, output per head drops, so they
leave, so it rises, so they come back. Fifty times the moving, and §21.1 in ruins.

#### What works: a place has a reputation, not a harvest

A year's yield is not why anybody moves house. What draws somebody to a town is what it has
been like for as long as they have been alive to notice — so `Place::fortune` is `prosperity`
smoothed over `REMEMBERED` years, and that is what `appeal` reads.

| remembered | biggest quarter | quarters empty | spread | moves | straight back | starved |
|---|---|---|---|---|---|---|
| none (before) | 0.89 | 0.40 | 0.140 | 570 | 1% | 2 / 0 |
| raw | 0.89 | 0.55 | 0.056 | 30,697 | 68% | 171 / 0 |
| 12 years | 0.58 | 0.40 | 0.135 | 2,727 | 14% | 7 / 0 |
| **25 years** | **0.47** | **0.30** | 0.101 | 1,602 | 9% | **1 / 1** |
| 40 years | 0.71 | 0.45 | 0.124 | 1,223 | 7% | 1 / 0 |

A generation. The concentration halves, no more quarters stand empty than before any of this
work, and tier neutrality comes out *better* than the baseline — one starved on a thin budget
and one on an ample one, which is the first time that column has read the same on both sides.

And it pays for the churn fix's population cost several times over. Seed 0x221 at year 220 read
326 alive before any of §30, 203 after the churn fix alone, and **630** now, spread over four
quarters instead of piled into one. Which is the whole argument in one number: people were
being sorted into a place that could not feed them, and the model had been saying so, per head,
every year, to nobody.

It is not monotone in `REMEMBERED`, which is the tell that it is a trade rather than a knob: at
forty the memory outlives the fact, a quarter that has genuinely gone downhill keeps drawing
people for a century, and the concentration comes back.

What it costs is sorting: the spread of affluence across inhabited quarters falls from 0.208
before §30 to 0.101. Some of that is the runaway being gone, since a world with everybody in
one quarter has a very large spread indeed and it does not mean the places diverged for good
reasons. How much of it is the runaway and how much is real sorting lost is not measured, and
is the honest open question left here.

#### What this cost to find

Three wrong turns in one afternoon, all of them recorded above rather than deleted: crowding
aversion made continuous (breaks §21.1), hunger as a reason to leave (relieves the hunger it
responds to), and the reasoned-not-measured claim that nothing local is scarce (the model had
been computing the scarcity all along and not showing it to the one decision that needed it).
The pattern in all three is the same and is worth naming: **the model already knew, and the
decision was reading the wrong number.** That is what the churn was, what the crowding gate
was, and what this was.

### 30.5.1 And the same shape again, in trades

The very next life the panel was pointed at, with the moving fixed, read:

> 143 gives up cook for keeper · 147 gives up keeper for cook · 160 gives up cook for farmer ·
> 167 gives up farmer for cook · 181 gives up cook for farmer

`Happening::PersonRetrains` was added for this — the chronicle had never recorded a change of
trade, so nothing could have shown it. Measured across the world: 11% of all retrainings go
back to the trade before last, and **88% of a year's switches are to the same trade**. That
second number is the diagnosis. `worth_taking_up` values each trade by re-running the year with
one more hand in it, everybody in a place reads the same array in the same year, and everybody
picks the same argmax — so the trade that was short is oversupplied by the people who noticed,
and next year the signal points back. A cobweb, textbook.

Two things looked wrong, and one of them was not. The comparison *seemed* to be asked the wrong
way — for somebody already at the forge, the question is what a year is worth with their hands
moved from A to B, not the marginal value of an extra hand in each, which reads like the housing
bug in another costume. It isn't. Both differences come out as the marginal value of B less the
marginal value of A, and on 80 switches across four allocations the two never once disagree
about whether a switch is worth making — `the_marginal_comparison_is_the_switching_question`
measures it rather than arguing it. The resemblance to the housing bug was the whole of the
evidence, and it was wrong.

So the cobweb has one cause: **the decision is simultaneous**. `RETRAINING`'s 8% damps the
settled; the young were ungated, reconsidering their trade every year with certainty, and
everybody in a place reads the same `worth` array in the same instant, so they moved as one and
overshot together.

Nobody reconsiders their livelihood on a schedule shared with their neighbours. `TRYING_THINGS`
gives the unsettled a yearly chance of a quarter — about once every four years, so two or three
times before they settle — instead of every year:

| chance | changes of trade | straight back |
|---|---|---|
| 1.00 (was) | 3,692 | 24% |
| 0.40 | 1,951 | 14% |
| 0.25 | 1,530 | 11% |
| 0.10 | 1,176 | 9% |

Monotone, so the choice is where the young stop finding a trade at all rather than where the
number bottoms out. A quarter is where reconsidering is an occasional event in a life rather
than an annual review.

What did *not* move is the share of a year's changes going to one and the same trade: 90%,
before and after. That number turns out not to be the diagnosis it looked like — when one trade
is genuinely short, everybody who reconsiders *should* move into it, and a herd is the correct
response to a shortage. The going-back rate is the one that means something, and it halved.

### 30.5.2 A settlement has a history for the same reason

The person scene reads the chronicle filtered by participant. The settlement scene now reads it
filtered by *place*, which is the same instrument turned ninety degrees and costs one more field
on an event — the places the happening actually names. A move names where it went; a change of
character names the place it happened to. Where somebody was standing when they were born is not
in the record and is not guessed at.

Arrivals are listed one at a time rather than totalled, on the same reasoning as §30.2: a
quarter that took nine households in four years and then none for thirty is telling you
something a count would hide.

It says the concentration out loud, which no number had. Stanhythe, seed 0x221, year 220:

> 117 · Stanhythe has become rural
> 124 · Lilosk Thaenith moves to Stanhythe · Suldia Tas moves to Stanhythe · …

Two people living there, in one household, with room for seventy-three, affluence 0.09, and a
roll-call of a farmer and an outcast. §30.5 measures that as `empty 0.40`. This is what it looks
like from inside the town.

### 30.6 What this says about the instruments

§18 argues the omniscient view exists to catch exactly this class of thing, and it has now paid
for itself twice: `Bonds::repaid` was dead code found by an atlas line reading "thought of:
poorly", and this. Both were found by *displaying* something rather than by asserting it, and
both were invisible to every assertion in the suite at the time.

The general shape is worth naming. Aggregates hide anything that is symmetric in the
population and antisymmetric in time — churn, oscillation, any flow that cancels. The only
instrument that sees those is one that follows a single subject through the whole run. This
project had six views and, until now, not one of them did.

And it compounds: the churn was itself hiding the dead crowding term underneath it, which had
been hiding an absorbing state. Three defects in one stack, none of them visible to anything
the suite measured, all of them found by printing one woman's life in order.

## 31. Every gap, in one place

This document has recorded what is missing in five separate sections written months apart —
§17.2, §23, §24.4, §26.9, §27.9 — and the entries went stale without anybody noticing. §26.9's
first bullet said the largest single thing this world lacked was an economy with more than one
product; §27 gave it five, and the bullet sat there for weeks saying otherwise. A gap list that
quietly rots is worse than none, because it is read as evidence.

So: one table, and the rule that closing anything means editing this row rather than adding a
paragraph somewhere else.

| Gap | Where | Status |
|---|---|---|
| Occupation and division of labour | §26.9 | **Closed** by §27 — five goods, five trades |
| Norms as learned, not ambient | §17.2 | **Closed** — §17.2.1 |
| Theory of mind: beliefs that can be wrong | §17.2 | **Built and inert** — §17.2.3. Nobody is measurably wrong |
| Households as political units | §26.9 | **Open.** Head and succession built and unread; two attempts measured worse (§26.10) |
| Property that outlives its members | §26.9 | **Open**, deferred on a measurement — elasticity is already out of band (§26.10) |
| Roles that are chosen | §26.9 | **Open.** An eighth deed built and reverted (§26.11) |
| Firms, employment, contracts | §27.9 | Untouched |
| Tools are one thing | §27.9 | **Open**, attempted and reverted — §27.10 |
| More goods | §27.9 | Untouched |
| Money, prices, credit, ownership | §27.9 | Partly deliberate — §27.4 argues against prices |
| Law | §26.9 | Untouched |
| Conquest and the state | §24.4 | Untouched, and named there as the next thing to argue about |
| Language, religion, ritual, kinship | §24.4 | **Deliberate.** Argued for, not deferred |
| Concentration counterforce | §30.5 | Partly closed; how much sorting was lost is unmeasured |
| Grid level, second planet, observer intervention | §23 | Parameters and decisions, not defects |

### 31.1 Two things any future gap has to pay for

Both were learned by ignoring them, and both cost a working change that had to be reverted.

**A decision read afresh from a moving quantity will oscillate.** Six instances now: households
admitted on one test and evicted by a stricter one (§30.4); migration reading raw output per
head, which answers the very move that reads it (§30.5); everyone re-choosing a trade from the
same signal in the same instant (§30.5.1); a household's character read from a standing that
moves (§26.10); and hosting's reputation saturating against a 2%-a-year decay (§26.11). The
damping is part of the design, not something to find afterwards — and note that §26.11 *did*
design a damper, for the wrong loop. Ask which quantity the new decision reads, how fast it
moves, and what the decision does to it.

**Equivalence at rest is not equivalence.** §27.10 made a change provably identical in
equilibrium — every trade equipped to the same tools-per-hand, which is what the single pooled
number meant — and it doubled churn anyway. What oscillates is decided by derivatives, and that
change replaced a denominator (everybody in the place) with a smaller one (the hands in one
trade). A smaller denominator is a larger derivative. Ask what a change does to the
*sensitivity* of the quantities decisions read, not only to their resting values.

**A new deed is not an addition, it is a re-normalisation.** `score_all` chooses by softmax over
every deed, so adding one reprices all seven of the others for everybody, whether or not they
ever do it. §26.11 raised churn from 6% to 16% and moving by 64% purely by existing — the same
result whether the new deed was in `CHOSEN` or not, and whether its own payoff was large or
tenfold smaller. Anything added to `Deed::ALL` has to budget for re-measuring what was
calibrated against the old set: §15's shared-environment share, §21's ceiling, §30's churn.

### 31.1.1 The one file nothing checks

The atlas is a string compiled into the binary. Nothing type-checks it, nothing runs it, and
for several hours tonight it did not run at all: a scripted edit duplicated a function header —
`function ranks(who) {function ranks(who) {` — and every atlas generated after that was a page
that loads, shows an empty frame, and writes a syntax error to a console nobody is reading.

**The whole suite went on passing at 714 tests.** It was found by trying to take a screenshot.

That is worth more than the fix. This project has built two views specifically because looking
at a thing catches what asserting about it cannot (§30.6), and the views themselves were the
one part with no assertion behind them at all. A broken atlas is a *silent* failure of the
instrument that exists to make failures visible.

`the_atlas_closes_everything_it_opens` walks the script tracking strings, template literals and
comments — which is the whole difficulty, since a brace inside a string is not a brace — and
asserts the delimiters balance. It is not a parser and does not pretend to be. It catches the
class of thing an editing script does to a file it cannot read, which is exactly what happened,
and it was verified by putting the bug back and watching it fail.

### 31.2 Which mechanisms are actually load-bearing

Four mechanisms in this project have turned out to be exactly right and never to fire: the
technique ceiling nobody could reach (§29.5), `Bonds::repaid` which nothing called,
`CROWDING_AVERSION` which bites only past a capacity no place reaches (§30.5), and the belief on
a tie (§17.2.3). Every one was found by accident — three of them on the same day, by looking at
something for an unrelated reason.

Accident is not a method. Switching a mechanism off and measuring is, and it costs two minutes
apiece now that `vitals` exists. Three seeds, 120 founders, 90 years:

| off | living | churn | biggest | spread | verdict |
|---|---|---|---|---|---|
| *baseline* | 667 | 9% | 0.55 | 0.11 | |
| `CROWDING_AVERSION` | 665 | 9% | 0.54 | 0.11 | **inert**, as §30.5 says |
| `VOUCHING` | 667 | **4%** | **0.60** | **0.16** | load-bearing |
| `RETRAINING` | 667 | 7% (1,128 moves) | **0.62** | 0.13 | load-bearing |
| `HEARD_OF` | 662 | **5%** | 0.53 | **0.08** | load-bearing |
| `READING` → 0 | 667 | **13%** (1,564 moves) | **0.62** | **0.08** | load-bearing — but see below |
| `READING` → 1.0 | 667 | 9% | 0.55 | 0.12 | **the lag is inert** |

One more, at **eight** seeds rather than three — because the two columns it moves have a noise
floor larger than the effect at three, which is §35.8:

| off | living | churn | biggest | empty | verdict |
|---|---|---|---|---|---|
| *baseline (8 seeds)* | 1997 | 9% | 0.55 | 0.35 | |
| `acts_are_possible` | 2064 | 8% | 0.55 | **0.40** | **nearly free**, at the aggregate |

The whole act vocabulary costs five points of `empty` and nothing else — not population, not
churn, not concentration, not discovery, not patronage, not the trade mix (§35.9). It is the
first mechanism in this table with a switch built for the purpose rather than a constant edited
by a script, and that is not tidiness: two ablations in this project have left the working tree
holding an edited constant after the container running them restarted. It is also the first whose
ablation was run against §15's balance sheet as well as against `vitals`, which is how the scale
error in §35.9.1 was found — the aggregate table above was already neutral while a calibration
band was six points out.

The last two rows of the first table are the point, and the first of them nearly produced a wrong conclusion.
Setting `READING` to zero does not make the belief *accurate*, it freezes it at nothing — which
removes the term from `choose_company` altogether, and since the belief tracks warmth closely
that is removing four tenths of the warmth signal. Of course it changes everything.

The question §17.2.3 actually asks is whether the belief's capacity to be *wrong* does any
work, and the ablation for that is `READING = 1.0`: belief exactly equal to truth, no lag, no
possibility of error. That is indistinguishable from the baseline. So the sharp statement is
that **the term is load-bearing as a carrier of warmth, and the divergence it exists for is
inert** — which is a more useful thing to know than either half alone.

Two other findings fall out and neither was being looked for.

**`VOUCHING` costs churn and costs sorting.** Turning off what your allies will lend you at a
door more than halves the going-back rate (9% → 4%) and makes the quarters *more* different
from each other (0.11 → 0.16). Admission that depends on who you know is a source of the
oscillation §30.4 was about, and it flattens the sorting §14.4 wants. That is not an argument
for removing it — chain migration is real and §25 wants it — but it is a cost that was not on
the books.

**Gossip holds the world together more than it looks.** `HEARD_OF` at zero drops the spread of
affluence across quarters from 0.11 to 0.08. Reputation travelling between people who have
never met is doing a third of the work of making places differ.

A second batch, once `vitals` had been widened to see what these ones claim — which had to
come first, because an instrument that cannot see a mechanism's output reports that switching
it off changed nothing, and that is the same sentence as "inert" meaning something else
entirely:

| off | living | churn | advances | taken up |
|---|---|---|---|---|
| *baseline* | 667 | 9% (82/929) | 12 | 108 |
| `WORKING_IT_OUT` | 667 | 9% (**82/929**) | **0** | 108 |
| `MENTOR_CHANCE` | 667 | **3%** (25/909) | 13 | 0 |
| `PATRONAGE` → 1.0 | 657 | 8% (**135/1,591**) | 15 | 101 |

**Discovery fires and changes nothing.** Switching off §29 entirely — nobody ever works
anything out — leaves the world *byte for byte identical* on every other line: the same 667
alive, the same 82 of 929 moves, the same trades to the person. Twelve advances over ninety
years move the frontier by about twelve per cent, `practised` lags behind that, and ninety years
is far too short for any of it to reach anybody's life. §29 is not inert — the events happen and
are real — but at the horizon this project routinely runs, **it is inconsequential**, and every
claim about it needs the six-century runs rather than a ninety-year one. That is a more precise
version of §29.7.2's finding and it was free.

**Patronage is where a third of the churn comes from.** Nobody being taken up drops the
going-back rate from 9% to 3%, and taking away a patron's *lift* while leaving the relation
raises the number of moves by seventy per cent. A door opened by somebody else is a door into
somewhere you had no other business being, so both directions make sense — but §25 sold
patronage as the largest single fact about a life, and it turns out also to be a large fact
about how much people shuffle between quarters. Nothing had costed that.

**And one that could not be asked here at all.** `RELIEF` — what neighbours give somebody who
is short — measured as very nearly nothing, because at 120 founders the hungriest quarter is
short of exactly 0.00 and famine relief had no famine to relieve. That is not a result, it is
the wrong fixture, and it is the same mistake as the first `READING` row in a different place.
Asked again at a crowded founding in §31.2.1.

The discipline this suggests: **ablate a mechanism when it is built, not when somebody trips
over it.** And ablate it against an instrument that can see what it claims, at a fixture where
it has something to do — zeroing a rate usually removes a term rather than neutralising an
effect, and measuring a famine mechanism in a world with no famine measures nothing at all.

### 31.2 What "deliberate" means in the table above

Two rows say deliberate rather than open, and they are not the same kind of claim.

**Language, religion, ritual, kinship** are argued against in §24.4 and §23's sixth question.
Languages need phonology and sound change to be worth having, and a people named for a practice
carries more real information than an invented word would. Religion and kinship rules are
genuinely open questions the design has not taken a position on — they are listed as deliberate
because *not having them* was a choice, not because having them would be wrong.

**Prices** are different: §27.4 argues that what a trade is worth is a comparison somebody
makes rather than a number anybody quotes, and the one attempt at authored prices produced
fifty-one cooks against forty-seven farmers and cost a third of the population. That is a
position with a measurement behind it. Money and ownership are not covered by that argument and
remain open.

## 32. Taking things, which does not happen

§24.4 kept conquest out of this world on the grounds that it needed a state, an army and a
border anybody could be stopped at. That was the wrong list. What a taking needs is a
**reason**, a **means**, and something worth **taking** — and the third was what was actually
missing. Until §26.12 there was nothing here that could change hands: standing is a capacity
and dies with its owner, and a place's tools belong to the place.

The other two this world already had. §25 says exclusion is its only sanction — no violence, no
law, no court, just a door that does not open for somebody the neighbours have turned against.
So a taking is not a new kind of thing. **It is the negation of the one thing that was already
political**: a door opened by force rather than passed.

`Happening::PlaceTaken` and `take_what_can_be_taken` are built, wired into the reckoning, and
they now fire — about once in three hundred world-years, which is what these worlds afford.
Getting there took two wrong triggers, and both were wrong in ways worth keeping.

### 32.1 Desperation cannot be the trigger

The first version keyed a raid on the raider's need: we are short, they are not, come on. It
could not fire, and the instrument said why in one line per pair. Every pair the model offered
read either

    reach true   want 0.000
    reach false  want 0.032

and never both. Reach feeds what a place produces *and* decides who its neighbours are, so a
place poor enough to want to raid is a place too isolated to reach anybody. That is not a bug in
either mechanism; it is a true consequence of both being built on the same term, and it means
**desperation cannot be the trigger in a world where isolation is what causes the desperation.**

Which is the more honest reading anyway. Raiding is not what the desperate do — it is what the
strong do to the wealthy. So the trigger became what the victim *has*, times how far the raider
outnumbers them.

### 32.2 Countries cannot touch, and that is a theorem

It still did not fire, at any rate up to 1.0 — a closed gate rather than a small number. The
guess written here was that the numbers never favoured the raider. That guess was wrong, and
measuring it took one probe: **zero adjacent cross-country pairs**, in every world, at every
size.

Not a rare configuration. A structurally impossible one. A country here is a set of places that
can reach each other *and* share their ways, and §24 makes ways converge under contact — so any
two places close enough to raid have long since become the same country, and any two countries
are by construction out of each other's reach.

§24.4 observed that countries here *"merge by converging, never by one taking another"* and
filed it as a missing feature, *"the obvious next thing to argue about"*. It is not a missing
feature. **It is a theorem about how a country is defined**, and no conquest mechanism keyed on
countries can ever fire, however it is written.

So a raid is between *places* that can reach each other, whoever they call themselves — which is
also truer: a raiding party does not check whether the next valley keeps the same customs. Keyed
that way it fires.

### 32.2.1 Opportunity-limited, not rate-limited

`TAKING` turns out to be a knob that does nothing. At 0.02, 0.10 and 0.30 the same worlds
produce the **same single taking** — because pressure is near-zero almost everywhere and
occasionally large, so the rate multiplies a number that is either negligible or already
decisive. What limits raiding here is how rarely a place is at once reachable, richer, and
smaller than its neighbour. It is left at 0.02, since raising it buys nothing.

### 32.2.2 What this is not

This is the taking of **things**, not of ground. Transferring a place itself means displacing
the households in it, and displacement runs through admission — the path by which five separate
mechanisms broke this world in a single night (§26.10, §26.11, §26.12, §30.4, §30.5). That half
was deliberately not attempted.

### 32.3 The one thing that went right

The instrument was built **before** the mechanism, for the first time in this project. §31.2's
rule — ablate against something that can see what the mechanism claims — was applied forwards
instead of backwards, and the `takings` line existed before there was anything to count.

It is what caught both wrong triggers within minutes rather than months, and what turned the
second one from a guess into a theorem. The other six inert mechanisms in this project were
found by a screenshot, a stray comment, an unrelated ablation, and by tripping over them. This
one was found on purpose, and then fixed — which is the first time round that loop has closed
here.

## 33. What people are to each other, counted

Asked plainly — are there allies, enemies, lovers, spouses, pets — and answered by counting
rather than by describing. 518 adults, 140 years, seed 0x221:

| | each |
|---|---|
| people they know | 358.5 |
| of whom allies | 28.1 |
| warmly regarded | 45.0 |
| coolly | 56.8 |
| **actively disliked** | **25.8** |
| owe or are owed days of help | 0.0 |

The most befriended adult has 73 allies; **the most resented has 110 enemies**.

**Enmity was never built.** There are almost exactly as many people somebody dislikes as likes,
and nothing anywhere implements it: `warmth` runs from loathing at −1 to devotion at +1, so
disliking is liking with the sign flipped and falls out of the same three lines. An *ally* is
not a label either — `known > 0.3 && warmth > 0.25`, read whenever asked, which is §26.1's rule
about positions applied to friendship.

**One partner, exclusive, and permanent.** 504 of 518 adults live with one. `seek_partner`
requires the opposite sex, an age near enough, and not close kin — and that is the whole of it.
There is no distinction between a wife, a husband and a partner: one relation, `PersonPairs`,
which the chronicle phrases as *"set up house together"*. No ceremony, no status, no contract.

**Nobody separates.** 647 pairings ever and the only exit is a death; the 73 people who paired
more than once are widowed. There is likewise no relation outside a household, so nothing in
this model can be unfaithful — no lovers, no mistresses, no affairs.

**No pets, and it is not an omission that could be patched.** Animals exist as *demes* — a
population with a size and a range (§11.2) — and never as individuals. There is nothing here a
person could own, name or bury, and giving them one means giving `ecology` an individual, which
is a different model.

The asymmetric relation is **patronage**: 118 taken up, and a patron is by construction older
and better off, since between equals it is just company. §31.2 measured it as a third of all
the churn in the world, which nothing had costed.

### 33.1 One number in that table is not a result

`owe or are owed` reads **0.0**, and that is the wrong fixture rather than a finding. Debt
accrues only through famine relief, and at 160 founders nobody goes hungry — so the reciprocity
channel §25 is built on is simply idle here. Exactly the `RELIEF` ablation of §31.2, met again
by walking into it from the other direction: a mechanism measured in a world that never calls on
it measures nothing at all, and the number it returns looks like an answer.

## 34. What somebody carries of their own life

The chronicle (§16) is the world's record: complete, ordered, external, and **nobody in the
world can consult it**. `person::memory` is the other half — what one person has, which is
partial, fades, and is about *whom*. The second clause is the one that matters: a grudge is a
remembered wrong against a name, and a reputation earned rather than assigned needs somebody to
have seen a thing and kept it.

Both are written from the same call. `World::remember` already computed who an event concerned,
for the chronicle's index; a personal memory filled from anywhere else would drift out of step
with the record of the same moment.

### 34.1 Why the curve is not an exponential

A half-life is the obvious model and it is wrong for a reason worth stating. Under exponential
decay **every memory vanishes on the same timescale** — changing the weight shifts *when* it
goes, never whether there is a tail. "She half remembers her mother's death at seventy" and "he
had forgotten that slight by spring" cannot come out of one exponential rate. Getting both needs
two rates, and then something has to decide which memories are special, which is a stored flag
and precisely what this project spends its time deleting.

    strength = weight / (1 + age / SPAN)

falls fast and then very slowly. A thing that landed at 1.0 is still faintly present decades on;
a thing that landed at 0.25 is gone within a year or two. **Permanence stops being a flag and
becomes a consequence of the curve** — §26.1's discipline about positions, applied to time. It
also happens to be closer to what the forgetting literature measures than an exponential is.

Measured, from one rule and with nothing marked as great: at thirty years a death still reads
0.15 and a move reads under 0.06, and the death is still falling rather than parked on a floor.

### 34.2 Nearness keeps a grudge sharp

Meeting somebody again brings back up what is held about them. So **distance forgives and
proximity does not**: the widow who moved away softens, the brother across the square does not.
One line, and it is the mechanism rather than a special case — retrieval strengthening a trace
is the same thing that makes a rehearsed fact stick. Ten years of being met leaves an injury a
third sharper than the same injury by somebody who left.

### 34.3 Forgetting has to compete

A bound of twenty-four things, evicting whichever is *currently faintest* rather than the
oldest. A crowded life forgets more than a quiet one, which is true and free — and the one thing
that mattered survives a crowd of small ones, which an oldest-first eviction would lose.

### 34.4 Who remembers what is not symmetric

The mapping is written by hand per happening rather than derived from `subjects()`, because the
asymmetries are the point rather than an inconvenience:

- **Parents remember a birth; the child does not remember being born.** Which saves the
  commonest event in the world from also being the emptiest memory in it.
- **A death reaches everybody who knew them**, weighted by how well — the only event here that
  touches mere acquaintances.
- **Both sides remember a patronage**, since being taken up and taking somebody up are each
  things a life gets organised around.
- **Only the robbed remember a raid.** A raid is a Tuesday to the raiders and a year to remember
  for everyone it was done to.

That last asymmetry is not decoration. It is most of what will make a wrong *feel* like one when
§35 gives this world wrongs: the injury persists on one side of the ledger and not the other,
without anybody having to declare which side was injured.

## 35. What one person does to another

Everything anybody did in this world before now was addressed to the world. They ate, they
worked, they moved. Even the social things were: `Deed::Socialize` relieved a need and named
nobody, and the mutual aid in `share_the_shortfall` picks whoever happens to be an ally with
something spare. **Nothing in the model was a person choosing a person and doing something to
them on purpose.**

`person::acts` is that. Five acts, each aimed at somebody, each scored from who the actor is and
what they hold about the target:

| | what it is | what it costs |
|---|---|---|
| **Give** | hand over some of what you have | an exact transfer of standing, booked as a debt |
| **Teach** | pass on what you know | a share of the teacher's year, into the pupil's upbringing |
| **Shun** | refuse them, and let everybody you are close to know | nothing, which is the point |
| **Rob** | take what they have | an exact transfer of estate |
| **Kill** | kill them | a death, recorded as `Cause::Violence` |

And a sixth thing that is not an act, because nobody chooses it: **withholding**, the name for
having done nothing in a moment that asked for something.

The design questions behind this were put to the reader and answered: acts get targets; targets
are *motivated*, so kindness can reach a stranger and violence needs hatred plus nothing left to
lose; harm is wrong everywhere while obligation varies between peoples; and the consequence of a
wrong lands **always, through conscience**, with no witness required.

### 35.1 Why this is not another `Deed`

The obvious home is `Deed::ALL`, and it is the wrong home for a reason already paid for twice.
Deeds are chosen by softmax over relative scores, so **a new deed is a re-normalisation and not
an addition** (§26.11): the one time an eighth was added it left eating and sleeping untouched in
the source and moved migration by 64% in the world, and it was reverted.

Acts are scored independently. Each appetite is a quantity in its own right, and `choose` rolls
each against its own bar rather than against the others — so adding a sixth act changes the other
five by *nothing*, not merely by little. That property is worth more than the elegance of one
list, because this vocabulary is going to grow.

### 35.2 Five ways the same mistake was made

All five had the same shape: **two quantities that are not on a common scale, used as though
they were.** None was visible in the code. Three read as findings about human nature until they
were measured, one read as a settlement collapsing, and one read as §15's shared-environment band
quietly failing.

**A product is not a sum.** Giving was scored as a sum of four ordinary reasons — kindness,
fondness, duty, what is owed — and landed near 1.0. Robbery was scored as a product of five
bounded factors and landed near 0.007, three hundred times under the same bar. Nothing was ever
robbed in any world, and the first reading of that was "this society has no thieves". Robbery
became a sum of three reasons — need, greed, spite — and started happening.

**A maximum lets the cheap act mask the grave one.** Shunning and killing are driven by the same
hatred, and shunning is far cheaper, so the shunning appetite is above the killing one at every
level of loathing anybody ever reaches. Under a maximum, **murder is structurally impossible**,
and no tuning fixes it: raise killing enough to win and every falling-out is a murder. There is
something true in there — a society reaches for the cheap sanction first — but "and therefore
nobody is ever killed" is not it. Each act is now rolled separately, and if more than one comes
up, the gravest is what happened.

**One bar across five acts is a unit mismatch.** With both of the above fixed, the strongest
anybody in a measured world ever wanted to kill anybody was **0.138**, against a shared bar of
0.25. That is not a fact about the population. Killing is a product of five conditions each of
which is already rare; giving is a sum of four ordinary ones. Because `choose` rolls each act
separately, a per-act bar changes only that act — which is the whole reason it is safe to have
one.

**A gift is not a famine.** `share_the_shortfall` books favours as `share * 365.0`, because there
`share` is a fraction of a year's food and the product is days of hunger. A gift is standing, and
a day of work is worth `WORK_GAIN` — 0.0017 — of it, so a fifteenth of a comfortable person's
position is *hundreds* of days of work. `Bonds::helped` warms the receiver by a twentieth per
day. Every gift arrived as instant devotion and left a debt nobody could ever clear, the tie
graph stopped meaning anything, and **in one of eight worlds every household in it ended up in a
single quarter** — §30.5's collapse, found by the test that exists for exactly that. A gift is now
measured in days of work, because that is the unit debts are kept in and the only unit in which a
favour has a size.

**A lesson is not a standing.** `Person::absorb` takes a childhood quality on
`Environment::upbringing`'s scale, which is `(quality - 0.5) * 2.5`: signed, centred on
nothing-special, running about −1.25 to 1.25. Teaching handed it the teacher's *standing*, which
runs 0 to 1 and averages near 0.4 — so every lesson was a strong positive shock to a quantity
centred on zero, and being taught by a middling neighbour counted as a better childhood than
being raised in the best quarter in the world. It cost six points of §15's shared-environment
share and took that band **below its floor**, which is how it was found. See §35.9.1.

### 35.3 A gate that never fires looks exactly like a broken one

`Toward::Kill` needs both halves of a sentence: they hate them, *and* they have nothing to lose.
When a conjunction never fires there is no way to tell a society from a bug without measuring the
halves separately — the same problem as §32.2, where conquest turned out to need adjacent
countries and there were **zero adjacent cross-country pairs in any world at any size**.

`sim/examples/who_could_kill.rs` measures the halves. In one 90-year world of 269 people:

    ties known well enough   19000
    of them, hate > 0.45       850   (4.47% of ties)
    adults with nothing to lose  8   (4.8% of adults)
    ties that are both          56

Both halves are common, and fifty-six ties satisfy both. So the conjunction was not the reason —
which is what sent the search to the bar, and found the unit mismatch above.

The same example then measures the appetite itself rather than its gates, which is the step that
mattered: *gates that both fire and an act that still never happens* is a different question from
*can anybody*, and only the number the code actually computes answers it.

**"Nothing to lose" is one minus the strongest thing holding you**, not a blend. A man with a
child to feed is held by that alone however poor and sick and friendless he is. The anchors are
what people would take from you, who needs you, and how much life you have left to forfeit —
three, not four: condition was folded into the last, because being ill is not a separate thing to
lose but a shorter remainder, and as a fourth anchor it required somebody to be at death's door
before anything else counted, which made the whole conjunction unreachable.

### 35.4 Wrongs, and the two kinds of them

**Harm is wrong everywhere.** `Toward::harm` does not depend on where you are standing or who
raised you. That is not a claim about metaethics; it is the minimum a model needs so that a
murderer cannot emigrate into innocence.

**Obligation is local.** What you owe the person in front of you when they are going short is a
thing a people has, and peoples differ. It is *read off how they spend their days* rather than
stored — a people that spends a large share of its doing on each other is one in which turning
away from somebody is conspicuous — so it drifts as a culture drifts and splits when a culture
splits, with no doctrine anywhere in the model.

And because a person carries their own upbringing's version of it (§17.2.1's `norms`), somebody
who moves **transgresses without knowing they have**: they withhold exactly as they always did,
in a place where that is not done, and are judged by a standard they never learned. Their
neighbours resent them; their conscience says nothing at all. That asymmetry is the whole reason
the local number and the personal number are kept apart, and it is `norms` finally doing
something *to* somebody rather than merely differing between people.

### 35.5 Conscience, which needs no witnesses

There are no witnesses in this model and there is no need for any. A wrong is kept by whoever did
it, always, as `What::DidWrong`. What that memory does is make the next one dearer: `restraint =
1 / (1 + guilt)`, where guilt is felt in proportion to benevolence and to how anxious somebody
is. So the same act sits differently in two people, and somebody at the floor of both carries
what they did as a fact rather than as a weight — which is how the model gets a person who can
keep doing it, without anybody writing down that they are a monster.

It fades on §34's hyperbolic curve, so a wrong done at twenty still faintly restrains at sixty
while a wrong done last year restrains hard. `What::Wronged` is weighted slightly *above*
`What::DidWrong`, and that ordering is a claim rather than a rounding: a wrong is felt harder by
whoever it was done to. It is also what makes a feud asymmetric — my grievance outlives your
remorse.

### 35.6 Withholding is a state, not an event

The first version assessed withholding on each of the sixteen evenings a year somebody spends in
company, and counted **24,575 wrongs in three worlds** — which savaged every tie in every
settlement and drove the largest quarter's share of households from 0.47 to 0.65. Not helping
somebody is a standing state; counting it once per evening makes the same failure sixteen wrongs.
It is now assessed once a year, and only against somebody *visibly* worse off, because an
obligation everybody is failing all the time is not an obligation but a tax on being sociable.

The second correction is subtler. Withholding used to keep a memory **and** damage the tie, which
is double-counting: a slight of this kind is carried rather than acted on, and what it does to
two people has to run through somebody deciding to do something about it. The grudge raises the
appetite for shunning, and shunning is what cools a tie. Removing the direct damage returned a
third of the extra migration and six points of settlement concentration.

### 35.7 The currency of this world is means, not food

Generosity was first keyed on hunger, which produced nothing, and then on the *hunger need*,
which produced everything. Both were wrong and the pair of them is instructive:

- At the sizes this project runs, `short` reads **0.00 to 0.02** — there is no famine, so there
  is nobody to relieve. Keying on real shortfall gave a vocabulary that never fired.
- `Need::Hunger` is the daily rhythm between meals and is non-zero for everybody always. Keying
  on it made two hundred people a year guilty of not sharing lunch.

What actually distinguishes people here is **means**. `poverty(means)` is the quantity generosity,
withholding and robbery all key on, with real shortfall left as a multiplier for the worlds that
have one.

### 35.8 The instrument had to be widened before it could say anything

`empty` moved from 0.33 to 0.47 on a change that added nineteen robberies to a world of six
hundred people. That is not the mechanism; it is the fact that *any* change reshuffles which
quarter happens to fill up. **A mechanism cannot be judged against a statistic whose noise floor
is larger than any effect it could have** — so `vitals` now takes `SEEDS=n` up to twelve, and the
ablation below is eight worlds against eight.

Two further things were needed before the comparison meant anything:

- **Acts draw from their own RNG stream.** They first drew from the evening's, and a single extra
  draw reseeds every choice made in the world after it. The first measurement reported migration
  up 39% and a third of the smiths gone, and most of that was not the mechanism — it was the
  shift. A mechanism that cannot be switched off without moving everything else cannot be
  ablated, and §31.2 is the whole method.
- **The ablation is a switch on the world**, `acts_are_possible`, not a script that edits a
  constant and rebuilds. Two ablations in this project have left the working tree holding an
  edited constant after the container running them restarted, and an ablation nobody can run
  without editing the source is an ablation nobody runs.

### 35.9 What it costs, over eight worlds

`SEEDS=8 cargo run --release --example vitals`, against `ACTS=0` on the same instrument:

| | off | on |
|---|---|---|
| living | 1997 | 2064 |
| churn | 9% (336/3626) | 8% (302/3595) |
| biggest | 0.55 | 0.55 |
| empty | 0.35 | 0.40 |
| spread | 0.11 | 0.12 |
| advances | 38 | 35 |
| taken up | 340 | 337 |
| trades | 924/35/25/149/45 | 985/48/16/147/36 |
| assimilation | 0.100 | 0.102 |
| **acts** | — | gave to 819, taught 161, shunned 285, robbed 88, **killed 6** |

Population, churn, concentration, discovery, patronage, assimilation and the trade mix all
land where they landed without it. The only column that moves is `empty`, by five points.

That neutrality is the *result of* the five corrections above and not an assumption behind them:
the same table read `biggest` **0.64** and `empty` **0.40** before the gift was booked in days,
and one world in eight had collapsed into a single quarter.

Six killings in eight worlds is about one murder per settlement per lifetime. The act tally and
the death records agree on the number, which is a check worth having: they are written by two
independent paths, and a mechanism reporting five killings in a world where nobody died of
violence would be a bug in one of them. `people_do_things_to_each_other_and_the_two_counts_of_it_agree`
found exactly that, on the first run — the tally was counting gifts that had nothing behind them.

### 35.9.1 A band found a bug, which is what bands are for

§15's `ENVIRONMENT` band — the share of lifetime outcome that shared upbringing explains — is
0.20 to 0.55, and `the_bands_the_design_meets_stay_met` averages it over three worlds. It read
**0.19** and failed.

`targets` says a measurement outside a band *"is a finding, not necessarily a fault — but it
should be looked at rather than shrugged off"*. Looking at it meant running the same sheet with
`ACTS=0`, which is why that switch is now read by `balance_tests` as well as by `vitals`:

| seed | off | on, before | on, after |
|---|---|---|---|
| 0x11 | 0.36 | 0.33 | 0.44 |
| 0x21 | 0.24 | **0.12** | 0.19 |
| 0x221 | 0.16 | 0.13 | 0.15 |
| **mean** | 0.253 | **0.193** | **0.260** |

The middle column is the vocabulary, unambiguously — not seed noise, because the same three seeds
answer twice. And what it was is the scale error in §35.2: teaching wrote a person's standing into
a predictor built from place quality, injecting a strong positive shock into a quantity centred on
zero, so the upbringing predictor gained variance that did not carry through to attainment and its
correlation with the outcome fell. Correcting the scale restores the band and leaves it a little
*above* where the world was without teaching at all, which is the right direction: being taught is
an environmental input, so it ought to raise the share that upbringing explains.

Six of the seven bands move by under two points across all three columns. This one moved by six,
and it was the only thing in the project that noticed.

### 35.9.2 And a test that had been measuring the wrong thing all along

`moving_is_not_a_thing_people_do_back_and_forth` pools three worlds and asks what share of moves
went straight back. It pooled them into **one map keyed by `PersonId`** — and handles are
per-arena, so person 5 of one world and person 5 of the next are the same key, and so are their
places. Three strangers' lives were stitched into one path, and a move in the second world counted
as a return to somewhere in the first.

It read 10.5% against a bar of a tenth. The same three worlds measured a world at a time read
**4%**, which is what `vitals` had been saying all along, because `vitals` builds its map inside
the seed loop.

The test has been reporting a number nobody could have obtained any other way for as long as it
has existed, and it took a mechanism that moved churn slightly to push it over its own bar and
make anybody look. §15.1's rule was "one seed is not a measurement"; this is the other half of it
— **pooling is not free, and what pools across worlds is the rate, never the paths.**

### 35.10 What this does not have

No language, no lies, no promises. A killing has **no witness**, so nobody but the killer ever
knows who did it, and the victim's friends carry only `What::Died`. That is honest rather than
convenient — this world has no mechanism by which one person tells another a fact — and it means
an unsolved killing is the only kind there is. Giving a wrong a witness is the next thing this
vocabulary needs, and it needs a way to say something first.

## 36. What somebody is trying to get out of their life

Nobody in this world has ever wanted anything in particular. They have **needs**, which are
appetites that come back every day and are answered by eating. They have **values**, fixed at
birth, which bend how they score everything. Since §35 they have **acts**, which are things they
do to whoever is in front of them tonight. None of that is a want with a shape — a person who was
robbed at twenty and spends the next forty years making sure it cannot happen again is not
expressible in any of it.

`person::dreams` is that. Six longings, each grown from something that happened:

| | where it comes from |
|---|---|
| **a home** | not having somebody, and not having a household that is yours rather than the one you were raised in — times how overdue it is |
| **to rise** | being near the bottom of *somewhere*, sharpened by having been taken from |
| **away** | hunger, having nobody, having been wronged here, being at the bottom |
| **to be looked to** | having been taken up (§25), and having somewhere to stand |
| **to make something** | having already worked one thing out (§29) |
| **never again** | having been robbed |

### 36.1 A dream is a reading, not a field

There is no `Person::dream`. A longing is computed from what somebody carries (§34) and where
they have ended up, every time it is asked — the discipline §26.1 applies to social position,
which is read out of the state and never stored **so that it can be lost**.

That is not tidiness. A stored dream has to be *given* to somebody at some moment by some rule,
and every such rule is an author deciding what a person wants. A reading cannot be authored: it
says what a life so far adds up to, and it changes when the life does. The man who wanted a house
of his own stops wanting one the year he has it, and nothing anywhere has to remember to clear a
flag.

Measured over three worlds, 400 adults, at the end of ninety years:

```
  195 of 400 adults want something in particular  (49%)

  a home                23   11.8%        under 35  35 to 55  over 55
  to rise              103   52.8%   a home   20.9%      6.0%     8.2%
  away                   2    1.0%   to rise  40.3%     56.7%    62.3%
  to be looked to       66   33.8%   looked   35.8%     35.8%    29.5%
  to make something      1    0.5%   to       35.8%     35.8%    29.5%
  never again            0    0.0%
```

The age table is the claim being tested. Wanting a home falls from a fifth of the young to under
a tenth of the old, because they get one; wanting to rise climbs, because the ones who were going
to have risen already. Nothing schedules either of those. Half the adults want nothing in
particular, which is the right shape — a world in which everybody is driven is one in which being
driven means nothing.

### 36.2 The same mistake, a fourth time, caught before it shipped

The first version scored each longing as a product of three or four sub-unit terms. Measured
before a line of it was wired to any decision:

```
  99 of 400 adults want something in particular  (25%)

  a home                 0    0.0%
  to rise               97   98.0%
  away                   0    0.0%
  to be looked to        0    0.0%
  to make something      0    0.0%
  never again            2    2.0%
```

**Four of six never occurred to anybody at all, and a fifth was ninety-eight percent of the
rest.** That is a constant with a name, and this project has shipped two of those already —
§30.5's dead crowding term and §17.2.3's belief on a tie — both of which read as mechanisms for
months. It is the same error §35.2 records three times over, and having just written that section
did not stop me making it a fourth time. All six are now a sum of reasons times a weight from
values.

Two of the four zeroes were something worse than a scale problem:

- **`has_a_home` was true of everybody.** It asked `home_of(..).is_some()`, and everybody lives
  in a household — a child lives in its parents'. A household of your *own* is one that nobody
  who raised you is still in, and until that was the question the commonest longing there is was
  one nobody in any world ever had.
- **`friendless` was measured against one.** `2 / (allies + 1)`, which reads 0.05 for people
  carrying tens of ties, which everybody here does. Against eight, it means something.

The instrument that caught all of this — `sim/examples/what_they_want.rs` — was written and run
**before** the dreams were connected to a single decision, on the principle that the first
question about a reading is not whether it works but whether it distinguishes anybody.

### 36.3 What a dream is allowed to do

Not a `Deed`. Deeds are chosen by softmax over relative scores, so anything added to that list
re-prices eating and sleeping (§26.11). A dream weights decisions that are already scored
*outside* that softmax — at present, §35's acts:

- **to be looked to** raises giving and teaching, which is how anybody becomes a person others
  look to;
- **never again** lowers giving and raises shunning, which is the only way this world has of
  holding people at arm's length;
- **to rise** raises robbery, beside greed rather than instead of it — the difference being that
  greed is who somebody is and this is what their life has made of them.

The first wiring gave teaching `+0.7` and took it from 161 acts in eight worlds to **1,327**, past
giving and past everything else together. A dream is meant to bend what somebody was going to do
anyway; one wiring that multiplies an act eightfold is not a bend, and an eightfold sensitivity is
the kind that comes back as a calibration band six months later. It is `0.35` now.

### 36.4 What it costs

`SEEDS=8`, against `ACTS=0` — which switches dreams off too, since they act only through acts:

| | off | acts only | acts + dreams |
|---|---|---|---|
| living | 1997 | 2064 | 2077 |
| churn | 9% | 8% | **7%** |
| biggest | 0.55 | 0.55 | 0.55 |
| empty | 0.35 | 0.40 | **0.33** |
| spread | 0.11 | 0.12 | **0.14** |
| advances | 38 | 35 | 39 |
| taken up | 340 | 337 | 368 |
| acts | — | 819/161/285/88/6 | 1100/692/310/120/3 |

Dreams are the first mechanism in a while that makes the aggregates *better* rather than costing
something: churn falls to its lowest reading of the three, `empty` returns to where it was without
any of this, and `spread` — how far apart the inhabited quarters are, which §14.4 wants above zero
— goes up by a quarter. The mechanism is teaching: a longing to be looked to produces lessons,
lessons go into upbringings, and a poor quarter whose children are taught is a quarter that stays
lived in.

That was not predicted and is not what dreams were built for. It is worth stating plainly because
the alternative is to claim afterwards that it was the plan.

### 36.5 What is not wired yet

**Moving.** `away` is a longing about leaving and it does not yet move anybody, which makes it the
one entry in the table that is at risk of being right and inert — the failure mode §31.2 exists to
find. It is deliberate for now: migration is the most fragile thing in this world, two of the
three reverted mechanisms moved it first, and two of 400 adults want to leave badly enough for it
to matter. Wiring it should be its own change with its own ablation.

**Trades.** `to make something` ought to push somebody toward the trades where things get made,
and does not.

**Comparison to particular people.** Every longing here is grown from what happened to *you*.
Nobody yet wants what they saw somebody else have — which is where envy lives, and is the more
interesting half of wanting. It needs people to compare themselves to named others rather than to
a rank, and the tie graph already holds everything that would take.

## 37. Leaving, built and taken out again

§33 counted what people are to each other and found the sharpest gap in the model: **nobody
separates**. Six hundred and forty-seven pairings in one world and the only exit is a death. There
is no ceremony to dissolve and no contract to break — `seek_partner` writes one entry in a map —
so what was missing was never a legal apparatus. It was the plain fact that people leave.

It was built, it worked, and it is not in the world. What follows is the measurement, because the
reason it is out is worth more than the mechanism would have been.

### 37.1 There was plenty to fire on

Two mechanisms here have been built on triggers that could not occur: conquest keyed on adjacent
countries, of which there were **zero in any world at any size** (§32.2), and generosity keyed on a
famine in worlds whose hungriest quarter reads 0.00 (§35.7). So the question came first, in
`sim/examples/how_it_goes.rs`, which is kept. Three worlds, a hundred and twenty years:

```
  living pairings              384
  both still fond              211    54.9%
  one of the two has gone       43    11.2%
  both have                    130    33.9%

  warmth between partners: worst -0.86, tenth -0.31, middle 0.04, best 0.77
  and the ones that have gone had been together 19 years on average
```

A third of living partnerships have gone cold on both sides. But the number that matters is the
middle one: **0.04**. The ordinary pairing in this world is between two people who feel almost
nothing about each other, because `seek_partner` asks for the opposite sex, an age near enough and
not close kin, and never asks whether they like each other. Nobody had looked, because until
something depended on it there was nothing to look at.

### 37.2 What it did, which was right

Wanting out was a **sum** of grievances — how cold they have gone, what they believe the other
makes of them, what they hold against them. Being held was the **strongest single thing** holding
them, never a blend: children who still depend on somebody, having nothing to set up with, what
their people think is done, the years already spent. The same shape as `acts::nothing_to_lose`, and
it earned its place a second time.

Over eight worlds: **1,722 pairings, 35 ended by somebody walking out — 2.0%**, against the 34%
that have gone cold. *Most miserable households do not end*, because one anchor is enough and
nearly everybody has one. Remarriage fell out for free. On the aggregates it was cheap: `biggest`
0.47 against 0.55 without it, churn 8%, population unchanged.

### 37.3 What it broke, and the four fixes that each broke something else

`people_come_to_know_the_people_they_live_among` asserts friendships are mostly with neighbours.
It went from **24 distant allies to 290** in one seventy-year world — a ratio of 2.28 against a bar
of 3.

The cause was not partings. It was that **`seek_partner` searches the entire world**, and a
pairing settles in the seeker's quarter, so the person who was *found* relocates. That was
harmless for as long as pairing happened once in a life at maturity: a founding population is all
in one place, and a twenty-year-old has almost nobody to leave behind. Make pairing something that
can happen twice and the second time it happens to somebody of forty with thirty people who stand
with them — and moving them costs thirty friendships their distance.

Four fixes, each measured:

| fix | near/far | biggest | churn |
|---|---|---|---|
| *committed baseline* | 24.5 | 0.55 | 7% |
| partings, nothing else | **2.28** | 0.47 | 8% |
| pair within the same quarter | 791 | **0.63** | 10% |
| pair within reach | good | 0.56 | **10%** |
| settle where the roots are | 920 | **0.71** | 9% |
| settle where the roots are, only when stark | 31.7 | **0.65** | **10%** |

Every one traded a guard for a guard. Restricting who you can pair with seals each quarter into a
breeding population and settlement concentrates; settling couples where the deeper roots are means
nobody ever goes anywhere and it concentrates harder. §30.5's guard is 0.75 of households in one
quarter and the last two rows put a seed at 0.80.

**Four compensating changes without a green run is the signal to stop.** §26.11, §27.10, the
household head, `Deed::Host` and per-trade tools were all taken out at this point, and this is the
same point.

### 37.4 Two things that came out of it and are kept

**`seek_partner` searching the whole world is a real defect**, and it is now on the record with a
measurement attached rather than being rediscovered by whoever next makes pairing more frequent.
It cannot be fixed on its own: every version of a fix concentrates settlement, because long-range
pairing is quietly one of the main things spreading this world's people around. That coupling was
not known before.

**`spread` and §15's shared-environment share move together**, which nothing had connected. `env`
measures how much of a lifetime outcome the quarter somebody grew up in explains, so when the
quarters become more alike there is less for it to explain. Partings first failed that band at
0.19 against a floor of 0.20, and the cause was that every parting founded a household of *one* —
a singleton sorts differently from a family. Sending leavers home to kin recovered it to 0.213.
Both halves are worth carrying forward, and the margin is the thing to watch: the band sits at
0.24 without any of this, and has been falling all session.

### 37.5 A fifth change, which also looked free and also was not

Teaching charges the teacher `slip(TEACHING)` — three hundredths of their standing,
multiplicatively — on the reasoning that a day spent teaching is a day not spent working. Six
hundred lessons across eight worlds is the well-off draining and re-earning the same few
hundredths over and over: a moving quantity under a decision read afresh each year, which is
§31.1's first rule waiting to happen. Removing it is also *truer* — passing on what you know is
not giving it away, and teaching is the one act in the vocabulary where nothing leaves the person
doing it.

Removing it improves the eight-world mean: `biggest` from 0.55 to **0.49**, `empty` from 0.33 to
0.32, against churn rising from 7% to 9%. It was kept, and then the suite failed anyway — **41 of
49 households in one quarter on seed 0x221**, 0.84 against §30.5's guard of 0.75.

That is the sixth measurement in this section to say the same thing, and it is the section's real
content. **A mean over eight worlds moving the right way is not evidence that no single world
broke.** `vitals` averages; the guards do not. Every change in the table above was adopted on a
mean and reverted on a seed.

### 37.6 What is kept

Nothing of the mechanism. What is kept is `sim/examples/how_it_goes.rs`, this write-up, and three
findings that were not known before and cost a day between them:

1. **`seek_partner` searches the whole world**, and the person who is *found* is the one who
   relocates. Harmless while pairing happens once in a life; a migration pump the moment it can
   happen twice. Every fix for it concentrates settlement, because long-range pairing turns out to
   be one of the main things spreading this world's people around.
2. **`spread` and §15's shared-environment share move together**, which nothing had connected. If
   the quarters become more alike there is less for "where you grew up" to explain. The band sits
   at 0.24 and has been falling all session; it is the tightest constraint in the project now.
3. **The ordinary pairing in this world is between two people who feel nothing about each other**
   — median warmth 0.04 — because `seek_partner` never asks whether they like one another. That is
   a fault in the *pairing*, not in the leaving, and it is the thing to fix first. A world where
   people chose each other would have fewer cold households to end, and the mechanism above would
   then be measuring something other than a bad matching rule.

## 38. Two functions for one idea

§37 went looking for why a third of this world's pairings go cold and found something underneath
it. `sim/examples/how_it_goes.rs`, over ten thousand pairs of adults in a running world:

```
  two people at random: suits 0.482 (warmth aims at -0.036), compatibility 0.540
```

Those are two different numbers for the same question — *how well do these two go together* —
computed from the same five traits by two functions that nobody had ever put side by side:

- `bonds::suits`, a Manhattan gap over ten. **This is what a relationship runs on**:
  `meet_repeatedly` drives warmth toward `suits * 2 - 1`.
- `Person::compatibility`, a Euclidean distance over six. **This is what a partner is chosen
  on**: `seek_partner` shortlists eight and takes the best.

So the rule choosing a spouse maximised a quantity that read six hundredths above the one their
marriage would actually run on.

### 38.1 Unifying them, and what that was worth

It was done — one `Personality::suits`, the Manhattan version kept because every tie in the world
is already calibrated against it — and measured:

| | two functions | one |
|---|---|---|
| median warmth between partners | 0.04 | 0.05 |
| both still fond | 54.9% | 55.4% |
| one of the two gone | 11.2% | 7.7% |

**Choosing the best of eight was buying about a hundredth of warmth**, and it still is. Two
monotonically-related functions rank eight nearly-identical candidates nearly identically, so the
duplication was never producing a *wrong* choice — only one that barely mattered either way. That
is worth recording because the instinct on finding a duplicate like this is to expect the fix to
matter; measuring it says the shortlist is the inert part, not the disagreement.

Then the suite failed. §15's shared-environment share read **0.17** against a floor of 0.20, and
the change is not in the world.

### 38.2 Which is the finding

**A de-duplication worth one hundredth of warmth moved a calibration band by seven hundredths.**
There is no causal path by which it could have: it changed which of eight nearly identical people
somebody pairs with. What it changed was the *trajectory*, and the band is measured over three
worlds.

That statistic has now read **0.253, 0.260, 0.240, 0.213, 0.197 and 0.170** across this session,
and at least two of those moves had no mechanism behind them. §35.8 learned exactly this about
`biggest` and `empty` — that they swing twenty points at three seeds on a change that added
nineteen robberies — and widened `vitals` to eight worlds in response.
**`the_bands_the_design_meets_stay_met` has the same problem and has not been widened**, because
each of its worlds is a hundred and sixty founders for a hundred and twenty years and three of
them already cost ten minutes of an eighteen-minute suite.

So that band is doing two jobs and doing one of them badly. It is a real constraint — it caught a
genuine scale error in §35.9.1, where the cause was found, fixed, and the number came back. It is
also, at three seeds, capable of failing for no reason at all. **Until it is widened, a failure
there means investigate; it does not by itself mean revert.** This section is the case where that
distinction was not yet available, so the change went out rather than the band being argued with —
which is the right way round to be wrong.

Nothing about the world is fixed by any of this. What is fixed is that the next person to watch
that band fail has six readings and knows two of them were noise.

### 38.3 The number underneath, which is left alone

`suits` for two adults at random is **0.482**, so `meet_repeatedly` aims their warmth at
**−0.036**. The ordinary pair of people in this world drift toward mild dislike, and that is a
fact about a normalising constant — a gap of ten being "about as unlike as two people get" — and
not about anybody's temperament. It is most of why a third of pairings go cold.

It is **not** changed here. Centring it would move every tie in the world at once: every ally
count, every vouching at a door, every shunning, every `share_the_shortfall`, and with them
§30.5's concentration guard and §15's bands. §37 spent five compensating changes learning what
happens when a plausible-looking constant is nudged in a world this coupled, and this one is
coupled to more than any of those were.

The right way to make this world's pairings less bleak is not to centre `suits`. It is to have
people *choose each other* — pairing currently asks for the opposite sex, a near enough age, not
close kin, and the best of eight, which as measured above is worth a hundredth. That is a change
to one rule with a bounded blast radius, and it is the next thing to try.

## 39. The tie graph is directed and nothing in it is

`bonds` opens by defending its central choice:

> A tie runs *from* somebody *to* somebody and carries what one of them holds about the other.
> It has to be directed: **unrequited regard is the ordinary case**, and a model where liking is
> always mutual cannot express a hanger-on, a patron, or a grudge somebody else has forgotten.

Building the atlas's tie list meant showing, beside what she holds about him, what he holds about
her. It was going to be the best thing on the page. Measured over one world — 6,002 pairs where
both sides are on the page, 140 years, 877 living:

| | median gap | 90th | worst | over 0.25 |
|---|---|---|---|---|
| warmth | **0.000** | 0.000 | 0.006 | 0 |
| regard | 0.003 | 0.010 | 0.067 | 0 |
| known | 0.000 | 0.000 | 0.027 | 0 |
| debt | 0.000 | 0.000 | 0.000 | 0 |

**Liking in this world is always mutual.** Not usually — always, to three decimal places, in every
one of six thousand pairs. The feature was a flag that could never once have fired, and the only
reason it was not shipped that way is that the fixture went looking for the most unrequited person
in the world and found nobody at all.

### 39.1 Why, and why it was invisible

`meet_repeatedly` is the only thing that moves warmth in the ordinary case, and it steps **both
sides at once, at the same rate, toward the same target**, from a `Tie::STRANGERS` that is
identical on both sides. Symmetric inputs, symmetric rule, symmetric result. Nothing was wrong;
the asymmetry simply had no source.

The things that *could* break the symmetry are all rare or tiny. `hearsay` is directional but both
directions are called on every evening and its rate is 0.06. `helped` is asymmetric and only
famine relief calls it. `wronged` and `cut` are asymmetric and fire a few hundred times in eight
worlds against millions of meetings. Debt is *exactly* antisymmetric because one call writes both
halves.

So the graph is undirected in everything but its type, at twice the storage — and this is the
third mechanism in this project to be **right, well argued, and inert**: `CROWDING_AVERSION`
(§30.5), the belief on a tie (§17.2.3), and now the direction on all four numbers it carries.

### 39.2 What is actually asymmetric, and it is worth more

**Attention.** Everybody keeps their strongest ties and lets the rest fade, so B can be among the
people A knows best while A is not among B's. That happened for **3,343 of 9,345** ties on the
page — better than a third.

That is the real "she thinks of him more than he thinks of her", and it is the one the atlas shows.
It is a better thing than the one that was planned: an asymmetry of *feeling* would be two people
disagreeing, where an asymmetry of *attention* is one person mattering more to the other, which is
both commoner in life and sadder to read.

### 39.3 What this costs and what it is worth

Making warmth genuinely directed is not obviously desirable and is definitely not free. The
obvious route — step each side by how much *that* side is enjoying it — needs a per-side quantity
that does not exist, and §35 and §37 between them are a long demonstration of what happens when a
new asymmetry is introduced into a system this coupled.

What is cheap and honest is to stop claiming it. The doc comment above should say that the
direction carries `welcome` — the one number that genuinely diverges, and which §17.2.2 built for
exactly that — and that the other four are symmetric in practice for a reason that is structural
rather than accidental. **A comment that argues for a property the code does not have is worse
than no comment**, because it stops the next person measuring.

## 40. Somebody was standing there

§35 built a vocabulary of things people do to each other and gave it **no witnesses at all**,
and said so: *"A killing has no witness, so nobody but the killer ever knows who did it. Giving a
wrong a witness is the next thing this vocabulary needs, and it needs a way to say something
first."*

Half of that was wrong. Telling still needs a language and this does not add one. But the thing
underneath language does not need words: **somebody was standing there**.

A witness sees an act and what they think of whoever did it moves. That is `regard` — and regard
is the one number on a tie that *travels*, through `hearsay`, which has existed since §17 and had
nothing but unpaid debts to carry. So one person seeing a robbery is enough for a town to come to
think poorly of a thief, by a route that was already built.

Who is standing there is whoever is to hand that evening, capped at three. The uncapped version
averaged **nine witnesses an act**, because the list it draws from is everybody you know here plus
a dozen faces out of the crowd — which is a settlement, not a doorway.

### 40.1 How public an act is, is a property of the act

`Toward::in_the_open` runs from shunning at 0.9 to killing at 0.04. Shunning is at the top because
it is not private by definition — it is refusing somebody in front of the people you both live
among, and a shunning nobody saw is two people drifting apart. Killing is at the bottom for the
same reason it is possible at all: it is done by somebody with nothing left to lose, and being
seen is the thing they would still lose.

### 40.2 Being seen to be good is worth nothing, which was measured rather than assumed

Giving and teaching started at 0.55 — a kindness done among people is seen, obviously. It is. And
people give and teach **four times as often** as they shun or rob, so witnessed decency swamped
witnessed wrongdoing: regard drifted upward against its 2%-a-year decay, saturated near the
ceiling for everybody routinely seen being kind, and the differences between people — which is
the only thing a rank can read — went flat.

Measured, that halved how far apart the quarters of a world end up. They are zero now: **a
reputation is made of exceptions, and nearly all the exceptions are bad ones.**

Sightings went 20,988 → 5,936 (capping the witnesses) → **923** (only wrongs are news). Under one
sighting per person per lifetime, which is the right order for a thing that is supposed to be an
event.

### 40.3 And then the measurement turned out to be the story

With the mechanism down to 923 sightings in eight worlds, the aggregates still read `spread` 0.10
against a control's 0.14, and `empty` 0.43 against 0.33. Which cannot be true. Nine hundred small
nudges to regard, spread over two thousand people and ninety years, do not move a settlement
pattern by a third.

So `vitals` now reports **its own noise floor**, from the same run that reports the numbers.
Twelve worlds, one unchanged build:

```
biggest   sd 0.159  se 0.046   worlds: 0.53 0.46 1.00 0.50 0.62 0.44 0.55 0.51 0.41 0.73 0.52 0.72
empty     sd 0.172  se 0.050   worlds: 0.60 0.20 0.80 0.40 0.40 0.40 0.20 0.40 0.20 0.40 0.60 0.40
spread    sd 0.057  se 0.016   worlds: 0.12 0.13 0.00 0.12 0.10 0.11 0.16 0.09 0.17 0.17 0.03 0.01
```

**One world in twelve puts every household in a single quarter.** Another has quarters that do
not differ at all. At eight worlds the standard error on `spread` is 0.020, so the witness
mechanism's apparent cost of 0.04 is **1.4 σ**, and `empty`'s 0.10 is 1.2 σ. Neither is a finding.
Neither ever was.

This is the third time this session the same thing has happened — §35.8 for `biggest` and `empty`
at three seeds, §38.2 for §15's shared-environment band, and now for the same statistics at
*eight*. The pattern is worth naming: **every one of these numbers has been used to accept or
reject a mechanism, and none of them had ever had its noise floor measured.** Widening the sample
was treated as the fix twice; it is not the fix, it is a way of making the error smaller than
whatever you are looking for, and that requires knowing how big the error is.

An instrument that does not report its own precision is not an instrument. It is a number.

### 40.3.1 And one thing that was not noise

The suite still failed, on the guard that asserts on **each** world rather than the mean: seed
0x221, 49 of 49 households in one quarter. Run against the guard's own fixture with the mechanism
switched off, that seed reads **0.73** — a hair under the 0.75 bar before any of this existed.
Witnesses tipped it to 1.00.

The lever turned out not to be the opinion at all. `saw` raised `known` toward `HEARD_OF` by a
tenth of the gap, on the reasonable grounds that a regard hung on a stranger gets swept away by
`year` — and **making somebody known puts them in the list your evenings are drawn from**, which
changes who you meet, which changes everything downstream of who you meet. At three hundredths
instead of a tenth, and the opinion itself halved, seed 0x221 goes back to 0.73 and the eight-world
means land on 0.54 / 0.33 / 0.14 against a control's 0.55 / 0.33 / 0.14.

The mechanism still fires 948 times. What was removed was not its effect; it was a side channel
nobody had counted, doing more than the thing it was attached to.

### 40.4 What that says about the rest of this document

Not everything, but not nothing. Differences of 0.15 and up in `biggest` are three standard
errors at eight worlds and stand: §37's "settle where the roots are" really did drive
concentration from 0.55 to 0.71. Differences under about 0.10 do not, and several judgements in
§35 to §38 rest on differences that size.

The per-seed guards are unaffected and remain the sharper tool — `a_world_does_not_end_up_in_one_quarter`
asserts on **each** world rather than the mean, and a single world at 0.80 is a fact about that
world whatever the spread between worlds is. That is why the reverts in §37 were right even where
the means that prompted them were not significant: the guard that actually failed was a per-seed
one.

### 40.4.1 And with the noise floor known, what witnesses actually cost

Twelve worlds, `WITNESS=0` against on, every difference beside its own standard error:

| | off | on | difference |
|---|---|---|---|
| living | 2966 | 2941 | — |
| churn | 10% | 12% | **0.7 σ** |
| biggest | 0.56 | 0.56 | — |
| empty | 0.35 | 0.33 | **0.4 σ** |
| spread | 0.13 | 0.14 | **0.4 σ** |

**Nothing.** Not "a small cost worth paying" — nothing distinguishable, on any line, and now with
the arithmetic to say so rather than a shrug. The mechanism fires 948 times in twelve worlds and
the world it fires in is the same world.

Getting there took two corrections that the noise floor could *not* have excused, and it is worth
separating them. Capping witnesses at three and making kindness unremarkable were both real: the
first cut sightings from 20,988 to 5,936 and the second to 923, and a mechanism firing twenty
times per act is wrong whatever the aggregates say. But the third correction — halving what a
witness makes of it and cutting the tie it creates by a factor of three — was prompted by
`a_world_does_not_end_up_in_one_quarter` failing on seed 0x221, and **that seed reads 0.73
against a 0.75 bar with the mechanism switched off**. It was a hair from failing on its own.

The correction was still right, and for a reason the failure did not name: the dominant lever was
never the opinion, it was the *tie*. Seeing somebody made you know them, and knowing somebody puts
them in the list your evenings are drawn from — which changes who you meet, which changes
everything downstream of who you meet. Cutting that to a third put seed 0x221 back to exactly the
0.73 it reads without any of this.

### 40.5 §15's band, quantified at last

The same treatment, over eight worlds:

| seed | 0x11 | 0x21 | 0x221 | 0x31 | 0x41 | 0x5ee | 0x77 | 0x8a |
|---|---|---|---|---|---|---|---|---|
| shared environment | 0.35 | 0.17 | 0.20 | 0.30 | 0.19 | 0.15 | 0.17 | 0.32 |

Mean **0.231**, σ ≈ 0.075. The floor is 0.20, so at the three seeds the test actually uses the
standard error is **0.043** and the margin is 0.7 of one — which puts the chance of failing on an
unchanged build at roughly **one run in four**.

That is not a test. It is a coin that comes up heads three times in four, and it has been read all
session as though a failure meant something. §38.2 guessed this from six readings; this measures
it. The band itself is fine — it caught a genuine scale error in §35.9.1 where the cause was found
and the number came back. What is broken is the sample size, and now the size of the problem is
known rather than argued about.

## 41. What a life does to somebody

Personality here is fixed at maturity. `origins` splits each of the five factors into genes,
household and chance, `mature()` seals it, and from twenty onward **nobody in this world is ever
changed by anything that happens to them**. A man robbed at thirty has precisely the temperament
at sixty that he would have had otherwise. His memory changes; he does not.

That is the largest single reason a soul here reads as a set of dispositions rather than a life,
and the material to fix it was already sitting there doing nothing: §34's memory is exactly *what
happened to somebody*, decaying on a curve, and it drove nothing but a few appetites.

### 41.1 Beside the origins, not inside them

`Person::weathering` is a second `Personality`, kept alongside `origins` rather than folded into
it, and the two answer different questions:

- `origins.total()` — the person who finished growing up.
- `personality` — the person now.
- `weathering` — what the years did, which is the difference.

Folding it in as a fourth channel of `Expression` would have made *"why is she like that"* and
*"what has happened to her"* the same question. §15's entire apparatus rests on the first being
answerable about a fixed endowment, and the counterfactual it supports — "the same person raised
somewhere else" — is meaningless once the endowment moves.

### 41.2 What moves it, and what does not

It reads what somebody carries and **nothing else**: not their standing, not their situation, not
what anybody thinks of them. That restriction is the design rather than an economy. A person is
changed by what happened to them, and what happened to them is exactly what a memory is; a version
keyed on circumstance would make temperament a lagging indicator of wealth.

Five claims, one line each:

| | |
|---|---|
| being wronged **hardens** you | neuroticism up, agreeableness down |
| being carried **softens** you | somebody fed you through a bad year |
| being taken up **opens** you | §25's largest fact is also somebody deciding you were worth something |
| burying people **wears** you | the heaviest memory in the model |
| a wrong you did yourself | costs agreeableness — conscience in a temperament, not only in a restraint |

**Conscientiousness is deliberately untouched.** It is the trait attainment runs on, and letting a
life move it would turn §15's decomposition into a measure of luck wearing the name of a
temperament. What a life does to somebody here shows up in how they are with people.

### 41.3 It is not a ratchet

The step is a twelfth of the way to a target each year, and the target is computed from memory
*as it stands now*. So a bad decade moves somebody and then, as §34's curve lets the memory go,
carries them back toward who they were. **What the forgetting forgets, this un-learns.** Nothing
anywhere resets it; the same one line does both.

Measured over twelve worlds: a life moves each trait by **0.095** on average and the most
weathered person alive by **0.35** — a tenth of a standard deviation for the ordinary life and a
third for the hardest. That is the right order. Half a standard deviation would have made
temperament a second name for biography; a hundredth would have been a mechanism that fires and
does nothing.

### 41.4 What it costs

Twelve worlds, against the same build with `CHANGE=0`, every difference beside its standard error:

| | before | with | |
|---|---|---|---|
| living | 2941 | 2997 | — |
| churn | 12% | 9% | 1.3 σ |
| biggest | 0.56 | 0.54 | 0.4 σ |
| empty | 0.33 | 0.35 | 0.4 σ |
| spread | 0.14 | 0.13 | 0.5 σ |

Nothing over one and a half standard errors. It is the second mechanism in a row to come out free
at the aggregate — which is what §40.3's arithmetic is *for*: before it, "churn went from 12% to
9%" would have been three paragraphs of speculation about why, and it is worth nothing at all.
