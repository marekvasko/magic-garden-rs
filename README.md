# magic-garden-rs

The [Magic Garden](https://github.com/marekvasko/magic-garden) game engine,
written again in Rust. It has the same design and the same rules as the Python
engine, and is written to be easy to read: no dependencies, no macros of its own, no
custom traits, no generics beyond one small queue type, and plain loops and `match`
wherever they do the job.

## The engine

The engine advances the world one tick at a time, strictly as

    new_state = evaluate(state, commands, events)

```
 user / AI / NPC                market queue   flower queue   periodic queue
       │ ingest                        └────────────┬──────────────┘
       ▼                                            ▼
  command batch                                 main queue
       └───────────────┬────────────────────────────┘ due events
                       ▼
        ┌─ evaluate(state, commands, events) ───────────────┐
        │  dispatch: each message → its handler             │
        │  handler:  → Transaction(reads, writes, modify)   │◄─┐ deferred messages,
        │  resolver: validate → deconflict → atomic commit  │──┘ immediate events
        └──────────────────────┬────────────────────────────┘
                               ▼
        new state · immediate events · future events (→ queues) · rejections
```

## Where to start reading

Read the files in this order; each one is short.

| File                 | Role                                                                     |
|----------------------|--------------------------------------------------------------------------|
| `src/state.rs`       | the state (a key-value store) and the objects in it                      |
| `src/messages.rs`    | all commands and events                                                  |
| `src/engine.rs`      | `Engine`: ingests and batches commands, owns the queues, runs ticks      |
| `src/queues.rs`      | market, flower and periodic queues behind one `MainQueue`                |
| `src/evaluator.rs`   | the pure `evaluate` function and the message → handler dispatch          |
| `src/transaction.rs` | `Transaction`: keys read, keys written, `modify(inputs) -> outputs`      |
| `src/resolver.rs`    | validates, deconflicts and atomically commits transactions               |
| `src/market.rs`      | limit order books with price-time priority matching                      |
| `src/flowers.rs`     | planting, growth, cutting, dying                                         |
| `src/entities.rs`    | actors and carriers (truck, sled, wheelbarrow): use slot, move, transfer |

### How a tick is resolved

Handlers build their transactions from a snapshot of the state. The resolver
walks them in order; a transaction touching a key already claimed by an earlier
one in the same round is *deferred* - its message is dispatched again, on fresh
state, in the next round. So the result is always the same as running the
messages one after another, due events first, then commands in ingest order.
A transaction whose `modify` returns `Rejected` is reported and changes nothing.

### Game rules in short

- **Market** - placing an order escrows its cost (coins for a buy, items for a
  sell). Crossing orders trade at the resting order's price, best price first,
  oldest first. Orders can be cancelled or given a time to live.
- **Flowers** - a seed grows to a sprout and then a bloom. Cutting a bloom puts
  one flower in the owner's inventory; the plant regrows, but survives only a
  limited number of cuts and then dies. A periodic compost sweep removes the dead.
- **Entities** - actors and carriers have a position and an inventory. A carrier
  has one use slot; the actor holding it can drive it (riding along) and unload it.

## Usage

See [`examples/quickstart.rs`](examples/quickstart.rs):

```sh
cargo run --example quickstart
```

## Development

```sh
cargo test
cargo clippy --all-targets
cargo fmt --check
```
