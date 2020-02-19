extern crate proc_macro;
use proc_macro::TokenStream;
use syn::{Token, DeriveInput, parse_macro_input};
use quote::*;
use proc_macro2;

#[proc_macro_derive(Request, attributes(request))]
pub fn derive_request(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let result = match ast.data {
        syn::Data::Struct(ref s) => derive_request_for_struct(&ast, &s.fields),
        _ => panic!("doesn't work with enums or unions yet"),
    };
    TokenStream::from(result)
}

fn derive_request_for_struct(ast: &syn::DeriveInput, fields: &syn::Fields) -> proc_macro2::TokenStream {
    match *fields {
        syn::Fields::Named(ref fields) => {
            impl_request_for_struct(&ast, Some(&fields.named))
        },
        syn::Fields::Unit => {
            impl_request_for_struct(&ast, None)
        },
        syn::Fields::Unnamed(..) => {
            panic!("doesn't work with unnamed fields yet")
        },
    }
}

fn impl_request_for_struct(ast: &syn::DeriveInput, fields: Option<&syn::punctuated::Punctuated<syn::Field, Token![,]>>) -> proc_macro2::TokenStream {
    let struct_name = &ast.ident;
    
    let request_method = ast.attrs.iter().find_map(|attr| {
        if let Ok(syn::Meta::List(metalist)) = attr.parse_meta() {
            if let Some(ident) = metalist.path.get_ident() {
                if ident == "request" {
                    if metalist.nested.len() != 1 {
                        panic!("must have one method name in request attribute");
                    }
                    return metalist.nested.first().map(nested_meta_to_ident);
                }
            }
        }
        None
    }).expect("must have request attribute with method name");

    let empty = Default::default();
    let fields_kvs = fields.unwrap_or(&empty).iter().map(|f| {
        let f_name = &f.ident;
        let f_name_str = format!("{}", f_name.as_ref().unwrap());
        quote!{
            #f_name_str: self.#f_name
        }
    });

    let request_method_name = format!("{}", request_method);

    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    quote! {
        impl #impl_generics crate::model::Request for #struct_name #ty_generics #where_clause {
            fn request(&self, token: &str) -> Result<String, crate::Error> {
                let json = serde_json::to_string(&serde_json::json!({
                    "method": #request_method_name,
                    "token": token,
                    #(#fields_kvs),*
                }))?;
                Ok(json)
            }
        }
    }
}

fn nested_meta_to_ident(nm: &syn::NestedMeta) -> proc_macro2::Ident {
    match nm {
        syn::NestedMeta::Meta(meta) => {
            match meta {
                syn::Meta::Path(path) => path.get_ident().cloned().unwrap(),
                _ => panic!("must be single path"),
            }
        },
        syn::NestedMeta::Lit(lit) => {
            match lit {
                syn::Lit::Str(litstr) => proc_macro2::Ident::new(&litstr.value(), proc_macro2::Span::call_site()),
                _ => panic!("must be string literal"),
            }
        }
    }
}

#[proc_macro_derive(Response, attributes(response))]
pub fn derive_response(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);
    let result = match ast.data {
        syn::Data::Struct(..) => derive_response_for_struct(&ast),
        _ => panic!("doesn't work with enums or unions yet"),
    };
    TokenStream::from(result)
}

fn derive_response_for_struct(ast: &syn::DeriveInput) -> proc_macro2::TokenStream {
    let struct_name = &ast.ident;

    let response_meta = ast.attrs.iter().find_map(|attr| {
        if let Ok(syn::Meta::List(metalist)) = attr.parse_meta() {
            if let Some(ident) = metalist.path.get_ident() {
                if ident == "response" {
                    return Some(metalist.nested);
                }
            }
        }
        None
    }).expect("must have response attribute with method name");
    
    let format = response_meta.iter().find_map(|m| {
        if let syn::NestedMeta::Meta(syn::Meta::NameValue(nv)) = m {
            if nv.path.is_ident("format") {
                if let syn::Lit::Str(ref strlit) = nv.lit {
                    return Some(strlit.value());
                }
            }
        }
        None
    }).expect("format must be set in response attribute");

    let ty = response_meta.iter().find_map(|m| {
        if let syn::NestedMeta::Meta(syn::Meta::NameValue(nv)) = m {
            if nv.path.is_ident("type") {
                if let syn::Lit::Str(ref strlit) = nv.lit {
                    return Some(strlit.value());
                }
            }
        }
        None
    });

    let (consume_block, output_ty) = match format.as_ref() {
        "csv" => {
            let cb = quote! { crate::model::consume_csv(&mut response) };
            let ty = ty.expect("type must be set in response attribute when format is csv");
            let ty: syn::Type = syn::parse_str(&format!("Vec<{}>", ty)).expect("invalid type in response attribute");
            (cb, ty)
        },
        "line" => {
            let cb = quote! { crate::model::consume_line(&mut response) };
            if ty.is_some() {
                panic!("type should not be set in response attribute when format is line");
            }
            let ty: syn::Type = syn::parse_str("Vec<String>").unwrap();
            (cb, ty)
        },
        "single" => {
            let cb = quote! { crate::model::consume_single(&mut response) };
            let ty = ty.expect("type must be set in response attribute when format is single");
            let ty: syn::Type = syn::parse_str(&ty).expect("invalid type in response attribute");
            (cb, ty)
        }
        "json" => {
            let cb = quote! { crate::model::consume_json(&mut response) };
            let ty = ty.expect("type must be set in response attribute when format is json");
            let ty: syn::Type = syn::parse_str(&ty).expect("invalid type in response attribute");
            (cb, ty)
        }
        _ => panic!("format {} not supported", format),
    };

    let (impl_generics, ty_generics, where_clause) = ast.generics.split_for_impl();
    quote! {
        impl #impl_generics crate::model::Response for #struct_name #ty_generics #where_clause {
            type Output = #output_ty;
            fn response(&self, mut response: reqwest::blocking::Response) -> Result<#output_ty, crate::Error> {
                #consume_block
            }
        }
    }
}