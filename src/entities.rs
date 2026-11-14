//! Entities: actors, and the carriers (truck, sled, wheelbarrow) they use to move things.

use crate::messages::{MoveEntity, ReleaseEntity, TransferItems, UseEntity};
use crate::state::{EntityKind, Key, Rejected, Value, expect_entity};
use crate::transaction::{Modify, Transaction};

fn too_far_away(id: &str) -> Rejected {
    Rejected::new(&format!("{} is too far away", id))
}

fn not_using(actor: &str, carrier: &str) -> Rejected {
    Rejected::new(&format!("{} is not using {}", actor, carrier))
}

/// Take the use slot of a free carrier standing at the actor's position.
pub fn use_entity(command: &UseEntity) -> Transaction {
    let actor_key = Key::Entity(command.actor.clone());
    let carrier_key = Key::Entity(command.entity_id.clone());

    let modify: Modify = Box::new(|inputs: &[Option<Value>]| {
        let actor = expect_entity(&inputs[0])?;
        let mut carrier = expect_entity(&inputs[1])?;

        if carrier.kind == EntityKind::Actor {
            return Err(Rejected::new("only carriers can be used"));
        }
        if carrier.used_by.is_some() {
            let reason = format!("{} is already in use", carrier.id);
            return Err(Rejected::new(&reason));
        }
        if carrier.position != actor.position {
            return Err(too_far_away(&carrier.id));
        }

        carrier.used_by = Some(actor.id);
        Ok(vec![Some(Value::Entity(carrier))])
    });

    // Reads the actor and the carrier, but only the carrier changes.
    Transaction::new(
        vec![actor_key, carrier_key.clone()],
        vec![carrier_key],
        modify,
    )
}

/// Free the use slot of a carrier.
pub fn release(command: &ReleaseEntity) -> Transaction {
    let keys = vec![Key::Entity(command.entity_id.clone())];

    let actor = command.actor.clone();
    let modify: Modify = Box::new(move |inputs: &[Option<Value>]| {
        let mut carrier = expect_entity(&inputs[0])?;
        if carrier.used_by.as_ref() != Some(&actor) {
            return Err(not_using(&actor, &carrier.id));
        }
        carrier.used_by = None;
        Ok(vec![Some(Value::Entity(carrier))])
    });

    Transaction::new(keys.clone(), keys, modify)
}

/// Move the actor, together with the carrier it is using if one is named.
pub fn move_entity(command: &MoveEntity) -> Transaction {
    let mut keys = vec![Key::Entity(command.actor.clone())];
    if command.entity_id != command.actor {
        keys.push(Key::Entity(command.entity_id.clone()));
    }

    let to = command.to;
    let modify: Modify = Box::new(move |inputs: &[Option<Value>]| {
        let mut actor = expect_entity(&inputs[0])?;
        let mut outputs = Vec::new();

        // The second input, if there is one, is the carrier.
        if inputs.len() > 1 {
            let mut carrier = expect_entity(&inputs[1])?;
            if carrier.used_by.as_ref() != Some(&actor.id) {
                return Err(not_using(&actor.id, &carrier.id));
            }
            if carrier.position != actor.position {
                return Err(too_far_away(&carrier.id));
            }
            carrier.position = to;
            outputs.push(Some(Value::Entity(carrier)));
        }

        actor.position = to;
        outputs.insert(0, Some(Value::Entity(actor)));
        Ok(outputs)
    });

    Transaction::new(keys.clone(), keys, modify)
}

/// Move items between two entities at the same position.
///
/// The actor must control the source: be it, or be using it.
pub fn transfer(command: &TransferItems) -> Result<Transaction, Rejected> {
    if command.quantity == 0 {
        return Err(Rejected::new("quantity must be positive"));
    }
    if command.source == command.target {
        return Err(Rejected::new("source and target must differ"));
    }
    let keys = vec![
        Key::Entity(command.source.clone()),
        Key::Entity(command.target.clone()),
    ];

    let command_copy = command.clone();
    let modify: Modify = Box::new(move |inputs: &[Option<Value>]| {
        let command = &command_copy;
        let mut source = expect_entity(&inputs[0])?;
        let mut target = expect_entity(&inputs[1])?;

        let is_source = command.actor == source.id;
        let uses_source = source.used_by.as_ref() == Some(&command.actor);
        if !is_source && !uses_source {
            let reason = format!("{} does not control {}", command.actor, source.id);
            return Err(Rejected::new(&reason));
        }
        if source.position != target.position {
            return Err(too_far_away(&target.id));
        }

        source.remove(&command.item, command.quantity)?;
        target.add(&command.item, command.quantity);
        Ok(vec![
            Some(Value::Entity(source)),
            Some(Value::Entity(target)),
        ])
    });

    Ok(Transaction::new(keys.clone(), keys, modify))
}
