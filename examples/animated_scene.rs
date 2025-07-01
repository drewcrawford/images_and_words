//! Animated scene rendering example using dynamic buffers for images_and_words.
//!
//! This example demonstrates advanced GPU rendering techniques with dynamic buffers
//! that update every frame to create smooth animations. It showcases the power of
//! the images_and_words middleware for high-performance animated graphics.
//!
//! ## Features Demonstrated
//!
//! - **Dynamic Buffers**: CPU-updatable buffers for per-frame animation data
//! - **Multibuffering**: Automatic synchronization between CPU writes and GPU reads
//! - **Uniform Buffer Animation**: Per-frame animation parameters via dynamic uniform buffers
//! - **Complex Shaders**: WGSL shaders with animation math and transformations
//! - **Frame Timing**: Precise animation timing with configurable frame rates
//!
//! ## Animation System
//!
//! The example creates multiple animated elements:
//! - **Morphing Triangle**: Procedural vertices that transform over time
//! - **Color Cycling**: Smooth color transitions using time-based uniforms
//! - **Rotation Animation**: Geometric transformations applied per frame
//! - **Scaling Effects**: Dynamic size changes synchronized with other animations
//!
//! ## Dynamic Buffer Architecture
//!
//! Uses the three-axis resource system:
//! - **Type**: Uniform buffers (animation parameters)
//! - **Mutability**: Dynamic (updated every frame from CPU)
//! - **Direction**: Forward (CPU→GPU data flow)
//!
//! ## Performance Optimizations
//!
//! - Multibuffering prevents CPU/GPU pipeline stalls
//! - Efficient per-frame updates with minimal CPU overhead
//! - Optimized shader calculations for smooth 60fps animation
//!
//! ## Usage
//!
//! ```bash
//! # Run with actual window display:
//! cargo run --example animated_scene --features=backend_wgpu,app_window
//!
//! # Run in testing mode (no window):
//! cargo run --example animated_scene --features=backend_wgpu,testing
//! ```

use images_and_words::bindings::BindStyle;
use images_and_words::bindings::bind_style::{BindSlot, Stage};
use images_and_words::bindings::forward::dynamic::buffer::{Buffer, CRepr};
use images_and_words::bindings::visible_to::GPUBufferUsage;
use images_and_words::images::Engine;
use images_and_words::images::projection::WorldCoord;
use images_and_words::images::render_pass::{DrawCommand, PassDescriptor};
use images_and_words::images::shader::{FragmentShader, VertexShader};
use images_and_words::images::view::View;
use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

/// Animation parameters passed to shaders each frame.
///
/// This structure contains all the time-based parameters needed for animation,
/// including the current time, frame count, and derived animation values.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct AnimationUniforms {
    /// Current time in seconds since animation start
    time: f32,
    /// Current frame number (wraps at u32::MAX)
    frame: u32,
    /// Sine wave based on time for smooth oscillation
    sine_time: f32,
    /// Cosine wave based on time for smooth oscillation
    cosine_time: f32,
}

unsafe impl CRepr for AnimationUniforms {}

/// WGSL vertex shader for animated scene with procedural geometry.
///
/// This shader generates animated triangle vertices procedurally using the vertex index,
/// similar to simple_scene but with dynamic animation parameters from uniform buffers.
/// It applies multiple transformations based on real-time animation data.
const ANIMATED_VERTEX_SHADER: &str = r#"
struct AnimationUniforms {
    time: f32,
    frame: u32,
    sine_time: f32,
    cosine_time: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> animation: AnimationUniforms;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;
    
    // Base triangle vertices (like simple_scene)
    var position: vec3<f32>;
    var base_color: vec3<f32>;
    
    switch vertex_index {
        case 0u: {
            position = vec3<f32>(-0.5, -0.5, 0.1); // Bottom-left
            base_color = vec3<f32>(1.0, 0.0, 0.0); // Red
        }
        case 1u: {
            position = vec3<f32>(0.5, -0.5, 0.1); // Bottom-right
            base_color = vec3<f32>(0.0, 1.0, 0.0); // Green
        }
        case 2u: {
            position = vec3<f32>(0.0, 0.5, 0.1); // Top
            base_color = vec3<f32>(0.0, 0.0, 1.0); // Blue
        }
        default: {
            position = vec3<f32>(0.0, 0.0, 0.1);
            base_color = vec3<f32>(1.0, 1.0, 1.0);
        }
    }
    
    // Apply animations based on uniform data
    
    // 1. Dynamic scaling
    let scale = 0.7 + 0.3 * animation.sine_time;
    position = position * scale;
    
    // 2. Rotation animation
    let rotation_angle = animation.time * 0.8;
    let cos_r = cos(rotation_angle);
    let sin_r = sin(rotation_angle);
    
    // Apply 2D rotation in XY plane
    let rotated_x = position.x * cos_r - position.y * sin_r;
    let rotated_y = position.x * sin_r + position.y * cos_r;
    position.x = rotated_x;
    position.y = rotated_y;
    
    // 3. Morphing/wobble effects
    let wobble_x = 0.1 * sin(animation.time * 3.0 + f32(vertex_index));
    let wobble_y = 0.1 * cos(animation.time * 2.5 + f32(vertex_index));
    position.x += wobble_x;
    position.y += wobble_y;
    
    // 4. Breathing effect (Z-axis movement)
    position.z += 0.01 * animation.cosine_time;
    
    output.clip_position = vec4<f32>(position, 1.0);
    
    // Animated colors with rainbow cycling
    let color_phase = animation.time * 1.5 + f32(vertex_index) * 2.0;
    let animated_color = vec3<f32>(
        base_color.r * (0.6 + 0.4 * sin(color_phase)),
        base_color.g * (0.6 + 0.4 * sin(color_phase + 2.0)),
        base_color.b * (0.6 + 0.4 * sin(color_phase + 4.0))
    );
    
