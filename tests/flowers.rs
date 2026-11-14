mod common;

use common::{HOME, actor, count, reasons, world};
use magic_garden_rs::engine::Engine;
use magic_garden_rs::flowers::GROWTH_TICKS;
use magic_garden_rs::messages::{Command, CutFlower, Event, MoveEntity, PlantFlower};
use magic_garden_rs::state::{Flower, Position, Stage, Value};

fn run(engine: &mut Engine, ticks: u64) {
    for _ in 0..ticks {
        engine.tick();
    }
}

/// Ann standing next to her blooming rose "f1"; Bob is there too.
fn garden(cuts_left: u32) -> Engine {
    let rose = Flower {
        id: "f1".to_string(),
        species: "rose".to_string(),
        owner: "ann".to_string(),
        position: HOME,
        stage: Stage::Bloom,
        cuts_left,
    };
    Engine::new(world(vec![
        actor("ann", &[]),
        actor("bob", &[]),
        Value::Flower(rose),
    ]))
}

fn plant(actor: &str, flower_id: &str) -> Command {
    Command::PlantFlower(PlantFlower {
        actor: actor.to_string(),
        flower_id: flower_id.to_string(),
        species: "rose".to_string(),
    })
}

fn cut(actor: &str, flower_id: &str) -> Command {
    Command::CutFlower(CutFlower {
        actor: actor.to_string(),
        flower_id: flower_id.to_string(),
    })
}

fn stage(engine: &Engine, flower_id: &str) -> Stage {
    engine.state.flower(flower_id).unwrap().stage
}

#[test]
fn planted_seed_grows_into_a_bloom() {
    let mut engine = Engine::new(world(vec![actor("ann", &[("rose seed", 1)])]));
    engine.ingest(plant("ann", "f1"));

    engine.tick();
    assert_eq!(stage(&engine, "f1"), Stage::Seed);
    assert_eq!(count(&engine.state, "ann", "rose seed"), 0);
    assert_eq!(engine.queue.flowers.len(), 1);

    run(&mut engine, GROWTH_TICKS);
    assert_eq!(stage(&engine, "f1"), Stage::Sprout);
    run(&mut engine, GROWTH_TICKS);
    assert_eq!(stage(&engine, "f1"), Stage::Bloom);
    assert!(engine.queue.flowers.is_empty());
}

#[test]
fn planting_needs_a_seed_and_a_fresh_id() {
    let mut engine = garden(3);
    engine.ingest(plant("ann", "f2"));
    engine.ingest(plant("bob", "f1"));

    let result = engine.tick();

    assert_eq!(
        reasons(&result),
        vec![
            "ann does not have enough rose seed",
            "flower id is already taken",
        ]
    );
}

#[test]
fn cut_flower_gives_a_bloom_and_regrows() {
    let mut engine = garden(3);
    engine.ingest(cut("ann", "f1"));

    engine.tick();
    assert_eq!(count(&engine.state, "ann", "rose"), 1);
    assert_eq!(stage(&engine, "f1"), Stage::Sprout);
    assert_eq!(engine.state.flower("f1").unwrap().cuts_left, 2);

    run(&mut engine, GROWTH_TICKS);
    assert_eq!(stage(&engine, "f1"), Stage::Bloom);
}

#[test]
fn flower_dies_on_its_last_cut_and_is_composted() {
    let mut engine = garden(1);
    engine.every(3, Event::CompostSweep);
    engine.ingest(cut("ann", "f1"));

    let result = engine.tick();
    let died = Event::FlowerDied {
        flower_id: "f1".to_string(),
    };
    assert_eq!(result.immediate, vec![died]);
    assert_eq!(stage(&engine, "f1"), Stage::Dead);
    assert_eq!(count(&engine.state, "ann", "rose"), 1);

    run(&mut engine, 3);
    assert!(engine.state.flower("f1").is_none());
}

#[test]
fn cut_rules() {
    let mut engine = garden(3);
    engine.ingest(cut("bob", "f1"));
    engine.ingest(cut("ann", "missing"));
    engine.ingest(cut("ann", "f1"));
    engine.ingest(cut("ann", "f1"));

    let result = engine.tick();

    assert_eq!(
        reasons(&result),
        vec![
            "flower not found",
            "not your flower",
            "flower is not in bloom",
        ]
    );
    assert_eq!(count(&engine.state, "ann", "rose"), 1);
}

#[test]
fn cutting_needs_to_be_next_to_the_flower() {
    let mut engine = garden(3);
    engine.ingest(Command::MoveEntity(MoveEntity {
        actor: "ann".to_string(),
        entity_id: "ann".to_string(),
        to: Position { x: 4, y: 4 },
    }));
    engine.ingest(cut("ann", "f1"));

    let result = engine.tick();

    assert_eq!(reasons(&result), vec!["flower is too far away"]);
}
