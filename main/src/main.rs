#![allow(dead_code)]

use person::generate;
use planet::{Moon, Planet, State};
use std::thread;
use std::time::Duration;

fn main() {
    let mut earth = Planet::new(
        State::Start,
        String::from("Earth"),
        planet::Size::Normal,
        true,
        planet::Classification::Terrestial,
        vec![Moon::new(String::from("Moon"))],
    );

    let rng = rand::thread_rng();
    let mut person = generate(rng);

    loop {
        person.choose_action(&earth);
        earth.choose_state();
        thread::sleep(Duration::from_secs(3));
    }
}
