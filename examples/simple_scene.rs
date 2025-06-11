//! Simple scene rendering example for images_and_words graphics middleware.
//!
//! This example demonstrates the complete graphics pipeline using images_and_words,
//! rendering a colorful triangle with smooth color interpolation. It showcases all
//! the major patterns and components of the middleware.
//!
//! ## Features Demonstrated
//!
//! - **Dual-mode rendering**: Works with both app_window (actual window) and testing modes
//! - **Engine lifecycle**: Creation, configuration, and resource management
//! - **WGSL shaders**: Procedural vertex generation and fragment processing
//! - **Render passes**: Complete pipeline setup with binding styles
//! - **Threading patterns**: Proper main thread handling for graphics operations
//! - **Resource management**: Arc-based engine sharing and lifecycle
//!
//! ## Major Architecture Patterns
//!
//! ### 1. **Three-Axis Resource System**
//! The middleware organizes GPU resources along three dimensions:
//! - **Type**: Buffer vs Texture
//! - **Mutability**: Static (immutable) vs Dynamic (mutable)  
//! - **Direction**: Forward (CPU→GPU), Reverse (GPU→CPU), Sideways (GPU→GPU)
//!
//! ### 2. **Engine-Port-View Architecture**
//! - `Engine`: Main rendering coordinator, manages GPU device and resources
//! - `Port`: Viewport with camera, handles render passes and frame scheduling
//! - `View`: Display surface abstraction (window or testing surface)
//!
//! ### 3. **Threading Model**
//! - Uses `test_executors` for async operations (not tokio)
//! - Graphics operations must occur on main UI thread (Metal requirement)
//! - `app_window::wgpu::wgpu_spawn()` ensures proper thread context
//!
//! ## Usage
//!
//! ```bash
//! # Run with actual window display:
//! cargo run --example simple_scene --features=backend_wgpu,app_window
//!
//! # Run in testing mode (no window):
//! cargo run --example simple_scene --features=backend_wgpu,testing
//! ```

// Note: These imports would be needed for vertex buffer-based rendering:
// use images_and_words::bindings::forward::dynamic::buffer::CRepr;
// use images_and_words::bindings::forward::r#static::buffer::Buffer;
// use images_and_words::bindings::visible_to::GPUBufferUsage;
use images_and_words::bindings::BindStyle;
use images_and_words::images::Engine;
use images_and_words::images::projection::WorldCoord;
use images_and_words::images::render_pass::{DrawCommand, PassDescriptor};
use images_and_words::images::shader::{FragmentShader, VertexShader};
use images_and_words::images::view::View;
use std::sync::Arc;

// Note: In this simplified example, we generate vertex data procedurally in the 
// vertex shader rather than using vertex buffers. This demonstrates the basic
// rendering pipeline without the complexity of vertex buffer management.
//
// For reference, here's how you would define a vertex type for buffer-based rendering:
//
// #[repr(C)]
// #[derive(Copy, Clone, Debug)]
// struct Vertex {
//     position: [f32; 3],
//     color: [f32; 4],
// }
// unsafe impl CRepr for Vertex {}

/// WGSL vertex shader source code.
///
/// This shader generates triangle vertices procedurally based on the vertex ID.
/// It creates a colorful triangle by outputting different positions and colors
/// for vertices 0, 1, and 2. This approach avoids the need for vertex buffers
/// in this simple demonstration.
const VERTEX_SHADER: &str = r#"
struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var output: VertexOutput;
    
    // Generate triangle vertices based on vertex index
    switch vertex_index {
        case 0u: {
            output.clip_position = vec4<f32>(-0.5, -0.5, 0.0, 1.0); // Bottom-left
            output.color = vec4<f32>(1.0, 0.0, 0.0, 1.0); // Red
        }
        case 1u: {
            output.clip_position = vec4<f32>(0.5, -0.5, 0.0, 1.0); // Bottom-right
            output.color = vec4<f32>(0.0, 1.0, 0.0, 1.0); // Green
        }
        case 2u: {
            output.clip_position = vec4<f32>(0.0, 0.5, 0.0, 1.0); // Top
            output.color = vec4<f32>(0.0, 0.0, 1.0, 1.0); // Blue
        }
        default: {
            output.clip_position = vec4<f32>(0.0, 0.0, 0.0, 1.0);
            output.color = vec4<f32>(1.0, 1.0, 1.0, 1.0);
        }
    }
    
    return output;
}
"#;

