// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
/*!
bit packing.

todo: should this be a crate?
*/

pub fn u32_to_u16s(packed: u32) -> (u16, u16) {
    ((packed >> 16) as u16, (packed & 0xFFFF) as u16)
}

pub fn u16s_to_u32(high: u16, low: u16) -> u32 {
    ((high as u32) << 16) | (low as u32)
}
