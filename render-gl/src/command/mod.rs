use crate::api as gl;
use crate::api::types::*;
use crate::api::Gl;
use crate::ImplementationParameters;
use crate::swapchain::GlSwapchain;
use crate::image::GlImage;
use autograph_render::traits;
use autograph_render::command::Command;
use autograph_render::command::CommandInner;
use autograph_render::vertex::IndexFormat;
use autograph_render::pipeline::Viewport;
use crate::pipeline::GlGraphicsPipeline;
use crate::DowncastPanic;

mod state;
pub use self::state::StateCache;
use crate::descriptor::ShaderResourceBindings;
use crate::framebuffer::GlFramebuffer;
use crate::buffer::GlBuffer;
use crate::descriptor::GlDescriptorSet;


pub struct SubmissionContext<'a, 'rcx> {
    state_cache: &'a mut StateCache,
    gl: &'a Gl,
    _impl_params: &'a ImplementationParameters,
    current_pipeline: Option<&'rcx GlGraphicsPipeline>,
}

impl<'a, 'rcx> SubmissionContext<'a, 'rcx> {
    pub fn new(
        gl: &'a Gl,
        state_cache: &'a mut StateCache,
        impl_params: &'a ImplementationParameters,
    ) -> SubmissionContext<'a, 'rcx> {
        SubmissionContext {
            state_cache,
            gl,
            _impl_params: impl_params,
            current_pipeline: None,
        }
    }

    fn cmd_clear_image_float(&mut self, image: &GlImage, color: &[f32; 4]) {
        if image.raw.target == gl::RENDERBUFFER {
            // create temporary framebuffer
            let mut tmpfb = 0;
            unsafe {
                self.gl.CreateFramebuffers(1, &mut tmpfb);
                self.gl.NamedFramebufferRenderbuffer(
                    tmpfb,
                    gl::COLOR_ATTACHMENT0,
                    gl::RENDERBUFFER,
                    image.raw.obj,
                );
                self.gl.NamedFramebufferDrawBuffers(tmpfb, 1, (&[gl::COLOR_ATTACHMENT0]).as_ptr());
                self.gl.ClearNamedFramebufferfv(tmpfb, gl::COLOR, 0, color.as_ptr());
                self.gl.DeleteFramebuffers(1, &tmpfb);
            }
        } else {
            // TODO specify which level to clear in command
            unsafe {
                self.gl.ClearTexImage(
                    image.raw.obj,
                    0,
                    gl::RGBA,
                    gl::FLOAT,
                    color.as_ptr() as *const _,
                );
            }
        }
    }

    fn cmd_clear_depth_stencil_image(
        &mut self,
        image: &GlImage,
        depth: f32,
        stencil: Option<u8>,
    ) {
        let obj = image.raw.obj;
        if image.raw.target == gl::RENDERBUFFER {
            // create temporary framebuffer
            let mut tmpfb = 0;
            unsafe {
                self.gl.CreateFramebuffers(1, &mut tmpfb);
                self.gl.NamedFramebufferRenderbuffer(
                    tmpfb,
                    gl::DEPTH_ATTACHMENT,
                    gl::RENDERBUFFER,
                    obj,
                );
                if let Some(_stencil) = stencil {
                    unimplemented!()
                } else {
                    self.gl.ClearNamedFramebufferfv(tmpfb, gl::DEPTH, 0, &depth);
                }
                self.gl.DeleteFramebuffers(1, &tmpfb);
            }
        } else {
            // TODO specify which level to clear in command
            unsafe {
                if let Some(_stencil) = stencil {
                    unimplemented!()
                } else {
                    self.gl.ClearTexImage(
                        obj,
                        0,
                        gl::DEPTH_COMPONENT,
                        gl::FLOAT,
                        &depth as *const f32 as *const _,
                    );
                }
            }
        }
    }

    //pub fn cmd_set_attachments(&mut self, color_attachments: &[R::])

    fn cmd_set_descriptor_sets(
        &mut self,
        descriptor_sets: &[&dyn traits::DescriptorSet],
    ) {
        let pipeline = self.current_pipeline.unwrap();
        let descriptor_map = pipeline.descriptor_map();
        let mut sr = ShaderResourceBindings::new();

        for (i, &ds) in descriptor_sets.iter().enumerate() {
            let ds : &GlDescriptorSet = ds.downcast_ref_unwrap();
            ds.collect(i as u32, descriptor_map, &mut sr);
        }

        self.state_cache.set_uniform_buffers(self.gl,
            &sr.uniform_buffers,
            &sr.uniform_buffer_offsets,
            &sr.uniform_buffer_sizes,
        );
        self.state_cache.set_shader_storage_buffers(self.gl,
                                                    &sr.shader_storage_buffers,
            &sr.shader_storage_buffer_offsets,
            &sr.shader_storage_buffer_sizes,
        );
        self.state_cache.set_textures(self.gl, &sr.textures);
        self.state_cache.set_samplers(self.gl, &sr.samplers);
        self.state_cache.set_images(self.gl, &sr.images);
    }

    fn cmd_present(&mut self, image: &GlImage, swapchain: &GlSwapchain) {
        // only handle default swapchain for now
        //assert_eq!(swapchain, 0, "invalid swapchain handle");
        // make a framebuffer and bind the image to it
        unsafe {
            let mut tmpfb = 0;
            self.gl.CreateFramebuffers(1, &mut tmpfb);
            // bind image to it
            if image.raw.target == gl::RENDERBUFFER {
                self.gl.NamedFramebufferRenderbuffer(
                    tmpfb,
                    gl::COLOR_ATTACHMENT0,
                    gl::RENDERBUFFER,
                    image.raw.obj,
                );
            } else {
                // TODO other levels / layers?
                self.gl.NamedFramebufferTexture(tmpfb, gl::COLOR_ATTACHMENT0, image.raw.obj, 0);
            }
            // blit to default framebuffer
            let (w, h): (u32, u32) = swapchain.inner.size();

            self.gl.BlitNamedFramebuffer(
                tmpfb,
                0,
                0,        // srcX0
                0,        // srcY0
                w as i32, // srcX1,
                h as i32, // srcY1,
                0,        // dstX0,
                0,        // dstY0,
                w as i32, // dstX1
                h as i32, // dstY1
                gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT,
                gl::NEAREST,
            );

            // destroy temp framebuffer
            self.gl.DeleteFramebuffers(1, &tmpfb);
        }

        // swap buffers
        swapchain.inner.present()
    }

