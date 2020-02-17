extern crate proc_macro;
use self::proc_macro::TokenStream;
use syn::{Token, DeriveInput, parse_macro_input};
use quote::*;
use proc_macro2;

#[proc_macro_derive(RequestCommand, attributes(request))]
pub fn derive(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let result = match ast.data {
        syn::Data::Struct(ref s) => new_for_struct(&ast, &s.fields),
        _ => panic!("doesn't work with unions yet"),
    };
    result.into()
}

fn new_for_struct(ast: &syn::DeriveInput, fields: &syn::Fields) -> proc_macro2::TokenStream {
    match *fields {
        syn::Fields::Named(ref fields) => {
            new_impl(&ast, Some(&fields.named), true)
        },
        syn::Fields::Unit => {
            new_impl(&ast, None, false)
        },
        syn::Fields::Unnamed(ref fields) => {
            new_impl(&ast, Some(&fields.unnamed), false)
        },
    }
}

fn new_impl(ast: &syn::DeriveInput, fields: Option<&syn::punctuated::Punctuated<syn::Field, Token![,]>>, named: bool) -> proc_macro2::TokenStream {
    let struct_name = &ast.ident;
    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    quote! {
        impl #impl_generics crate::model::RequestCommand for #struct_name #ty_generics #where_clause {
            fn request_body(&self, token: &str) -> Result<String, crate::Error> {
                let json = serde_json::to_string(&json!({

                }))?;
                Ok(json)
            }
        }
    }
}