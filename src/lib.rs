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

#[derive(Debug)]
pub struct Person {
    pub name: String,
    pub height: Height,
    pub country: Country,
    pub weight: Weight,
}

impl Person {
    pub fn new(name: String, height: Height, country: Country, weight: Weight) -> Person {
        Person {
            name,
            height,
            country,
            weight,
        }
    }
}
