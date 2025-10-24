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

    pub fn says(&self, words: &str) {
        println!("{} says \"{}\"", self.name, words);
    }
}
