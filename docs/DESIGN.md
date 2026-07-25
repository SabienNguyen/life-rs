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
ocean/       Basins, currents, temperature/salinity, upwelling, sea level
biome/       Derived terrestrial + marine classification from climate & substrate
life/        Shared substrate: Organism, Phenotype, Needs, aging, death
genetics/    Loci, trait specs, meiosis, phenotype expression, allele-frequency pools
ecology/     Vegetation fields, animal demes, trophic web, dispersal, disturbance
evolution/   Selection, drift, mutation, gene flow, speciation, extinction, phylogeny
person/      Humans: identity, personality, skills, memory, intent
society/     Households, kinship, places, environment vectors, settlements, economy
chronicle/   Append-only event log, indices, compaction, biography & history assembly
observer/    Read-only query API at every scale
sim/         Systems + scheduler; owns `World`
cosmos/      Systems, stars, orbits (thin for a long time)
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
| **5** ◑ | `climate`: energy balance, insolation, moisture, ice, carbon cycle. Ocean circulation — gyres, overturning, upwelling, nutrients — deferred to Phase 7, where something finally reads it |
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
| **12** ◑ | Deep-time integration: adaptive stepping ✓ (the lithosphere subdivides its own step so no plate ever jumps a cell), supercontinent cycle ✓, orbital forcing ✓ as machinery — obliquity varies on its 41 kyr cycle, which megayear steps cannot resolve and honestly return the mean of. Keyframing and a named mass-extinction mechanism are not built; extinction pulses do emerge from climate shocks |
| **13** ✗ | Globe + phylogeny rendering (`wgpu`), timeline scrubbing, continuous zoom. **Superseded.** The self-contained HTML viewer does the job better in every way that matters here — five layers, a time scrubber, hover readout, and a file anybody can open — and it is what actually found four of the modelling bugs in Phase 4. A `wgpu` client would be a second renderer for the same data |
| **14** | Economy, culture, technology; `cosmos`; save/load; profiling. Determinism goldens exist in the form that survives: every subsystem tests that the same seed produces the same result, self-comparing rather than pinned to a constant, so a legitimate change does not require re-blessing a hash |

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

**What is still missing is the join.** People live in abstract neighbourhoods and the
planet has no people on it. Giving a place a grid cell is the remaining piece of Phase 12
and it is a small change to make and a large one to get right: the two halves run at
scales eleven rungs apart, and connecting them means deciding what a person is when the
clock is striding a megayear at a time. The level-of-detail machinery in §6 exists for
exactly that and has not yet been asked to do it.

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
  tested hardest.
- Graph invariants: kinship acyclic, no parent born after a child, every id resolves.

**Physical**
- Earth-like inputs → Earth-like outputs: land fraction, latitudinal temperature
  gradient, biome area distribution, precipitation in the right places.
- Conservation: mass, energy, water, and carbon balance to tolerance over 1 Myr.
- Milankovitch response: glacial cycles at the right periods under orbital forcing.
- Stability: 100 Myr with no NaN, no runaway, no dead planet from numerical drift.

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
6. **How much culture?** Distinct languages, naming, kinship norms, and religions add
   enormous texture per line of code, but they need their own evolutionary dynamics —
   transmission, drift, and selection over ideas rather than genes. Tempting, and it's
   a whole subsystem.
