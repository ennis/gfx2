use crate::commandext::CommandBufferExt;
use autograph_api::{
    buffer::{Buffer, BufferData, BufferTypeless, StructuredBufferData, TypedConstantBufferView},
    command::{CommandBuffer, DrawIndexedParams, DrawParams},
    format::Format,
    image::{
        DepthStencilView, Image1d, Image2d, Image2dBuilder, Image3d, ImageCreateInfo,
        RenderTargetBuilder, RenderTargetImage2d, RenderTargetView,
    },
    pipeline::{Arguments, GraphicsPipeline, TypedSignature},
    Arena, Backend, Api,
};
use std::{any::TypeId, cell::RefCell, collections::HashMap, mem};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct ImageDesc1d {
    format: Format,
    width: u32,
    mips: u32,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct ImageDesc2d {
    format: Format,
    width: u32,
    height: u32,
    mips: u32,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
struct ImageDesc3d {
    format: Format,
    width: u32,
    height: u32,
    depth: u32,
    mips: u32,
}

#[derive(Eq, PartialEq, derivative::Derivative)]
#[derivative(Copy(bound = ""), Clone(bound = ""), Debug(bound = ""))]
enum BlackboardResource<B: Backend> {
    Image1d {
        desc: ImageDesc1d,
        img: *const B::Image,
    },
    Image2d {
        desc: ImageDesc2d,
        img: *const B::Image,
    },
    Image3d {
        desc: ImageDesc3d,
        img: *const B::Image,
    },
    Buffer {
        size: usize,
        buf: *const B::Buffer,
        typeid: Option<TypeId>,
    },
}

pub struct Blackboard<'a, B: Backend> {
    parent: Option<&'a Blackboard<'a, B>>,
    arena: Arena<'a, B>,
    lookup: RefCell<HashMap<String, BlackboardResource<B>>>,
}

impl<'a, B: Backend> Blackboard<'a, B> {
    pub fn new(r: &'a Api<B>) -> Blackboard<'a, B> {
        Blackboard {
            lookup: RefCell::new(HashMap::new()),
            arena: r.create_arena(),
            parent: None,
        }
    }

    pub fn arena(&self) -> &Arena<'a, B> {
        &self.arena
    }

    pub fn buffer<T: Copy + 'static>(&self, name: &str, data: &T) -> Buffer<B, T> {
        self.buffer_by_name(name).unwrap_or_else(|| {
            let buf = self.arena.upload(data);
            self.lookup.borrow_mut().insert(
                name.to_string(),
                BlackboardResource::Buffer {
                    size: mem::size_of::<T>(),
                    typeid: Some(TypeId::of::<T>()),
                    buf: buf.inner() as *const _,
                },
            );
            buf
        })
    }

    pub fn buffer_slice<T: Copy + 'static>(&self, name: &str, data: &[T]) -> Buffer<B, [T]> {
        self.buffer_by_name(name).unwrap_or_else(|| {
            let buf = self.arena.upload_slice(data);
            self.lookup.borrow_mut().insert(
                name.to_string(),
                BlackboardResource::Buffer {
                    size: mem::size_of_val(data),
                    typeid: Some(TypeId::of::<[T]>()),
                    buf: buf.inner() as *const _,
                },
            );
            buf
        })
    }

    pub fn buffer_by_name<T: BufferData + ?Sized>(&self, name: &str) -> Option<Buffer<B, T>> {
        if let Some(BlackboardResource::Buffer { buf, typeid, .. }) = self.get_resource(name) {
            if typeid == Some(TypeId::of::<T>()) {
                return unsafe { Some(Buffer::from_raw(&*buf)) };
            }
        }
        None
    }

    ///
    pub fn image_2d<'b: 'n, 'n>(
        &'b self,
        name: &'n str,
        format: Format,
        width: u32,
        height: u32,
    ) -> Image2dBuilder<Image2d<'b, B>, impl Fn(&ImageCreateInfo) -> Image2d<'b, B> + 'n> {
        Image2dBuilder::new(format, (width, height), move |c| {
            let desc = ImageDesc2d {
                format,
                width,
                height,
                mips: c.mipmaps.count(width, height, 1),
            };

            if let Some(BlackboardResource::Image2d { desc: d2, img }) = self.get_resource(name) {
                assert_eq!(d2, desc);
                // reborrow to 'self lifetime: OK because inside own arena (and stable addresses), or
                // any parent, which lives longer
                unsafe { Image2d::from_raw(&*img) }
            } else {
                let img = self.arena.create_image(
                    c.scope,
                    c.format,
                    c.dimensions,
                    c.mipmaps,
                    c.samples,
                    c.usage,
                    c.data,
                );
                self.lookup.borrow_mut().insert(
                    name.to_string(),
                    BlackboardResource::Image2d {
                        desc,
                        img: img.inner() as *const _,
                    },
                );
                unsafe { Image2d::from_raw(img.inner()) }
            }
        })
    }

    pub fn image_2d_by_name(&self, name: &str) -> Option<Image2d<B>> {
        if let Some(BlackboardResource::Image2d { img, .. }) = self.get_resource(name) {
            unsafe { Some(Image2d::from_raw(&*img)) }
        } else {
            None
        }
    }

    fn get_resource(&self, name: &str) -> Option<BlackboardResource<B>> {
        if let Some(r) = self.lookup.borrow().get(name) {
            Some(*r)
        } else if let Some(r) = self.parent.and_then(|p| p.get_resource(name)) {
            Some(r)
        } else {
            None
        }
    }
}
