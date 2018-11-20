//! Renderer manifesto:
//!
//! - Easy to use
//! - Flexible
//! - Not too verbose
//! - Dynamic
//!
//! Based on global command reordering with sort keys.
//! (see https://ourmachinery.com/post/a-modern-rendering-architecture/)
//! Submission order is fully independant from the execution
//! order on the GPU. The necessary barriers and synchronization
//! is determined once the list of commands is sorted.
//! Thus, adding a post-proc effect is as easy as adding a command buffer with the correct resource
//! names and sequence IDs so that it happens after main rendering.
//! This means that any component in the engine can modify
//! the render pipeline 'non-locally' by submitting a command buffer.
//! This might not be a good thing per se, but at least it's flexible.
//!
//! The `Renderer` instances should be usable across threads
//! (e.g. can allocate and upload from different threads at once).
//!
//! `CommandBuffers` are renderer-agnostic.
//! They contain commands with a sort key that indicates their relative execution order.
//!
//! Unsolved questions: properly handle the sort keys.
//!
//! Idea: named resources (resource semantics)
//! - A way to assign arbitrary IDs to resources that has meaning to the application
//!     - e.g. the temporary render layer of mesh group X for appearance Y
//!     - 64-bit handles
//! - Pre-described and allocated on first use
//! - Resource templates: ID + mask that describes how to allocate a resource with the specified bit pattern
//!     - registered in advance (pipeline config)
//! - advantage: no need to pass resource handles around, just refer to them by semantics (convention over configuration)
//! - would allow draw calls like:
//! ```
//!     draw (
//!         sort_key = sequence_id!{ opaque, layer=group_id, depth=d, pass_immediate=0 },
//!         target   = stylized_layer(objgroup)
//!     )
//! ```
//! To test in the high-level renderer:
//! - 2D texturing (ss splats anchored on submeshes)
//! - Splat-based shading (splat proxies in screen space masked, directed by projected light direction)
//! - Good contour detection (explicit crease edges + fast DF)
//! - Good cast shadows (soft & hard, raycasting and temporal integration?)
//! - Real depth-sorted transparency everywhere (assume non-intersecting objects)
//! - Per-object-group screen-space calculations (e.g. curvature, surface descriptors, etc.)
//!       - Discretize into coarse grid to evaluate filter only where needed
//! - Performance might be abysmal, but that's not an issue
//!     - for our purposes, 5 fps is good enough! -> target animation
//! - Stroke-based rendering with arbitrary curves (curve DF)
//!     - big unknown, prepare for poor performance
//! - High-quality ambient occlusion?
//!
//! UI is important!
//!
//! Renderer backend: object-safe or compile-time?
//! - avoid costly recompilation times -> object-safe

use std::sync::Mutex;
use downcast_rs::Downcast;

pub mod backend;
mod command_buffer;
mod format;
mod handles;
mod image;
mod sync;
mod util;
mod shader_interface;
mod sampler;

/*
define_sort_key! {
    [sequence:3  , layer:8, depth:16, pass_immediate:4],
    [opaque:3 = 3, layer:8, depth:16, pass_immediate:4],
    [shadow:3 = 1, view: 6, layer:8, depth:16, pass_immediate:4]

    sequence,objgroup,comp-pass(pre,draw,post),effect,effect-pass(pre,draw,post)
}

sequence_id!{ opaque, layer=group_id, depth=d, pass_immediate=0 }*/

pub use self::command_buffer::CommandBuffer;
pub use self::format::*;
pub use self::handles::*;
pub use self::image::*;
pub use self::sampler::*;

#[derive(Copy, Clone, Debug)]
pub enum MemoryType {
    DeviceLocal,
    HostUpload,
    HostReadback,
}

pub enum Queue {
    Graphics,
    Compute,
    Transfer,
}

bitflags! {
    #[derive(Default)]
    pub struct ShaderStageFlags: u32 {
        const SHADER_STAGE_VERTEX = (1 << 0);
        const SHADER_STAGE_GEOMETRY = (1 << 1);
        const SHADER_STAGE_FRAGMENT = (1 << 2);
        const SHADER_STAGE_TESS_CONTROL = (1 << 3);
        const SHADER_STAGE_TESS_EVAL = (1 << 4);
        const SHADER_STAGE_COMPUTE = (1 << 5);
    }
}

pub struct LayoutBinding
{
    pub descriptor_type: DescriptorType,
    pub stage_flags: ShaderStageFlags,
    pub count: usize,
}

pub enum DescriptorType
{
    Sampler,  // TODO
    SampledImage,
    StorageImage,
    UniformBuffer,
    StorageBuffer,
    InputAttachment
}

pub enum Descriptor<R: RendererBackend>
{
    SampledImage {
        img: R::ImageHandle,
        sampler: SamplerDesc,
    },
    UniformBuffer {
        buffer: BufferSlice<R::BufferHandle>
    },
}

pub struct GraphicsShaderPipeline<'a>
{
    pub vertex: &'a [u8],
    pub geometry: Option<&'a [u8]>,
    pub fragment: &'a [u8],
    pub tess_eval: Option<&'a [u8]>,
    pub tess_control: Option<&'a [u8]>
}

pub struct BufferSlice<Handle>
{
    pub buffer: Handle,
    pub offset: usize,
    pub size: usize,
}

