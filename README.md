# life-rs

A simulation of a universe — planets, environments, animals, and people with
genetics, personalities, families, and lives you can inspect one at a time.

The long-term goal: at any moment, pick a random person out of the whole living
population and see everything about them — who they are, what they want, what they
are doing right now and why, their family and relationships, everything notable that
has happened to them since birth, and *why they turned out that way*: which traits
came from their parents and which from the place they grew up.

**→ [`docs/DESIGN.md`](docs/DESIGN.md) is the architecture plan** — entity model,
genetics, environment and neighborhood effects, level-of-detail scaling, the
omniscient observer API, and the phased roadmap from here to there.

## Status

Early. The workspace has three crates — `planet`, `person`, `main` — running a single
planet and a single person, each on a hand-written state machine, printing to stdout.
Phase 0 of the design (handle-based `World`, seeded RNG, tick scheduler) is the next
step; it removes the borrowed references that currently make families impossible to
represent.

```
cargo run -p main
```

## Roadmap of Development

Detailed phases live in [`docs/DESIGN.md` §12](docs/DESIGN.md). The original Phase 1
notes are kept below as the record of where the project started.

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
