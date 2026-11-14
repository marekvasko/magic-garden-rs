//! The state evaluator: `new_state = evaluate(state, commands, events)`.
//!
//! A pure function: it never changes its inputs and always gives the same
//! answer for the same inputs. Each round it dispatches the pending messages
//! to their handlers, which answer with transactions, and hands those to the
//! resolver. Deferred messages and immediate events form the next round; the
//! tick is over when nothing is pending.

use crate::entities;
use crate::flowers;
use crate::market;
use crate::messages::{Command, Event, Message, Scheduled};
use crate::resolver::{Rejection, resolve};
use crate::state::{Rejected, State};
use crate::transaction::Transaction;

/// Far more rounds than a sane tick needs; guards against events that
/// trigger each other forever.
pub const MAX_ROUNDS: usize = 10_000;

/// The result of one tick.
#[derive(Debug, Clone, PartialEq)]
pub struct Evaluation {
    /// The state at the start of the next tick.
    pub state: State,
    /// Events that happened during this tick, in order.
    pub immediate: Vec<Event>,
    /// Events to put on the queues.
    pub future: Vec<Scheduled>,
    /// Commands and events that broke a rule, and why.
    pub rejections: Vec<Rejection>,
}

/// Ask the handler of `message` for its transaction.
///
/// A handler may look at `state` to decide what to do, but only at keys it
/// then lists in the transaction's reads or writes - that is what lets the
/// resolver notice when what the handler saw has gone stale.
///
/// The answer is `Ok(None)` when there is nothing to do, and `Err(Rejected)`
/// when a rule is broken.
pub fn dispatch(state: &State, message: &Message) -> Result<Option<Transaction>, Rejected> {
    match message {
        Message::Command(command) => dispatch_command(state, command),
        Message::Event(event) => Ok(dispatch_event(state, event)),
    }
}

fn dispatch_command(state: &State, command: &Command) -> Result<Option<Transaction>, Rejected> {
    let transaction = match command {
        Command::PlaceOrder(command) => market::place_order(state, command)?,
        Command::CancelOrder(command) => {
            let actor = Some(command.actor.as_str());
            match market::remove_order(state, &command.item, &command.order_id, actor) {
                Some(transaction) => transaction,
                None => return Err(Rejected::new("order not found")),
            }
        }
        Command::PlantFlower(command) => flowers::plant(state, command),
        Command::CutFlower(command) => flowers::cut(state, command)?,
        Command::UseEntity(command) => entities::use_entity(command),
        Command::ReleaseEntity(command) => entities::release(command),
        Command::MoveEntity(command) => entities::move_entity(command),
        Command::TransferItems(command) => entities::transfer(command)?,
    };
    Ok(Some(transaction))
}

fn dispatch_event(state: &State, event: &Event) -> Option<Transaction> {
    match event {
        Event::OrderExpiry { item, order_id } => market::remove_order(state, item, order_id, None),
        Event::FlowerGrowth { flower_id } => flowers::grow(state, flower_id),
        Event::CompostSweep => flowers::sweep(state),
        // Notifications: nothing to do.
        Event::TradeExecuted { .. } => None,
        Event::FlowerDied { .. } => None,
    }
}

/// Apply one tick's due events, then its commands, each in the order given.
pub fn evaluate(state: &State, commands: Vec<Command>, events: Vec<Event>) -> Evaluation {
    let mut state = state.clone();
    let mut immediate = Vec::new();
    let mut future = Vec::new();
    let mut rejections = Vec::new();

    let mut pending = Vec::new();
    for event in events {
        pending.push(Message::Event(event));
    }
    for command in commands {
        pending.push(Message::Command(command));
    }

    let mut rounds = 0;
    while !pending.is_empty() {
        rounds += 1;
        if rounds > MAX_ROUNDS {
            panic!(
                "tick {} did not settle within {} rounds",
                state.tick, MAX_ROUNDS
            );
        }

        // Dispatch: every pending message proposes a transaction.
        let mut transactions = Vec::new();
        for message in pending {
            match dispatch(&state, &message) {
                Ok(Some(transaction)) => transactions.push((message, transaction)),
                Ok(None) => {}
                Err(rejected) => rejections.push(Rejection {
                    cause: message,
                    reason: rejected.reason,
                }),
            }
        }

        // Resolve: validate, deconflict, commit.
        let resolution = resolve(state, transactions);
        state = resolution.state;
        immediate.extend(resolution.immediate.clone());
        future.extend(resolution.future);
        rejections.extend(resolution.rejections);

        // The next round: whatever had to wait, then whatever just happened.
        pending = resolution.deferred;
        for event in resolution.immediate {
            pending.push(Message::Event(event));
        }
    }

    state.tick += 1;
    Evaluation {
        state,
        immediate,
        future,
        rejections,
    }
}
