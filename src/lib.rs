mod eventual;
pub use eventual::*;
pub mod error;
pub use error::Closed;
mod combinators;
pub use combinators::*;

// This is a convenience trait to make it easy to pass either an Eventual or an
// EventualReader into functions.
pub trait IntoReader {
    type Output: Value;
    fn into_reader(self) -> EventualReader<Self::Output>;
}

pub trait Value: 'static + Send + Clone + Eq {}
impl<T> Value for T where T: 'static + Send + Clone + Eq {}
