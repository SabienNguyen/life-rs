use std::fmt::{self}; 
use rand::distributions::{Distribution, Standard};
use faker_rand::en_us::names::FullName;
use planet::Planet;

#[derive(Debug)]
pub enum Height {
    Short,
    Average,
    Tall,
}

#[derive(Debug)]
pub enum Ethnicity {
    Hispanic,
    African,
    Asian,
    White,
    PacificIslander,
    Indigenous,
}

#[derive(Debug)]
pub enum HairColor {
    Black,
    White,
    Brown,
    Blonde,
    Silver,
    Red,
}

#[derive(Debug)]
pub enum Weight {
    Underweight,
    Normal,
    Overweight,
}

#[derive(Debug)]
pub enum Country {
    Usa,
    Gbr,
    Deu,
    Can,
    Fra,
    Chn,
    Jpn,
    Vnm,
}

impl fmt::Display for Country {
    fn fmt(&'_ self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Country::Can => write!(f, "Canada"),
            Country::Chn => write!(f, "China"),
            Country::Deu => write!(f, "Germany"),
            Country::Fra => write!(f, "France"),
            Country::Gbr => write!(f, "United Kingdoms"),
            Country::Jpn => write!(f, "Japan"),
            Country::Usa => write!(f, "United States"),
            Country::Vnm => write!(f, "Vietnam"),
        }
    }
}

#[derive(Debug)]
pub enum Outlook {
    Optimistic,
    Pessimistic,
    Realist,
}

#[derive(Debug)]
pub struct PhysicalAttrs {
    pub weight: Weight,
    pub height: Height,
}

#[derive(Debug)]
pub struct Personality {
    pub outlook: Outlook,
    pub confident: bool,
}

#[derive(Debug)]
pub struct Person<'a> {
    pub name: String,
    pub country: Country,
    pub planet: &'a Planet,
    pub physical: PhysicalAttrs,
    pub personality: Personality,
}

impl PhysicalAttrs {
    pub fn new(weight: Weight, height: Height) -> PhysicalAttrs {
        PhysicalAttrs { weight, height }
    }
}

impl Personality {
    pub fn new(outlook: Outlook, confident: bool) -> Personality {
        Personality { outlook, confident }
    }
}

impl<'a> Person<'a> {
    pub fn new(
        name: String,
        height: Height,
        country: Country,
        weight: Weight,
        outlook: Outlook,
        confident: bool,
        planet: &'_ Planet,
    ) -> Person<'_> {
        Person {
            name,
            country,
            physical: PhysicalAttrs::new(weight, height),
            personality: Personality::new(outlook, confident),
            planet,
        }
    }

    pub fn introduction(&'_ self) {
        println!(
            "Hi! My name is {} and I am from {} and live on the planet {}",
            self.name, self.country, self.planet.name,
        );
    }

    pub fn says(&self, words: &str) {
        println!("{} says \"{}\"", self.name, words);
    }
}

impl Distribution<Country> for Standard {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> Country {
        match rng.gen_range(0..=7) {
            0 => Country::Can,
            1 => Country::Chn,
            2 => Country::Deu,
            3 => Country::Fra,
            4 => Country::Gbr,
            5 => Country::Jpn,
            6 => Country::Usa,
            7 => Country::Vnm,
            _ => unreachable!(),
        }
    }
}

impl Distribution<Weight> for Standard {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> Weight {
        match rng.gen_range(0..=2) {
            0 => Weight::Underweight,
            1 => Weight::Normal,
            2 => Weight::Overweight,
            _ => unreachable!(),
        }
    }
}

impl Distribution<Height> for Standard {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> Height {
        match rng.gen_range(0..=2) {
            0 => Height::Short,
            1 => Height::Average,
            2 => Height::Tall,
            _ => unreachable!(),
        }
    }
}

impl Distribution<Outlook> for Standard {
    fn sample<R: rand::Rng + ?Sized>(&self, rng: &mut R) -> Outlook {
        match rng.gen_range(0..=2) {
            0 => Outlook::Optimistic,
            1 => Outlook::Pessimistic,
            2 => Outlook::Realist,
            _ => unreachable!(),
        }
    }
}

pub fn generate<T: rand::Rng>(mut r_thread: T, planet: &'_ Planet) -> Person<'_> {
    Person {
        name: rand::random::<FullName>().to_string(),
        country: rand::random(),
        physical: PhysicalAttrs::new(rand::random(), rand::random()),
        personality: Personality::new(rand::random(), r_thread.gen_bool(0.5)),
        planet: &planet,
    }
}
