use magic_garden_rs::messages::{Event, Scheduled};
use magic_garden_rs::queues::MainQueue;

fn growth(flower_id: &str) -> Event {
    Event::FlowerGrowth {
        flower_id: flower_id.to_string(),
    }
}

fn expiry() -> Event {
    Event::OrderExpiry {
        item: "rose".to_string(),
        order_id: "rose-1".to_string(),
    }
}

fn at(due: u64, event: Event) -> Scheduled {
    Scheduled { due, event }
}

#[test]
fn events_go_to_their_own_queue() {
    let mut queue = MainQueue::new();
    queue.schedule(at(3, expiry()));
    queue.schedule(at(3, growth("f1")));
    queue.repeat(Event::CompostSweep, 10, 10);

    assert_eq!(queue.market.len(), 1);
    assert_eq!(queue.flowers.len(), 1);
    assert_eq!(queue.periodic.len(), 1);
}

#[test]
fn pop_due_merges_queues_by_tick_then_scheduling_order() {
    let mut queue = MainQueue::new();
    queue.schedule(at(2, growth("late")));
    queue.schedule(at(1, expiry()));
    queue.schedule(at(1, growth("f")));
    queue.schedule(at(5, growth("not yet")));

    assert_eq!(
        queue.pop_due(2),
        vec![expiry(), growth("f"), growth("late")]
    );
    assert_eq!(queue.pop_due(2), vec![]);
    assert_eq!(queue.next_due(), Some(5));
}

#[test]
fn periodic_events_repeat() {
    let mut queue = MainQueue::new();
    queue.repeat(Event::CompostSweep, 3, 3);

    let mut fired = Vec::new();
    for tick in 0..10 {
        if !queue.pop_due(tick).is_empty() {
            fired.push(tick);
        }
    }

    assert_eq!(fired, vec![3, 6, 9]);
}

#[test]
#[should_panic(expected = "period must be at least one tick")]
fn period_must_be_positive() {
    MainQueue::new().repeat(Event::CompostSweep, 0, 0);
}

#[test]
fn next_due_of_empty_queue() {
    assert_eq!(MainQueue::new().next_due(), None);
}
