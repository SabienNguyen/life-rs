use std::fmt::{self};

#[derive(Debug)]
pub enum Height {
    Short,
    Average,
    Tall,
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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
pub struct Person {
    pub name: String,
    pub country: Country,
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

impl Person {
    pub fn new(
        name: String,
        height: Height,
        country: Country,
        weight: Weight,
        outlook: Outlook,
        confident: bool,
    ) -> Person {
        Person {
            name,
            country,
            physical: PhysicalAttrs::new(weight, height),
            personality: Personality::new(outlook, confident),
        }
    }

    pub fn introduction(&self) {
        println!(
            "Hi! My name is {} and I am from {}",
            self.name, self.country
        );
    }

    pub fn says(&self, words: &str) {
        println!("{} says \"{}\"", self.name, words);
    }
}
