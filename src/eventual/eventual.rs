use crate::IntoReader;

use super::shared_state::SharedState;
use super::*;
use tokio::select;

pub struct Eventual<T> {
    state: Arc<SharedState<T>>,
}

impl<T> Eventual<T>
where
    T: Value,
{
    pub fn new() -> (EventualWriter<T>, Self) {
        let state = Arc::new(SharedState::new());
        (EventualWriter::new(&state), Eventual { state })
    }

    // TODO: Probably delete this if it doesn't work out.
    pub fn spawn_loop<F, Fut>(mut f: F) -> Self
    where
        F: 'static + Send + FnMut() -> Fut,
        Fut: Future<Output = Result<T, Closed>> + Send,
    {
        Eventual::spawn(move |mut writer| async move {
            while let Ok(v) = f().await {
                writer.write(v)
            }
        })
    }

    pub fn spawn<F, Fut>(f: F) -> Self
    where
        F: 'static + Send + FnOnce(EventualWriter<T>) -> Fut,
        Fut: Future<Output = ()> + Send,
    {
        let (writer, eventual) = Eventual::new();
        tokio::spawn(async move {
            // This would return an error if the readers
            // are dropped. Ok to ignore this.
            // TODO: Enable closed waiting
            //let closed = writer.closed();

            select!(
                _ = async { f(writer).await }  => {}
                //_ = closed => {}
            );
        });
        eventual
    }
}

impl<T> Eventual<T>
where
    T: Value,
{
    pub fn subscribe(&self) -> EventualReader<T> {
        EventualReader::new(self.state.clone())
    }
}

impl<T> Clone for Eventual<T> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}

impl<T> IntoReader for &'_ Eventual<T>
where
    T: Value,
{
    type Output = T;
    fn into_reader(self) -> EventualReader<Self::Output> {
        self.subscribe()
    }
}
