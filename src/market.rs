//! The market: one limit order book per item, matched by price-time priority.
//!
//! Placing an order puts its full cost in escrow: coins for a buy, the items
//! for a sell. Trades happen at the price of the order that was resting in
//! the book.

use crate::messages::{Event, PlaceOrder, Scheduled};
use crate::state::{
    COIN, Entity, Key, Order, OrderBook, Rejected, Side, State, Value, expect_book, expect_entity,
};
use crate::transaction::{Modify, Transaction};

// --- matching: plain functions on an order book -------------------------------

/// Part of an incoming order traded against one resting order.
#[derive(Debug, Clone, PartialEq)]
pub struct Fill {
    /// The owner of the resting order.
    pub maker: String,
    pub price: u32,
    pub quantity: u32,
}

/// Trade `taker` against the opposite side of `book`, then rest whatever is left of it.
pub fn match_order(book: &mut OrderBook, taker: &Order) -> Vec<Fill> {
    let mut fills = Vec::new();
    let mut remaining = taker.quantity;

    let opposite = match taker.side {
        Side::Buy => &mut book.asks,
        Side::Sell => &mut book.bids,
    };

    // The best resting order is always first in the list.
    while remaining > 0 && !opposite.is_empty() && crosses(taker, &opposite[0]) {
        let maker = &mut opposite[0];
        let quantity = remaining.min(maker.quantity);
        fills.push(Fill {
            maker: maker.owner.clone(),
            price: maker.price,
            quantity,
        });
        remaining -= quantity;
        maker.quantity -= quantity;
        if maker.quantity == 0 {
            opposite.remove(0);
        }
    }

    if remaining > 0 {
        let mut rest = taker.clone();
        rest.quantity = remaining;
        rest_order(book, rest);
    }
    fills
}

/// Can these two orders trade with each other?
fn crosses(taker: &Order, maker: &Order) -> bool {
    match taker.side {
        Side::Buy => taker.price >= maker.price,
        Side::Sell => taker.price <= maker.price,
    }
}

/// Put an order into the book behind every order with a price at least as
/// good: sorted by price first, then by time of arrival.
fn rest_order(book: &mut OrderBook, order: Order) {
    let queue = match order.side {
        Side::Buy => &mut book.bids,
        Side::Sell => &mut book.asks,
    };

    let mut index = queue.len();
    for (i, resting) in queue.iter().enumerate() {
        let resting_is_worse = match order.side {
            Side::Buy => resting.price < order.price,
            Side::Sell => resting.price > order.price,
        };
        if resting_is_worse {
            index = i;
            break;
        }
    }
    queue.insert(index, order);
}

/// Give the next order id of the book to a new order built from `command`.
fn new_order(book: &mut OrderBook, command: &PlaceOrder) -> Order {
    let id = format!("{}-{}", book.item, book.next_order_number);
    book.next_order_number += 1;
    Order {
        id,
        owner: command.actor.clone(),
        side: command.side,
        price: command.price,
        quantity: command.quantity,
    }
}

// --- settlement: moving coins and items between inventories -------------------

fn give(entities: &mut [Entity], owner: &str, item: &str, amount: u32) -> Result<(), Rejected> {
    for entity in entities.iter_mut() {
        if entity.id == owner {
            entity.add(item, amount);
            return Ok(());
        }
    }
    Err(Rejected::new("entity not found"))
}

fn take(entities: &mut [Entity], owner: &str, item: &str, amount: u32) -> Result<(), Rejected> {
    for entity in entities.iter_mut() {
        if entity.id == owner {
            return entity.remove(item, amount);
        }
    }
    Err(Rejected::new("entity not found"))
}

/// Escrow the taker's whole order, then pay out each fill to both parties.
fn settle(
    entities: &mut [Entity],
    item: &str,
    taker: &Order,
    fills: &[Fill],
) -> Result<(), Rejected> {
    match taker.side {
        Side::Buy => {
            take(entities, &taker.owner, COIN, taker.price * taker.quantity)?;
            for fill in fills {
                give(entities, &taker.owner, item, fill.quantity)?;
                // The taker bid more than the maker asked: return the difference.
                let saved = (taker.price - fill.price) * fill.quantity;
                give(entities, &taker.owner, COIN, saved)?;
                give(entities, &fill.maker, COIN, fill.price * fill.quantity)?;
            }
        }
        Side::Sell => {
            take(entities, &taker.owner, item, taker.quantity)?;
            for fill in fills {
                give(entities, &taker.owner, COIN, fill.price * fill.quantity)?;
                give(entities, &fill.maker, item, fill.quantity)?;
            }
        }
    }
    Ok(())
}

