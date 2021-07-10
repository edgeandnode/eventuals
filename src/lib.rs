mod eventual;
pub use eventual::{Eventual, EventualReader, EventualWriter};
pub mod error;
pub use error::Closed;
mod combinators;
pub use combinators::*;

// This is a convenience trait to make it easy to pass either an Eventual or an
// EventualReader into functions.
// TODO: Implement
pub trait IntoReader {
    type Output;
    fn into_reader(self) -> EventualReader<Self::Output>;
}

pub trait Value: 'static + Send + Clone + Eq {}
impl<T> Value for T where T: 'static + Send + Clone + Eq {}

// This is the goal:
/*
fn log_errors(logger: Logger, source: Eventual<Result<T, Err>>) -> Eventual<T> {
    let out = Eventual::new();
    tokio::spawn(async move {
        loop {
            match source.next().await {
                Ok(Ok(v)) => out.write(v),
                Ok(Err(e)) => error!(logger, e),
                Err(_) => break;
            }
        }
    });
    out
}
*/
