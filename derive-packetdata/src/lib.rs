use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, Fields};

#[proc_macro_derive(PacketData)]
pub fn derive_packet_data(input: TokenStream) -> TokenStream {
	let ast = syn::parse(input).unwrap();
	impl_codegen(&ast)
}
fn codegen_read(fields: &Fields) -> proc_macro2::TokenStream {
	match fields {
		Fields::Named(ref fields) => {
			let f = fields.named.iter().map(|f| {
				let name = &f.ident;
				let ty = &f.ty;
				quote! {#name: <#ty>::read(buf)?}
			});
			quote! {{#(#f,)*}}
		}
		Fields::Unnamed(ref _fields) => unimplemented!(),
		Fields::Unit => {
			quote! {}
		}
	}
}
fn codegen_write(fields: &Fields) -> proc_macro2::TokenStream {
	match fields {
		Fields::Named(ref fields) => {
			let f = fields.named.iter().map(|f| {
				let name = &f.ident;
				quote! {self.#name.write(buf)?}
			});
			quote! {#(#f;)*}
		}
		Fields::Unnamed(ref _fields) => unimplemented!(),
		Fields::Unit => {
			quote! {}
		}
	}
}
fn impl_codegen(ast: &syn::DeriveInput) -> TokenStream {
	let name = &ast.ident;
	let read = match &ast.data {
		Data::Struct(ref data) => codegen_read(&data.fields),
		Data::Enum(_) => unimplemented!(),
		Data::Union(_) => unimplemented!(),
	};
	let write = match &ast.data {
		Data::Struct(ref data) => codegen_write(&data.fields),
		Data::Enum(_) => unimplemented!(),
		Data::Union(_) => unimplemented!(),
	};
	let gen = quote! {
		#[automatically_derived]
		impl PacketData for #name {
			fn read<R: Read>(buf: &mut R) -> io::Result<Self> {
				Ok(#name #read)
			}
			fn write<W: std::io::Write>(&self, buf: &mut W) -> io::Result<()> {
				#write
				Ok(())
			}
		}
	};
	gen.into()
}
