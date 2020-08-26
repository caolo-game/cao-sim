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

struct TableMeta {
    key: TokenTree,
    fields: Vec<TokenTree>,
    rows: Vec<TokenTree>,
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
                    if segment.ident != "cao_storage" {
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
            let entry = groups_by_id
                .entry(format!("{}", key))
                .or_insert_with(|| TableMeta {
                    key,
                    fields: Vec::with_capacity(16),
                    rows: Vec::with_capacity(16),
                });
            entry.fields.push(tokens.next().expect("field name"));
            tokens.next().expect("delimeter");
            entry.rows.push(tokens.next().expect("row name"));
        }
    }
    // create a deferrer type, to hold deferred generic updates.
    //
    let deferrer_by_key = groups_by_id
        .iter()
        .map(|(key, _)| {
            let kname = quote::format_ident!("{}", key.to_lowercase());
            let key = quote::format_ident!("{}", key);
            let tt = quote! {
                #kname : Vec<#key>
            };
            (kname, key, tt)
        })
        .collect::<Vec<_>>();

    let dbk = deferrer_by_key.iter().map(|(_, _, v)| v);
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

    // implement the functionality that's generic over the key for all key types
    //
    let implementations = groups_by_id.into_iter().map(
        |(
            _key,
            TableMeta {
                key: key_token,
                fields,
                rows,
            },
        )| {
            assert_eq!(fields.len(), rows.len());
            let deletes = fields.iter().map(|field| {
                quote! {
                    self.#field.delete(id);
                }
            });

            quote! {
                impl <#impl_generics> DeleteById<#key_token> for #name #ty_generics #where_clause {
                    fn delete(&mut self, id: &#key_token) {
                        #(#deletes)*
                    }
                }
            }
        },
    );
    let result = quote! {
        #(#implementations)*

        #deferrer
    };

    TokenStream::from(result)
}
