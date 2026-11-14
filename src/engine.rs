//! The engine: collects commands, keeps the event queues, and advances the
//! world tick by tick.

use crate::evaluator::{Evaluation, evaluate};
use crate::messages::{Command, Event, Scheduled};
use crate::queues::MainQueue;
use crate::state::State;

pub struct Engine {
    pub state: State,
    pub queue: MainQueue,
    /// Commands waiting for the next tick.
    batch: Vec<Command>,
}

impl Engine {
    pub fn new(state: State) -> Engine {
        Engine {
            state,
            queue: MainQueue::new(),
            batch: Vec::new(),
        }
    }

    /// Accept a command from a user, AI or NPC; it runs in the next tick.
    pub fn ingest(&mut self, command: Command) {
        self.batch.push(command);
    }

    /// Plan a one-off event.
    pub fn schedule(&mut self, scheduled: Scheduled) {
        self.queue.schedule(scheduled);
    }

    /// Fire `event` every `period` ticks, counting from now.
    pub fn every(&mut self, period: u64, event: Event) {
        self.queue.repeat(event, period, self.state.tick + period);
    }

    /// Run one tick: `new_state = evaluate(state, batched commands, due events)`.
    pub fn tick(&mut self) -> Evaluation {
        let commands = std::mem::take(&mut self.batch);
        let events = self.queue.pop_due(self.state.tick);

        let evaluation = evaluate(&self.state, commands, events);

        self.state = evaluation.state.clone();
        for scheduled in &evaluation.future {
            self.queue.schedule(scheduled.clone());
        }
        evaluation
    }
}
