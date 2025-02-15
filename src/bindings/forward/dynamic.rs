/*!
Resources that are dynamic.  That is, they change frequently.

On discrete architectures, generally you want either the special AMD memory or host memory
that you read over PCI express.

On unified architectures, generally you have some shared memory that has a suboptimal layout.
*/
pub mod frame_texture;
pub mod buffer;
