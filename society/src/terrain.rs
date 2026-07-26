//! What the ground under a neighbourhood is, as against what the people on it made of it.
//!
//! Everything else about a place is derived from its residents and moves with them: a
//! quarter gets richer because the people in it did, safer because it got richer, crowded
//! because more of them arrived. Terrain is the part that does not. A settlement on a
//! frozen plateau can be as industrious as it likes and there will still be nothing to
//! grow, nobody passing through, and a hard winter every year.
//!
//! Four numbers, and they are deliberately few. The physical world upstream knows a great
//! deal more — elevation, crustal thickness, sediment depth, net primary production,
//! which of fifteen biomes this is — but almost none of that is a fact a person's life
//! turns on. What a life turns on is whether the land feeds you, whether anyone can reach
//! you, how hard the year is, and how many of you the place will hold. So the join
//! between the planet and the people is four numbers wide, and the projection down to
//! them happens once, in `settlement`, where the planet is still in scope.
//!
//! Kept here rather than in the crate that computes it so that `society` needs no
//! knowledge of grids, plates or climates — a place that is not on a map simply has no
//! terrain, and everything works exactly as it did before.

/// The physical facts of where a place sits.
#[derive(Clone, Debug, PartialEq)]
pub struct Terrain {
    /// Which cell of the planet's grid this place stands on.
    pub cell: u32,
    pub latitude: f32,
    pub longitude: f32,
    /// Height above sea level, in metres.
    pub elevation_m: f32,
    /// What the land grows, 0 to 1, against the most productive land there is.
    ///
    /// The ceiling on how much wealth can come out of the ground, which is most of it
    /// for anyone without an economy — and this world has no economy yet, so it is all
    /// of it.
    pub fertility: f32,
    /// How easily the rest of the world gets here, 0 to 1.
    ///
    /// Coast, gentle ground, and neighbours worth reaching. This is the term that
    /// separates a port from a valley with the same soil: ties *out* need somewhere to
    /// go, and a place nobody passes through has none however rich it is.
    pub reach: f32,
    /// How hard the place is to live in at all, 0 to 1.
    ///
    /// Cold, drought and altitude. Not the same as infertility — a rice terrace at
    /// altitude and a desert oasis are both productive and both punishing — and it acts
    /// on safety and on the pressure a resident carries rather than on their income.
    pub harshness: f32,
    /// Households the land will carry, before anything is imported.
    pub carrying: u32,
    /// What grows here, for reading rather than for arithmetic.
    pub biome: &'static str,
}

impl Terrain {
    /// Somewhere unremarkable that is nevertheless on a map — the fixture tests want.
    pub fn middling(cell: u32) -> Terrain {
        Terrain {
            cell,
            latitude: 0.0,
            longitude: 0.0,
            elevation_m: 100.0,
            fertility: 0.5,
            reach: 0.5,
            harshness: 0.0,
            carrying: u32::MAX,
            biome: "grassland",
        }
    }

    /// The ceiling terrain puts on how well off a place can be.
    ///
    /// Not a floor and not a target: somewhere fertile is *allowed* to be rich and does
    /// not thereby become rich. What this rules out is the thing that made geography
    /// decorative — a quarter on bare rock accumulating wealth because its residents
    /// happened to score well, with the land it stands on never once objecting.
    pub fn prosperity_ceiling(&self) -> f32 {
        // Reach counts for a third of it, which is roughly how much of the historical
        // record is ports and river mouths outperforming their soil.
        (0.25 + 0.5 * self.fertility + 0.25 * self.reach).clamp(0.0, 1.0)
    }

    /// What the land does to the pressure of living here, before anyone else does
    /// anything.
    pub fn hardship(&self) -> f32 {
        self.harshness.clamp(0.0, 1.0)
    }

    /// A short description of where this is, for the observer.
    pub fn describe(&self) -> String {
        let ns = if self.latitude >= 0.0 { 'N' } else { 'S' };
        let ew = if self.longitude >= 0.0 { 'E' } else { 'W' };
        format!(
            "{} at {:.0}°{ns} {:.0}°{ew}, {:.0} m",
            self.biome,
            self.latitude.abs(),
            self.longitude.abs(),
            self.elevation_m,
        )
    }
}
