use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::Path;
use heck::{ToLowerCamelCase, ToSnakeCase};
use proc_macro2::Span;
use quote::quote;
use swc_core::common::comments::{Comments, SingleThreadedComments};
use swc_core::common::sync::Lrc;
use swc_core::common::{FileName, Globals, Mark, SourceMap, GLOBALS};
use swc_core::ecma::ast::*;
use swc_core::ecma::codegen::text_writer::JsWriter;
use swc_core::ecma::codegen::{Config, Emitter};
use swc_core::ecma::parser::{lexer::Lexer, Parser, StringInput, Syntax, TsSyntax};
use swc_core::ecma::transforms::typescript::strip;

use crate::syntax::{InteropMode, ItemSpec};

#[derive(Debug, Clone)]
pub struct ParamInfo {
    pub name: String,
    pub rust_param_type: proc_macro2::TokenStream,
    pub is_callback: bool,
    pub callback_arg_type: Option<proc_macro2::TokenStream>,
}

#[derive(Debug, Clone)]
pub struct ExportedFunction {
    pub name: String,
    pub mode: InteropMode,
    pub params: Vec<ParamInfo>,
    pub return_rust_type: Option<proc_macro2::TokenStream>,
    pub doc_comment: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AnalyzedModule {
    pub module_hash: String,
    pub inlined_js: String,
    pub exports: Vec<ExportedFunction>,
}

pub fn analyze_file(
    file_path: &Path,
    macro_items: &[ItemSpec],
    wildcard: bool,
    call_span: Span,
) -> syn::Result<AnalyzedModule> {
    let source_code = std::fs::read_to_string(file_path).map_err(|e| {
        syn::Error::new(
            call_span,
            format!("Failed to read bridge file '{}': {}", file_path.display(), e),
        )
    })?;

    analyze_source(&source_code, file_path.to_string_lossy().as_ref(), macro_items, wildcard, call_span)
}

pub fn analyze_source(
    source_code: &str,
    file_name: &str,
    macro_items: &[ItemSpec],
    wildcard: bool,
    call_span: Span,
) -> syn::Result<AnalyzedModule> {
    let cm: Lrc<SourceMap> = Default::default();
    let comments = SingleThreadedComments::default();

    let fm = cm.new_source_file(
        Lrc::new(FileName::Custom(file_name.to_string())),
        source_code.to_string(),
    );

    let lexer = Lexer::new(
        Syntax::Typescript(TsSyntax {
            tsx: false,
            decorators: false,
            no_early_errors: true,
            ..Default::default()
        }),
        Default::default(),
        StringInput::from(&*fm),
        Some(&comments),
    );

    let mut parser = Parser::new_from(lexer);
    let module = parser.parse_module().map_err(|e| {
        syn::Error::new(call_span, format!("TypeScript parse error in '{}': {:?}", file_name, e))
    })?;

    // 1. Check for top-level static import rejection (GD-03)
    for item in &module.body {
        if let ModuleItem::ModuleDecl(ModuleDecl::Import(_)) = item {
            return Err(syn::Error::new(
                call_span,
                "Top-level static import is not supported in v1 bindable files. Use preloaded globals (window.*) or dynamic import() inside an async query.",
            ));
        }
        if let ModuleItem::ModuleDecl(ModuleDecl::ExportDefaultDecl(_) | ModuleDecl::ExportDefaultExpr(_)) = item {
            return Err(syn::Error::new(
                call_span,
                "Default exports are not supported in bind_js!. Use named exports ('export function name()') to avoid Rust identifier conflicts.",
            ));
        }
    }

    // 2. Extract exported functions and analyze signatures
    let mut exports = Vec::new();
    for item in &module.body {
        if let ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export_decl)) = item {
            match &export_decl.decl {
                Decl::Fn(fn_decl) => {
                    let is_target = is_item_targeted(&fn_decl.ident.sym, macro_items, wildcard);
                    let func = extract_fn_decl(fn_decl, export_decl.span, &comments, macro_items, is_target, call_span)?;
                    exports.push(func);
                }
                Decl::Var(var_decl) => {
                    for decl in &var_decl.decls {
                        let is_target = match &decl.name {
                            Pat::Ident(binding) => is_item_targeted(&binding.id.sym, macro_items, wildcard),
                            _ => false,
                        };
                        if let Some(func) = extract_var_decl(decl, export_decl.span, &comments, macro_items, is_target, call_span)? {
                            exports.push(func);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    // 3. Compute deterministic module hash
    let mut hasher = DefaultHasher::new();
    file_name.hash(&mut hasher);
    source_code.hash(&mut hasher);
    let module_hash = format!("{:x}", hasher.finish());

    // 4. Strip TypeScript types into standard inlined JavaScript
    let inlined_js = strip_ts_types(cm, module);

    Ok(AnalyzedModule {
        module_hash,
        inlined_js,
        exports,
    })
}

fn extract_comment_mode(
    comments: &SingleThreadedComments,
    spans: &[swc_core::common::Span],
) -> (Option<InteropMode>, Option<String>) {
    let mut mode = None;
    let mut doc_lines = Vec::new();

    for span in spans {
        if let Some(leading) = comments.get_leading(span.lo) {
            for c in leading {
                let text = c.text.trim();
                if text.contains("#[command]") {
                    mode = Some(InteropMode::Command);
                } else if text.contains("#[query]") {
                    mode = Some(InteropMode::Query);
                } else if text.contains("#[watcher(raf)]") || text.contains("#[monitor(raf)]") {
                    mode = Some(InteropMode::Watcher { is_raf: true });
                } else if text.contains("#[watcher]") || text.contains("#[monitor]") {
                    mode = Some(InteropMode::Watcher { is_raf: false });
                } else if text.starts_with('*') || (!text.starts_with('#') && !text.is_empty()) {
                    doc_lines.push(text.trim_start_matches('*').trim().to_string());
                }
            }
        }
    }

    let doc = if doc_lines.is_empty() {
        None
    } else {
        Some(doc_lines.join("\n"))
    };

    (mode, doc)
}

fn find_macro_override(name: &str, macro_items: &[ItemSpec]) -> Option<InteropMode> {
    let name_camel = name.to_lower_camel_case();
    let name_snake = name.to_snake_case();
    macro_items
        .iter()
        .find(|item| {
            let item_camel = item.original_name.to_lower_camel_case();
            let item_snake = item.original_name.to_snake_case();
            item_camel == name_camel || item_snake == name_snake
        })
        .and_then(|item| item.mode)
}

fn is_item_targeted(name: &str, macro_items: &[ItemSpec], wildcard: bool) -> bool {
    if wildcard {
        return true;
    }
    let name_camel = name.to_lower_camel_case();
    let name_snake = name.to_snake_case();
    macro_items.iter().any(|item| {
        let item_camel = item.original_name.to_lower_camel_case();
        let item_snake = item.original_name.to_snake_case();
        item_camel == name_camel || item_snake == name_snake
    })
}

fn extract_fn_decl(
    fn_decl: &FnDecl,
    export_span: swc_core::common::Span,
    comments: &SingleThreadedComments,
    macro_items: &[ItemSpec],
    is_target: bool,
    call_span: Span,
) -> syn::Result<ExportedFunction> {
    let name = fn_decl.ident.sym.to_string();
    let (comment_mode, doc_comment) = extract_comment_mode(
        comments,
        &[export_span, fn_decl.function.span, fn_decl.ident.span],
    );
    let macro_mode = find_macro_override(&name, macro_items);

    let mut params = Vec::new();
    for param in &fn_decl.function.params {
        let (p_name, ts_type) = match &param.pat {
            Pat::Ident(binding) => {
                let name = binding.id.sym.to_string();
                let ty = binding.type_ann.as_ref().map(|ann| &*ann.type_ann);
                (name, ty)
            }
            _ => ("_".to_string(), None),
        };

        let param_info = parse_param_type(&p_name, ts_type, call_span)?;
        params.push(param_info);
    }

    let (return_type, is_promise, is_void, is_fn) = parse_return_type(
        fn_decl.function.return_type.as_ref().map(|ann| &*ann.type_ann),
        call_span,
    )?;

    let mode = determine_mode(
        macro_mode,
        comment_mode,
        fn_decl.function.is_async,
        is_promise,
        is_void,
        &params,
    );

    if is_target && is_fn && !matches!(mode, InteropMode::Watcher { .. }) {
        return Err(syn::Error::new(
            call_span,
            "Higher-order functions returning functions are not supported in bind_js!. Return concrete serializable data or use a Watcher callback (annotated with #[watcher]).",
        ));
    }

    Ok(ExportedFunction {
        name,
        mode,
        params,
        return_rust_type: return_type,
        doc_comment,
    })
}

fn extract_var_decl(
    decl: &VarDeclarator,
    export_span: swc_core::common::Span,
    comments: &SingleThreadedComments,
    macro_items: &[ItemSpec],
    is_target: bool,
    call_span: Span,
) -> syn::Result<Option<ExportedFunction>> {
    let name = match &decl.name {
        Pat::Ident(binding) => binding.id.sym.to_string(),
        _ => return Ok(None),
    };

    let (comment_mode, doc_comment) = extract_comment_mode(comments, &[export_span, decl.span]);
    let macro_mode = find_macro_override(&name, macro_items);

    let (params_ast, return_ann, is_async) = match &decl.init {
        Some(init_expr) => match &**init_expr {
            Expr::Arrow(arrow) => {
                let params = arrow
                    .params
                    .iter()
                    .map(|pat| match pat {
                        Pat::Ident(b) => (b.id.sym.to_string(), b.type_ann.as_ref().map(|ann| &*ann.type_ann)),
                        _ => ("_".to_string(), None),
                    })
                    .collect::<Vec<_>>();
                (params, arrow.return_type.as_ref().map(|ann| &*ann.type_ann), arrow.is_async)
            }
            Expr::Fn(fn_expr) => {
                let params = fn_expr
                    .function
                    .params
                    .iter()
                    .map(|p| match &p.pat {
                        Pat::Ident(b) => (b.id.sym.to_string(), b.type_ann.as_ref().map(|ann| &*ann.type_ann)),
                        _ => ("_".to_string(), None),
                    })
                    .collect::<Vec<_>>();
                (params, fn_expr.function.return_type.as_ref().map(|ann| &*ann.type_ann), fn_expr.function.is_async)
            }
            _ => return Ok(None),
        },
        None => return Ok(None),
    };

    let mut params = Vec::new();
    for (p_name, ts_type) in params_ast {
        let param_info = parse_param_type(&p_name, ts_type, call_span)?;
        params.push(param_info);
    }

    let (return_type, is_promise, is_void, is_fn) = parse_return_type(return_ann, call_span)?;

    let mode = determine_mode(
        macro_mode,
        comment_mode,
        is_async,
        is_promise,
        is_void,
        &params,
    );

    if is_target && is_fn && !matches!(mode, InteropMode::Watcher { .. }) {
        return Err(syn::Error::new(
            call_span,
            "Higher-order functions returning functions are not supported in bind_js!. Return concrete serializable data or use a Watcher callback (annotated with #[watcher]).",
        ));
    }

    Ok(Some(ExportedFunction {
        name,
        mode,
        params,
        return_rust_type: return_type,
        doc_comment,
    }))
}

fn determine_mode(
    macro_mode: Option<InteropMode>,
    comment_mode: Option<InteropMode>,
    is_async: bool,
    is_promise: bool,
    is_void: bool,
    _params: &[ParamInfo],
) -> InteropMode {
    // 1. Macro invocation attribute (1st priority)
    if let Some(p) = macro_mode {
        return p;
    }

    // 2. TS doc comment priority (2nd priority)
    if let Some(p) = comment_mode {
        return p;
    }

    // 3. Return type AST inference (3rd priority, GD-02)
    // Watcher strictly requires explicit annotation (#[watcher] or #[watcher(raf)]).
    // Non-annotated functions are partitioned into Command or Query:
    if is_promise || (is_async && !is_void) {
        return InteropMode::Query;
    }

    if is_void {
        return InteropMode::Command;
    }

    InteropMode::Query
}

fn parse_param_type(
    name: &str,
    ts_type: Option<&TsType>,
    _call_span: Span,
) -> syn::Result<ParamInfo> {
    if let Some(ty) = ts_type {
        match ty {
            // Callback: (event: T) => void
            TsType::TsFnOrConstructorType(TsFnOrConstructorType::TsFnType(fn_type)) => {
                let arg_type = if let Some(first_param) = fn_type.params.first() {
                    match first_param {
                        TsFnParam::Ident(b) => b.type_ann.as_ref().map(|ann| map_ts_to_rust_return(&ann.type_ann)),
                        _ => None,
                    }
                } else {
                    None
                };

                return Ok(ParamInfo {
                    name: name.to_string(),
                    rust_param_type: quote! {},
                    is_callback: true,
                    callback_arg_type: arg_type,
                });
            }
            _ => {
                let rust_type = map_ts_to_rust_param(ty);
                return Ok(ParamInfo {
                    name: name.to_string(),
                    rust_param_type: rust_type,
                    is_callback: false,
                    callback_arg_type: None,
                });
            }
        }
    }

    // Default untyped: ::oxidase::serde_json::Value
    Ok(ParamInfo {
        name: name.to_string(),
        rust_param_type: quote! { ::oxidase::serde_json::Value },
        is_callback: false,
        callback_arg_type: None,
    })
}

fn parse_return_type(
    ts_type: Option<&TsType>,
    _call_span: Span,
) -> syn::Result<(Option<proc_macro2::TokenStream>, bool, bool, bool)> {
    let ty = match ts_type {
        Some(t) => t,
        None => return Ok((None, false, true, false)),
    };

    match ty {
        TsType::TsKeywordType(kw) => match kw.kind {
            TsKeywordTypeKind::TsVoidKeyword | TsKeywordTypeKind::TsUndefinedKeyword => {
                Ok((None, false, true, false))
            }
            _ => {
                let r_ty = map_ts_to_rust_return(ty);
                Ok((Some(r_ty), false, false, false))
            }
        },
        TsType::TsTypeRef(type_ref) => {
            let type_name = match &type_ref.type_name {
                TsEntityName::Ident(id) => id.sym.to_string(),
                _ => "".to_string(),
            };

            if type_name == "Promise" {
                if let Some(type_args) = &type_ref.type_params {
                    if let Some(inner) = type_args.params.first() {
                        if let TsType::TsKeywordType(kw) = &**inner {
                            if kw.kind == TsKeywordTypeKind::TsVoidKeyword {
                                return Ok((None, true, true, false));
                            }
                        }
                        let inner_rust = map_ts_to_rust_return(inner);
                        return Ok((Some(inner_rust), true, false, false));
                    }
                }
                return Ok((None, true, true, false));
            }

            let r_ty = map_ts_to_rust_return(ty);
            Ok((Some(r_ty), false, false, false))
        }
        TsType::TsFnOrConstructorType(_) => {
            // Higher-order function returning a function
            Ok((None, false, false, true))
        }
        _ => {
            let r_ty = map_ts_to_rust_return(ty);
            Ok((Some(r_ty), false, false, false))
        }
    }
}

fn map_ts_to_rust_param(ty: &TsType) -> proc_macro2::TokenStream {
    match ty {
        TsType::TsKeywordType(kw) => match kw.kind {
            TsKeywordTypeKind::TsStringKeyword => quote! { &str },
            TsKeywordTypeKind::TsNumberKeyword => quote! { f64 },
            TsKeywordTypeKind::TsBooleanKeyword => quote! { bool },
            TsKeywordTypeKind::TsAnyKeyword | TsKeywordTypeKind::TsUnknownKeyword => {
                quote! { ::oxidase::serde_json::Value }
            }
            _ => quote! { ::oxidase::serde_json::Value },
        },
        TsType::TsArrayType(arr) => {
            let inner = map_ts_to_rust_param(&arr.elem_type);
            quote! { &[#inner] }
        }
        TsType::TsTupleType(tuple) => {
            let elem_types: Vec<_> = tuple.elem_types.iter().map(|e| map_ts_to_rust_param(&e.ty)).collect();
            let first = elem_types.first();
            let all_same = elem_types.iter().all(|t| format!("{}", t) == format!("{}", first.unwrap()));
            if all_same && !elem_types.is_empty() {
                let count = elem_types.len();
                let inner = &elem_types[0];
                quote! { [#inner; #count] }
            } else {
                quote! { (#(#elem_types),*) }
            }
        }
        TsType::TsUnionOrIntersectionType(TsUnionOrIntersectionType::TsUnionType(union)) => {
            // Check if string literal union
            let all_literals = union.types.iter().all(|t| matches!(&**t, TsType::TsLitType(TsLitType { lit: TsLit::Str(_), .. })));
            if all_literals {
                return quote! { &str };
            }
            // Check if Option (T | undefined | null)
            let non_null: Vec<_> = union
                .types
                .iter()
                .filter(|t| !matches!(&***t, TsType::TsKeywordType(kw) if kw.kind == TsKeywordTypeKind::TsUndefinedKeyword || kw.kind == TsKeywordTypeKind::TsNullKeyword))
                .collect();
            if non_null.len() == 1 {
                let inner = map_ts_to_rust_param(non_null[0]);
                return quote! { Option<#inner> };
            }
            quote! { ::oxidase::serde_json::Value }
        }
        TsType::TsTypeRef(type_ref) => {
            if let TsEntityName::Ident(id) = &type_ref.type_name {
                let sym = id.sym.as_ref();
                if sym == "Array" {
                    if let Some(params) = &type_ref.type_params {
                        if let Some(first) = params.params.first() {
                            let inner = map_ts_to_rust_param(first);
                            return quote! { &[#inner] };
                        }
                    }
                }
                let ident = syn::Ident::new(&id.sym.to_string(), proc_macro2::Span::call_site());
                quote! { &#ident }
            } else {
                quote! { ::oxidase::serde_json::Value }
            }
        }
        _ => quote! { ::oxidase::serde_json::Value },
    }
}

fn map_ts_to_rust_return(ty: &TsType) -> proc_macro2::TokenStream {
    match ty {
        TsType::TsKeywordType(kw) => match kw.kind {
            TsKeywordTypeKind::TsStringKeyword => quote! { String },
            TsKeywordTypeKind::TsNumberKeyword => quote! { f64 },
            TsKeywordTypeKind::TsBooleanKeyword => quote! { bool },
            TsKeywordTypeKind::TsVoidKeyword => quote! { () },
            _ => quote! { ::oxidase::serde_json::Value },
        },
        TsType::TsArrayType(arr) => {
            let inner = map_ts_to_rust_return(&arr.elem_type);
            quote! { Vec<#inner> }
        }
        TsType::TsTupleType(tuple) => {
            let elem_types: Vec<_> = tuple.elem_types.iter().map(|e| map_ts_to_rust_return(&e.ty)).collect();
            let first = elem_types.first();
            let all_same = elem_types.iter().all(|t| format!("{}", t) == format!("{}", first.unwrap()));
            if all_same && !elem_types.is_empty() {
                let count = elem_types.len();
                let inner = &elem_types[0];
                quote! { [#inner; #count] }
            } else {
                quote! { (#(#elem_types),*) }
            }
        }
        TsType::TsUnionOrIntersectionType(TsUnionOrIntersectionType::TsUnionType(union)) => {
            // Check if string literal union -> String default
            let all_literals = union.types.iter().all(|t| matches!(&**t, TsType::TsLitType(TsLitType { lit: TsLit::Str(_), .. })));
            if all_literals {
                return quote! { String };
            }
            let non_null: Vec<_> = union
                .types
                .iter()
                .filter(|t| !matches!(&***t, TsType::TsKeywordType(kw) if kw.kind == TsKeywordTypeKind::TsUndefinedKeyword || kw.kind == TsKeywordTypeKind::TsNullKeyword))
                .collect();
            if non_null.len() == 1 {
                let inner = map_ts_to_rust_return(non_null[0]);
                return quote! { Option<#inner> };
            }
            quote! { ::oxidase::serde_json::Value }
        }
        TsType::TsTypeRef(type_ref) => {
            if let TsEntityName::Ident(id) = &type_ref.type_name {
                let sym = id.sym.as_ref();
                if sym == "Array" {
                    if let Some(params) = &type_ref.type_params {
                        if let Some(first) = params.params.first() {
                            let inner = map_ts_to_rust_return(first);
                            return quote! { Vec<#inner> };
                        }
                    }
                }
                let ident = syn::Ident::new(&id.sym.to_string(), proc_macro2::Span::call_site());
                quote! { #ident }
            } else {
                quote! { ::oxidase::serde_json::Value }
            }
        }
        _ => quote! { ::oxidase::serde_json::Value },
    }
}

pub fn strip_ts_types(cm: Lrc<SourceMap>, mut module: Module) -> String {
    GLOBALS.set(&Globals::default(), || {
        let unresolved_mark = Mark::new();
        let top_level_mark = Mark::new();

        // Convert export decls to standard statements so the inlined code is valid inside a function IIFE
        for item in &mut module.body {
            if let ModuleItem::ModuleDecl(ModuleDecl::ExportDecl(export_decl)) = item {
                *item = ModuleItem::Stmt(Stmt::Decl(export_decl.decl.clone()));
            }
        }

        let mut program = Program::Module(module);
        let mut pass = strip(unresolved_mark, top_level_mark);
        pass.process(&mut program);

        let mut buf = Vec::new();
        {
            let mut emitter = Emitter {
                cfg: Config::default(),
                cm: cm.clone(),
                comments: None,
                wr: Box::new(JsWriter::new(cm.clone(), "\n", &mut buf, None)),
            };
            emitter.emit_program(&program).expect("JS emission failed");
        }
        String::from_utf8(buf).expect("Invalid UTF-8 JS output")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reject_top_level_import() {
        let ts = r#"
            import { something } from 'other';
            export function test(): void {}
        "#;
        let res = analyze_source(ts, "test.ts", &[], true, Span::call_site());
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Top-level static import is not supported"));
    }

    #[test]
    fn test_reject_default_export() {
        let ts = r#"
            export default function test(): void {}
        "#;
        let res = analyze_source(ts, "test.ts", &[], true, Span::call_site());
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Default exports are not supported"));
    }

    #[test]
    fn test_reject_higher_order_function() {
        let ts = r#"
            export function curried(x: number): (y: number) => number {
                return (y) => x + y;
            }
        "#;
        let res = analyze_source(ts, "test.ts", &[], true, Span::call_site());
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("Higher-order functions"));
    }

    #[test]
    fn test_selective_import_ignores_untargeted_higher_order_function() {
        let ts = r#"
            export function normalAction(): void {}
            export function curried(x: number): (y: number) => number {
                return (y) => x + y;
            }
        "#;
        let macro_items = vec![ItemSpec {
            mode: None,
            original_name: "normalAction".to_string(),
            rename_as: None,
        }];
        // wildcard = false: curried is not in macro_items, so it should NOT error!
        let res = analyze_source(ts, "test.ts", &macro_items, false, Span::call_site());
        assert!(res.is_ok(), "Untargeted higher-order function must be ignored when wildcard is false");
    }

    #[test]
    fn test_macro_watcher_override_permits_higher_order_function() {
        let ts = r#"
            export function watchCustom(x: number): () => void {
                return () => {};
            }
        "#;
        let macro_items = vec![ItemSpec {
            mode: Some(InteropMode::Watcher { is_raf: false }),
            original_name: "watchCustom".to_string(),
            rename_as: None,
        }];
        let res = analyze_source(ts, "test.ts", &macro_items, true, Span::call_site());
        assert!(res.is_ok(), "Macro #[watcher] must permit higher-order cleanup return");
        let analyzed = res.unwrap();
        assert_eq!(analyzed.exports[0].mode, InteropMode::Watcher { is_raf: false });
    }

    #[test]
    fn test_analyze_command_query_watcher() {
        let ts = r#"
            // #[command]
            export function focusElement(id: string): void {
                document.getElementById(id)?.focus();
            }

            export async function getBoundingRect(id: string): Promise<Rect> {
                return { x: 0, y: 0 };
            }

            // #[watcher]
            export function watchScroll(emit: (scrollY: number) => void): () => void {
                const handler = () => emit(window.scrollY);
                window.addEventListener('scroll', handler);
                return () => window.removeEventListener('scroll', handler);
            }

            /**
             * #[watcher(raf)]
             */
            export function watchResize(emit: (w: number) => void): () => void {
                return () => {};
            }
        "#;

        let res = analyze_source(ts, "test.ts", &[], true, Span::call_site()).unwrap();
        assert_eq!(res.exports.len(), 4);

        // Command
        assert_eq!(res.exports[0].name, "focusElement");
        assert_eq!(res.exports[0].mode, InteropMode::Command);
        assert_eq!(res.exports[0].params.len(), 1);
        assert_eq!(res.exports[0].params[0].name, "id");

        // Query
        assert_eq!(res.exports[1].name, "getBoundingRect");
        assert_eq!(res.exports[1].mode, InteropMode::Query);
        assert!(res.exports[1].return_rust_type.is_some());

        // Watcher (Immediate)
        assert_eq!(res.exports[2].name, "watchScroll");
        assert_eq!(res.exports[2].mode, InteropMode::Watcher { is_raf: false });
        assert!(res.exports[2].params[0].is_callback);

        // Watcher (rAF coalesced)
        assert_eq!(res.exports[3].name, "watchResize");
        assert_eq!(res.exports[3].mode, InteropMode::Watcher { is_raf: true });
        assert!(res.exports[3].params[0].is_callback);

        // Inlined JS contains pure JS without types
        println!("INLINED JS:\n{}", res.inlined_js);
    }

    #[test]
    fn test_array_generic_and_bracket_syntax_parity() {
        let ts = r#"
            export function processBrackets(items: string[]): number[] {
                return [items.length];
            }

            export function processGenerics(items: Array<string>): Array<number> {
                return [items.length];
            }

            export async function fetchArrayAsync(): Promise<Array<string>> {
                return ["a", "b"];
            }
        "#;

        let res = analyze_source(ts, "test.ts", &[], true, Span::call_site()).unwrap();
        assert_eq!(res.exports.len(), 3);

        // Bracket syntax (T[])
        assert_eq!(res.exports[0].name, "processBrackets");
        assert_eq!(res.exports[0].params[0].rust_param_type.to_string(), "& [& str]");
        assert_eq!(res.exports[0].return_rust_type.as_ref().unwrap().to_string(), "Vec < f64 >");

        // Generic syntax (Array<T>)
        assert_eq!(res.exports[1].name, "processGenerics");
        assert_eq!(res.exports[1].params[0].rust_param_type.to_string(), "& [& str]");
        assert_eq!(res.exports[1].return_rust_type.as_ref().unwrap().to_string(), "Vec < f64 >");

        // Async Query returning Promise<Array<T>>
        assert_eq!(res.exports[2].name, "fetchArrayAsync");
        assert_eq!(res.exports[2].return_rust_type.as_ref().unwrap().to_string(), "Vec < String >");
    }
}

