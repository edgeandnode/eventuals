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

    Eventual::spawn(|mut writer| async move {
        while let Ok(v) = source.next().await {
            writer.write(f(v).await)?;
        }
        Ok(())
    })
}

pub fn timer(interval: Duration) -> Eventual<Instant> {
    Eventual::spawn(move |mut writer| async move {
        loop {
            writer.write(Instant::now())?;
            tokio::time::sleep(interval).await;
        }
    })
}
