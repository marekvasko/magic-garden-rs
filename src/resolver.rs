//! The transaction resolver: validate, deconflict, commit atomically.
//!
//! All transactions of one round were built from the same snapshot of the
//! state. The resolver walks them in order:
//!
//! * **validate** - `modify` may return `Rejected`; the transaction is then
//!   dropped and reported, having changed nothing.
//! * **deconflict** - a transaction touching a key that an earlier
//!   transaction of this round already claimed was built from stale data.
//!   It is deferred: its cause is handed back to be dispatched again, on
//!   fresh state, in the next round. First come, first served.
//! * **commit** - all writes of a transaction are applied together or not
//!   at all.

use std::collections::BTreeSet;

use crate::messages::{Event, Message, Scheduled};
use crate::state::{Key, State};
use crate::transaction::Transaction;

/// A message that broke a rule, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Rejection {
    pub cause: Message,
    pub reason: String,
}

pub struct Resolution {
    pub state: State,
    pub immediate: Vec<Event>,
    pub future: Vec<Scheduled>,
    pub deferred: Vec<Message>,
    pub rejections: Vec<Rejection>,
}

/// Resolve one round of transactions, each paired with the message that caused it.
pub fn resolve(state: State, transactions: Vec<(Message, Transaction)>) -> Resolution {
    let mut state = state;
    let mut claimed: BTreeSet<Key> = BTreeSet::new();
    let mut immediate = Vec::new();
    let mut future = Vec::new();
    let mut deferred = Vec::new();
    let mut rejections = Vec::new();

    for (cause, transaction) in transactions {
        let keys: Vec<&Key> = transaction
            .reads
            .iter()
            .chain(transaction.writes.iter())
            .collect();

        // Deconflict.
        let conflicts = keys.iter().any(|key| claimed.contains(*key));
        if conflicts {
            // Claim the keys anyway, so that no later transaction overtakes this one.
            for key in keys {
                claimed.insert(key.clone());
            }
            deferred.push(cause);
            continue;
        }

        // Validate.
        let mut inputs = Vec::new();
        for key in &transaction.reads {
            inputs.push(state.values.get(key).cloned());
        }
        let outputs = match (transaction.modify)(&inputs) {
            Ok(outputs) => outputs,
            Err(rejected) => {
                rejections.push(Rejection {
                    cause,
                    reason: rejected.reason,
                });
                continue;
            }
        };
        assert_eq!(
            outputs.len(),
            transaction.writes.len(),
            "modify must return exactly one output per written key"
        );

        // Commit. Nothing below can fail, so the writes happen all together.
        for (key, output) in transaction.writes.iter().zip(outputs) {
            match output {
                Some(value) => state.values.insert(key.clone(), value),
                None => state.values.remove(key),
            };
        }
        for key in keys {
            claimed.insert(key.clone());
        }
        immediate.extend(transaction.immediate);
        future.extend(transaction.future);
    }

    Resolution {
        state,
        immediate,
        future,
        deferred,
        rejections,
    }
}
