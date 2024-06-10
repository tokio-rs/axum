use pin_project_lite::pin_project;
use std::{ops::Deref, sync::Arc};

pub(crate) use self::mutex::*;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct PercentDecodedStr(Arc<str>);

impl PercentDecodedStr {
    pub(crate) fn new<S>(s: S) -> Option<Self>
    where
        S: AsRef<str>,
    {
        percent_encoding::percent_decode(s.as_ref().as_bytes())
            .decode_utf8()
            .ok()
            .map(|decoded| Self(decoded.as_ref().into()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl Deref for PercentDecodedStr {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

pin_project! {
    #[project = EitherProj]
    pub(crate) enum Either<A, B> {
        A { #[pin] inner: A },
        B { #[pin] inner: B },
    }
}

pub(crate) fn try_downcast<T, K>(k: K) -> Result<T, K>
where
    T: 'static,
    K: Send + 'static,
{
    let mut k = Some(k);
    if let Some(k) = <dyn std::any::Any>::downcast_mut::<Option<T>>(&mut k) {
        Ok(k.take().unwrap())
    } else {
        Err(k.unwrap())
    }
}

#[test]
fn test_try_downcast() {
    assert_eq!(try_downcast::<i32, _>(5_u32), Err(5_u32));
    assert_eq!(try_downcast::<i32, _>(5_i32), Ok(5_i32));
}

// `AxumMutex` is a wrapper around `std::sync::Mutex` which, in test mode, tracks the number of
// times it's been locked on the current task. That way we can write a test to ensure we don't
// accidentally introduce more locking.
//
// When not in test mode, it is just a type alias for `std::sync::Mutex`.
#[cfg(not(test))]
mod mutex {
    #[allow(clippy::disallowed_types)]
    pub(crate) type AxumMutex<T> = std::sync::Mutex<T>;
}

#[cfg(test)]
#[allow(clippy::disallowed_types)]
mod mutex {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        LockResult, Mutex, MutexGuard,
    };

    tokio::task_local! {
        pub(crate) static NUM_LOCKED: AtomicUsize;
    }

    pub(crate) async fn mutex_num_locked<F, Fut>(f: F) -> (usize, Fut::Output)
    where
        F: FnOnce() -> Fut,
        Fut: std::future::IntoFuture,
    {
        NUM_LOCKED
            .scope(AtomicUsize::new(0), async move {
                let output = f().await;
                let num = NUM_LOCKED.with(|num| num.load(Ordering::SeqCst));
                (num, output)
            })
            .await
    }

    pub(crate) struct AxumMutex<T>(Mutex<T>);

    impl<T> AxumMutex<T> {
        pub(crate) fn new(value: T) -> Self {
            Self(Mutex::new(value))
        }

        pub(crate) fn get_mut(&mut self) -> LockResult<&mut T> {
            self.0.get_mut()
        }

        pub(crate) fn into_inner(self) -> LockResult<T> {
            self.0.into_inner()
        }

        pub(crate) fn lock(&self) -> LockResult<MutexGuard<'_, T>> {
            _ = NUM_LOCKED.try_with(|num| {
                num.fetch_add(1, Ordering::SeqCst);
            });
            self.0.lock()
        }
    }
}
