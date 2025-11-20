// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0

//! Exfiltrate debugging commands for images_and_words.
//!
//! This module provides custom commands for the exfiltrate debugging tool.

use exfiltrate::command::{Command, Response};
use std::time::Duration;
use wasm_safe_mutex::mpsc;

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

#[cfg(target_arch = "wasm32")]
use web_time::Instant;

/// Custom command to capture a screenshot of the main port.
///
/// This command waits for the next frame to be rendered and returns it as an image.
struct IwDumpMainportScreenshot;

impl Command for IwDumpMainportScreenshot {
    fn name(&self) -> &'static str {
        "iw_dump_mainport_screenshot"
    }

    fn short_description(&self) -> &'static str {
        "Captures a screenshot of the main rendering port. Use this to inspect the current frame being rendered."
    }

    fn full_description(&self) -> &'static str {
        "Captures a screenshot of the main rendering port.\n\
         \n\
         This command triggers a frame capture on the next rendered frame and returns \
         the framebuffer contents as an RGBA8 image. The command will wait up to 10 seconds \
         for a frame to be rendered.\n\
         \n\
         Usage: exfiltrate iw_dump_mainport_screenshot\n\
         \n\
         Returns: An RGBA8 image of the current framebuffer\n\
         \n\
         Error conditions:\n\
         - If no frame is rendered within 10 seconds, returns an error\n\
         - If the render loop is not active, returns an error"
    }

    fn execute(&self, _args: Vec<String>) -> Result<Response, Response> {
        // Create a oneshot channel for receiving the frame data
        let (tx, rx) = mpsc::channel();

        // Set the global DUMP_NEXT_FRAME to our sender
        {
            use crate::imp::DUMP_NEXT_FRAME;
            let mut dump_frame = DUMP_NEXT_FRAME.lock_sync();
            if dump_frame.is_some() {
                return Err(Response::from("A frame dump is already pending"));
            }
            *dump_frame = Some(tx);
        }

        // Wait for the frame data with a 10 second timeout
        let mut images = Vec::new();
        let mut expected_count = 2; // Default to 2 (color + depth) if no Expect message received (backward compat/robustness)

        let start = Instant::now();
        let timeout = Duration::from_secs(10);

        loop {
            if images.len() >= expected_count {
                break;
            }

            let remaining = timeout
                .checked_sub(start.elapsed())
                .unwrap_or(Duration::ZERO);
            if remaining.is_zero() {
                return Err(Response::from(
                    "Timeout waiting for frame. Ensure the render loop is active and rendering frames.",
                ));
            }

            match rx.recv_sync_timeout(Instant::now() + remaining) {
                Ok(msg) => {
                    use crate::imp::DumpMessage;
                    match msg {
                        DumpMessage::Expect(count) => {
                            expected_count = count;
                        }
                        DumpMessage::Image(img) => {
                            images.push(img);
                        }
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    return Err(Response::from(
                        "Timeout waiting for frame. Ensure the render loop is active and rendering frames.",
                    ));
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    // If we have at least one image, return what we have?
                    // Or is it an error if we expected more?
                    // For now, let's error if we didn't get what we expected.
                    return Err(Response::from(
                        "Channel disconnected before receiving all expected images.",
                    ));
                }
            }
        }

        Ok(Response::Images(images))
    }
}

/// Registers all exfiltrate commands for this library.
///
/// This is called automatically during library initialization when the exfiltrate feature is enabled.
pub(crate) fn register_commands() {
    exfiltrate::add_command(IwDumpMainportScreenshot);
}