// --- handlers ------------------------------------------------------------------

/// Escrow the order's cost, trade what crosses, rest the remainder.
pub fn place_order(state: &State, command: &PlaceOrder) -> Result<Transaction, Rejected> {
    if command.price == 0 || command.quantity == 0 {
        return Err(Rejected::new("price and quantity must be positive"));
    }

    // Try the order on a copy of the book, to learn whose inventories the
    // trades will touch and which events will follow.
    let mut seen_book = match state.book(&command.item) {
        Some(book) => book.clone(),
        None => OrderBook::new(&command.item),
    };
    let order = new_order(&mut seen_book, command);
    let fills = match_order(&mut seen_book, &order);

    let mut owners = vec![command.actor.clone()];
    for fill in &fills {
        if !owners.contains(&fill.maker) {
            owners.push(fill.maker.clone());
        }
    }
    let mut keys = vec![Key::Book(command.item.clone())];
    for owner in &owners {
        keys.push(Key::Entity(owner.clone()));
    }

    // The real change: the same steps again, on the transaction's inputs.
    let command_copy = command.clone();
    let modify: Modify = Box::new(move |inputs: &[Option<Value>]| {
        let command = &command_copy;
        let mut book = match &inputs[0] {
            Some(_) => expect_book(&inputs[0])?,
            None => OrderBook::new(&command.item),
        };
        let mut entities = Vec::new();
        for input in &inputs[1..] {
            entities.push(expect_entity(input)?);
        }

        let order = new_order(&mut book, command);
        let fills = match_order(&mut book, &order);
        settle(&mut entities, &command.item, &order, &fills)?;

        let mut outputs = vec![Some(Value::Book(book))];
        for entity in entities {
            outputs.push(Some(Value::Entity(entity)));
        }
        Ok(outputs)
    });

    let mut transaction = Transaction::new(keys.clone(), keys, modify);

    for fill in &fills {
        let (buyer, seller) = match command.side {
            Side::Buy => (command.actor.clone(), fill.maker.clone()),
            Side::Sell => (fill.maker.clone(), command.actor.clone()),
        };
        transaction.immediate.push(Event::TradeExecuted {
            item: command.item.clone(),
            price: fill.price,
            quantity: fill.quantity,
            buyer,
            seller,
        });
    }

    let order_rests = seen_book.find(&order.id).is_some();
    if let Some(ttl) = command.ttl
        && order_rests
    {
        transaction.future.push(Scheduled {
            due: state.tick + ttl,
            event: Event::OrderExpiry {
                item: command.item.clone(),
                order_id: order.id.clone(),
            },
        });
    }

    Ok(transaction)
}

/// Take an order out of the book and return its escrow to its owner.
///
/// `canceller` is the actor asking for it; only the owner may cancel.
/// It is `None` when the order simply expired.
/// Returns `None` when the order is not in the book (any more).
pub fn remove_order(
    state: &State,
    item: &str,
    order_id: &str,
    canceller: Option<&str>,
) -> Option<Transaction> {
    let seen_order = state.book(item)?.find(order_id)?;
    let keys = vec![
        Key::Book(item.to_string()),
        Key::Entity(seen_order.owner.clone()),
    ];

    let item = item.to_string();
    let order_id = order_id.to_string();
    let canceller = canceller.map(|actor| actor.to_string());

    let modify: Modify = Box::new(move |inputs: &[Option<Value>]| {
        let mut book = expect_book(&inputs[0])?;
        let mut owner = expect_entity(&inputs[1])?;

        let order = match book.find(&order_id) {
            Some(order) => order.clone(),
            None => return Err(Rejected::new("order not found")),
        };
        if let Some(actor) = &canceller
            && *actor != owner.id
        {
            return Err(Rejected::new("not your order"));
        }

        book.bids.retain(|bid| bid.id != order_id);
        book.asks.retain(|ask| ask.id != order_id);
        match order.side {
            Side::Buy => owner.add(COIN, order.price * order.quantity),
            Side::Sell => owner.add(&item, order.quantity),
        }

        Ok(vec![Some(Value::Book(book)), Some(Value::Entity(owner))])
    });

    Some(Transaction::new(keys.clone(), keys, modify))
}
