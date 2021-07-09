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
mod reader;
mod shared_state;
mod writer;
use change::Change;

pub(self) use shared_state::*;

pub use {eventual::Eventual, reader::EventualReader, writer::EventualWriter};
