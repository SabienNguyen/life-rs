use planet::Planet;
use rand::distributions::{Distribution, Standard};

#[derive(Debug)]
pub struct Animal<'a> {
    classification: Classification,
    planet: &'a Planet,
}

#[derive(Debug)]
pub enum Classification {
    Mammal,
    Reptile,
    Bird,
    Amphibian,
    Fish,
}

impl<'a> Animal<'a> {
    pub fn new(classification: Classification, planet: &'a Planet) -> Animal<'a>{
        Animal {classification, planet }
    }
}

impl Distribution<Classification> for Standard {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> Classification {
        match rng.gen_range(0..=4) {
            0 => Classification::Mammal,
            1 => Classification::Amphibian,
            2 => Classification::Bird,
            3 => Classification::Reptile,
            4 => Classification::Fish,
            _ => unreachable!(),
        }
    }
}

pub fn generate<T: rand::Rng>(mut r_thread: T, planet: &'_ Planet) -> Animal<'_> {
    Animal {
        classification: rand::random(),
        planet,
    }
}