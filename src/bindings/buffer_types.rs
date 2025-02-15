/*! Defines types for use in buffers.
*/
#[allow(non_camel_case_types)]
#[repr(transparent)]
#[derive(Copy,Clone,PartialEq)]
pub struct bool(
    #[cfg(target_os = "windows")]
    u32,
    #[cfg(target_os = "macos")]
    u8,
);
impl bool {
    pub const fn new(value: std::primitive::bool) -> Self {
        bool(if value { 1 } else { 0 })
    }
}
impl std::convert::From<bool> for std::primitive::bool {
    fn from(value: bool) -> Self {
        value.0 != 0
    }
}
impl std::convert::From<std::primitive::bool> for bool {
    fn from(value: std::primitive::bool) -> Self {
        bool::new(value)
    }
}