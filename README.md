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

Early. The workspace has three crates — `planet`, `person`, `main` — running a single
planet and a single person, each on a hand-written state machine, printing to stdout.

```
cargo run -p main
```

Next up is Phase 0 of the design: a handle-based `World`, seeded RNG, and the
time-scale ladder. It removes the borrowed references that currently make families
impossible to represent, and puts in the scheduling foundation that deep time needs.

## Roadmap

Four milestones, detailed in [`docs/DESIGN.md` §19](docs/DESIGN.md):

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
