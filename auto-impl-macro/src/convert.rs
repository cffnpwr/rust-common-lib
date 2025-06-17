use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, DeriveInput, Ident, Type, Token, bracketed, parse::{Parse, ParseStream}};

use crate::common::{
    gen_all_types, parse_ident_from_expr, parse_type_from_expr,
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

        attr.parse_args_with(|input: ParseStream| {
            while !input.is_empty() {
                let name: Ident = input.parse()?;
                input.parse::<Token![=]>()?;
                
                if name == "method" {
                    let expr: syn::Expr = input.parse()?;
                    output.method = Some(parse_ident_from_expr(&expr)?);
                } else if name == "error" {
                    let expr: syn::Expr = input.parse()?;
                    output.error = Some(parse_type_from_expr(&expr)?);
                } else if name == "types" {
                    // 角括弧内の型配列をパース
                    let content;
                    bracketed!(content in input);
                    
                    // カンマ区切りの型をパース
                    let types = content.parse_terminated(Type::parse, Token![,])?;
                    output.sources = types.into_iter().collect();
                } else {
                    return Err(syn::Error::new_spanned(&name, format!("Unknown attribute: {}", name)));
                }
                
                if !input.is_empty() {
                    input.parse::<Token![,]>()?;
                }
            }
            Ok(())
        })?;
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    #[test]
    fn test_generate_auto_try_from() {
        // [正常系] 完全な属性でのTryFrom実装生成
        let input: DeriveInput = parse_quote! {
            #[auto_try_from(method = try_from_bytes, error = HardwareTypeError, types = [&[u8], [u8; 2], Vec<u8>, Box<[u8]>])]
            enum HardwareType {
                Ethernet = 1,
            }
        };
        let result = generate_auto_try_from(input);
        assert!(result.is_ok());
        let tokens = result.unwrap();
        
        // 生成されたコードの基本的な内容をチェック
        let tokens_str = tokens.to_string();
        
        // キーワードの存在を確認
        assert!(tokens_str.contains("impl"));
        assert!(tokens_str.contains("std"));
        assert!(tokens_str.contains("convert"));
        assert!(tokens_str.contains("TryFrom"));
        assert!(tokens_str.contains("HardwareType"));
        assert!(tokens_str.contains("HardwareTypeError"));
        assert!(tokens_str.contains("try_from_bytes"));
        
        // 各型に対するimplが生成されていることを確認
        assert!(tokens_str.contains("& [u8]"));
        assert!(tokens_str.contains("[u8 ; 2]"));
        assert!(tokens_str.contains("Vec"));
        assert!(tokens_str.contains("Box"));

        // [異常系] 空の属性
        let input: DeriveInput = parse_quote! {
            #[auto_try_from()]
            struct MyStruct;
        };
        let result = generate_auto_try_from(input);
        assert!(result.is_err());

        // [異常系] methodのみ指定
        let input: DeriveInput = parse_quote! {
            #[auto_try_from(method = from_source)]
            struct MyStruct;
        };
        let result = generate_auto_try_from(input);
        assert!(result.is_err());

        // [異常系] 必須パラメータ不足 - errorのみ
        let input: DeriveInput = parse_quote! {
            #[auto_try_from(error = MyError)]
            struct MyStruct;
        };
        let result = generate_auto_try_from(input);
        assert!(result.is_err());

        // [異常系] 必須パラメータ不足 - typesのみ
        let input: DeriveInput = parse_quote! {
            #[auto_try_from(types = [u8])]
            struct MyStruct;
        };
        let result = generate_auto_try_from(input);
        assert!(result.is_err());

        // [異常系] 未知の属性名
        let input: DeriveInput = parse_quote! {
            #[auto_try_from(unknown_param = value)]
            struct MyStruct;
        };
        let result = generate_auto_try_from(input);
        assert!(result.is_err());
    }

}
