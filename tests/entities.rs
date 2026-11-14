mod common;

use common::{HOME, actor, carrier, count, reasons, transfer, world};
use magic_garden_rs::evaluator::evaluate;
use magic_garden_rs::messages::{Command, MoveEntity, ReleaseEntity, UseEntity};
use magic_garden_rs::state::{EntityKind, Position, State};

const FIELD: Position = Position { x: 5, y: 2 };

fn yard() -> State {
    world(vec![
        actor("ann", &[("rose", 3)]),
        actor("bob", &[]),
        carrier("truck", EntityKind::Truck, HOME),
        carrier("sled", EntityKind::Sled, FIELD),
    ])
}

fn use_entity(actor: &str, entity_id: &str) -> Command {
    Command::UseEntity(UseEntity {
        actor: actor.to_string(),
        entity_id: entity_id.to_string(),
    })
}

fn release(actor: &str, entity_id: &str) -> Command {
    Command::ReleaseEntity(ReleaseEntity {
        actor: actor.to_string(),
        entity_id: entity_id.to_string(),
    })
}

fn move_entity(actor: &str, entity_id: &str, to: Position) -> Command {
    Command::MoveEntity(MoveEntity {
        actor: actor.to_string(),
        entity_id: entity_id.to_string(),
        to,
    })
}

fn user_of(state: &State, entity_id: &str) -> Option<String> {
    state.entity(entity_id).unwrap().used_by.clone()
}

#[test]
fn use_slot_holds_one_actor() {
    let commands = vec![use_entity("ann", "truck"), use_entity("bob", "truck")];

    let result = evaluate(&yard(), commands, vec![]);

    assert_eq!(user_of(&result.state, "truck"), Some("ann".to_string()));
    assert_eq!(reasons(&result), vec!["truck is already in use"]);
}

#[test]
fn release_frees_the_slot() {
    let commands = vec![
        use_entity("ann", "truck"),
        release("bob", "truck"),
        release("ann", "truck"),
        use_entity("bob", "truck"),
    ];

    let result = evaluate(&yard(), commands, vec![]);

    assert_eq!(user_of(&result.state, "truck"), Some("bob".to_string()));
    assert_eq!(reasons(&result), vec!["bob is not using truck"]);
}

#[test]
fn use_rules() {
    let commands = vec![use_entity("ann", "sled"), use_entity("ann", "bob")];

    let result = evaluate(&yard(), commands, vec![]);

    let mut found = reasons(&result);
    found.sort();
    assert_eq!(
        found,
        vec!["only carriers can be used", "sled is too far away"]
    );
}

#[test]
fn carrier_moves_goods_with_its_user() {
    let commands = vec![
        use_entity("ann", "truck"),
        transfer("ann", "ann", "truck", "rose", 2),
        move_entity("ann", "truck", FIELD),
        transfer("ann", "truck", "sled", "rose", 2),
    ];

    let result = evaluate(&yard(), commands, vec![]);

    assert!(result.rejections.is_empty());
    assert_eq!(result.state.entity("ann").unwrap().position, FIELD);
    assert_eq!(result.state.entity("truck").unwrap().position, FIELD);
    assert_eq!(count(&result.state, "truck", "rose"), 0);
    assert_eq!(count(&result.state, "sled", "rose"), 2);
    assert_eq!(count(&result.state, "ann", "rose"), 1);
}

#[test]
fn moving_a_carrier_needs_its_use_slot() {
    let result = evaluate(&yard(), vec![move_entity("ann", "truck", FIELD)], vec![]);

    assert_eq!(reasons(&result), vec!["ann is not using truck"]);
    assert_eq!(result.state.entity("truck").unwrap().position, HOME);
}

#[test]
fn transfer_rules() {
    let commands = vec![
        transfer("bob", "ann", "bob", "rose", 1),
        transfer("ann", "ann", "sled", "rose", 1),
        transfer("ann", "ann", "bob", "rose", 9),
        transfer("ann", "ann", "bob", "rose", 0),
        transfer("ann", "ann", "ann", "rose", 1),
    ];

    let result = evaluate(&yard(), commands, vec![]);

    let mut found = reasons(&result);
    found.sort();
    assert_eq!(
        found,
        vec![
            "ann does not have enough rose",
            "bob does not control ann",
            "quantity must be positive",
            "sled is too far away",
            "source and target must differ",
        ]
    );
    assert_eq!(count(&result.state, "ann", "rose"), 3);
}
