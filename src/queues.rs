//! Event queues: independent market, flower and periodic queues behind one main queue.

use std::collections::BTreeMap;

use crate::messages::{Event, Scheduled};

/// An item taken out of a queue.
pub struct Entry<T> {
    pub due: u64,
    /// Counts up with every push; orders entries that are due at the same tick.
    pub number: u64,
    pub item: T,
}

/// Items waiting for their due tick.
pub struct EventQueue<T> {
    /// Sorted by (due tick, insertion number), so the first entry fires first.
    entries: BTreeMap<(u64, u64), T>,
}

impl<T> EventQueue<T> {
    pub fn new() -> EventQueue<T> {
        EventQueue {
            entries: BTreeMap::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn push(&mut self, due: u64, number: u64, item: T) {
        self.entries.insert((due, number), item);
    }

    /// The due tick of the earliest entry.
    pub fn next_due(&self) -> Option<u64> {
        let ((due, _number), _item) = self.entries.first_key_value()?;
        Some(*due)
    }

    /// Remove and return every entry due at or before `now`.
    pub fn pop_due(&mut self, now: u64) -> Vec<Entry<T>> {
        let mut due_entries = Vec::new();
        while let Some(due) = self.next_due() {
            if due > now {
                break;
            }
            if let Some(((due, number), item)) = self.entries.pop_first() {
                due_entries.push(Entry { due, number, item });
            }
        }
        due_entries
    }
}

impl<T> Default for EventQueue<T> {
    fn default() -> EventQueue<T> {
        EventQueue::new()
    }
}

/// An event that fires again every `period` ticks.
pub struct Repeating {
    pub event: Event,
    pub period: u64,
}

/// The engine's single source of due events, fed by three independent queues.
pub struct MainQueue {
    pub market: EventQueue<Event>,
    pub flowers: EventQueue<Event>,
    pub periodic: EventQueue<Repeating>,
    next_number: u64,
}

impl MainQueue {
    pub fn new() -> MainQueue {
        MainQueue {
            market: EventQueue::new(),
            flowers: EventQueue::new(),
            periodic: EventQueue::new(),
            next_number: 0,
        }
    }

    fn take_number(&mut self) -> u64 {
        let number = self.next_number;
        self.next_number += 1;
        number
    }

    /// Plan a one-off event; it goes to the queue its kind belongs to.
    pub fn schedule(&mut self, scheduled: Scheduled) {
        let number = self.take_number();
        match scheduled.event {
            Event::OrderExpiry { .. } | Event::TradeExecuted { .. } => {
                self.market.push(scheduled.due, number, scheduled.event);
            }
            Event::FlowerGrowth { .. } | Event::FlowerDied { .. } | Event::CompostSweep => {
                self.flowers.push(scheduled.due, number, scheduled.event);
            }
        }
    }

    /// Fire `event` at `first_due` and then every `period` ticks.
    pub fn repeat(&mut self, event: Event, period: u64, first_due: u64) {
        assert!(period >= 1, "period must be at least one tick");
        let number = self.take_number();
        self.periodic
            .push(first_due, number, Repeating { event, period });
    }

    /// Remove and return all events due at or before `now`, in firing order.
    pub fn pop_due(&mut self, now: u64) -> Vec<Event> {
        let mut due_entries = Vec::new();
        due_entries.extend(self.market.pop_due(now));
        due_entries.extend(self.flowers.pop_due(now));

        for entry in self.periodic.pop_due(now) {
            let repeating = entry.item;
            due_entries.push(Entry {
                due: entry.due,
                number: entry.number,
                item: repeating.event.clone(),
            });
            // Put it back for its next round.
            let number = self.take_number();
            self.periodic
                .push(entry.due + repeating.period, number, repeating);
        }

        due_entries.sort_by_key(|entry| (entry.due, entry.number));

        let mut events = Vec::new();
        for entry in due_entries {
            events.push(entry.item);
        }
        events
    }

    /// The tick of the earliest waiting event, if any.
    pub fn next_due(&self) -> Option<u64> {
        let candidates = [
            self.market.next_due(),
            self.flowers.next_due(),
            self.periodic.next_due(),
        ];
        candidates.into_iter().flatten().min()
    }
}

impl Default for MainQueue {
    fn default() -> MainQueue {
        MainQueue::new()
    }
}