pub trait RendererBackend: Sync {
    type SwapchainHandle: Copy;
    type BufferHandle: Copy;
    type ImageHandle: Copy;
    type DescriptorSetHandle: Copy;
    type DescriptorSetLayoutHandle: Copy;
    type GraphicsPipelineHandle: Copy;

    fn create_swapchain(&self) -> Self::SwapchainHandle;

    fn default_swapchain(&self) -> Option<Self::SwapchainHandle>;

    fn swapchain_dimensions(&self, swapchain: Self::SwapchainHandle) -> (u32, u32);

    fn create_image(
        &self,
        format: Format,
        dimensions: &Dimensions,
        mipcount: MipmapsCount,
        samples: u32,
        usage: ImageUsageFlags,
        initial_data: Option<&[u8]>,
    ) -> Self::ImageHandle;

    fn upload_transient(&self, data: &[u8]) -> BufferSlice<Self::BufferHandle>;

    fn destroy_image(&self, image: Self::ImageHandle);

    fn create_buffer(&self, size: u64) -> Self::BufferHandle;

    fn destroy_buffer(&self, buffer: Self::BufferHandle);

    fn submit_frame(&self);

    fn create_graphics_pipeline(&self, shaders: &GraphicsShaderPipeline) -> Self::GraphicsPipelineHandle;

    fn create_descriptor_set_layout(&self, bindings: &[LayoutBinding]) -> Self::DescriptorSetLayoutHandle;

    fn create_descriptor_set(&self, layout: Self::DescriptorSetLayoutHandle, resources: &[Descriptor<Self>]) -> Self::DescriptorSetHandle where Self: Sized;
}

pub struct Renderer<R: RendererBackend> {
    backend: R,
    cmdbufs: Mutex<Vec<CommandBuffer<R>>>,
}

impl<R: RendererBackend> Renderer<R> {
    pub fn new(backend: R) -> Renderer<R> {
        Renderer { backend, cmdbufs: Mutex::new(Vec::new()) }
    }

    /// Creates a swapchain.
    pub fn create_swapchain(&self) -> R::SwapchainHandle {
        self.backend.create_swapchain()
    }

    /// Returns the default swapchain handle, if any.
    pub fn default_swapchain(&self) -> Option<R::SwapchainHandle> {
        self.backend.default_swapchain()
    }

    /// Get swapchain dimensions.
    pub fn swapchain_dimensions(&self, swapchain: R::SwapchainHandle) -> (u32, u32) {
        self.backend.swapchain_dimensions(swapchain)
    }

    /// Creates a command buffer.
    pub fn create_command_buffer(&self) -> CommandBuffer<R> {
        CommandBuffer::new()
    }

    /// Creates a graphics pipeline.
    /// Pipeline = all shaders + input layout + output layout (expected buffers)
    /// Creation process?
    pub fn create_graphics_pipeline(&self) -> R::GraphicsPipelineHandle {
        unimplemented!()
    }

    /// Creates an image.
    /// Initial data is uploaded to the image memory, and will be visible to all operations
    /// from the current frame and after.
    /// (the first operation that depends on the image will block on transfer complete)
    pub fn create_image(
        &self,
        format: Format,
        dimensions: Dimensions,
        mipcount: MipmapsCount,
        samples: u32,
        usage: ImageUsageFlags,
        initial_data: Option<&[u8]>,
    ) -> R::ImageHandle {
        self.backend
            .create_image(format, &dimensions, mipcount, samples, usage, initial_data)
    }

    /// Uploads data to a transient pool.
    /// The buffer becomes invalid as soon as out of the current frame.
    /// The buffer can be used as uniform input to pipelines.
    pub fn upload_transient(&self, data: &[u8]) -> BufferSlice<R::BufferHandle> {
        self.backend.upload_transient(data)
    }

    /// Destroys an image handle. The actual image is destroyed when
    /// it is not in use anymore by the GPU.
    pub fn destroy_image(&self, image: R::ImageHandle) {
        self.backend.destroy_image(image)
    }

    /// Creates a GPU (device local) buffer.
    /// This function only creates a handle (name) and description of the buffer.
    /// For the memory to be allocated, it has to be initialized by a command in a command buffer.
    /// This function is thread-safe.
    pub fn create_buffer(&self, size: u64) -> R::BufferHandle {
        self.backend.create_buffer(size)
    }

    /// Destroys a GPU buffer. The actual buffer is destroyed when
    /// it is not in use anymore by the GPU.
    /// TODO: do it in a command buffer?
    pub fn destroy_buffer(&self, buffer: R::BufferHandle) {
        self.backend.destroy_buffer(buffer)
    }

    /// Submits a command buffer.
    pub fn submit_command_buffer(&self, cmdbuf: CommandBuffer<R>) {
        self.cmdbufs.lock().unwrap().push(cmdbuf);
    }

    /// Signals the end of the current frame, and starts another.
    pub fn end_frame(&self) {
        // TODO sort command buffers
        unimplemented!()
        //self.backend.submit_frame()
    }
}

/*
// primitive types
// buffer interface types: impl BufferInterface
// sampled image types: SampledImage{1,2,3}D

#[derive(ShaderInterface)]
#[shader_interface(set = "0")]
struct Interface0 {
    #[uniform_constant(index = "0")]
    a: f32,
    #[uniform_constant(index = "1")]
    b: f32,
    #[texture_binding(index = "0")]
    tex: gfx::SampledTexture2D,
    #[uniform_buffer(index = "0")]
    camera_params: gfx::BufferSlice<CameraParams>,
}
*/