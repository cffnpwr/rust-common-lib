use proc_macro2::Span;
use syn::token::Bracket;
use syn::{
    Expr, ExprArray, ExprRepeat, Ident, Lit, Token, Type, TypeArray, TypeReference, TypeSlice,
};

// hoge=fugaの形式から識別子を取得
pub(crate) fn parse_ident_from_expr(expr: &Expr) -> syn::Result<Ident> {
    match expr {
        Expr::Path(expr_path) if expr_path.path.segments.len() == 1 => {
            Ok(expr_path.path.segments[0].ident.clone())
        }
        Expr::Lit(expr_lit) => match &expr_lit.lit {
            Lit::Str(lit_str) => {
                // 文字列リテラルから識別子を生成
                syn::parse_str(&lit_str.value()).map_err(|_| {
                    syn::Error::new_spanned(
                        expr,
                        "Expected a valid identifier from string literal.",
                    )
                })
            }
            _ => Err(syn::Error::new_spanned(expr, "Expected a string literal.")),
        },
        _ => Err(syn::Error::new_spanned(
            expr,
            "Expected a single identifier.",
        )),
    }
}

// hoge=fugaの形式から型を取得
pub(crate) fn parse_type_from_expr(expr: &Expr) -> syn::Result<Type> {
    match expr {
        Expr::Path(expr_path) if expr_path.path.segments.len() == 1 => {
            // 単一の識別子から型を生成
            Ok(Type::Path(syn::TypePath {
                qself: None,
                path: expr_path.path.clone(),
            }))
        }
        Expr::Reference(expr_ref) => {
            // 参照型の場合
            let ref_type = &*expr_ref.expr;
            match ref_type {
                Expr::Array(inner_array) if inner_array.elems.len() == 1 => {
                    // スライスの場合
                    let elem_type = &inner_array.elems[0];
                    Ok(Type::Reference(TypeReference {
                        and_token: expr_ref.and_token,
                        lifetime: None,
                        mutability: None,
                        elem: Box::new(Type::Slice(TypeSlice {
                            bracket_token: Bracket(Span::call_site()),
                            elem: Box::new(parse_type_from_expr(elem_type)?),
                        })),
                    }))
                }
                ref_type => Ok(Type::Reference(TypeReference {
                    and_token: expr_ref.and_token,
                    lifetime: None,
                    mutability: None,
                    elem: Box::new(parse_type_from_expr(ref_type)?),
                })),
            }
        }
        Expr::Repeat(ExprRepeat { expr, len, .. }) => match &**expr {
            Expr::Path(expr_path) if expr_path.path.segments.len() == 1 => {
                // Sized arrayの形式の場合
                let elem_type = Type::Path(syn::TypePath {
                    qself: None,
                    path: expr_path.path.clone(),
                });
                Ok(Type::Array(TypeArray {
                    bracket_token: Bracket(Span::call_site()),
                    elem: Box::new(elem_type),
                    semi_token: Token![;](Span::call_site()),
                    len: *len.clone(),
                }))
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    expr,
                    "Expected type path for array element.",
                ));
            }
        },
        _ => Err(syn::Error::new_spanned(
            expr,
            "Expected type path, sized array or reference.",
        )),
    }
}

// hoge=[fuga, piyo, ...]の形式から型のリストを取得
pub(crate) fn parse_types_from_expr(expr: &Expr) -> syn::Result<Vec<Type>> {
    match expr {
        Expr::Array(ExprArray { elems, .. }) => {
            let mut types = Vec::new();
            for elem in elems {
                let ty = parse_type_from_expr(elem)?;
                types.push(ty);
            }
            Ok(types)
        }
        _ => Err(syn::Error::new_spanned(expr, "Expected array of types")),
    }
}

// 型のリストからリスト中のすべての型の参照型も含める
pub(crate) fn gen_all_types(types: &[Type]) -> syn::Result<Vec<Type>> {
    let mut all_types = Vec::new();
    for t in types {
        if all_types.iter().any(|x| x == t) {
            continue; // 既に存在する型はスキップ
        }
        // 元の型を追加
        all_types.push(t.clone());

        // 参照型でない場合は参照型を追加
        if !matches!(t, Type::Reference(_)) {
            let ref_type = Type::Reference(TypeReference {
                and_token: Token![&](Span::call_site()),
                lifetime: None,
                mutability: None,
                elem: Box::new(t.clone()),
            });
            all_types.push(ref_type);
        }
    }
    Ok(all_types)
}

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::*;

    #[test]
    fn test_parse_ident_from_expr() {
        let expr: Expr = parse_quote! { "MyMethod" };
        let ident = parse_ident_from_expr(&expr).unwrap();
        assert_eq!(ident, Ident::new("MyMethod", Span::call_site()));

        let expr: Expr = parse_quote! { MyMethod };
        let ident = parse_ident_from_expr(&expr).unwrap();
        assert_eq!(ident, Ident::new("MyMethod", Span::call_site()));

        let expr: Expr = parse_quote! { 123 };
        let result = parse_ident_from_expr(&expr);
        assert!(result.is_err());

        let expr: Expr = parse_quote! { [u8; 2] };
        let result = parse_ident_from_expr(&expr);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_type_from_expr() {
        let expr: Expr = parse_quote! { MyType };
        let ty = parse_type_from_expr(&expr).unwrap();
        assert_eq!(ty, Type::Path(parse_quote! { MyType }));

        let expr: Expr = parse_quote! { &MyRefType };
        let ty = parse_type_from_expr(&expr).unwrap();
        assert_eq!(ty, Type::Reference(parse_quote! { &MyRefType }));

        let expr: Expr = parse_quote! { &[u8] };
        let ty = parse_type_from_expr(&expr).unwrap();
        assert_eq!(ty, Type::Reference(parse_quote! { &[u8] }));

        let expr: Expr = parse_quote! { [u8; 2] };
        let ty = parse_type_from_expr(&expr).unwrap();
        assert_eq!(ty, Type::Array(parse_quote! { [u8; 2] }));

        let expr: Expr = parse_quote! { "literal" };
        let result = parse_type_from_expr(&expr);
        assert!(result.is_err());

        let expr: Expr = parse_quote! { ["literal"; 2] };
        let result = parse_type_from_expr(&expr);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_types_from_expr() {
        let expr: Expr = parse_quote! { [MyType, &MyRefType, &[u8], [u8; 2]] };
        let result = parse_types_from_expr(&expr);
        assert!(result.is_ok());
        let types = result.unwrap();
        assert_eq!(types.len(), 4);
        assert_eq!(types[0], Type::Path(parse_quote! { MyType }));
        assert_eq!(types[1], Type::Reference(parse_quote! { &MyRefType }));
        assert_eq!(types[2], Type::Reference(parse_quote! { &[u8] }));
        assert_eq!(types[3], Type::Array(parse_quote! { [u8; 2] }));

        let expr: Expr = parse_quote! { MyType };
        let result = parse_types_from_expr(&expr);
        assert!(result.is_err());
    }

    #[test]
    fn test_gen_all_types() {
        let types = vec![
            Type::Path(parse_quote! { MyType }),
            Type::Reference(parse_quote! { &MyType }),
            Type::Reference(parse_quote! { &MyRefType }),
        ];
        let all_types = gen_all_types(&types).unwrap();
        assert_eq!(all_types.len(), 3);
        assert!(all_types.contains(&Type::Path(parse_quote! { MyType })));
        assert!(all_types.contains(&Type::Reference(parse_quote! { &MyRefType })));
        assert!(all_types.contains(&Type::Reference(parse_quote! { &MyType })));
    }
}
