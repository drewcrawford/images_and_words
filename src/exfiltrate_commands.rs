// SPDX-License-Identifier: Parity-7.0.0 OR PolyForm-Noncommercial-1.0.0

//! Exfiltrate debugging commands for images_and_words.
//!
//! This module provides custom commands for the exfiltrate debugging tool.

use exfiltrate::command::{Command, ImageInfo, Response};
use std::sync::mpsc;
use std::time::Duration;

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
        match rx.recv_timeout(Duration::from_secs(10)) {
            Ok(dumped) => Ok(Response::Images(vec![dumped])),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(Response::from(
                "Timeout waiting for frame. Ensure the render loop is active and rendering frames.",
            )),
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(Response::from(
                "Channel disconnected. This is an internal error.",
            )),
        }
    }
}

/// Registers all exfiltrate commands for this library.
///
/// This is called automatically during library initialization when the exfiltrate feature is enabled.
pub(crate) fn register_commands() {
    exfiltrate::add_command(IwDumpMainportScreenshot);
}
