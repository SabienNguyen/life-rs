#[derive(Debug)]
pub enum Classification {
    Terrestial,
    Jovian,
}

#[derive(Debug)]
pub enum Size {
    ExtraLarge,
    Large,
    Normal,
    Small,
    ExtraSmall,
}

#[derive(Debug)]
pub struct Moon {
    pub name: String,
}

#[derive(Debug)]
pub struct Planet {
    pub name: String,
    pub size: Size,
    pub livable: bool,
    pub classification: Classification,
    pub moons: Vec<Moon>,
}

impl Planet {
    pub fn new(
        name: String,
        size: Size,
        livable: bool,
        classification: Classification,
        moons: Vec<Moon>,
    ) -> Planet {
        Planet {
            name,
            size,
            livable,
            classification,
            moons,
        }
    }
}

impl Moon {
    pub fn new(name: String) -> Moon {
        Moon { name }
    }
}
