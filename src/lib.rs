//! The Magic Garden game engine.
//!
//! The engine advances the world one tick at a time:
//!
//! ```text
//! new_state = evaluate(state, commands, events)
//! ```
//!
//! Start reading in `engine.rs`, then follow a tick through `evaluator.rs`
//! and `resolver.rs`.

pub mod engine;
pub mod entities;
pub mod evaluator;
pub mod flowers;
pub mod market;
pub mod messages;
pub mod queues;
pub mod resolver;
pub mod state;
pub mod transaction;
