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

**Phases 0 (foundations) and 1 (person depth) are in.**

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

Next is Phase 2: genetics and families — polygenic inheritance, households, kinship,
birth, and a population that sustains itself instead of only declining.

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
