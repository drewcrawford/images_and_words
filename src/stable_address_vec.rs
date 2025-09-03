// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
/*!
A collection type with stable addresses while elements are added to the collection.

This is primarily useful for building lists with inner references which could in theory
be done in two passes, but this is more convenient.
*/

use std::cell::UnsafeCell;

#[allow(dead_code)] //nop implementation does not use
pub struct StableAddressVec<T> {
    vec: UnsafeCell<Vec<T>>,
}

impl<T> StableAddressVec<T> {
    /**
    Creates a new StableAddressVec with the specified capacity.

    Unlike a normal Vec, this capacity cannot be changed later, as reallocation
    would change the element addresses.
    */
    #[allow(dead_code)] //nop implementation does not use
    pub fn with_capactiy(capacity: usize) -> Self {
        Self {
            vec: UnsafeCell::new(Vec::with_capacity(capacity)),
        }
    }

    #[allow(dead_code)] //nop implementation does not use
    pub fn push(&self, value: T) -> &T {
        let (next_len, capacity) = unsafe {
            //safe because we are the only ones with access to the vec, and we only perform read ops
            let vec = &*self.vec.get();
            (vec.len() + 1, vec.capacity())
        };

        assert!(
            next_len <= capacity,
            "Cannot push to a StableAddressVec that has reached capacity"
        );
        //safe because we won't reallocate
        unsafe {
            (*self.vec.get()).push(value);
            //safe because we just pushed the value
            &(&(*self.vec.get()))[next_len - 1]
        }
    }

    pub fn into_vec(self) -> Vec<T> {
        //safe because we are the only ones with access to the vec, and we are consuming it
        unsafe { (*self.vec.get()).drain(..).collect() }
    }
}

impl<T> From<StableAddressVec<T>> for Vec<T> {
    fn from(val: StableAddressVec<T>) -> Self {
        val.into_vec()
    }
}
