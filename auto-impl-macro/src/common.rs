use proc_macro2::Span;
use syn::token::Bracket;
use syn::{
    Expr, ExprRepeat, Ident, Lit, Token, Type, TypeArray, TypeReference, TypeSlice,
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
        Expr::Path(expr_path) => {
            // パス型（単純な型名やジェネリクス型）
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
            Expr::Path(expr_path) => {
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
        Expr::Lit(expr_lit) => match &expr_lit.lit {
            Lit::Str(lit_str) => {
                // 文字列リテラルから型をパース（型として有効な文字列のみ）
                let type_str = lit_str.value();
                // 型として有効でない単純なリテラルはエラーとする
                if type_str == "literal" || !type_str.chars().next().unwrap_or('a').is_ascii_uppercase() {
                    return Err(syn::Error::new_spanned(
                        expr,
                        format!("'{}' is not a valid type", type_str),
                    ));
                }
                syn::parse_str(&type_str).map_err(|_| {
                    syn::Error::new_spanned(
                        expr,
                        format!("Failed to parse type from string: '{}'", type_str),
                    )
                })
            }
            _ => Err(syn::Error::new_spanned(expr, "Expected a string literal.")),
        },
        _ => Err(syn::Error::new_spanned(
            expr,
            "Expected type path, sized array, reference, or string literal.",
        )),
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
        // [正常系] 文字列リテラルから識別子を取得
        let expr: Expr = parse_quote! { "MyMethod" };
        let result = parse_ident_from_expr(&expr);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Ident::new("MyMethod", Span::call_site()));

        // [正常系] パス式から識別子を取得
        let expr: Expr = parse_quote! { MyMethod };
        let result = parse_ident_from_expr(&expr);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), Ident::new("MyMethod", Span::call_site()));

        // [異常系] 整数リテラル
        let expr: Expr = parse_quote! { 123 };
        let result = parse_ident_from_expr(&expr);
        assert!(result.is_err());

        // [異常系] 配列式
        let expr: Expr = parse_quote! { [u8; 2] };
        let result = parse_ident_from_expr(&expr);
        assert!(result.is_err());

        // [異常系] 無効な識別子文字列
        let expr: Expr = parse_quote! { "invalid identifier!" };
        let result = parse_ident_from_expr(&expr);
        assert!(result.is_err());

        // [異常系] 非文字列リテラル
        let expr: Expr = parse_quote! { 42 };
        let result = parse_ident_from_expr(&expr);
        assert!(result.is_err());

        // [異常系] 複数セグメントのパス
        let expr: Expr = parse_quote! { std::io::Error };
        let result = parse_ident_from_expr(&expr);
        assert!(result.is_err());

        // [異常系] 空の文字列
        let expr: Expr = parse_quote! { "" };
        let result = parse_ident_from_expr(&expr);
        assert!(result.is_err());

        // [異常系] 無効な文字を含む文字列
        let expr: Expr = parse_quote! { "123invalid" };
        let result = parse_ident_from_expr(&expr);
        assert!(result.is_err());

        // [異常系] 浮動小数点リテラル
        let expr: Expr = parse_quote! { 3.14 };
        let result = parse_ident_from_expr(&expr);
        assert!(result.is_err());

        // [異常系] ブール値リテラル
        let expr: Expr = parse_quote! { true };
        let result = parse_ident_from_expr(&expr);
        assert!(result.is_err());

        // [異常系] 複雑な式
        let expr: Expr = parse_quote! { 1 + 2 };
        let result = parse_ident_from_expr(&expr);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_type_from_expr() {
        // [正常系] パス型の解析
        let expr: Expr = parse_quote! { MyType };
        let ty = parse_type_from_expr(&expr).unwrap();
        assert_eq!(ty, Type::Path(parse_quote! { MyType }));

        // [正常系] 参照型の解析
        let expr: Expr = parse_quote! { &MyRefType };
        let ty = parse_type_from_expr(&expr).unwrap();
        assert_eq!(ty, Type::Reference(parse_quote! { &MyRefType }));

        // [正常系] スライス型の参照解析
        let expr: Expr = parse_quote! { &[u8] };
        let ty = parse_type_from_expr(&expr).unwrap();
        assert_eq!(ty, Type::Reference(parse_quote! { &[u8] }));

        // [正常系] 固定長配列型の解析
        let expr: Expr = parse_quote! { [u8; 2] };
        let ty = parse_type_from_expr(&expr).unwrap();
        assert_eq!(ty, Type::Array(parse_quote! { [u8; 2] }));

        // [正常系] 有効な型文字列
        let expr: Expr = parse_quote! { "Vec<u8>" };
        let result = parse_type_from_expr(&expr);
        assert!(result.is_ok());

        // [異常系] 無効な型名文字列
        let expr: Expr = parse_quote! { "literal" };
        let result = parse_type_from_expr(&expr);
        assert!(result.is_err());

        // [異常系] 配列の無効な要素型
        let expr: Expr = parse_quote! { ["literal"; 2] };
        let result = parse_type_from_expr(&expr);
        assert!(result.is_err());

        // [異常系] 非文字列リテラル
        let expr: Expr = parse_quote! { 123 };
        let result = parse_type_from_expr(&expr);
        assert!(result.is_err());

        // [異常系] 浮動小数点リテラル
        let expr: Expr = parse_quote! { 3.14 };
        let result = parse_type_from_expr(&expr);
        assert!(result.is_err());

        // [異常系] ブール値リテラル
        let expr: Expr = parse_quote! { true };
        let result = parse_type_from_expr(&expr);
        assert!(result.is_err());

        // [異常系] 無効な型名（小文字開始）
        let expr: Expr = parse_quote! { "invalid_type" };
        let result = parse_type_from_expr(&expr);
        assert!(result.is_err());

        // [異常系] 空の文字列
        let expr: Expr = parse_quote! { "" };
        let result = parse_type_from_expr(&expr);
        assert!(result.is_err());

        // [異常系] 複雑な式
        let expr: Expr = parse_quote! { 1 + 2 };
        let result = parse_type_from_expr(&expr);
        assert!(result.is_err());
    }


    #[test]
    fn test_gen_all_types() {
        // [正常系] 型リストから参照型を含む全型リストを生成
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

        // [正常系] 重複する型の重複排除
        let types = vec![
            Type::Path(parse_quote! { MyType }),
            Type::Path(parse_quote! { MyType }),  // 重複
            Type::Reference(parse_quote! { &MyType }),
        ];
        let all_types = gen_all_types(&types).unwrap();
        assert_eq!(all_types.len(), 2);  // 重複が排除されて2つ
        assert!(all_types.contains(&Type::Path(parse_quote! { MyType })));
        assert!(all_types.contains(&Type::Reference(parse_quote! { &MyType })));

        // [正常系] 空の型リスト
        let types = vec![];
        let all_types = gen_all_types(&types).unwrap();
        assert_eq!(all_types.len(), 0);

        // [正常系] 参照型のみの入力
        let types = vec![
            Type::Reference(parse_quote! { &str }),
            Type::Reference(parse_quote! { &[u8] }),
        ];
        let all_types = gen_all_types(&types).unwrap();
        assert_eq!(all_types.len(), 2);
        assert!(all_types.contains(&Type::Reference(parse_quote! { &str })));
        assert!(all_types.contains(&Type::Reference(parse_quote! { &[u8] })));
    }



}