/// WGSL fragment shader source code.
///
/// This shader receives interpolated color values from the vertex shader
/// and outputs the final pixel color. It simply passes through the color
/// without any additional processing.
const FRAGMENT_SHADER: &str = r#"
@fragment
fn fs_main(@location(0) color: vec4<f32>) -> @location(0) vec4<f32> {
    return color;
}
"#;

// Triangle vertex data is now generated procedurally in the vertex shader.
// The shader creates a triangle with:
// - Bottom-left vertex: red color at (-0.5, -0.5, 0.0)
// - Bottom-right vertex: green color at (0.5, -0.5, 0.0)  
// - Top vertex: blue color at (0.0, 0.5, 0.0)
//
// The GPU will interpolate colors between vertices, creating a smooth gradient.

/// Main entry point demonstrating the dual-mode pattern.
///
/// This function showcases how images_and_words applications can work in two modes:
/// 1. **App Window Mode**: Creates an actual window and renders to it
/// 2. **Testing Mode**: Uses a virtual surface for automated testing
///
/// ## Threading Pattern Explanation
///
/// The app_window version uses a complex threading pattern required by Metal on macOS:
/// - `app_window::application::main()`: Establishes the main application context
/// - `test_executors::sleep_on()`: Bridges sync/async boundary
/// - `app_window::application::on_main_thread()`: Ensures main UI thread execution  
/// - `app_window::wgpu::wgpu_spawn()`: Special spawner for GPU operations
///
/// This layered approach ensures graphics operations happen on the correct thread
/// while maintaining the async execution model needed for the middleware.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting simple scene example...");
    
    #[cfg(feature="app_window")] {
        // App Window Mode: Create actual window with proper threading
        app_window::application::main(|| {
            test_executors::sleep_on(async {
                run_app_window_example().await
            });
        });
        Ok(())
    }

    #[cfg(not(feature = "app_window"))]
    {
        // Testing Mode: Use virtual surface for headless testing
        println!("app_window feature not enabled, using test view...");
        test_executors::sleep_on(run_testing_example())
    }
}

/// App Window Mode: Creates and renders to an actual window.
///
/// This function demonstrates the complete lifecycle of a windowed graphics application:
/// 1. **Window Creation**: Uses app_window to create a native OS window
/// 2. **Surface Extraction**: Gets the rendering surface from the window
/// 3. **View Setup**: Creates images_and_words View from the surface
/// 4. **Engine Creation**: Initializes the rendering engine with camera position
/// 5. **Rendering**: Executes the main render loop
/// 6. **Cleanup**: Properly destroys resources when complete
///
/// ## Key Concepts
///
/// - **Surface**: Low-level rendering target provided by the OS window
/// - **View**: images_and_words abstraction over the surface
/// - **Engine**: Core rendering coordinator that manages GPU resources
/// - **WorldCoord**: 3D coordinate system for camera positioning
#[cfg(feature = "app_window")]
async fn run_app_window_example()  {
    use app_window::window::Window;
    use app_window::coordinates::{Position, Size};
    
    // Step 1: Create a window using app_window
    println!("Creating window...");
    let mut window = Window::new(
        Position::default(),        // Default position (centered)
        Size::new(800.0, 600.0),   // 800x600 resolution
        "images_and_words - Simple Scene Example".to_string()
    ).await;
    
    // Step 2: Extract the rendering surface from the window
    let surface = window.surface().await;

    // Step 3: Create images_and_words View from the surface
    println!("Creating view from surface...");
    let view = View::from_surface(surface).expect("View creation failed");
    
    // Step 4: Create the graphics engine with initial camera position
    println!("Creating graphics engine...");
    let initial_camera_position = WorldCoord::new(0.0, 0.0, 2.0); // 2 units back from origin
    let engine_arc = Engine::rendering_to(view, initial_camera_position)
        .await
        .expect("Engine creation failed");
    
    // Step 5: Execute the main rendering loop
    let _ = run_rendering_with_engine_arc(engine_arc).await;
    
    // Step 6: Cleanup - keep window alive until rendering completes
    println!("Keeping window alive during rendering...");
    drop(window); // Explicitly drop when we're done
}

