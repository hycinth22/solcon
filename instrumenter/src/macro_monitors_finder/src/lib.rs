use proc_macro::*;
use syn::ItemStruct;
use quote::{quote, ToTokens};
use std::cell::OnceCell;

fn get_defpath_from_marker_and_remove_marker(field: &mut syn::Field) -> Option<String> {
    let result = OnceCell::new();
    field.attrs.retain(|attr| {
        let path = &attr.path();
        if path.leading_colon.is_some() {
            return true;
        }
        if path.segments.len() != 1 {
            return true;
        }
        if path.segments.first().unwrap().ident == "monitor_defpath" {
            if let Ok(syn::MetaNameValue{
                value: syn::Expr::Lit(syn::ExprLit{
                    lit: syn::Lit::Str(litstr), ..
                }),
                ..
            }) = attr.meta.require_name_value() {
                if result.set(litstr.value()).is_err() {
                    panic!("duplicated monitor_defpath in a field");
                }
            }
            return false;
        }
        return true;
    });
    result.into_inner()
}

#[proc_macro_attribute]
pub fn generate_impl_monitors_finder_from_monitors_info(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut struct_def: ItemStruct = syn::parse_macro_input!(item);
    let matchbranches = struct_def.fields.iter_mut().filter_map(|field| {
        let defpath = get_defpath_from_marker_and_remove_marker(field);
        if let Some(def_path) = defpath {
            let Some(ref ident) = field.ident else { return None; }; // skip fields without name
            let branch = quote! {
                #def_path => {
                    self.#ident = Some(def_id);
                    info!(concat!("configure monitors.", stringify!(#ident)));
                }
            };
            Some(branch)
        } else {
            None
        }
    });
    let struct_name = &struct_def.ident;
    let expanded_impl = quote! {
        impl MonitorsFinder for #struct_name {
            fn try_match_with_our_function(&mut self, tcx: TyCtxt<'_>, fn_def_id: &DefId) -> bool {
                let def_id = fn_def_id.clone();
                let fn_defpath_str = tcx.def_path_str(def_id);
                trace!("try_match_with_our_function {}", fn_defpath_str);
                let prefix = format!("{}::", config::MONITORS_LIB_CRATE_NAME);
                let Some(fn_defpath_str) = fn_defpath_str.strip_prefix(&prefix) else { 
                    return false;
                };
                match fn_defpath_str {
                    #(#matchbranches)*
                    &_=> {}
                }
                return true;
           }
        }
    };
    for field in struct_def.fields.iter_mut() {
        field.attrs.retain(|attr| {
            let path = &attr.path();
            if path.leading_colon.is_some() {
                return true;
            }
            if path.segments.len() != 1 {
                return true;
            }
            if path.segments.first().unwrap().ident == "monitor_defpath" {
                return false;
            }
            return true;
        });
    }
    let mut result = struct_def.into_token_stream();
    result.extend(expanded_impl);
    result.into()
}

