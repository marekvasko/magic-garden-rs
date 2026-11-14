//! Everything the engine reacts to: commands from actors and events from the world.

use crate::state::{Position, Side};

// --- commands: requests from a user, AI or NPC -------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct PlaceOrder {
    pub actor: String,
    pub item: String,
    pub side: Side,
    pub price: u32,
    pub quantity: u32,
    /// Ticks an unfilled order stays in the book; `None` means until cancelled.
    pub ttl: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CancelOrder {
    pub actor: String,
    pub item: String,
    pub order_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlantFlower {
    pub actor: String,
    pub flower_id: String,
    pub species: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CutFlower {
    pub actor: String,
    pub flower_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UseEntity {
    pub actor: String,
    pub entity_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReleaseEntity {
    pub actor: String,
    pub entity_id: String,
}

/// Move the actor itself, or a carrier it is using (the actor rides along).
#[derive(Debug, Clone, PartialEq)]
pub struct MoveEntity {
    pub actor: String,
    pub entity_id: String,
    pub to: Position,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransferItems {
    pub actor: String,
    pub source: String,
    pub target: String,
    pub item: String,
    pub quantity: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    PlaceOrder(PlaceOrder),
    CancelOrder(CancelOrder),
    PlantFlower(PlantFlower),
    CutFlower(CutFlower),
    UseEntity(UseEntity),
    ReleaseEntity(ReleaseEntity),
    MoveEntity(MoveEntity),
    TransferItems(TransferItems),
}

// --- events: things that happen in the world ---------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// A resting order ran out of time. Waits in the market queue.
    OrderExpiry { item: String, order_id: String },

    /// A flower is ready for its next stage. Waits in the flower queue.
    FlowerGrowth { flower_id: String },

    /// Dead flowers are cleared away. Usually repeats in the periodic queue.
    CompostSweep,

    /// Notification: a trade happened. Nothing reacts to it.
    TradeExecuted {
        item: String,
        price: u32,
        quantity: u32,
        buyer: String,
        seller: String,
    },

    /// Notification: a flower died. Nothing reacts to it.
    FlowerDied { flower_id: String },
}

/// Anything that can be dispatched to a handler.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    Command(Command),
    Event(Event),
}

/// An event planned for a future tick.
#[derive(Debug, Clone, PartialEq)]
pub struct Scheduled {
    pub due: u64,
    pub event: Event,
}