    // Add global brightness pulsing
    let brightness = 0.8 + 0.2 * animation.sine_time;
    output.color = vec4<f32>(animated_color * brightness, 1.0);
    
    return output;
}
"#;

/// WGSL fragment shader for animated scene.
///
/// The fragment shader receives interpolated colors from the vertex shader
/// and outputs them with basic processing. Animation effects are primarily
/// handled in the vertex shader.
const ANIMATED_FRAGMENT_SHADER: &str = r#"
@fragment
fn fs_main(@location(0) color: vec4<f32>) -> @location(0) vec4<f32> {
    return color;
}
"#;

/// Main entry point for the animated scene example.
///
/// Creates a window and renders animated scene with dynamic buffers.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting animated scene example with dynamic buffers...");

    app_window::application::main(|| {
        app_window::wgpu::wgpu_begin_context(async move {
            app_window::wgpu::wgpu_in_context(run_app_window_example())
        })
    });
    Ok(())
}

/// Creates and renders animated scene to an actual window.
async fn run_app_window_example() {
    use app_window::coordinates::{Position, Size};
    use app_window::window::Window;

    println!("Creating window for animated scene...");
    let mut window = Window::new(
        Position::default(),
        Size::new(800.0, 600.0),
        "images_and_words - Animated Scene Example".to_string(),
    )
    .await;

    let surface = window.surface().await;

    println!("Creating view from surface...");
    let view = View::from_surface(surface).expect("View creation failed");

    println!("Creating graphics engine...");
    let initial_camera_position = WorldCoord::new(0.0, 0.0, 2.0);
    let engine_arc = Engine::rendering_to(view, initial_camera_position)
        .await
        .expect("Engine creation failed");

    run_animated_rendering_with_engine_arc(engine_arc)
        .await
        .expect("Engine creation failed");

    println!("Keeping window alive during rendering...");
    drop(window);
}

/// Core animated rendering pipeline using dynamic uniform buffers.
///
/// This function demonstrates the complete workflow for animated graphics:
/// 1. **Dynamic Buffer Creation**: Creates uniform buffer for animation parameters
/// 2. **Shader Compilation**: Compiles WGSL shaders with animation support
/// 3. **Resource Binding**: Binds dynamic uniform buffer to shader binding points
/// 4. **Animation Loop**: Updates buffer each frame with new animation data
/// 5. **Render Execution**: Draws animated geometry with proper timing
async fn run_animated_rendering_with_engine_arc(
    engine: Arc<Engine>,
) -> Result<(), Box<dyn std::error::Error>> {
    let device = engine.bound_device();

    println!("Creating animated shaders...");
    let vertex_shader = VertexShader::new("animated_vertex", ANIMATED_VERTEX_SHADER.to_string());
    let fragment_shader =
        FragmentShader::new("animated_fragment", ANIMATED_FRAGMENT_SHADER.to_string());

    // Step 1: Create dynamic uniform buffer for animation parameters
    println!("Creating dynamic uniform buffer...");
    let uniform_buffer = Buffer::<AnimationUniforms>::new(
        device.clone(),
        1, // Single uniform struct
        GPUBufferUsage::VertexShaderRead,
        "animation_uniforms",
        |_| AnimationUniforms {
            time: 0.0,
            frame: 0,
            sine_time: 0.0,
            cosine_time: 1.0,
        },
    )
    .await
    .expect("Failed to create uniform buffer");

    // Step 2: Create bind style and bind dynamic uniform buffer
    println!("Setting up resource bindings...");
    let mut bind_style = BindStyle::new();

    // Bind animation uniforms to binding 0 (accessible to vertex stage)
    bind_style.bind_dynamic_buffer(BindSlot::new(0), Stage::Vertex, &uniform_buffer);

    // Step 3: Create render pass descriptor
    println!("Creating animated render pass...");
    let pass_descriptor = PassDescriptor::new(
        "animated_pass".to_string(),
        vertex_shader,
        fragment_shader,
        bind_style,
        DrawCommand::TriangleList(1), // Draw one triangle
        false,                        // No depth testing
        true,                         // Enable alpha blending for smooth color transitions
    );

    // Step 4: Register render pass with engine
    println!("Adding render pass to engine...");
    let mut port = engine.main_port_mut();
    port.add_fixed_pass(pass_descriptor).await;

    // Step 5: Animation loop with dynamic buffer updates
    println!("Starting animation loop...");
    println!("Rendering complex animated scene with dynamic colors and transformations...");

    let mut frame_count = 0u32;
    let max_frames = 600; // 10 seconds at 60fps
    let start_time = Instant::now();

    while frame_count < max_frames {
        let elapsed = start_time.elapsed().as_secs_f32();

        // Update animation uniforms each frame
        {
            let mut uniform_guard = uniform_buffer.access_write().await;
            let animation_data = AnimationUniforms {
                time: elapsed,
                frame: frame_count,
                sine_time: elapsed.sin(),
                cosine_time: elapsed.cos(),
            };
            uniform_guard.write(&[animation_data], 0);
            // Guard automatically marks buffer as dirty when dropped
        }

        // Render the frame
        port.force_render().await;
        frame_count += 1;

        // Progress reporting
        if frame_count % 60 == 0 {
            println!(
                "Rendered {} animated frames (time: {:.2}s)",
                frame_count, elapsed
            );
        }

        // Frame rate limiting: target 60fps
        portable_async_sleep::async_sleep(std::time::Duration::from_millis(16)).await;
    }

    println!(
        "Animation complete! Rendered {} frames with dynamic buffers.",
        frame_count
    );
    Ok(())
}
