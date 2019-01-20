extern crate autograph_render;
#[macro_use]
extern crate autograph_render_macros;

use autograph_render::{
    Buffer, BufferTypeless, DescriptorSet, Framebuffer, IndexFormat, RendererBackend, ScissorRect,
    Viewport,
};

#[derive(DescriptorSetInterface)]
struct TestDescriptorSetInterface<'a, R: RendererBackend> {
    #[descriptor(uniform_buffer, index = "0")]
    buffer: BufferTypeless<'a, R>,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Vertex {
    pub pos: [f32; 2],
    pub tex: [f32; 2],
}

#[derive(PipelineInterface)]
pub struct TestPipelineInterface<'a, R: RendererBackend> {
    #[framebuffer]
    pub framebuffer: Framebuffer<'a, R>,
    #[descriptor_set]
    pub per_object: DescriptorSet<'a, R>,
    #[viewport]
    pub viewport: Viewport,
    #[vertex_buffer]
    pub vertex_buffer: Buffer<'a, R, [Vertex]>,
    #[vertex_buffer]
    pub vertex_buffer_prev: Buffer<'a, R, [Vertex]>,
    #[descriptor_set]
    pub per_frame: DescriptorSet<'a, R>,
    #[index_buffer]
    pub indices: Buffer<'a, R, [u16]>,
}

#[test]
fn test_descriptor_set_interface() {}

#[test]
fn test_pipeline_interface() {
    struct Visitor<'a, R: RendererBackend> {
        descriptor_sets: Vec<DescriptorSet<'a, R>>,
        vertex_buffers: Vec<DescriptorSet<'a, R>>,
        viewports: Vec<DescriptorSet<'a, R>>,
        scissors: Vec<DescriptorSet<'a, R>>,
        index_buffer: Option<DescriptorSet<'a, R>>,
    };

    impl<'a, R: RendererBackend> autograph_render::interface::PipelineInterfaceVisitor<'a, R>
        for Visitor<'a, R>
    {
        fn visit_descriptor_sets<I: IntoIterator<Item = DescriptorSet<'a, R>>>(
            &mut self,
            descriptor_sets: I,
        ) {
            self.descriptor_sets.extend(descriptor_sets);
        }

        fn visit_vertex_buffers<I: IntoIterator<Item = BufferTypeless<'a, R>>>(
            &mut self,
            vertex_buffers: I,
        ) {
            self.vertex_buffers.extend(descriptor_sets);
        }

        fn visit_index_buffer(
            &mut self,
            index_buffer: BufferTypeless<'a, R>,
            offset: usize,
            ty: IndexFormat,
        ) {
            self.index_buffer = Some(index_buffer);
        }

        fn visit_framebuffer(&mut self, framebuffer: Framebuffer<'a, R>) {
            self.cmdbuf.set_framebuffer(self.sortkey, framebuffer);
        }

        fn visit_dynamic_viewports<I: IntoIterator<Item = Viewport>>(&mut self, viewports: I) {
            self.cmdbuf.set_viewports(self.sortkey, viewports);
        }

        fn visit_dynamic_scissors<I: IntoIterator<Item = ScissorRect>>(&mut self, scissors: I) {
            self.cmdbuf.set_scissors(self.sortkey, scissors);
        }
    }
}