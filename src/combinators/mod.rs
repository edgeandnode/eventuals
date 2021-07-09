use crate::*;
use std::future::Future;
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

    let (mut writer, readers) = Eventual::new();
    tokio::spawn(async move {
        while let Ok(v) = source.next().await {
            writer.write(f(v).await);
        }
    });
    readers
}
