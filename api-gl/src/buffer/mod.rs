use crate::{
    api::{types::*, Gl},
    AliasInfo,
};
use slotmap::new_key_type;
use std::ptr;

mod upload;

pub(crate) use self::upload::{MappedBuffer, UploadBuffer};

//--------------------------------------------------------------------------------------------------

/// Copy + Clone to bypass a restriction of slotmap on stable rust.
#[derive(Copy, Clone, Debug)]
pub struct RawBuffer {
    pub obj: GLuint,
    pub size: usize,
}

impl RawBuffer {
    pub(crate) fn destroy(self, gl: &Gl) {
        unsafe {
            gl.DeleteBuffers(1, &self.obj);
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct BufferDescription {
    pub size: usize,
}

//--------------------------------------------------------------------------------------------------
pub fn create_buffer(
    gl: &Gl,
    byte_size: usize,
    flags: GLenum,
    initial_data: Option<&[u8]>,
) -> GLuint {
    let mut obj: GLuint = 0;
    unsafe {
        gl.CreateBuffers(1, &mut obj);
        gl.NamedBufferStorage(
            obj,
            byte_size as isize,
            if let Some(data) = initial_data {
                data.as_ptr() as *const GLvoid
            } else {
                ptr::null()
            },
            flags,
        );
    }

    obj
}

new_key_type! {
    pub(crate) struct BufferAliasKey;
}

#[derive(Debug)]
pub struct GlBuffer {
    pub(crate) raw: RawBuffer,
    pub(crate) should_destroy: bool,
    pub(crate) alias_info: Option<AliasInfo<BufferAliasKey>>,
    pub(crate) offset: usize,
}
