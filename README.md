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

**Phases 0 (foundations), 1 (person depth), 2 (genetics and families) and
3 (neighbourhoods) are in.**

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
    genes          0.16  within target
    upbringing     0.16  outside 0.20–0.55
    entangled      0.23  inseparable — parents supply both
    luck           0.46  outside 0.15–0.45
  elasticity       0.62  outside 0.20–0.50
  siblings         0.47  within target
  mobility         0.67  within target
  upbringing gap   1.49  outside 0.30–1.20
```

Genes and upbringing come out *exactly* balanced — 0.39 each, counting the entangled
share on both sides. That entangled fifth is reported rather than divided up: parents
supply both genes and neighbourhood, so the split is genuinely ambiguous and saying so
is more honest than a false precision.

Elasticity is above target — this world is less mobile than intended. Worth knowing
*why*: moving the transfer-at-birth constant across a 2.75× range barely shifts it.
Advantage travels through the neighbourhood a child grows up in, not through what they
inherit. So the fix is §14.4's escape routes — bridging ties, mentors, schooling shocks
— which aren't implemented yet, not a tuning dial.

Next is Phase 4: the geodesic grid and tectonics beneath these places.

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
