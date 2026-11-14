mod common;

use common::{actor, buy, count, order, reasons, sell, world};
use magic_garden_rs::engine::Engine;
use magic_garden_rs::evaluator::{Evaluation, evaluate};
use magic_garden_rs::messages::{CancelOrder, Command, Event};
use magic_garden_rs::state::{Side, State};

fn traders() -> State {
    world(vec![
        actor("ann", &[("coin", 100)]),
        actor("bob", &[("rose", 10)]),
        actor("cid", &[("rose", 10)]),
        actor("dan", &[("coin", 100)]),
    ])
}

fn cancel(actor: &str, order_id: &str) -> Command {
    Command::CancelOrder(CancelOrder {
        actor: actor.to_string(),
        item: "rose".to_string(),
        order_id: order_id.to_string(),
    })
}

/// The (buyer, seller, price) of every trade, in order.
fn trades(evaluation: &Evaluation) -> Vec<(&str, &str, u32)> {
    let mut trades = Vec::new();
    for event in &evaluation.immediate {
        if let Event::TradeExecuted {
            buyer,
            seller,
            price,
            ..
        } = event
        {
            trades.push((buyer.as_str(), seller.as_str(), *price));
        }
    }
    trades
}

#[test]
fn resting_order_escrows_its_cost() {
    let result = evaluate(
        &traders(),
        vec![buy("ann", 7, 2), sell("bob", 9, 3)],
        vec![],
    );

    assert_eq!(count(&result.state, "ann", "coin"), 86);
    assert_eq!(count(&result.state, "bob", "rose"), 7);
    let book = result.state.book("rose").unwrap();
    assert_eq!(book.bids.len(), 1);
    assert_eq!((book.bids[0].price, book.bids[0].quantity), (7, 2));
    assert_eq!(book.asks.len(), 1);
    assert_eq!((book.asks[0].price, book.asks[0].quantity), (9, 3));
    assert!(result.immediate.is_empty());
}

#[test]
fn crossing_orders_trade_at_the_resting_price() {
    let result = evaluate(
        &traders(),
        vec![sell("bob", 5, 3), buy("ann", 8, 2)],
        vec![],
    );

    assert_eq!(trades(&result), vec![("ann", "bob", 5)]);
    assert_eq!(count(&result.state, "ann", "coin"), 90);
    assert_eq!(count(&result.state, "ann", "rose"), 2);
    assert_eq!(count(&result.state, "bob", "coin"), 10);
    assert_eq!(count(&result.state, "bob", "rose"), 7);
    assert_eq!(result.state.book("rose").unwrap().asks[0].quantity, 1);
}

#[test]
fn asks_are_matched_lowest_first_then_oldest() {
    let orders = vec![
        sell("bob", 6, 1),
        sell("cid", 5, 1),
        sell("bob", 5, 1),
        buy("ann", 6, 3),
    ];

    let result = evaluate(&traders(), orders, vec![]);

    assert_eq!(
        trades(&result),
        vec![("ann", "cid", 5), ("ann", "bob", 5), ("ann", "bob", 6)]
    );
}

#[test]
fn bids_are_matched_highest_first_then_oldest() {
    let orders = vec![
        buy("ann", 5, 1),
        buy("dan", 6, 1),
        buy("ann", 6, 1),
        sell("bob", 5, 3),
    ];

    let result = evaluate(&traders(), orders, vec![]);

    assert_eq!(
        trades(&result),
        vec![("dan", "bob", 6), ("ann", "bob", 6), ("ann", "bob", 5)]
    );
}

#[test]
fn unfilled_remainder_rests_in_the_book() {
    let result = evaluate(
        &traders(),
        vec![sell("bob", 5, 1), buy("ann", 5, 4)],
        vec![],
    );

    let book = result.state.book("rose").unwrap();
    assert!(book.asks.is_empty());
    assert_eq!(book.bids[0].quantity, 3);
    assert_eq!(count(&result.state, "ann", "coin"), 80);
    assert_eq!(count(&result.state, "ann", "rose"), 1);
}

#[test]
fn order_without_funds_is_rejected() {
    let orders = vec![buy("bob", 5, 1), sell("ann", 5, 1), buy("ann", 0, 1)];

    let result = evaluate(&traders(), orders, vec![]);

    assert_eq!(
        reasons(&result),
        vec![
            "price and quantity must be positive",
            "bob does not have enough coin",
            "ann does not have enough rose",
        ]
    );
    assert_eq!(count(&result.state, "bob", "rose"), 10);
}

#[test]
fn cancel_returns_the_escrow() {
    let placed = evaluate(&traders(), vec![buy("ann", 7, 2)], vec![]);
    let order_id = placed.state.book("rose").unwrap().bids[0].id.clone();

    let result = evaluate(&placed.state, vec![cancel("ann", &order_id)], vec![]);

    assert_eq!(count(&result.state, "ann", "coin"), 100);
    assert!(result.state.book("rose").unwrap().bids.is_empty());
}

#[test]
fn only_the_owner_may_cancel() {
    let placed = evaluate(&traders(), vec![buy("ann", 7, 2)], vec![]);
    let order_id = placed.state.book("rose").unwrap().bids[0].id.clone();

    let commands = vec![cancel("bob", &order_id), cancel("ann", "nope")];
    let result = evaluate(&placed.state, commands, vec![]);

    let mut found = reasons(&result);
    found.sort();
    assert_eq!(found, vec!["not your order", "order not found"]);
    assert_eq!(result.state.book("rose").unwrap().bids.len(), 1);
}

#[test]
fn order_expires_after_its_ttl() {
    let mut engine = Engine::new(traders());
    engine.ingest(order("bob", Side::Sell, 9, 4, Some(2)));

    engine.tick();
    assert_eq!(engine.queue.market.len(), 1);
    assert_eq!(count(&engine.state, "bob", "rose"), 6);
    engine.tick();
    assert_eq!(engine.state.book("rose").unwrap().asks.len(), 1);
    engine.tick();

    assert!(engine.state.book("rose").unwrap().asks.is_empty());
    assert_eq!(count(&engine.state, "bob", "rose"), 10);
}

#[test]
fn expiry_of_a_filled_order_does_nothing() {
    let mut engine = Engine::new(traders());
    engine.ingest(order("bob", Side::Sell, 5, 2, Some(1)));
    engine.ingest(buy("ann", 5, 2));
    engine.tick();

    let result = engine.tick();

    assert!(engine.queue.market.is_empty());
    assert!(result.rejections.is_empty());
    assert_eq!(count(&engine.state, "bob", "rose"), 8);
    assert_eq!(count(&engine.state, "bob", "coin"), 10);
}
