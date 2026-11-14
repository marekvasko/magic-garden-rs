mod common;

use common::{HOME, actor, buy, count, sell, transfer, world};
use magic_garden_rs::engine::Engine;
use magic_garden_rs::evaluator::evaluate;
use magic_garden_rs::messages::{Event, Scheduled};
use magic_garden_rs::state::{Flower, Stage, Value};

#[test]
fn tick_advances_time_and_drains_the_batch() {
    let mut engine = Engine::new(world(vec![actor("ann", &[("coin", 3)]), actor("bob", &[])]));
    engine.ingest(transfer("ann", "ann", "bob", "coin", 1));

    engine.tick();
    engine.tick();

    assert_eq!(engine.state.tick, 2);
    assert_eq!(count(&engine.state, "bob", "coin"), 1);
}

#[test]
fn evaluate_is_pure() {
    let state = world(vec![actor("ann", &[("coin", 3)]), actor("bob", &[])]);
    let commands = vec![transfer("ann", "ann", "bob", "coin", 1)];

    let first = evaluate(&state, commands.clone(), vec![]);
    let second = evaluate(&state, commands, vec![]);

    assert_eq!(first, second);
    assert_eq!(count(&state, "ann", "coin"), 3);
    assert_eq!(first.state.tick, state.tick + 1);
}

#[test]
fn commands_on_the_same_keys_apply_in_ingest_order() {
    let state = world(vec![
        actor("ann", &[("coin", 1)]),
        actor("bob", &[]),
        actor("cid", &[]),
    ]);
    let commands = vec![
        transfer("ann", "ann", "bob", "coin", 1),
        transfer("bob", "bob", "cid", "coin", 1),
        transfer("cid", "cid", "ann", "coin", 1),
    ];

    let result = evaluate(&state, commands, vec![]);

    assert!(result.rejections.is_empty());
    assert_eq!(count(&result.state, "ann", "coin"), 1);
}

#[test]
fn due_events_run_before_commands() {
    let sprout = Flower {
        id: "f1".to_string(),
        species: "rose".to_string(),
        owner: "ann".to_string(),
        position: HOME,
        stage: Stage::Sprout,
        cuts_left: 3,
    };
    let mut engine = Engine::new(world(vec![actor("ann", &[]), Value::Flower(sprout)]));
    engine.schedule(Scheduled {
        due: 0,
        event: Event::FlowerGrowth {
            flower_id: "f1".to_string(),
        },
    });

    engine.tick();

    assert_eq!(engine.state.flower("f1").unwrap().stage, Stage::Bloom);
}

#[test]
fn many_orders_on_one_book_settle_within_a_tick() {
    let mut engine = Engine::new(world(vec![
        actor("ann", &[("coin", 1000)]),
        actor("bob", &[("rose", 50)]),
    ]));
    for _ in 0..50 {
        engine.ingest(sell("bob", 3, 1));
        engine.ingest(buy("ann", 3, 1));
    }

    let result = engine.tick();

    assert_eq!(result.immediate.len(), 50);
    assert_eq!(count(&engine.state, "ann", "coin"), 850);
    assert_eq!(count(&engine.state, "ann", "rose"), 50);
    assert_eq!(count(&engine.state, "bob", "coin"), 150);
    assert_eq!(count(&engine.state, "bob", "rose"), 0);
}
