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
    // create a deferrer type, to hold deferred generic updates.
    //
    let deferrer_by_key = groups_by_id
        .iter()
        .map(|(key, _)| {
            let kname = key.to_lowercase();
            let kname = quote::format_ident!("{}", kname);
            let key = quote::format_ident!("{}", key);
            let tt = quote! {
                #kname : Vec<#key>
            };
            (kname, key, tt)
        })
        .collect::<Vec<_>>();

    let dbk = deferrer_by_key.as_slice().iter().map(|(_, _, v)| v);
    let implementations = deferrer_by_key.iter().map(|(k, ty, _)| {
        quote! {
            impl DeferredDeleteById<#ty> for DeferredDeletes {
                fn deferred_delete(&mut self, key: #ty) {
                    self.#k.push(key);
                }
                fn clear_defers(&mut self) {
                    self.#k.clear();
                }
                /// Execute deferred deletes, will clear `self`!
                fn execute<Store: DeleteById<#ty>>(&mut self, store: &mut Store) {
                    let mut deletes = Vec::new();
                    std::mem::swap(&mut deletes, &mut self.#k);
                    for id in deletes.into_iter() {
                        store.delete(&id);
                    }
                }
            }
        }
    });

    let clears = deferrer_by_key.iter().map(|(k, _, _)| {
        quote! {
            self.#k.clear();
        }
    });

    let executes = deferrer_by_key.iter().map(|(_, ty, _)| {
        quote! {
            <Self as DeferredDeleteById::<#ty>>::execute(self,store);
        }
    });

    let deferrer = quote! {
        /// Holds delete requests
        /// Should execute and clear on tick end
        #[derive(Debug, Clone, Default)]
        pub struct DeferredDeletes {
            #(#dbk),*
        }

        impl DeferredDeletes {
            pub fn clear(&mut self) {
                #(#clears);*;
            }
            pub fn execute_all(&mut self, store: &mut Storage) {
                #(#executes);*;
            }
        }

        #(#implementations)*
    };

    // implement the generic delete for all key types
    //
    let implementations = groups_by_id.into_iter().map(|(_, (key, fields))| {
        let deletes = fields.as_slice().iter().map(|field| {
            quote! {
                self.#field.delete(id);
            }
        });
        quote! {
            impl #impl_generics DeleteById<#key> for #name #ty_generics #where_clause {
                fn delete(&mut self, id: &#key) {
                    #(#deletes);*;
                }
            }
        }
    });
    let result = quote! {
        #(#implementations)
        *

        #deferrer
    };

    TokenStream::from(result)
}
