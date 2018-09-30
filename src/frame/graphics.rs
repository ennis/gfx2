use super::*;
use sid_vec::ToIndex;

//--------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub(crate) struct GraphicsTask {
    pub(crate) renderpass: RenderPassId,
    pub(crate) color_attachments: Vec<vk::AttachmentReference>,
    pub(crate) input_attachments: Vec<vk::AttachmentReference>,
    pub(crate) resolve_attachments: Vec<vk::AttachmentReference>,
    pub(crate) depth_attachment: Option<vk::AttachmentReference>,
    pub(crate) shader_images: Vec<ImageId>,
}

//--------------------------------------------------------------------------------------------------

/// Task builder specifically for graphics
pub struct GraphicsTaskBuilder<'a, 'ctx: 'a> {
    graph: &'a mut FrameGraph,
    resources: &'a mut Resources<'ctx>,
    renderpasses: &'a mut RenderPasses,
    task: TaskId,
    graphics_task: GraphicsTask,
}

impl<'a, 'ctx: 'a> GraphicsTaskBuilder<'a, 'ctx> {
    pub(super) fn new(
        name: impl Into<String>,
        renderpass: RenderPassId,
        graph: &'a mut FrameGraph,
        resources: &'a mut Resources<'ctx>,
        renderpasses: &'a mut RenderPasses,
    ) -> GraphicsTaskBuilder<'a, 'ctx> {
        // create a dummy node in the graph that we will fill up later.
        // this avoids looking into the graph every time we modify something,
        // and still allows us to create dependencies in the graph
        let task = graph.create_task_on_queue(name, 0, TaskDetails::Other);
        renderpasses[renderpass].tasks.push(task);

        GraphicsTaskBuilder {
            graph,
            resources,
            renderpasses,
            task,
            graphics_task: GraphicsTask {
                renderpass,
                shader_images: Vec::new(),
                color_attachments: Vec::new(),
                input_attachments: Vec::new(),
                resolve_attachments: Vec::new(),
                depth_attachment: None,
            },
        }
    }

    /// Adds the specified as an image sample dependency on the task.
    pub fn sample_image(&mut self, img: &ImageRef) {
        img.set_read().expect("R/W conflict");

        self.resources
            .add_or_check_image_usage(img.id, vk::IMAGE_USAGE_SAMPLED_BIT);

        self.graph.add_dependency(
            img.task,
            self.task,
            Dependency {
                src_stage_mask: img.src_stage_mask,
                dst_stage_mask: vk::PIPELINE_STAGE_VERTEX_SHADER_BIT,
                barrier: BarrierDetail::Image(ImageBarrier {
                    id: img.id,
                    old_layout: vk::ImageLayout::Undefined,
                    new_layout: vk::ImageLayout::ShaderReadOnlyOptimal,
                    src_access_mask: vk::AccessFlags::empty(),
                    dst_access_mask: vk::ACCESS_SHADER_READ_BIT,
                }),
                latency: img.latency,
            },
        );

        self.graphics_task.shader_images.push(img.id);
    }

    //----------------------------------------------------------------------------------------------
    // BIND ATTACHMENTS

    pub fn set_depth_attachment(&mut self, depth_attachment: &AttachmentRef) {
        self.graphics_task.depth_attachment = Some(vk::AttachmentReference {
            attachment: depth_attachment.id.index.to_index() as u32,
            layout: vk::ImageLayout::DepthStencilAttachmentOptimal, // FIXME may be read only
        });

        if depth_attachment.task != self.task {
            self.graph.add_dependency(
                depth_attachment.task,
                self.task,
                Dependency {
                    src_stage_mask: depth_attachment.src_stage_mask,
                    dst_stage_mask: vk::PIPELINE_STAGE_EARLY_FRAGMENT_TESTS_BIT, // FIXME not sure
                    barrier: BarrierDetail::Subpass(SubpassBarrier {
                        id: depth_attachment.id.img,
                        old_layout: vk::ImageLayout::Undefined, // unused
                        new_layout: vk::ImageLayout::Undefined, // unused
                        src_access_mask: vk::AccessFlags::empty(),
                        dst_access_mask: vk::ACCESS_DEPTH_STENCIL_ATTACHMENT_WRITE_BIT
                            | vk::ACCESS_DEPTH_STENCIL_ATTACHMENT_READ_BIT,
                    }),
                    latency: depth_attachment.latency,
                },
            );
        }

        self.resources.add_or_check_image_usage(
            depth_attachment.id.img,
            vk::IMAGE_USAGE_DEPTH_STENCIL_ATTACHMENT_BIT,
        );

        /*
        if let Some(dependency) = depth_attachment.dependency {
            self.frame.add_dependency_access_flags(
                dependency,
                vk::ACCESS_DEPTH_STENCIL_ATTACHMENT_READ_BIT
                    | vk::ACCESS_DEPTH_STENCIL_ATTACHMENT_WRITE_BIT,
            );
        }*/
    }

    /// Specifies input attachments for the pass.
    pub fn set_input_attachments(&mut self, input_attachments: &[&AttachmentRef]) {
        self.graphics_task.input_attachments = input_attachments
            .iter()
            .map(|a| vk::AttachmentReference {
                attachment: a.id.index.to_index() as u32,
                layout: vk::ImageLayout::ColorAttachmentOptimal, // FIXME should not be changed?
            }).collect::<Vec<_>>();

        for i in input_attachments {
            // avoid self-dependencies for now (unrelated to subpass self dependencies)
            if i.task != self.task {
                self.graph.add_dependency(
                    i.task,
                    self.task,
                    Dependency {
                        src_stage_mask: i.src_stage_mask,
                        dst_stage_mask: vk::PIPELINE_STAGE_TOP_OF_PIPE_BIT, // FIXME not sure
                        barrier: BarrierDetail::Subpass(SubpassBarrier {
                            id: i.id.img,
                            old_layout: vk::ImageLayout::Undefined,
                            new_layout: vk::ImageLayout::ColorAttachmentOptimal, // FIXME duplicated with attachment reference
                            src_access_mask: vk::AccessFlags::empty(),
                            dst_access_mask: vk::ACCESS_INPUT_ATTACHMENT_READ_BIT,
                        }),
                        latency: i.latency,
                    },
                );
            } else {
                // same task, should update creation bits directly
            }

            // update usage bits of the resource
            self.resources
                .add_or_check_image_usage(i.id.img, vk::IMAGE_USAGE_INPUT_ATTACHMENT_BIT);
        }
    }

    /// Specifies the color attachments for the pass.
    pub fn set_color_attachments(&mut self, color_attachments: &[&AttachmentRef]) {
        self.graphics_task.color_attachments = color_attachments
            .iter()
            .map(|a| vk::AttachmentReference {
                attachment: a.id.index.to_index() as u32,
                layout: vk::ImageLayout::ColorAttachmentOptimal,
            }).collect::<Vec<_>>();

        // update access bits of the dependency
        for c in color_attachments {
            /*if let Some(dependency) = c.dependency {
                let load_op = self
                    .graphics_task
                    .get_attachment_desc(c.vk_ref.attachment)
                    .load_op;

                let access = if load_op == vk::AttachmentLoadOp::Load {
                    vk::ACCESS_COLOR_ATTACHMENT_READ_BIT | vk::ACCESS_COLOR_ATTACHMENT_WRITE_BIT
                } else {
                    vk::ACCESS_COLOR_ATTACHMENT_WRITE_BIT
                };

                self.frame.add_dependency_access_flags(dependency, access);
            }*/
            if c.task != self.task {
                self.graph.add_dependency(
                    c.task,
                    self.task,
                    Dependency {
                        src_stage_mask: c.src_stage_mask,
                        dst_stage_mask: vk::PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT, // FIXME not sure
                        barrier: BarrierDetail::Subpass(SubpassBarrier {
                            id: c.id.img,
                            old_layout: vk::ImageLayout::Undefined,
                            new_layout: vk::ImageLayout::ColorAttachmentOptimal, // FIXME duplicated with attachment reference
                            src_access_mask: vk::AccessFlags::empty(),
                            dst_access_mask: vk::ACCESS_COLOR_ATTACHMENT_READ_BIT
                                | vk::ACCESS_COLOR_ATTACHMENT_WRITE_BIT,
                        }),
                        latency: c.latency,
                    },
                );
            }

            self.resources
                .add_or_check_image_usage(c.id.img, vk::IMAGE_USAGE_COLOR_ATTACHMENT_BIT);
        }
    }

    //----------------------------------------------------------------------------------------------
    // ATTACHMENT LOAD/STORE/CREATE

    /// Imports a resource to be used as an attachment in the subpass.
    pub fn load_attachment(
        &mut self,
        img: &ImageRef,
        load_op: vk::AttachmentLoadOp,
    ) -> AttachmentRef {
        let img_create_info = self.resources.get_image_create_info(img.id);

        let attachment_index = self.renderpasses[self.graphics_task.renderpass].add_attachment(
            img.id,
            vk::AttachmentDescription {
                flags: vk::AttachmentDescriptionFlags::empty(),
                format: img_create_info.format,
                samples: img_create_info.samples,
                load_op,
                store_op: vk::AttachmentStoreOp::DontCare,
                stencil_load_op: load_op,
                stencil_store_op: vk::AttachmentStoreOp::DontCare,
                initial_layout: vk::ImageLayout::Undefined,
                final_layout: vk::ImageLayout::Undefined,
            },
        );

        // create reference
        AttachmentRef {
            task: img.task,
            id: AttachmentId {
                renderpass: self.graphics_task.renderpass,
                index: attachment_index,
                img: img.id,
            },
            read: Cell::new(false),
            written: Cell::new(false),
            src_stage_mask: vk::PIPELINE_STAGE_TOP_OF_PIPE_BIT, // no need to sync, just created it
            latency: 0,
        }
    }

    /// TODO DOCUMENT
    pub fn store_attachment(
        &mut self,
        attachment_ref: AttachmentRef,
        store_op: vk::AttachmentStoreOp,
    ) -> ImageRef {
        self.renderpasses[self.graphics_task.renderpass].attachments_desc
            [attachment_ref.id.index]
            .stencil_store_op = store_op;

        ImageRef {
            id: attachment_ref.id.img,
            task: self.task,
            src_stage_mask: vk::PIPELINE_STAGE_COLOR_ATTACHMENT_OUTPUT_BIT,
            read: Cell::new(false),
            written: Cell::new(false),
            latency: 0, // FIXME better estimate
        }
    }

    /// Creates a new image that will be used as an attachment by the task.
    pub fn create_attachment(
        &mut self,
        name: impl Into<String>,
        (width, height): (u32, u32),
        format: vk::Format,
        samples: vk::SampleCountFlags,
        load_op: vk::AttachmentLoadOp, // should be either CLEAR or DONT_CARE
    ) -> AttachmentRef {
        // declare image resource
        let desc = vk::ImageCreateInfo {
            s_type: vk::StructureType::ImageCreateInfo,
            p_next: ptr::null(),
            flags: vk::ImageCreateFlags::default(),
            image_type: vk::ImageType::Type2d,
            format,
            extent: vk::Extent3D {
                width,
                height,
                depth: 1,
            },
            mip_levels: 1, // FIXME ?
            array_layers: 1,
            samples,
            tiling: vk::ImageTiling::Optimal,
            usage: vk::ImageUsageFlags::empty(), // added on use
            sharing_mode: vk::SharingMode::Concurrent,
            queue_family_index_count: 0,
            p_queue_family_indices: ptr::null(),
            initial_layout: vk::ImageLayout::ColorAttachmentOptimal,
        };
        let img = self.resources.create_image(name, desc);

        // declare attachment
        let attachment_index = self.renderpasses[self.graphics_task.renderpass].add_attachment(
            img,
            vk::AttachmentDescription {
                flags: vk::AttachmentDescriptionFlags::empty(),
                format,
                samples,
                load_op,
                store_op: vk::AttachmentStoreOp::DontCare,
                stencil_load_op: load_op,
                stencil_store_op: vk::AttachmentStoreOp::DontCare,
                initial_layout: vk::ImageLayout::Undefined, // don't care
                final_layout: vk::ImageLayout::Undefined,
            },
        );

        // create reference
        AttachmentRef {
            task: self.task,
            id: AttachmentId {
                renderpass: self.graphics_task.renderpass,
                index: attachment_index,
                img,
            },
            read: Cell::new(false),
            written: Cell::new(false),
            src_stage_mask: vk::PIPELINE_STAGE_TOP_OF_PIPE_BIT, // no need to sync, just created it
            latency: 0,
        }
    }

    pub(super) fn finish(mut self) -> TaskId {
        self.graph.0.node_weight_mut(self.task).unwrap().details =
            TaskDetails::Graphics(self.graphics_task);
        self.task
    }
}
