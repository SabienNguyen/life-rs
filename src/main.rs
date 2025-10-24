use life::Country;
use life::Height;
use life::Person;
use life::Weight;

fn main() {
    let person1 = Person::new(
        String::from("Sabien Nguyen"),
        Height::Tall,
        Country::Usa,
        Weight::Normal,
    );

    println!("{:}", person1.name);
}
