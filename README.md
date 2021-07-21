Eventuals give you the most up-to-date snapshots of some value. They are like Futures that update over time, continually resolving to an eventually consistent state. You can chain eventuals together, inspect their present value, and subscribe for updates. The eventual paradigm is the culmination of many hard lessons in asynchronous programming.


Consider this code using an imaginary set_interval inspired by JavaScript:
```rust
set_interval(|| async { let update = fetch().await; set_ui_data(update); }, 2000)
```

The above is a typical pattern used to update a UI from external data over time. There are a couple of problems with this code. First, the fetch operations may overlap. This overlap can result in stale data being shown in the UI after new data! Furthermore, there is no clear way to cancel - either from the reading or the writing perspective.


Eventuals solve these and other common issues!

```rust

let cancel_on_drop = timer(2000).map(|_| fetch).pipe(set_ui_update);

```

Here, `set_ui_update` is guaranteed to progress only forward in time. Different pipeline stages may execute concurrently, and some updates may be skipped if newer values are available. Still, the eventual state will always be consistent with the final writes to data.


# Subscriptions

Subscriptions are views of the latest update of an eventual from the reader's perspective. A subscription will only observe an update if the value has changed since the last observation and may skip updates between the previous and current observations. Taken together, this ensures that any reader performs the least amount of work necessary to be eventually consistent with the state of the writer.


You can get a subscription by calling `eventual.subscribe()` and `subscription.next().await` any time you are ready to process an update. If there has been an update since the previously processed update the Future is ready immediately, otherwise it will resolve as soon as an update is available.


# Snapshots
There are two ways to get the current value of an eventual without a subscription. These are `eventual.value_immediate()` and `eventual.value`, depending on whether you want to wait for the first update.

# Unit tests

One useful application of eventuals is to make components testable without mocks. Instead of making a mock server (which requires traits) you can supply your component with an eventual and feed the component data. It's not dependency injection. It's data injection.


# FAQ

    What is the difference between an eventual and a Stream?

 A Stream is a collection of distinct, ordered values that are made available asynchronously. An eventual, however, is an asynchronous view of a single value changing over time.


    The API seems sparse. I see map and join, but where are filter, reduce, and select?

It is natural to want to apply the full range of functional techniques to eventuals, but this is an anti-pattern. The goal of eventuals is to make the final value of any view of any data pipeline deterministically consistent with the latest write. The path to producing a result may not be deterministic, but the final value is consistent if each building block is also consistent. Not every functional block passes this test. To demonstrate, we can compare the behavior of map and an imaginary filter combinator on the same sequence of writes.


Consider `eventual.map(|n| async move { n * 2 })` and `eventual.filter(|n| async move { n % 2 == 0 })` both consuming the series writes `[0, 1, 2, 3, 4, 5]` over time. `map`, in this case, will always eventually resolve to the value `10`. It may also produce some subset of intermediate values along the way (like `[0, 4, 8, 10]` or `[2, 10]`). Still, it will always progress forward in time and always resolves to a deterministic value consistent with the final write. However, the final value that `filter` produces would be a function of which intermediate values were observed. If filter observes the subset of writes  `[0, 2, 5]`, it will resolve to `2`, but if it observes the subset of writes `[1, 4, 5]`, it will resolve to `4`. This bug is exactly the kind eventuals are trying to avoid.
