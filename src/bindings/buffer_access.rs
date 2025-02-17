/*!
Defines types for use in imp::Buffer.
*/

//whether the buffer can be mapped to CPU.
pub enum MapType {
    /// The buffer cannot be mapped to the CPU.
    None,
    /// The buffer can be mapped to the CPU for reading.
    Read,
    /// The buffer can be mapped to the CPU for writing.
    Write,
    /// The buffer can be mapped to the CPU for reading and writing.
    ReadWrite,
}