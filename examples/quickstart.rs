//! A first look at the engine: run with `cargo run --example quickstart`.

use magic_garden_rs::engine::Engine;
use magic_garden_rs::messages::{Command, Event, PlaceOrder};
use magic_garden_rs::state::{Entity, EntityKind, Position, Side, State, Value};

fn main() {
    let mut ann = Entity::new("ann", EntityKind::Actor, Position { x: 0, y: 0 });
    ann.add("coin", 100);

    let mut state = State::new();
    state.insert(Value::Entity(ann));

    let mut engine = Engine::new(state);
    engine.every(20, Event::CompostSweep);

    engine.ingest(Command::PlaceOrder(PlaceOrder {
        actor: "ann".to_string(),
        item: "rose".to_string(),
        side: Side::Buy,
        price: 5,
        quantity: 2,
        ttl: None,
    }));
    let evaluation = engine.tick(); // .state, .immediate, .future, .rejections

    let ann = evaluation.state.entity("ann").unwrap();
    println!(
        "tick {}: ann has {} coins",
        evaluation.state.tick,
        ann.count("coin")
    );
    println!("order book: {:?}", evaluation.state.book("rose").unwrap());
}
