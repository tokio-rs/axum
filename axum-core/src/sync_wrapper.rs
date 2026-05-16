//! Minimal in-tree replacement for the `sync_wrapper` crate, trimmed to the
//! surface that `axum` and `axum-core` actually use (`new`, `get_pin_mut`,
//! `unsafe impl Sync`). Derived from <https://github.com/Actyx/sync_wrapper>
//! (Apache-2.0).
//!
//! The wrapper encapsulates the `unsafe impl Sync` so the outer body types it
//! appears in stay strictly `&mut`-only via `pin_project!`. In particular,
//! `pin_project_lite` also generates `project_ref(self: Pin<&Self>)`; wrapping
//! the `#[pin]` field in `SyncWrapper<S>` means the shared projection only
//! yields `Pin<&SyncWrapper<S>>`, which exposes no safe access to the inner.

use std::pin::Pin;

/// Wraps a `!Sync` value to make the wrapper `Sync` by only ever handing out
/// `&mut`/owned access to the inner value.
#[repr(transparent)]
#[allow(missing_debug_implementations)]
pub struct SyncWrapper<T>(T);

impl<T> SyncWrapper<T> {
    /// Wraps `value`.
    pub const fn new(value: T) -> Self {
        Self(value)
    }

    /// Returns a pinned mutable reference to the inner value.
    #[must_use]
    pub fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut T> {
        // SAFETY: this projects `Pin<&mut SyncWrapper<T>>` to its single inner
        // field. `SyncWrapper` provides no operation that could move `self.0`
        // while `self` is pinned (no `Drop`/`PinnedDrop`, no accessor that
        // takes the value out), so structural pinning of the field is upheld.
        #[allow(unsafe_code)]
        unsafe {
            Pin::map_unchecked_mut(self, |this| &mut this.0)
        }
    }
}

// SAFETY: every accessor on `SyncWrapper` requires `&mut self` or ownership,
// and none returns a safe `&T`, so the inner value can never be observed from
// two threads concurrently regardless of whether `T: Sync`.
#[allow(unsafe_code)]
unsafe impl<T> Sync for SyncWrapper<T> {}
