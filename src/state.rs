//! The world state: a key-value store holding every game object.
//!
//! Every object lives under one key. Transactions name the keys they read
//! and write, which is how the resolver detects conflicts between them.

use std::collections::BTreeMap;

/// The inventory item used as money.
pub const COIN: &str = "coin";

/// A command or event broke a game rule; it changes nothing.
#[derive(Debug, Clone, PartialEq)]
pub struct Rejected {
    pub reason: String,
}

impl Rejected {
    pub fn new(reason: &str) -> Rejected {
        Rejected {
            reason: reason.to_string(),
        }
    }
}

// --- entities ----------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EntityKind {
    /// A player, AI or NPC.
    Actor,
    Truck,
    Sled,
    Wheelbarrow,
}

/// Anything with a position and an inventory.
///
/// Carriers (truck, sled, wheelbarrow) also have a use slot: the one actor
/// currently operating them.
#[derive(Debug, Clone, PartialEq)]
pub struct Entity {
    pub id: String,
    pub kind: EntityKind,
    pub position: Position,
    /// Item name -> quantity. Items with quantity zero are not stored.
    pub inventory: BTreeMap<String, u32>,
    pub used_by: Option<String>,
}

impl Entity {
    /// A new entity with an empty inventory that nobody is using.
    pub fn new(id: &str, kind: EntityKind, position: Position) -> Entity {
        Entity {
            id: id.to_string(),
            kind,
            position,
            inventory: BTreeMap::new(),
            used_by: None,
        }
    }

    /// How many of `item` the entity holds.
    pub fn count(&self, item: &str) -> u32 {
        match self.inventory.get(item) {
            Some(quantity) => *quantity,
            None => 0,
        }
    }

    /// Put `amount` of `item` into the inventory.
    pub fn add(&mut self, item: &str, amount: u32) {
        let quantity = self.count(item) + amount;
        if quantity > 0 {
            self.inventory.insert(item.to_string(), quantity);
        }
    }

    /// Take `amount` of `item` out of the inventory, if there is enough.
    pub fn remove(&mut self, item: &str, amount: u32) -> Result<(), Rejected> {
        let quantity = self.count(item);
        if quantity < amount {
            let reason = format!("{} does not have enough {}", self.id, item);
            return Err(Rejected::new(&reason));
        }
        if quantity == amount {
            self.inventory.remove(item);
        } else {
            self.inventory.insert(item.to_string(), quantity - amount);
        }
        Ok(())
    }
}

// --- flowers -----------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Stage {
    Seed,
    Sprout,
    Bloom,
    Dead,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Flower {
    pub id: String,
    pub species: String,
    pub owner: String,
    pub position: Position,
    pub stage: Stage,
    /// How many more times the flower survives being cut.
    pub cuts_left: u32,
}

// --- market ------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Side {
    Buy,
    Sell,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Order {
    pub id: String,
    pub owner: String,
    pub side: Side,
    pub price: u32,
    pub quantity: u32,
}

/// Resting limit orders for one item, each side sorted best first.
///
/// Within one price the oldest order comes first (price-time priority).
#[derive(Debug, Clone, PartialEq)]
pub struct OrderBook {
    pub item: String,
    pub bids: Vec<Order>,
    pub asks: Vec<Order>,
    pub next_order_number: u32,
}

impl OrderBook {
    /// An empty book.
    pub fn new(item: &str) -> OrderBook {
        OrderBook {
            item: item.to_string(),
            bids: Vec::new(),
            asks: Vec::new(),
            next_order_number: 1,
        }
    }

    /// Look for an order on either side of the book.
    pub fn find(&self, order_id: &str) -> Option<&Order> {
        let mut all_orders = self.bids.iter().chain(self.asks.iter());
        all_orders.find(|order| order.id == order_id)
    }
}

// --- the store ---------------------------------------------------------------

/// The name under which one object is stored.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Key {
    /// An entity, by its id.
    Entity(String),
    /// A flower, by its id.
    Flower(String),
    /// An order book, by the item it trades.
    Book(String),
}

/// One stored object.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Entity(Entity),
    Flower(Flower),
    Book(OrderBook),
}

impl Value {
    /// The key this object belongs under.
    pub fn key(&self) -> Key {
        match self {
            Value::Entity(entity) => Key::Entity(entity.id.clone()),
            Value::Flower(flower) => Key::Flower(flower.id.clone()),
            Value::Book(book) => Key::Book(book.item.clone()),
        }
    }
}

/// The whole world at the start of one tick.
#[derive(Debug, Clone, PartialEq)]
pub struct State {
    pub tick: u64,
    pub values: BTreeMap<Key, Value>,
}

impl State {
    /// An empty world at tick zero.
    pub fn new() -> State {
        State {
            tick: 0,
            values: BTreeMap::new(),
        }
    }

    /// Store an object under its own key, replacing any previous one.
    pub fn insert(&mut self, value: Value) {
        self.values.insert(value.key(), value);
    }

    pub fn entity(&self, id: &str) -> Option<&Entity> {
        match self.values.get(&Key::Entity(id.to_string())) {
            Some(Value::Entity(entity)) => Some(entity),
            _ => None,
        }
    }

    pub fn flower(&self, id: &str) -> Option<&Flower> {
        match self.values.get(&Key::Flower(id.to_string())) {
            Some(Value::Flower(flower)) => Some(flower),
            _ => None,
        }
    }

    pub fn book(&self, item: &str) -> Option<&OrderBook> {
        match self.values.get(&Key::Book(item.to_string())) {
            Some(Value::Book(book)) => Some(book),
            _ => None,
        }
    }
}

impl Default for State {
    fn default() -> State {
        State::new()
    }
}

// --- reading transaction inputs ----------------------------------------------
//
// A transaction's `modify` function receives its inputs as `Option<Value>`.
// These helpers turn one input into the type the function needs, rejecting
// the transaction when the object is missing.

pub fn expect_entity(input: &Option<Value>) -> Result<Entity, Rejected> {
    match input {
        Some(Value::Entity(entity)) => Ok(entity.clone()),
        _ => Err(Rejected::new("entity not found")),
    }
}

pub fn expect_flower(input: &Option<Value>) -> Result<Flower, Rejected> {
    match input {
        Some(Value::Flower(flower)) => Ok(flower.clone()),
        _ => Err(Rejected::new("flower not found")),
    }
}

pub fn expect_book(input: &Option<Value>) -> Result<OrderBook, Rejected> {
    match input {
        Some(Value::Book(book)) => Ok(book.clone()),
        _ => Err(Rejected::new("order book not found")),
    }
}
