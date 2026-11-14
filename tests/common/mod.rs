//! Small builders shared by the tests.
#![allow(dead_code)]

use magic_garden_rs::evaluator::Evaluation;
use magic_garden_rs::messages::{Command, PlaceOrder, TransferItems};
use magic_garden_rs::state::{Entity, EntityKind, Position, Side, State, Value};

pub const HOME: Position = Position { x: 0, y: 0 };

/// An actor at `HOME` holding the given items.
pub fn actor(id: &str, items: &[(&str, u32)]) -> Value {
    let mut entity = Entity::new(id, EntityKind::Actor, HOME);
    for (item, quantity) in items {
        entity.add(item, *quantity);
    }
    Value::Entity(entity)
}

pub fn carrier(id: &str, kind: EntityKind, position: Position) -> Value {
    Value::Entity(Entity::new(id, kind, position))
}

pub fn world(values: Vec<Value>) -> State {
    let mut state = State::new();
    for value in values {
        state.insert(value);
    }
    state
}

/// How many of `item` the entity `id` holds.
pub fn count(state: &State, id: &str, item: &str) -> u32 {
    state.entity(id).expect("entity exists").count(item)
}

pub fn reasons(evaluation: &Evaluation) -> Vec<&str> {
    evaluation
        .rejections
        .iter()
        .map(|rejection| rejection.reason.as_str())
        .collect()
}

pub fn order(actor: &str, side: Side, price: u32, quantity: u32, ttl: Option<u64>) -> Command {
    Command::PlaceOrder(PlaceOrder {
        actor: actor.to_string(),
        item: "rose".to_string(),
        side,
        price,
        quantity,
        ttl,
    })
}

pub fn buy(actor: &str, price: u32, quantity: u32) -> Command {
    order(actor, Side::Buy, price, quantity, None)
}

pub fn sell(actor: &str, price: u32, quantity: u32) -> Command {
    order(actor, Side::Sell, price, quantity, None)
}

pub fn transfer(actor: &str, source: &str, target: &str, item: &str, quantity: u32) -> Command {
    Command::TransferItems(TransferItems {
        actor: actor.to_string(),
        source: source.to_string(),
        target: target.to_string(),
        item: item.to_string(),
        quantity,
    })
}