    fn cmd_set_framebuffer(&mut self, fb: &'rcx GlFramebuffer) {
        self.state_cache.set_draw_framebuffer(self.gl, fb.obj);
    }

    fn cmd_set_graphics_pipeline(&mut self, pipeline: &'rcx GlGraphicsPipeline) {
        // switching pipelines
        self.current_pipeline = Some(pipeline);
        pipeline.bind(self.gl, self.state_cache);
    }

    fn cmd_set_vertex_buffers(&mut self, buffers: &[&'rcx dyn traits::Buffer]) {
        let pipeline = self
            .current_pipeline
            .expect("cmd_set_vertex_buffers called with no pipeline bound");
        let vertex_input_bindings = pipeline.vertex_input_bindings();

        let mut objs = smallvec::SmallVec::<[GLuint; 8]>::new();
        let mut offsets = smallvec::SmallVec::<[GLintptr; 8]>::new();
        let mut strides = smallvec::SmallVec::<[GLsizei; 8]>::new();

        for (i, &vb) in buffers.iter().enumerate() {
            let vb : &GlBuffer = vb.downcast_ref_unwrap();
            objs.push(vb.raw.obj);
            offsets.push(vb.offset as isize);
            strides.push(vertex_input_bindings[i].stride as i32);
        }

        self.state_cache
            .set_vertex_buffers(self.gl, &objs, &offsets, &strides);
    }

    fn cmd_set_viewports(&mut self, viewports: &[Viewport]) {
        self.state_cache.set_viewports(self.gl, viewports);
    }

    fn cmd_set_index_buffer(&mut self, index_buffer: &'rcx GlBuffer, offset: usize, ty: IndexFormat) {
        self.state_cache
            .set_index_buffer(self.gl, index_buffer.raw.obj, offset, ty);
    }

    fn cmd_draw(
        &mut self,
        vertex_count: u32,
        instance_count: u32,
        first_vertex: u32,
        first_instance: u32,
    ) {
        let pipeline = self
            .current_pipeline
            .expect("cmd_set_vertex_buffers called with no pipeline bound");
        self.state_cache.draw(
            self.gl,
            pipeline.input_assembly_state.topology,
            vertex_count,
            instance_count,
            first_vertex,
            first_instance,
        );
    }

    fn cmd_draw_indexed(
        &mut self,
        index_count: u32,
        instance_count: u32,
        first_index: u32,
        vertex_offset: i32,
        first_instance: u32,
    ) {
        let pipeline = self
            .current_pipeline
            .expect("cmd_set_vertex_buffers called with no pipeline bound");
        self.state_cache.draw_indexed(
            self.gl,
            pipeline.input_assembly_state.topology,
            index_count,
            instance_count,
            first_index,
            vertex_offset,
            first_instance,
        );
    }

    pub fn submit_command(&mut self, command: &Command<'rcx>) {
        match command.cmd {
            CommandInner::PipelineBarrier {} => {
                // no-op on GL
            }
            CommandInner::ClearImageFloat { image, color } => {
                self.cmd_clear_image_float(image.downcast_ref_unwrap(), &color);
            }
            CommandInner::ClearDepthStencilImage {
                image,
                depth,
                stencil,
            } => {
                self.cmd_clear_depth_stencil_image(image.downcast_ref_unwrap(), depth, stencil);
            }
            CommandInner::SetDescriptorSets {
                ref descriptor_sets,
            } => {
                self.cmd_set_descriptor_sets(descriptor_sets);
            }
            CommandInner::SetVertexBuffers { ref vertex_buffers } => {
                self.cmd_set_vertex_buffers(vertex_buffers);
            }
            CommandInner::SetIndexBuffer {
                index_buffer,
                offset,
                ty,
            } => {
                self.cmd_set_index_buffer(index_buffer.downcast_ref_unwrap(), offset, ty);
            }
            CommandInner::DrawHeader { pipeline } => {
                self.cmd_set_graphics_pipeline(pipeline.downcast_ref_unwrap());
            }
            CommandInner::SetScissors { .. } => {}
            //CommandInner::SetAllScissors { scissor } => {}
            CommandInner::SetViewports { ref viewports } => {
                self.cmd_set_viewports(viewports);
            }
            //CommandInner::SetAllViewports { viewport } => {}
            CommandInner::SetFramebuffer { framebuffer } => {
                self.cmd_set_framebuffer(framebuffer.downcast_ref_unwrap());
            }
            CommandInner::Draw {
                vertex_count,
                instance_count,
                first_vertex,
                first_instance,
            } => self.cmd_draw(vertex_count, instance_count, first_vertex, first_instance),
            CommandInner::DrawIndexed {
                index_count,
                instance_count,
                first_index,
                vertex_offset,
                first_instance,
            } => self.cmd_draw_indexed(
                index_count,
                instance_count,
                first_index,
                vertex_offset,
                first_instance,
            ),
            CommandInner::Present { image, swapchain } => {
                self.cmd_present(image.downcast_ref_unwrap(), swapchain.downcast_ref_unwrap());
            }
        }
    }
}