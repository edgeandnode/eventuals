use by_address::ByAddress;
use std::{
    borrow::Borrow, cmp::Ordering, convert::AsRef, error::Error, fmt, hash::Hash, ops::Deref,
    sync::Arc,
};

/// This type is a thin wrapper around T to enable cheap clone and comparisons.
/// Internally it is an Arc that is compared by address instead of by the
/// implementation of the pointed to value.
///
/// Additionally, Ptr implements Error where T: Error. This makes working with
/// TryEventuals easier since many error types do not impl Value, but when
/// wrapped in Ptr do.
///
/// One thing to be aware of is that because values are compared by address
/// subscribers may be triggered unnecessarily in some contexts. If this is
/// undesirable use Arc instead.
///
/// This type is not specifically Eventual related, but is a useful pattern.
#[repr(transparent)]
#[derive(Debug, Default)]
pub struct Ptr<T> {
    inner: ByAddress<Arc<T>>,
}

impl<T> Ptr<T> {
    #[inline]
    pub fn new(wrapped: T) -> Self {
        Self {
            inner: ByAddress(Arc::new(wrapped)),
        }
    }
}

impl<T> Deref for Ptr<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}

impl<T> Borrow<T> for Ptr<T> {
    #[inline]
    fn borrow(&self) -> &T {
        self.inner.borrow()
    }
}

impl<T> AsRef<T> for Ptr<T> {
    #[inline]
    fn as_ref(&self) -> &T {
        self.inner.as_ref()
    }
}

impl<T> Hash for Ptr<T> {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}

impl<T> Ord for Ptr<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.inner.cmp(&other.inner)
    }
}

impl<T> PartialOrd for Ptr<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.inner.partial_cmp(&other.inner)
    }
}

impl<T> PartialEq for Ptr<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.inner.eq(&other.inner)
    }
}

impl<T> Clone for Ptr<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T> Eq for Ptr<T> {}

impl<T> fmt::Display for Ptr<T>
where
    T: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl<T> Error for Ptr<T>
where
    T: Error,
{
    // TODO: Consider. Ptr is supposed to be a thin wrapper around error so it
    // can be "transient" and treated as an error. But this API offers the
    // option of making it be more like a wrapped error that acknowledges it
    // originated from the original error. Semantically that seems like a
    // different thing (Ptr<Error> is not a new error caused by a previous one!)
    // but at the same time that might make the backtrace accessible whereas
    // here it cannot be because backtrace is a nightly API.
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.inner.source()
    }
}

impl<T> From<T> for Ptr<T> {
    #[inline]
    fn from(t: T) -> Self {
        Self::new(t)
    }
}
