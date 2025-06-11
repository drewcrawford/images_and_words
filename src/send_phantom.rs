// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
use std::marker::PhantomData;

/// A wrapper around PhantomData that is always Send
#[derive(Debug)]
pub struct SendPhantom<T>(PhantomData<T>);

impl<T> SendPhantom<T> {
    pub fn new() -> Self {
        SendPhantom(PhantomData)
    }
}

impl<T> Default for SendPhantom<T> {
    fn default() -> Self {
        Self::new()
    }
}

// PhantomData is always Send regardless of T, so this is safe
unsafe impl<T> Send for SendPhantom<T> {}
