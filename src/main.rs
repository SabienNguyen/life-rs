#![allow(dead_code)]

mod person;
use life::generate;

fn main() {
    let mut rng = rand::thread_rng();
    for _ in 0..10 {
        let curr = generate(&mut rng);
        curr.introduction();
    }
}
