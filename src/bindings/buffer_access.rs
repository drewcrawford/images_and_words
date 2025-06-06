/*!
Defines types for use in imp::Buffer.
*/

//whether the buffer can be mapped to CPU.
#[non_exhaustive]
pub enum MapType {
    /// The buffer can be mapped to the CPU for reading.
    #[allow(dead_code)]
    Read,
    /// The buffer can be mapped to the CPU for writing.
    Write,
}