/// Testing Mode: Renders without creating a window.
///
/// This function demonstrates headless rendering for automated testing and CI:
/// 1. **Virtual View**: Creates a testing view without an actual window
/// 2. **Engine Setup**: Same engine creation as windowed mode
/// 3. **Headless Rendering**: Executes rendering without visual output
///
/// ## Use Cases
///
/// - **Continuous Integration**: Run graphics tests without display
/// - **Performance Benchmarking**: Measure rendering performance
/// - **Automated Testing**: Verify rendering pipeline correctness
/// - **Development**: Test graphics code without window management
///
/// The rendering output isn't visible but all GPU operations execute normally,
/// making this perfect for validation and performance measurement.
#[cfg(not(feature = "app_window"))]
async fn run_testing_example() -> Result<(), Box<dyn std::error::Error>> {
    // Step 1: Create a virtual view for testing (no actual window)
    let view = View::for_testing();
    
    // Step 2: Create the graphics engine (same as windowed mode)
    println!("Creating graphics engine...");
    let initial_camera_position = WorldCoord::new(0.0, 0.0, 2.0);
    let engine = Engine::rendering_to(view, initial_camera_position)
        .await
        .expect("Failed to create engine");

    // Step 3: Execute headless rendering
    // Note: Engine is returned as Arc<Engine> to handle internal references
    run_rendering_with_engine_arc(engine).await
}


/// Core rendering pipeline demonstration.
///
/// This function implements the complete graphics pipeline using images_and_words:
/// 1. **Shader Creation**: Compiles WGSL vertex and fragment shaders
/// 2. **Resource Binding**: Sets up bind styles for GPU resources
/// 3. **Render Pass Setup**: Configures the rendering pipeline
/// 4. **Port Integration**: Adds render pass to engine's main port
/// 5. **Render Loop**: Executes frames with timing control
///
/// ## Pipeline Architecture
///
/// The rendering pipeline follows this flow:
/// ```
/// Vertex Shader → Primitive Assembly → Rasterization → Fragment Shader → Output
/// ```
///
/// ## Resource Management Pattern
///
/// - **Engine**: Shared via Arc for thread-safe access
/// - **Port**: Exclusive access through PortGuard for mutations
/// - **Shaders**: Owned by render pass, compiled once
/// - **BindStyle**: Configures resource binding layout
///
/// ## Performance Considerations
///
/// - Uses `force_render()` for demonstration (normally event-driven)
/// - 16ms frame timing targets 60fps
/// - 300 frame limit prevents infinite loops
async fn run_rendering_with_engine_arc(engine: Arc<Engine>) -> Result<(), Box<dyn std::error::Error>> {
    // Get the bound GPU device (could be used for creating buffers/textures)
    let _device = engine.bound_device();

    // Step 1: Create and compile shaders from WGSL source
    println!("Creating shaders...");
    let vertex_shader = VertexShader::new("simple_vertex", VERTEX_SHADER.to_string());
    let fragment_shader = FragmentShader::new("simple_fragment", FRAGMENT_SHADER.to_string());

    // Step 2: Set up resource binding style (empty for this simple example)
    println!("Setting up resource bindings...");
    let bind_style = BindStyle::new();
    
    // Note: This example uses procedural vertex generation in shaders.
    // For real applications with vertex buffers, you would:
    // 1. Create vertex/index buffers: Buffer::new(device, data, usage, name, initial_fn)
    // 2. Define vertex layouts: VertexLayout describing attribute locations
    // 3. Bind resources: bind_style.bind_static_vertex_buffer(buffer)
    // 4. Reference in shaders: @location(0) position: vec3<f32>

    // Step 3: Create render pass descriptor with complete pipeline configuration
    println!("Creating render pass...");
    let pass_descriptor = PassDescriptor::new(
        "triangle_pass".to_string(),    // Debug name
        vertex_shader,                  // Vertex stage
        fragment_shader,                // Fragment stage  
        bind_style,                     // Resource bindings
        DrawCommand::TriangleList(3),   // Draw 3 vertices as triangle list
        false,                          // Depth testing disabled
        false,                          // Alpha blending disabled
    );

    // Step 4: Register render pass with engine's main port
    println!("Adding render pass to engine...");
    let mut port = engine.main_port_mut(); // Get exclusive port access
    port.add_fixed_pass(pass_descriptor).await;

    // Step 5: Execute main rendering loop with frame timing
    println!("Starting render loop...");
    println!("Rendering a colorful triangle for demonstration...");
    
    let mut frame_count = 0;
    let max_frames = 300; // 5 seconds at 60fps for demonstration
    
    while frame_count < max_frames {
        // Render one frame (force_render bypasses dirty checking)
        port.force_render().await;
        frame_count += 1;
        
        // Progress reporting every second
        if frame_count % 60 == 0 {
            println!("Rendered {} frames", frame_count);
        }

        // Frame rate limiting: target ~60fps (16.67ms per frame)
        portable_async_sleep::async_sleep(std::time::Duration::from_millis(16)).await;
    }
    
    println!("Rendering complete! Rendered {} frames total.", frame_count);
    Ok(())
}