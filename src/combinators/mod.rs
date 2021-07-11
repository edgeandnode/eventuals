use crate::*;
use std::time::Duration;
use std::{future::Future, time::Instant};
use tokio;

pub fn map<E, I, O, F, Fut>(source: E, mut f: F) -> Eventual<O>
where
    E: IntoReader<Output = I>,
    F: 'static + Send + FnMut(I) -> Fut,
    I: Value,
    O: Value,
    Fut: Send + Future<Output = O>,
{
    let mut source = source.into_reader();

    // TODO: This probably isn't going to work. Borrow checker for the first,
    // and the second isn't the correct signature for TryFutureExt
    //Eventual::spawn_loop(move || async move { Ok(f(source.next().await?).await) })
    //Eventual::spawn_loop(move || source.next().map_ok(f))

    Eventual::spawn(|mut writer| async move {
        while let Ok(v) = source.next().await {
            writer.write(f(v).await);
        }
    })
}

pub fn timer(interval: Duration) -> Eventual<Instant> {
    Eventual::spawn(move |mut writer| async move {
        loop {
            writer.write(Instant::now());
            tokio::time::sleep(interval).await;
        }
    })
}

// TODO: Add join and throttle
//
// TODO: Add pipe? The "GC" semantics make this unclear. The idea
// behind pipe is to produce some side effect, which is a desirable
// end goal for eventuals (eg: pipe this value into a UI, or log the latest)
// but the part that is not clear here is what to do when the UI goes out of
// scope. Should pipe provide an explicit handle that cancels on drop?
//
// TODO: Probably do not add filter or reduce, they don't make as much sense for eventuals.
