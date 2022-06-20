use proc_macro::TokenStream;
use quote::{format_ident, quote};
use regex::Regex;
use std::mem::replace;
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    *,
};

mod kw {
    syn::custom_keyword!(export);
    syn::custom_keyword!(persistent);
}

/// Declare the primary extension entry point for the crate.
///
/// This is equivalent to [macro@sqlite3_ext_init], but it will automatically name the export
/// according to the name of the crate (e.g. `sqlite3_myextension_init`).
///
/// # Examples
///
/// Specify a persistent extension:
///
/// ```no_run
/// #[sqlite3_ext_init(persistent)]
/// fn init(db: &Connection) -> Result<()> {
///     Ok(())
/// }
/// ```
#[proc_macro_attribute]
pub fn sqlite3_ext_main(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr = proc_macro2::TokenStream::from(attr);
    let item = parse_macro_input!(item as syn::ItemFn);
    let crate_name = std::env::var("CARGO_CRATE_NAME").unwrap();
    let export_base = crate_name.to_lowercase();
    let export_base = Regex::new("[^a-z]").unwrap().replace_all(&export_base, "");
    let init_ident = format_ident!("sqlite3_{}_init", export_base);
    let expanded = quote! {
        #[sqlite3_ext_init(export = #init_ident, #attr)]
        #item
    };
    TokenStream::from(expanded)
}

/// Declare the entry point to an extension.
///
/// This method generates an `extern "C"` function suitable for use by SQLite's loadable
/// extension feature. An export name can optionally be provided. Consult [the SQLite
/// documentation](https://www.sqlite.org/loadext.html#loading_an_extension) for information
/// about naming the exported method, but generally you can use [macro@sqlite3_ext_main] to
/// automatically name the export correctly.
///
/// If the persistent keyword is included in the attribute, the extension will be loaded
/// permanently. See [the SQLite
/// documentation](https://www.sqlite.org/loadext.html#persistent_loadable_extensions) for more
/// information.
///
/// # Example
///
/// Specifying a nonstandard entry point name:
///
/// ```no_run
/// #[sqlite3_ext_init(export = "nonstandard_entry_point", persistent)]
/// fn init(db: &Connection) -> Result<()> {
///     Ok(())
/// }
/// ```
///
/// This extension could be loaded from SQLite:
///
/// ```sql
/// SELECT load_extension('path/to/extension', 'nonstandard_entry_point');
/// ```
///
/// # Implementation
///
/// This macro renames the original Rust function and instead creates an
/// `sqlite3_ext::Extension` object in its place. Because `Extension` dereferences to the
/// original function, you generally won't notice this change. This behavior allows you to use
/// the original identifier to pass the auto extension methods.
#[proc_macro_attribute]
pub fn sqlite3_ext_init(attr: TokenStream, item: TokenStream) -> TokenStream {
    let directives =
        parse_macro_input!(attr with Punctuated::<ExtAttr, Token![,]>::parse_terminated);
    let mut export: Option<Ident> = None;
    let mut persistent: Option<kw::persistent> = None;
    for d in directives {
        match d {
            ExtAttr::Export(ExtAttrExport { value }) => {
                if let Some(_) = export {
                    return Error::new(value.span(), "export specified multiple times")
                        .into_compile_error()
                        .into();
                } else {
                    export = Some(value)
                }
            }
            ExtAttr::Persistent(tok) => {
                persistent = Some(tok);
            }
        }
    }
    let mut item = parse_macro_input!(item as syn::ItemFn);
    let extension_vis = replace(&mut item.vis, syn::Visibility::Inherited);
    let name = item.sig.ident.clone();
    let load_result = match persistent {
        None => quote!(::sqlite3_ext::ffi::SQLITE_OK),
        Some(tok) => {
            if let Some(_) = export {
                // Persistent loadable extensions were added in SQLite 3.14.0. If
                // the entry point for the extension returns
                // SQLITE_OK_LOAD_PERSISTENT, then the load fails. We want to
                // detect this situation and allow the load to complete anyways:
                // any API which requires persistent extensions would return an
                // error, but ignored errors imply that the persistent loading
                // requirement is optional.
                quote!(::sqlite3_ext::sqlite3_require_version!(
                    3_014_000,
                    ::sqlite3_ext::ffi::SQLITE_OK_LOAD_PERMANENTLY,
                    ::sqlite3_ext::ffi::SQLITE_OK
                ))
            } else {
                return Error::new(tok.span, "unexported extension cannot be persistent")
                    .into_compile_error()
                    .into();
            }
        }
    };

    let c_export = export.as_ref().map(|_| quote!(#[no_mangle] pub));
    let c_name = match export {
        None => format_ident!("{}_entry", item.sig.ident),
        Some(x) => x,
    };

    let expanded = quote! {
        #[allow(non_upper_case_globals)]
        #extension_vis static #name: ::sqlite3_ext::Extension = {
            #c_export
            unsafe extern "C" fn #c_name(
                db: *mut ::sqlite3_ext::ffi::sqlite3,
                err_msg: *mut *mut ::std::os::raw::c_char,
                api: *mut ::sqlite3_ext::ffi::sqlite3_api_routines,
            ) -> ::std::os::raw::c_int {
                ::sqlite3_ext::ffi::init_api_routines(api);
                match #name(::sqlite3_ext::Connection::from_ptr(db)) {
                    Ok(_) => #load_result,
                    Err(e) => ::sqlite3_ext::ffi::handle_error(e, err_msg),
                }
            }

            #item

            ::sqlite3_ext::Extension::new(#c_name, #name)
        };
    };
    TokenStream::from(expanded)
}

enum ExtAttr {
    Export(ExtAttrExport),
    Persistent(kw::persistent),
}

struct ExtAttrExport {
    value: Ident,
}

impl Parse for ExtAttr {
    fn parse(input: ParseStream) -> Result<Self> {
        let lookahead = input.lookahead1();
        if lookahead.peek(kw::export) {
            input.parse().map(ExtAttr::Export)
        } else if lookahead.peek(kw::persistent) {
            input.parse().map(ExtAttr::Persistent)
        } else {
            Err(lookahead.error())
        }
    }
}

impl Parse for ExtAttrExport {
    fn parse(input: ParseStream) -> Result<Self> {
        input.parse::<kw::export>()?;
        input.parse::<token::Eq>()?;
        Ok(ExtAttrExport {
            value: input.parse()?,
        })
    }
}
