//! Proc-macro for auto-deriving pipeline interfaces:
//! - `PipelineInterface`
//! - `BufferLayout` for verifying the layout of uniform buffer data with SPIR-V
//! - `AttachmentGroup` for groups of attachments
//! - `VertexLayout` for verifying the layout of vertex buffers
//!
#![recursion_limit = "128"]

extern crate darling; // this is a _good crate_
extern crate proc_macro;
extern crate proc_macro2;
extern crate quote;
extern crate syn;

mod descriptor_set_interface;
mod layout;
mod pipeline_interface;

fn autograph_name() -> syn::Path {
    syn::parse_str("autograph_render").unwrap()
}

#[proc_macro_derive(StructuredBufferData)]
pub fn structured_buffer_data_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).expect("Couldn't parse item");

    let result = match ast.data {
        syn::Data::Struct(ref s) => layout::generate_structured_buffer_data(&ast, &s.fields),
        _ => panic!("StructuredBufferData trait can only be automatically derived on structs."),
    };

    result.into()
}

#[proc_macro_derive(VertexData)]
pub fn vertex_data_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).expect("Couldn't parse item");

    let result = match ast.data {
        syn::Data::Struct(ref s) => layout::generate_vertex_data(&ast, &s.fields),
        _ => panic!("BufferLayout trait can only be automatically derived on structs."),
    };

    result.into()
}

#[proc_macro_derive(DescriptorSetInterface, attributes(descriptor))]
pub fn descriptor_set_interface_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).expect("Couldn't parse item");

    let result = match ast.data {
        syn::Data::Struct(ref s) => descriptor_set_interface::generate(&ast, &s.fields),
        _ => panic!("DescriptorSetInterface trait can only be derived on structs"),
    };

    result.into()
}

#[proc_macro_derive(
    PipelineInterface,
    attributes(
        interface,
        framebuffer,
        descriptor_set,
        descriptor_set_array,
        viewport,
        viewport_array,
        scissor,
        scissor_array,
        vertex_buffer,
        vertex_buffer_array,
        index_buffer
    )
)]
pub fn pipeline_interface_derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).expect("Couldn't parse item");

    let result = match ast.data {
        syn::Data::Struct(ref s) => pipeline_interface::generate(&ast, &s.fields),
        _ => panic!("PipelineInterface trait can only be derived on structs"),
    };

    result.into()
}