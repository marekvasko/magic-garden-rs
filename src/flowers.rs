//! Flowers: planted from seed, growing in stages, cut for blooms until they die.

use crate::messages::{CutFlower, Event, PlantFlower, Scheduled};
use crate::state::{Flower, Key, Rejected, Stage, State, Value, expect_entity, expect_flower};
use crate::transaction::{Modify, Transaction};

/// Ticks a flower spends in each growing stage.
pub const GROWTH_TICKS: u64 = 5;

/// How many cuts a freshly planted flower survives.
pub const CUTS_PER_FLOWER: u32 = 3;

/// The inventory item that grows into `species`.
pub fn seed_of(species: &str) -> String {
    format!("{} seed", species)
}

fn growth_event(flower_id: &str, due: u64) -> Scheduled {
    Scheduled {
        due,
        event: Event::FlowerGrowth {
            flower_id: flower_id.to_string(),
        },
    }
}

/// Spend one seed to plant a flower where the actor stands.
pub fn plant(state: &State, command: &PlantFlower) -> Transaction {
    let keys = vec![
        Key::Entity(command.actor.clone()),
        Key::Flower(command.flower_id.clone()),
    ];

    let command_copy = command.clone();
    let modify: Modify = Box::new(move |inputs: &[Option<Value>]| {
        let command = &command_copy;
        let mut actor = expect_entity(&inputs[0])?;
        if inputs[1].is_some() {
            return Err(Rejected::new("flower id is already taken"));
        }

        actor.remove(&seed_of(&command.species), 1)?;
        let flower = Flower {
            id: command.flower_id.clone(),
            species: command.species.clone(),
            owner: actor.id.clone(),
            position: actor.position,
            stage: Stage::Seed,
            cuts_left: CUTS_PER_FLOWER,
        };

        Ok(vec![
            Some(Value::Entity(actor)),
            Some(Value::Flower(flower)),
        ])
    });

    let mut transaction = Transaction::new(keys.clone(), keys, modify);
    transaction
        .future
        .push(growth_event(&command.flower_id, state.tick + GROWTH_TICKS));
    transaction
}

/// Advance a flower one stage: seed -> sprout -> bloom.
///
/// Returns `None` when the flower is gone or is not growing.
pub fn grow(state: &State, flower_id: &str) -> Option<Transaction> {
    let seen_stage = state.flower(flower_id)?.stage;
    if seen_stage != Stage::Seed && seen_stage != Stage::Sprout {
        return None;
    }
    let keys = vec![Key::Flower(flower_id.to_string())];

    let modify: Modify = Box::new(|inputs: &[Option<Value>]| {
        let mut flower = expect_flower(&inputs[0])?;
        flower.stage = match flower.stage {
            Stage::Seed => Stage::Sprout,
            Stage::Sprout => Stage::Bloom,
            other => other,
        };
        Ok(vec![Some(Value::Flower(flower))])
    });

    let mut transaction = Transaction::new(keys.clone(), keys, modify);
    if seen_stage == Stage::Seed {
        // A sprout still has one more stage to grow.
        transaction
            .future
            .push(growth_event(flower_id, state.tick + GROWTH_TICKS));
    }
    Some(transaction)
}

/// Cut a bloom into the actor's inventory.
/// The flower regrows, or dies on its last cut.
pub fn cut(state: &State, command: &CutFlower) -> Result<Transaction, Rejected> {
    let seen_flower = match state.flower(&command.flower_id) {
        Some(flower) => flower,
        None => return Err(Rejected::new("flower not found")),
    };
    let dies = seen_flower.cuts_left <= 1;

    let keys = vec![
        Key::Flower(command.flower_id.clone()),
        Key::Entity(command.actor.clone()),
    ];

    let modify: Modify = Box::new(|inputs: &[Option<Value>]| {
        let mut flower = expect_flower(&inputs[0])?;
        let mut actor = expect_entity(&inputs[1])?;

        if flower.owner != actor.id {
            return Err(Rejected::new("not your flower"));
        }
        if flower.position != actor.position {
            return Err(Rejected::new("flower is too far away"));
        }
        if flower.stage != Stage::Bloom {
            return Err(Rejected::new("flower is not in bloom"));
        }

        flower.cuts_left = flower.cuts_left.saturating_sub(1);
        if flower.cuts_left == 0 {
            flower.stage = Stage::Dead;
        } else {
            flower.stage = Stage::Sprout;
        }
        actor.add(&flower.species, 1);

        Ok(vec![
            Some(Value::Flower(flower)),
            Some(Value::Entity(actor)),
        ])
    });

    let mut transaction = Transaction::new(keys.clone(), keys, modify);
    if dies {
        transaction.immediate.push(Event::FlowerDied {
            flower_id: command.flower_id.clone(),
        });
    } else {
        transaction
            .future
            .push(growth_event(&command.flower_id, state.tick + GROWTH_TICKS));
    }
    Ok(transaction)
}

/// Remove every dead flower from the world.
///
/// Returns `None` when there are no dead flowers.
pub fn sweep(state: &State) -> Option<Transaction> {
    let mut dead = Vec::new();
    for (key, value) in &state.values {
        if let Value::Flower(flower) = value
            && flower.stage == Stage::Dead
        {
            dead.push(key.clone());
        }
    }
    if dead.is_empty() {
        return None;
    }

    // Writing `None` to a key deletes it.
    let modify: Modify = Box::new(|inputs: &[Option<Value>]| Ok(vec![None; inputs.len()]));

    Some(Transaction::new(dead.clone(), dead, modify))
}
