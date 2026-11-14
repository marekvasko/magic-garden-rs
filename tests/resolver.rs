mod common;

use common::{actor, count, world};
use magic_garden_rs::messages::{Event, Message};
use magic_garden_rs::resolver::resolve;
use magic_garden_rs::state::{Key, Rejected, Value, expect_entity};
use magic_garden_rs::transaction::{Modify, Transaction};

/// A message naming a transaction, so the tests can tell them apart.
fn label(name: &str) -> Message {
    Message::Event(Event::FlowerDied {
        flower_id: name.to_string(),
    })
}

/// A transaction moving one coin from `source` to `target`.
fn give_coin(source: &str, target: &str) -> (Message, Transaction) {
    let keys = vec![
        Key::Entity(source.to_string()),
        Key::Entity(target.to_string()),
    ];
    let modify: Modify = Box::new(|inputs: &[Option<Value>]| {
        let mut giver = expect_entity(&inputs[0])?;
        let mut taker = expect_entity(&inputs[1])?;
        giver.remove("coin", 1)?;
        taker.add("coin", 1);
        Ok(vec![Some(Value::Entity(giver)), Some(Value::Entity(taker))])
    });
    let name = format!("{source}->{target}");
    (label(&name), Transaction::new(keys.clone(), keys, modify))
}

#[test]
fn commits_writes_and_collects_events() {
    let state = world(vec![actor("ann", &[("coin", 5)]), actor("bob", &[])]);
    let (cause, mut transaction) = give_coin("ann", "bob");
    transaction.immediate.push(Event::CompostSweep);

    let resolution = resolve(state, vec![(cause, transaction)]);

    assert_eq!(count(&resolution.state, "ann", "coin"), 4);
    assert_eq!(count(&resolution.state, "bob", "coin"), 1);
    assert_eq!(resolution.immediate, vec![Event::CompostSweep]);
}

#[test]
fn rejected_transaction_changes_nothing() {
    let state = world(vec![actor("ann", &[]), actor("bob", &[])]);

    let resolution = resolve(state.clone(), vec![give_coin("ann", "bob")]);

    assert_eq!(resolution.state, state);
    assert_eq!(resolution.rejections.len(), 1);
    assert_eq!(
        resolution.rejections[0].reason,
        "ann does not have enough coin"
    );
}

#[test]
fn conflicting_transaction_is_deferred() {
    let state = world(vec![
        actor("ann", &[("coin", 5)]),
        actor("bob", &[]),
        actor("cid", &[]),
    ]);

    let resolution = resolve(
        state,
        vec![give_coin("ann", "bob"), give_coin("bob", "cid")],
    );

    assert_eq!(count(&resolution.state, "bob", "coin"), 1);
    assert_eq!(resolution.deferred, vec![label("bob->cid")]);
    assert!(resolution.rejections.is_empty());
}

#[test]
fn independent_transactions_commit_together() {
    let state = world(vec![
        actor("ann", &[("coin", 1)]),
        actor("bob", &[]),
        actor("cid", &[("coin", 1)]),
        actor("dan", &[]),
    ]);

    let resolution = resolve(
        state,
        vec![give_coin("ann", "bob"), give_coin("cid", "dan")],
    );

    assert!(resolution.deferred.is_empty());
    assert_eq!(count(&resolution.state, "bob", "coin"), 1);
    assert_eq!(count(&resolution.state, "dan", "coin"), 1);
}

#[test]
fn later_transaction_cannot_overtake_a_deferred_one() {
    let state = world(vec![
        actor("ann", &[("coin", 5)]),
        actor("bob", &[]),
        actor("cid", &[("coin", 5)]),
        actor("dan", &[]),
    ]);

    let resolution = resolve(
        state,
        vec![
            give_coin("ann", "bob"),
            give_coin("bob", "cid"),
            give_coin("cid", "dan"),
        ],
    );

    assert_eq!(
        resolution.deferred,
        vec![label("bob->cid"), label("cid->dan")]
    );
}

#[test]
fn rejected_transaction_does_not_block_later_ones() {
    let state = world(vec![
        actor("ann", &[]),
        actor("bob", &[("coin", 1)]),
        actor("cid", &[]),
    ]);

    let resolution = resolve(
        state,
        vec![give_coin("ann", "bob"), give_coin("bob", "cid")],
    );

    assert!(resolution.deferred.is_empty());
    assert_eq!(count(&resolution.state, "cid", "coin"), 1);
}

#[test]
fn output_none_deletes_the_key() {
    let state = world(vec![actor("ann", &[])]);
    let keys = vec![Key::Entity("ann".to_string())];
    let modify: Modify = Box::new(|_inputs: &[Option<Value>]| Ok(vec![None]));

    let resolution = resolve(
        state,
        vec![(
            label("remove"),
            Transaction::new(keys.clone(), keys, modify),
        )],
    );

    assert!(resolution.state.values.is_empty());
}

#[test]
fn modify_may_reject() {
    let modify: Modify = Box::new(|_inputs: &[Option<Value>]| Err(Rejected::new("no")));

    let resolution = resolve(
        world(vec![]),
        vec![(label("refuse"), Transaction::new(vec![], vec![], modify))],
    );

    assert_eq!(resolution.rejections[0].reason, "no");
}
