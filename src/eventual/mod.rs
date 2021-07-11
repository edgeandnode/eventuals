use super::Closed;
use std::{
    future::Future,
    ops::Deref,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

mod change;
mod eventual;
mod eventual_ext;
mod reader;
mod shared_state;
mod writer;
use change::Change;

pub(self) use {crate::Value, shared_state::*};

pub use {
    eventual::Eventual, eventual_ext::EventualExt, reader::EventualReader, writer::EventualWriter,
};
