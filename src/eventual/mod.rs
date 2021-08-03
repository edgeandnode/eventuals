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
mod ptr;
mod reader;
mod shared_state;
mod writer;

pub(self) use {crate::Value, shared_state::*};

pub use {
    eventual::Eventual,
    eventual_ext::{EventualExt, TryEventualExt},
    ptr::Ptr,
    reader::EventualReader,
    writer::EventualWriter,
};

#[cfg(feature = "trace")]
pub use change::idle;
