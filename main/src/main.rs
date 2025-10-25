#![allow(dead_code)]

use person::generate;
use planet::{Moon, Planet};

fn main() {
    let earth = Planet::new(
        String::from("Earth"),
        planet::Size::Normal,
        true,
        planet::Classification::Terrestial,
        vec![Moon::new(String::from("Moon"))],
    );

    let mut rng = rand::thread_rng();
    for _ in 0..10 {
        let curr = generate(&mut rng, &earth);
        curr.introduction();
    }
}
