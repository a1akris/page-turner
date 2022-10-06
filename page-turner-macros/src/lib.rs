use darling::{ast, util, Error, FromDeriveInput, FromField};
use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Field};

const KEY_ATTRIBUTE_NAME: &str = "page_key";

type ExpandedResult<T = TokenStream, E = Error> = std::result::Result<T, E>;

#[derive(Debug)]
struct QueryField {
    ident: Option<Ident>,
    ty: syn::Type,
    is_key: bool,
}

impl FromField for QueryField {
    fn from_field(field: &Field) -> Result<Self, Error> {
        let ident = field.ident.clone();
        let ty = field.ty.clone();
        let is_key = field
            .attrs
            .get(0)
            .and_then(|attr| attr.path.segments.iter().next())
            .map(|segment| segment.ident == KEY_ATTRIBUTE_NAME)
            .unwrap_or(false);

        Ok(QueryField { ident, ty, is_key })
    }
}

#[derive(Debug, FromDeriveInput)]
#[darling(supports(struct_named))]
struct QueryStruct {
    ident: Ident,
    data: ast::Data<util::Ignored, QueryField>,
}

#[proc_macro_derive(PageQuery, attributes(page_key))]
pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match gen_paginated_query_impl(input) {
        Ok(token_stream) => token_stream,
        Err(err) => err.write_errors().into(),
    }
}

fn gen_paginated_query_impl(input: DeriveInput) -> ExpandedResult {
    let QueryStruct {
        ident: struct_name,
        data,
    } = QueryStruct::from_derive_input(&input)?;

    let fields = data.take_struct().expect("We support only named structs");

    let key_field = fields.iter().find(|field| field.is_key).ok_or_else(|| {
        Error::custom(format_args!(
            "Use #[{}] attribute to mark a field representing a key",
            KEY_ATTRIBUTE_NAME
        ))
        .with_span(&struct_name)
    })?;

    let QueryField {
        ident: key_ident,
        ty: key_type,
        ..
    } = key_field;

    let (type_setter, field_setter) = if is_option(key_type) {
        let option_inner_type =
            extract_generic(key_type).expect("We checked that type is Option right above");

        let type_setter = quote! {
            type PageKey = #option_inner_type;
        };

        let field_setter = quote! {
            self.#key_ident = Some(key);
        };

        (type_setter, field_setter)
    } else {
        let type_setter = quote! {
            type PageKey = #key_type;
        };

        let field_setter = quote! {
            self.#key_ident = key;
        };

        (type_setter, field_setter)
    };

    Ok(quote! {
        impl ::page_turner::PageQuery for #struct_name {
            #type_setter

            fn set_page_key(&mut self, key: Self::PageKey) {
                #field_setter
            }
        }
    }
    .into())
}

fn is_option(ty: &syn::Type) -> bool {
    const OPTION_PATHS: &[&str] = &[
        "Option",
        "std::option::Option",
        "core::option::Option",
        "option::Option",
    ];

    if let syn::Type::Path(ty) = ty {
        let path_str = ty
            .path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>()
            .join("::");

        OPTION_PATHS.contains(&path_str.as_str())
    } else {
        false
    }
}

fn extract_generic(ty: &syn::Type) -> Option<syn::GenericArgument> {
    if let syn::Type::Path(ty) = ty {
        ty.path
            .segments
            .iter()
            .find_map(|segment| match segment.arguments {
                syn::PathArguments::AngleBracketed(ref args) => Some(args),
                _ => None,
            })
            .and_then(|generics| generics.args.iter().next())
            .cloned()
    } else {
        None
    }
}
