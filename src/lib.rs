/*! The GPU module.

> She shuts the doors and lights

> And lays her body on the bed

> Where images and words are running deep

> She has too much pride to pull

> the sheets above her head

> So quietly she lays and waits for sleep

Consisting of:

 * Images: A graphics module
 * Words: A compute module


*/


mod entry_point;
pub mod images;
pub mod bindings;
pub mod pixel_formats;
mod imp;
mod multibuffer;
mod bittricks;

pub use vectormatrix;

pub type Priority = some_executor::Priority;

