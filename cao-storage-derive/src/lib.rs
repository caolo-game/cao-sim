//! Helper crate to generate the caolo-sim crate's data storage structs'
//!
#![crate_type = "proc-macro"]
use proc_macro::TokenStream;
use proc_macro2::TokenTree;
use quote::quote;
use std::collections::HashMap;
use syn::{parse_macro_input, AttrStyle, DeriveInput, Ident};

#[proc_macro_derive(CaoStorage, attributes(cao_storage_table, cao_storage_iterby))]
pub fn derive_storage(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    impl_storage(input)
}

#[derive(Debug)]
struct TableMeta {
    /// type of the key/id
    key: TokenTree,
    /// name of individual fields
    fields: Vec<TokenTree>,
    /// types of individual fields
    rows: Vec<TokenTree>,
}

#[derive(Debug)]
struct IterMeta<'a> {
    /// Type of the key/id
    key: TokenTree,
    /// Name of the primary field
    primary_field: TokenTree,
    /// Name of the fields to join
    fields: &'a [TokenTree],
    /// Name of the field types to join
    rows: &'a [TokenTree],
}

fn impl_tables(
    name: &Ident,
    generics: &syn::Generics,
    table_groups: &HashMap<String, TableMeta>,
) -> proc_macro2::TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    // create a deferrer type, to hold deferred generic updates.
    //
    let deferrer_by_key = table_groups
        .iter()
        .map(|(key, _)| {
            let kname = quote::format_ident!("{}", key.to_lowercase());
            let key = quote::format_ident!("{}", key);
            let tt = quote! {
                pub(crate) #kname : Vec<#key>
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
                    let deletes = std::mem::take(&mut self.#k);
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
    let implementations = table_groups.iter().map(
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
    quote! {
        #(#implementations)*

        #deferrer
    }
}

fn impl_iterators<'a>(
    name: &Ident,
    generics: &syn::Generics,
    its: impl Iterator<Item = IterMeta<'a>>,
) -> proc_macro2::TokenStream {
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let implementations = its.map(
        |IterMeta {
             primary_field,
             fields,
             rows,
             key
         }| {
            let fun_name = quote::format_ident!("iterby_{}", format!("{}", primary_field));
            let ty_name = quote::format_ident!("IterBy_{}_Tuple", format!("{}", primary_field));
            let ty_fields = fields.iter().zip(rows.iter()).map(|(f, r)| {
                quote! {
                    pub #f : Option<&'boi #r>
                }
            });
            let gets = fields.iter().map(|f| {
                quote! {
                    let #f = self.#f.get_by_id(&id)
                }
            });
            quote! {
               #[derive(serde::Serialize)]
               pub struct #ty_name <'boi> {
                   pub __id: #key,

                   #(
                       #[serde(skip_serializing_if = "Option::is_none")] 
                       #ty_fields
                    ),*,
               }
               impl <#impl_generics> #name #ty_generics #where_clause {
                    pub fn #fun_name <'boi> (&'boi self) -> impl Iterator<Item=#ty_name <'boi>> + 'boi {
                        self.#primary_field.iter().map(move |(id, _)| {
                            #(#gets);*;
                            #ty_name {
                                __id: id,
                                #(#fields),*
                            }
                        } )
                    }
                }
            }
        },
    );
    quote! {
        #(#implementations)*
    }
}

fn impl_storage(input: DeriveInput) -> TokenStream {
    let name: &Ident = &input.ident;
    let generics: syn::Generics = input.generics;

    let mut table_groups: HashMap<String, TableMeta> = HashMap::with_capacity(16);
    let mut iterators = Vec::with_capacity(16);

    'a: for attr in input.attrs {
        if let AttrStyle::Outer = attr.style {
            match attr.path.segments.first() {
                None => continue 'a,
                Some(segment) => {
                    let ident = segment.ident.to_string();
                    let token = attr.tokens.into_iter().next().expect("group");
                    let group = match token {
                        TokenTree::Group(group) => group,
                        _ => panic!("expected a group, got {:?}", token),
                    };
                    let mut tokens = group.stream().into_iter();
                    match ident.as_str() {
                        "cao_storage_table" => {
                            let key = tokens.next().expect("key name");
                            tokens.next().expect("delimeter");
                            let entry =
                                table_groups.entry(format!("{}", key)).or_insert_with(|| {
                                    TableMeta {
                                        key,
                                        fields: Vec::with_capacity(16),
                                        rows: Vec::with_capacity(16),
                                    }
                                });
                            entry.fields.push(tokens.next().expect("field name"));
                            tokens.next().expect("delimeter");
                            entry.rows.push(tokens.next().expect("row name"));
                        }
                        "cao_storage_iterby" => {
                            let field = tokens.next().expect("field name");
                            tokens.next().expect("delimeter");
                            let key = tokens.next().expect("key name");
                            iterators.push((field, key));
                        }
                        _ => unreachable!("got unreachable identifier {}", ident),
                    }
                }
            }
        }
    }

    let tables = impl_tables(name, &generics, &table_groups);

    let iters = impl_iterators(
        name,
        &generics,
        iterators.into_iter().map(|(field, key)| {
            let entry = match table_groups.get(format!("{}", key).as_str()) {
                Some(x) => x,
                None => panic!("Primary iterator key: ({}) does not exist!", key),
            };
            IterMeta {
                key,
                primary_field: field,
                fields: entry.fields.as_slice(),
                rows: entry.rows.as_slice(),
            }
        }),
    );

    let result = quote! {
        #tables

        #iters
    };

    TokenStream::from(result)
}
