use faker_rand::en_us::names::FullName;
use person::{Country, Height, Outlook, Person, Personality, PhysicalAttrs, Weight};
use rand::distributions::{Distribution, Standard};
mod person;

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

pub fn generate<T: rand::Rng>(mut r_thread: T) -> Person {
    Person {
        name: rand::random::<FullName>().to_string(),
        country: rand::random(),
        physical: PhysicalAttrs::new(rand::random(), rand::random()),
        personality: Personality::new(rand::random(), r_thread.gen_bool(0.5)),
    }
}
