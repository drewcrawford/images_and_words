// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0
//! Debug capture functionality for frame dumping (exfiltrate feature)

use exfiltrate::command::ImageInfo;
use exfiltrate::rgb::RGBA8;
use wgpu::TextureFormat;

#[derive(Clone, Copy)]
pub(super) enum DumpImageFormat {
    Color(TextureFormat),
    Depth16Unorm,
}

pub(super) fn dump_image(
    map_result: Result<(), wgpu::BufferAsyncError>,
    buffer: wgpu::Buffer,
    bytes_per_row: u32,
    scaled_size: (u32, u32),
    sender: Option<wasm_safe_mutex::mpsc::Sender<crate::imp::DumpMessage>>,
    remark: &'static str,
    format: DumpImageFormat,
) {
    if let Err(err) = map_result {
        logwise::error_sync!(
            "Failed to map buffer for debug capture: {err}",
            err = logwise::privacy::LogIt(&err)
        );
        return;
    }

    if scaled_size.0 == 0 || scaled_size.1 == 0 {
        logwise::warn_sync!("Skipping debug capture for zero-sized surface");
        buffer.unmap();
        return;
    }

    let pixels = {
        let mapped = buffer.slice(..).get_mapped_range();
        let mapped_slice: &[u8] = &mapped;
        let result = match format {
            DumpImageFormat::Color(surface_format) => {
                read_color_pixels(mapped_slice, bytes_per_row, scaled_size, surface_format)
            }
            DumpImageFormat::Depth16Unorm => {
                read_depth_pixels(mapped_slice, bytes_per_row, scaled_size)
            }
        };
        drop(mapped);
        result
    };
    buffer.unmap();

    let Some(pixels) = pixels else {
        return;
    };

    if let Some(sender) = sender {
        let image_info = ImageInfo::new(pixels, scaled_size.0, Some(remark.to_string()));
        if let Err(err) = sender.send_sync(crate::imp::DumpMessage::Image(image_info)) {
            logwise::error_sync!(
                "Failed to send dumped image to exfiltrate.  The receiver was likely dropped.  Error: {err}",
                err = logwise::privacy::LogIt(&err)
            );
        }
    }
}

fn read_color_pixels(
    mapped: &[u8],
    bytes_per_row: u32,
    scaled_size: (u32, u32),
    surface_format: TextureFormat,
) -> Option<Vec<RGBA8>> {
    let width = scaled_size.0 as usize;
    let height = scaled_size.1 as usize;
    let stride = bytes_per_row as usize;
    let row_bytes = width * 4;
    if stride < row_bytes {
        logwise::error_sync!(
            "Row stride smaller than expected pixel data for framebuffer dump",
            stride = stride,
            expected = row_bytes
        );
        return None;
    }

    let mut pixels = Vec::with_capacity(width * height);
    for row in 0..height {
        let offset = row * stride;
        let row_slice = &mapped[offset..offset + row_bytes];
        match surface_format {
            TextureFormat::Bgra8Unorm | TextureFormat::Bgra8UnormSrgb => {
                for chunk in row_slice.chunks_exact(4) {
                    pixels.push(RGBA8::new(chunk[2], chunk[1], chunk[0], chunk[3]));
                }
            }
            TextureFormat::Rgba8Unorm | TextureFormat::Rgba8UnormSrgb => {
                for chunk in row_slice.chunks_exact(4) {
                    pixels.push(RGBA8::new(chunk[0], chunk[1], chunk[2], chunk[3]));
                }
            }
            _ => {
                logwise::error_sync!(
                    "Unsupported texture format for framebuffer dump",
                    format = logwise::privacy::LogIt(&surface_format)
                );
                return None;
            }
        }
    }

    Some(pixels)
}

fn read_depth_pixels(
    mapped: &[u8],
    bytes_per_row: u32,
    scaled_size: (u32, u32),
) -> Option<Vec<RGBA8>> {
    let width = scaled_size.0 as usize;
    let height = scaled_size.1 as usize;
    let stride = bytes_per_row as usize;
    let row_bytes = width * 2;
    if stride < row_bytes {
        logwise::error_sync!(
            "Row stride smaller than expected depth data",
            stride = stride,
            expected = row_bytes
        );
        return None;
    }

    let mut pixels = Vec::with_capacity(width * height);
    for row in 0..height {
        let offset = row * stride;
        let row_slice = &mapped[offset..offset + row_bytes];
        for chunk in row_slice.chunks_exact(2) {
            let depth = u16::from_le_bytes([chunk[0], chunk[1]]);
            let normalized = (depth >> 8) as u8;
            pixels.push(RGBA8::new(normalized, normalized, normalized, 255));
        }
    }

    Some(pixels)
}
