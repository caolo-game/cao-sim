//! Helper crate to generate the caolo-sim crate's data storage structs'
//!
#![crate_type = "proc-macro"]
use proc_macro::TokenStream;
use proc_macro2::TokenTree;
use quote::quote;
use std::collections::HashMap;
use syn::{parse_macro_input, AttrStyle, DeriveInput};

#[proc_macro_derive(CaoStorage, attributes(cao_storage))]
pub fn derive_storage(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    impl_storage(input)
}

fn impl_storage(input: DeriveInput) -> TokenStream {
    let name = &input.ident;
    let generics = input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let mut groups_by_id = HashMap::new();
    'a: for attr in input.attrs {
        if let AttrStyle::Outer = attr.style {
            match attr.path.segments.first() {
                None => continue 'a,
                Some(segment) => {
                    if format!("{}", segment.ident) != "cao_storage" {
                        continue 'a;
                    }
                }
            }
            let group = match attr.tokens.into_iter().next().expect("group") {
                TokenTree::Group(group) => group,
                _ => panic!("expected a group"),
            };
            let mut tokens = group.stream().into_iter();
            let key = tokens.next().expect("key name");
            tokens.next().expect("delimeter");
            groups_by_id
                .entry(format!("{}", key))
                .or_insert_with(|| (key, Vec::new()))
                .1
                .push(tokens.next().expect("field name"));
        }
    }

    let implementations = groups_by_id.into_iter().map(|(_, (key, fields))| {
        let deletes = fields.as_slice().iter().map(|field| {
            quote! {
                self.#field.delete(id);
            }
        });
        quote! {
            impl #impl_generics Epic<#key> for #name #ty_generics #where_clause {
                fn delete(&mut self, id: &#key) {
                    #(#deletes);*;
                }
            }
        }
    });
    let result = quote! {
        #(#implementations)
        *
    };

    TokenStream::from(result)
}
