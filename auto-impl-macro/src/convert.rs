use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::{Attribute, DeriveInput, Ident, Meta, MetaNameValue, Token, Type};

use crate::common::{
    gen_all_types, parse_ident_from_expr, parse_type_from_expr, parse_types_from_expr,
};

#[derive(Default)]
struct AutoTryFromAttrs {
    method: Option<Ident>,
    error: Option<Type>,
    sources: Vec<Type>,
}

pub(crate) fn generate_auto_try_from(input: DeriveInput) -> syn::Result<TokenStream> {
    let name = &input.ident;
    let attrs = parse_auto_try_from_attrs(&input.attrs)?;
    let method = attrs.method.ok_or_else(|| {
        syn::Error::new_spanned(name, "Missing `method` in `#[auto_try_from(...)]`")
    })?;
    let error = attrs.error.ok_or_else(|| {
        syn::Error::new_spanned(name, "Missing `error` in `#[auto_try_from(...)]`")
    })?;
    let sources = attrs.sources;
    let sources = gen_all_types(&sources)?;

    let impls = sources.iter().map(|source| {
        quote! {
            impl ::std::convert::TryFrom<#source> for #name {
                type Error = #error;

                fn try_from(value: #source) -> ::std::result::Result<Self, Self::Error> {
                    Self::#method(value)
                }
            }
        }
    });
    Ok(quote! {
        #(#impls)*
    })
}

fn parse_auto_try_from_attrs(attrs: &[Attribute]) -> syn::Result<AutoTryFromAttrs> {
    let mut output = AutoTryFromAttrs::default();
    for attr in attrs {
        if !attr.path().is_ident("auto_try_from") {
            continue;
        }

        match &attr.meta {
            Meta::List(meta_list) => {
                let nested =
                    meta_list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
                for meta in nested {
                    match meta {
                        Meta::NameValue(MetaNameValue { path, value, .. }) => {
                            if path.is_ident("method") {
                                output.method = Some(parse_ident_from_expr(&value)?);
                            } else if path.is_ident("error") {
                                output.error = Some(parse_type_from_expr(&value)?);
                            } else if path.is_ident("types") {
                                output.sources = parse_types_from_expr(&value)?;
                            }
                        }
                        _ => {
                            return Err(syn::Error::new_spanned(
                                meta,
                                "Expected a name-value pair. Expected format: `hoge = fuga`",
                            ));
                        }
                    }
                }
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    attr,
                    "Expected a list of attributes.",
                ));
            }
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    #[test]
    fn test_generate_auto_try_from() {
        let input: DeriveInput = parse_quote! {
            #[auto_try_from(method = from_source, error = MyError, types = [SourceType, &SourceRefType])]
            struct MyStruct;
        };
        let expected: TokenStream = quote! {
            impl ::std::convert::TryFrom<SourceType> for MyStruct {
                type Error = MyError;

                fn try_from(value: SourceType) -> ::std::result::Result<Self, Self::Error> {
                    Self::from_source(value)
                }
            }
            impl ::std::convert::TryFrom<&SourceType> for MyStruct {
                type Error = MyError;

                fn try_from(value: &SourceType) -> ::std::result::Result<Self, Self::Error> {
                    Self::from_source(value)
                }
            }
            impl ::std::convert::TryFrom<&SourceRefType> for MyStruct {
                type Error = MyError;

                fn try_from(value: &SourceRefType) -> ::std::result::Result<Self, Self::Error> {
                    Self::from_source(value)
                }
            }
        };

        let result = generate_auto_try_from(input);
        assert!(result.is_ok());
        let tokens = result.unwrap();
        assert_eq!(tokens.to_string(), expected.to_string());

        let input: DeriveInput = parse_quote! {
            #[auto_try_from()]
            struct MyStruct;
        };
        let result = generate_auto_try_from(input);
        assert!(result.is_err());

        let input: DeriveInput = parse_quote! {
            #[auto_try_from(method = from_source)]
            struct MyStruct;
        };
        let result = generate_auto_try_from(input);
        assert!(result.is_err());

        let input: DeriveInput = parse_quote! {
            #[auto_try_from(method = from_source, error)]
            struct MyStruct;
        };
        let result = generate_auto_try_from(input);
        assert!(result.is_err());
    }
}
