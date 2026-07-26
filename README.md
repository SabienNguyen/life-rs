# life-rs

A simulation of a world — from plate tectonics and ocean currents down to one person's
Tuesday afternoon.

**Watch a planet evolve over millions of years:** continents drifting and colliding,
oceans opening, ice ages arriving and retreating, life radiating and going extinct and
radiating again. **Then stop anywhere in that history and zoom all the way in** — pick
a random person out of the living population and see everything about them: what they
want, what they are doing right now and why, their family, everything notable that has
happened to them since birth, and why they turned out that way — which traits came from
their parents and which from the place they grew up.

Every new world is different. Nothing is scripted: extinctions happen because CO₂
spiked and the oceans went anoxic, deserts form where the moisture doesn't reach, and
species adapt because the ones that didn't left fewer descendants.

**→ [`docs/DESIGN.md`](docs/DESIGN.md) is the architecture plan** — the time-scale
ladder that makes megayears and individual lives coexist, tectonics and climate,
biomes and ecology, genetics and evolution, neighborhood effects on behavior, the
omniscient observer API, and the phased roadmap from here to there.

## Status

**Phases 0 (foundations), 1 (person depth), 2 (genetics and families),
3 (neighbourhoods) and level-of-detail are in.**

```
cargo run -p main                                   # a new world, three days
cargo run -p main -- --seed 0x2b --days 5           # replay a specific world
cargo run -p main -- --days 1 --dossier             # end with a close look at someone
cargo run -p main -- --people 200 --years 60 --min pivotal
```

```
[yr 100 day 1   00:00] Hi! My name is Dr. Armando Lang and I am from United States.
                       I am 35 years old, and optimistic.
[yr 100 day 1   08:00] Dr. Armando Lang is eating
[yr 100 day 1   10:45] Dr. Armando Lang is working

why they are doing that:
  sleeping     1.035
  washing      0.022
  drinking     0.020
  ...
```

**Phase 0** — `sim-core`: generational handles and arenas, seeded splittable randomness,
the time-scale ladder from a 15-minute tick to a megayear, a future-event scheduler, the
chronicle. `sim` owns a handle-based `World`.

- **No borrowed references.** A person holds `home: PlanetId`, not `&Planet`. Cycles —
  families, food webs — are representable.
- **Reproducible worlds.** Every draw descends from the world seed; nothing calls
  `thread_rng()`. Same seed, same world, down to the transcript. A new world draws fresh
  entropy and is genuinely different.
- **Time that reaches deep.** Integer simulated seconds, no drift over 1e13 steps. Day
  phase is *derived* from the clock, so it cannot fall out of step. An empty world
  crosses a million years instantly.
- **Nothing is polled.** ~7.8M events/sec on one core.

**Phase 1** — `life`: needs, aging, health, Siler-model mortality. `person`: OCEAN
personality, values, and utility-scored behaviour replacing the state machine.

- **Needs are continuous and lazy.** Computed from time elapsed since anyone last
  looked, so a dormant population costs nothing.
- **Behaviour is scored, not branched.** Every option is priced by need × temperament ×
  circadian × the four environment channels, then chosen by softmax. The prices are
  kept, so the simulation can show its working.
- **Personality is an output.** Five continuous traits; `Outlook::Pessimistic` is a
  label read off the vector, not a stored fact. Phase 2 replaces the sampling with
  inheritance from a genome.
- **People age and die.** Competing juvenile / baseline / senescent hazards, so life
  tables have the right bathtub shape — median lifespan ~74, and a closed founding
  population thins out mostly of old age.

**Phase 2** — `genetics`: 256 biallelic loci as bitsets (a genome is 64 bytes),
pleiotropic trait architecture, meiosis with crossover and mutation. `society`:
households, partnerships, and kinship stored as parent edges with everything else
derived.

- **Personality is inherited.** Computed from a genome, a household, and chance, with
  the three kept apart — so a dossier can say which did what, and "what if they'd been
  raised elsewhere" is a substitution rather than another lifetime.
- **Regression to the mean falls out.** Parents pass on half their alleles, not their
  phenotype. Selecting on phenotype recovers a slope near the heritability, by
  construction rather than by a fudge factor.
- **Siblings resemble each other twice over** — half their variable alleles, and the
  same household term.
- **Populations sustain themselves.** Pairing is assortative, close kin are excluded,
  and births now offset deaths where Phase 1 could only decline.
- **A guardrail, as a test.** Founder populations differ at physical loci only;
  behavioural loci draw from one shared pool. Asserted in `genetics/src/pool.rs` so it
  cannot rot.

```
where their temperament came from:
  openness           -0.68   = genes +0.66  home -0.81  chance -0.52
  conscientiousness  +0.43   = genes +0.43  home -0.81  chance +0.81

  raised badly, conscientiousness would be +0.57; raised well, +1.91 (is +0.43)
```

**Phase 3** — `society::place`: neighbourhoods whose character is *read off their
residents*, and the four channels by which a place changes what people do.

- **Nothing is authored.** Quarters start identical and unremarkable. Which becomes the
  enclave and which the slum comes out of who ends up living there — affluence is what
  residents have, churn is how often they leave, norms are literally what they did.
- **Archetypes are readings.** Distressed urban, working-class, suburb, metropolitan
  core, affluent enclave, rural — the nearest label to a point in the vector space, so
  a neighbourhood can *become* something else.
- **Community and reach are separate.** A poor neighbourhood is not socially empty, it
  is socially enclosed: bonding capital comes from staying put *or from needing each
  other*, bridging capital needs means. Collapsing them into one number loses the
  mechanism that actually limits mobility.
