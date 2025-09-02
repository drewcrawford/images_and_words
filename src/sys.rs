// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0

//! System abstractions for cross-platform compatibility.
//!
//! This module provides platform-specific implementations of system functionality
//! that differs between native and WASM targets.

pub mod time {
    //! Time abstractions for cross-platform compatibility.
    //!
    //! On native platforms, this re-exports `std::time` types.
    //! On WASM platforms, this re-exports `web_time` types for compatibility.

    #[cfg(not(target_arch = "wasm32"))]
    pub use std::time::Instant;

    #[cfg(target_arch = "wasm32")]
    pub use web_time::{Duration, Instant};
}
