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
    state: State,
    pub name: String,
    pub size: Size,
    pub livable: bool,
    pub classification: Classification,
    pub moons: Vec<Moon>,
}

impl Planet {
    pub fn new(
        state: State,
        name: String,
        size: Size,
        livable: bool,
        classification: Classification,
        moons: Vec<Moon>,
    ) -> Planet {
        Planet {
            state,
            name,
            size,
            livable,
            classification,
            moons,
        }
    }

    fn declares(&self, declaration: &String) {
        println!("{}", declaration)
    }
    pub fn get_state(&self) -> &State {
        &self.state
    }

    pub fn choose_state(&mut self) {
        match self.state {
            State::Start => {
                self.declares(&format!("Hello, I am planet {}", self.name));
                self.state = State::Morning;
            },
            State::Morning => {
                self.declares(&format!("It is now morning on planet {}", self.name));
                self.state = State::Afternoon;
            },
            State::Afternoon => {
                self.declares(&format!("It is now the afternoon on planet {}", self.name));
                self.state = State::Evening;
            }
            State::Evening => {
                self.declares(&format!("It is now evening on planet {}", self.name));
                self.state = State::Night;
            },
            State::Night => {
                self.declares(&format!("It is now nighttime on planet {}", self.name));
                self.state = State::Morning;
            },
        }
    }
}

impl Moon {
    pub fn new(name: String) -> Moon {
        Moon { name }
    }
}

#[derive(Debug)]
pub enum State {
    Start,
    Morning,
    Afternoon,
    Evening,
    Night,
}
