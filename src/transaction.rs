//! A transaction: one atomic change of state, proposed by a handler.

use crate::messages::{Event, Scheduled};
use crate::state::{Key, Rejected, Value};

/// Maps the values under `reads` to the new values for `writes`.
///
/// `None` stands for "no value": a missing input, or an output to delete.
/// Return `Err(Rejected)` to refuse the change.
pub type Modify = Box<dyn Fn(&[Option<Value>]) -> Result<Vec<Option<Value>>, Rejected>>;

pub struct Transaction {
    /// The keys whose values are passed to `modify`, in this order.
    pub reads: Vec<Key>,
    /// The keys that receive the values `modify` returns, in this order.
    pub writes: Vec<Key>,
    pub modify: Modify,
    /// Events that happen, in this same tick, once the transaction commits.
    pub immediate: Vec<Event>,
    /// Events to schedule once the transaction commits.
    pub future: Vec<Scheduled>,
}

impl Transaction {
    /// A transaction that produces no events; push to `immediate` or `future` to add some.
    pub fn new(reads: Vec<Key>, writes: Vec<Key>, modify: Modify) -> Transaction {
        Transaction {
            reads,
            writes,
            modify,
            immediate: Vec::new(),
            future: Vec::new(),
        }
    }
}