- **Where you grow up sticks.** Childhood exposure accumulates age-weighted and freezes
  at twenty, so a move at forty barely registers and the street someone was raised on
  stays legible in them.
- **Sorting is real.** Households move to the best place that will *have* them, and
  housing scarcity is what excludes.

```
  place        reads as            afflu safety   bond bridge   jobs hholds
  Northside    working-class        0.14   0.23   0.87   0.18   0.40    31
  The Wharf    working-class        0.43   0.35   0.74   0.51   0.65    89
  Elmhurst     rural                0.03   0.25   0.93   0.07   0.31     0
```

**The balance harness** (`observer`) answers the question the whole design is built
around — is this a story about inheritance or about circumstance?

```
cargo run -p main -- --people 90 --years 130 --min pivotal --balance --quiet

  measured over 342 lives
  outcome variance
    genes          0.18  within target
    upbringing     0.13  outside 0.20–0.55
    entangled      0.24  inseparable — parents supply both
    luck           0.46  outside 0.15–0.45
  elasticity       0.55  outside 0.20–0.50
  siblings         0.38  within target
  mobility         0.70  within target
  upbringing gap   1.01  within target
```

Counting the entangled share on both sides — genes 0.42, circumstance 0.37 — neither
decides a life. That entangled quarter is reported rather than divided up: parents supply
both genes and neighbourhood, so the split is genuinely ambiguous, and saying so beats a
false precision.

**Escape routes** (§14.4) are what keep it from being deterministic doom: unearned
windfalls and ruins, young adults who will uproot for work and are taken in more readily
because they are renting a room rather than buying a house, and patrons who open doors.
Patronage runs on *bonding* capital, not bridging — bridging ties belong to the already
comfortable, so a way out routed through them would only widen the gap. Dense
mutual-dependence community is what a poor neighbourhood actually has, and it is what
makes such places produce people who get out.

**A frontier, not an optimum.** An escape route works by decoupling where someone ends up
from where they began, so anything that lowers elasticity also lowers how much upbringing
can explain and raises what is left to chance:

| escape routes | elasticity | genes | circumstance | luck |
| --- | --- | --- | --- | --- |
| off | 0.62 | 0.39 | 0.39 | 0.46 |
| **as shipped** | **0.55** | **0.42** | **0.37** | **0.46** |
| stronger | 0.40 | 0.41 | 0.15 | 0.55 |
| stronger still | 0.33 | 0.39 | 0.07 | 0.59 |

§15 wants elasticity 0.20–0.50 *and* circumstance near 0.40 *and* luck near 0.30. This
model cannot give all three at once. The shipped values buy the design's central claim —
that neither cause decides a life — and leave elasticity and luck each a little outside
their bands, which the harness reports rather than hides.

**Level of detail** (§6, pulled forward): people in places nobody is watching stop
deliberating every few hours and live a year at a time instead.

```
300 people, 150 years
  every place watched   52.7 s
  none watched           0.7 s     — 73x
```

The coarse year is a *projection*, not an approximation: work compounds in closed form,
so a population simulated coarsely lands where the same population simulated finely
lands. That equivalence is a test, because a world that quietly changes while you are
not looking is one you cannot trust when you look back at it. `--detail 0` coarsens
everything; `World::watch(place)` brings a neighbourhood back into focus and its people
resume deliberating within the day.

One known cost, measured rather than hidden: a coarsely lived year keeps needs where a
competent adult maintains them, so nobody unwatched ever has a bad month. Health runs
slightly higher, and through the fertility check that means slightly more childbearing —
about a fifth more population over 150 years.

### The join

Those neighbourhoods now stand on a real planet. Founding a world draws continents from
plate motion, solves a climate against them, reads the biomes off the result, and then
puts the quarters where somebody could actually live — one country's worth of ground, and
each quarter named after what it stands on:

```
── the planet under them ──
  35% land, 100% of it in one mass, 7 plates, highest point 2054 m
  18.0 °C on average, 5910 ppm carbon dioxide, 6% under ice, 1206 mm of rain a year

── neighbourhoods ──
  Twyport       working-class        afflu 0.41  safety 0.40  jobs 0.53
                └ savanna at 30°N 69°E, 77 m, soil 53%, reach 72%
  Shawtor       rural                afflu 0.24  safety 0.26  jobs 0.44
                └ desert at 32°N 90°E, 1834 m, soil 16%, reach 51%
```

Nothing there is authored. The ground bounds what work there is, whether anyone passes
through, and how hard the year is — and then who lives there decides the rest.

Next is people at deep-time resolution: the planet under a populated world is still a
single frame, because a megayear is thirty thousand lifetimes.

## Roadmap

Four milestones, detailed in [`docs/DESIGN.md` §20](docs/DESIGN.md):

1. **A world that lives** — handle-based world, scale ladder, person depth, genetics, families
2. **A world that has places** — geodesic grid, tectonics, climate, oceans, biomes, ecology, neighborhoods
3. **A world you can watch** — chronicle, observer API, TUI, level-of-detail
4. **A world with history** — evolution, speciation, deep time, globe rendering, continuous zoom

The original Phase 1 notes are kept below as the record of where the project started.

### Phase 1: Static World and People
This phase should allow the creation of a world with livable properties that contain people with detailed properties. At this stage there should be no interactions between the world and the people, aside from the acknowledgement from the people that they exist on that world. The idea in this phase is that people act like they live in a box, while they desire to eat and sleep, they do not have the desire or the ability to talk or interact with anything else.

- what does persons need to have
	- name
	- to feel/know when they are hungry
	- to feel/know when they are tired
	- some additional properties relate to occupation
	- gender
	- country of origin
	- ethnicity
- What does the planet need to have
	- Planet keeps track of time of day/clock
