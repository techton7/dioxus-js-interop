use heck::{ToLowerCamelCase, ToSnakeCase};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::Ident;

use crate::analyzer::{AnalyzedModule, ExportedFunction};
use crate::syntax::{BindJsInput, InteropMode};

pub fn generate_bindings(
    input: &BindJsInput,
    analyzed: &AnalyzedModule,
    resolved_path: &std::path::Path,
    call_span: Span,
) -> syn::Result<TokenStream> {
    let module_hash = &analyzed.module_hash;
    let inlined_js = &analyzed.inlined_js;
    let resolved_path_str = resolved_path.to_string_lossy();

    // Collect all exported JS function names for the IIFE return object
    let export_keys_str = analyzed
        .exports
        .iter()
        .map(|e| e.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");

    let mut generated_items = Vec::new();
    let mut matched_specs = vec![false; input.items.len()];

    let epoch_ident = quote::format_ident!("__DIOXUS_LOADED_EPOCH_{}", module_hash);
    let ensure_fn_ident = quote::format_ident!("__dioxus_ensure_module_{}", module_hash);

    for export in &analyzed.exports {
        let name_camel = export.name.to_lower_camel_case();
        let name_snake = export.name.to_snake_case();

        // Check if there is a matching ItemSpec in input.items
        let matched_item = input.items.iter().enumerate().find(|(_, item)| {
            let item_camel = item.original_name.to_lower_camel_case();
            let item_snake = item.original_name.to_snake_case();
            item_camel == name_camel || item_snake == name_snake
        });

        let (mode, rust_fn_ident) = if let Some((idx, item)) = matched_item {
            matched_specs[idx] = true;
            let mode = item.mode.unwrap_or(export.mode);

            let rust_name = item.rename_as.as_ref().map(|id| {
                Ident::new(&id.to_string().to_snake_case(), id.span())
            }).unwrap_or_else(|| {
                Ident::new(&name_snake, call_span)
            });

            (mode, rust_name)
        } else if input.wildcard {
            (export.mode, Ident::new(&name_snake, call_span))
        } else {
            // Unselected export when wildcard is false
            continue;
        };

        match mode {
            InteropMode::Command => {
                let tokens = generate_command(export, module_hash, &rust_fn_ident, &ensure_fn_ident);
                generated_items.push(tokens);
            }
            InteropMode::Query => {
                let tokens = generate_query(export, module_hash, &rust_fn_ident, &ensure_fn_ident, &epoch_ident);
                generated_items.push(tokens);
            }
            InteropMode::Watcher { is_raf } => {
                let tokens = generate_watcher(export, module_hash, &rust_fn_ident, &ensure_fn_ident, &epoch_ident, is_raf);
                generated_items.push(tokens);
            }
        }
    }

    // Verify all requested items in input.items were matched
    for (idx, item) in input.items.iter().enumerate() {
        if !matched_specs[idx] {
            return Err(syn::Error::new(
                call_span,
                format!(
                    "Function '{}' was not found in exported functions of '{}'",
                    item.original_name,
                    input.file_path.value()
                ),
            ));
        }
    }

    let ensure_js = include_str!("../templates/ensure_module.js")
        .replace("__MODULE_HASH__", module_hash)
        .replace("__INLINED_JS__", inlined_js)
        .replace("__EXPORT_KEYS__", &export_keys_str);

    let ensure_module_fn = quote! {
        const _: &[u8] = include_bytes!(#resolved_path_str);
        static #epoch_ident: ::std::sync::atomic::AtomicU64 = ::std::sync::atomic::AtomicU64::new(0);

        #[inline(always)]
        fn #ensure_fn_ident() {
            let current_epoch = ::dioxus_js_interop::internal::current_epoch();
            if #epoch_ident.load(::std::sync::atomic::Ordering::Acquire) != current_epoch {
                let _ = ::dioxus::document::eval(#ensure_js);
                #epoch_ident.store(current_epoch, ::std::sync::atomic::Ordering::Release);
            }
        }
    };

    Ok(quote! {
        #ensure_module_fn
        #(#generated_items)*
    })
}

fn generate_command(
    export: &ExportedFunction,
    module_hash: &str,
    rust_fn_ident: &Ident,
    ensure_fn_ident: &Ident,
) -> TokenStream {
    let js_name = &export.name;
    let doc = export.doc_comment.as_deref().unwrap_or("");

    let param_names = export.params.iter().map(|p| {
        Ident::new(&p.name.to_snake_case(), Span::call_site())
    }).collect::<Vec<_>>();

    let param_types = export.params.iter().map(|p| &p.rust_param_type).collect::<Vec<_>>();

    let payload_tokens = if param_names.is_empty() {
        quote! { "[]".to_string() }
    } else {
        quote! { ::dioxus_js_interop::serde_json::to_string(&(#(#param_names,)*)).expect("Serialization failed in command") }
    };

    let js = include_str!("../templates/command.js")
        .replace("__MODULE_HASH__", module_hash)
        .replace("__JS_NAME__", js_name);
    let (part1, part2) = js.split_once("__PAYLOAD__").expect("command template missing __PAYLOAD__");

    quote! {
        #[doc = #doc]
        pub fn #rust_fn_ident(#(#param_names: #param_types),*) {
            #ensure_fn_ident();

            let payload = #payload_tokens;
            let script = format!("{}{}{}", #part1, payload, #part2);
            let _ = ::dioxus::document::eval(&script);
        }
    }
}

fn generate_query(
    export: &ExportedFunction,
    module_hash: &str,
    rust_fn_ident: &Ident,
    ensure_fn_ident: &Ident,
    epoch_ident: &Ident,
) -> TokenStream {
    let js_name = &export.name;
    let doc = export.doc_comment.as_deref().unwrap_or("");

    let param_names = export.params.iter().map(|p| {
        Ident::new(&p.name.to_snake_case(), Span::call_site())
    }).collect::<Vec<_>>();

    let param_types = export.params.iter().map(|p| &p.rust_param_type).collect::<Vec<_>>();

    let ret_type = export.return_rust_type.as_ref().cloned().unwrap_or_else(|| quote! { () });

    let payload_tokens = if param_names.is_empty() {
        quote! { "[]".to_string() }
    } else {
        quote! { ::dioxus_js_interop::serde_json::to_string(&(#(#param_names,)*)).expect("Serialization failed in query") }
    };

    let query_js = include_str!("../templates/query.js")
        .replace("__MODULE_HASH__", module_hash)
        .replace("__JS_NAME__", js_name);
    let (query_part1, query_part2) = query_js.split_once("__PAYLOAD__").expect("query template missing __PAYLOAD__");

    let retry_js = include_str!("../templates/query_retry.js")
        .replace("__MODULE_HASH__", module_hash)
        .replace("__JS_NAME__", js_name);
    let (retry_part1, retry_part2) = retry_js.split_once("__PAYLOAD__").expect("retry template missing __PAYLOAD__");

    quote! {
        #[doc = #doc]
        pub async fn #rust_fn_ident(#(#param_names: #param_types),*) -> Result<#ret_type, ::dioxus_js_interop::JsError> {
            #ensure_fn_ident();

            let payload = #payload_tokens;
            let script = format!("{}{}{}", #query_part1, payload, #query_part2);
            let mut eval = ::dioxus::document::eval(&script);

            let raw_val: ::dioxus_js_interop::serde_json::Value = eval.recv().await
                .map_err(|e| ::dioxus_js_interop::JsError::Transport(e.to_string()))?;

            let resp: ::dioxus_js_interop::RpcResponse<::dioxus_js_interop::serde_json::Value> = ::dioxus_js_interop::serde_json::from_value(raw_val)
                .map_err(|e| ::dioxus_js_interop::JsError::Deserialization(format!("Failed to deserialize RPC response frame: {}", e)))?;

            let decode_resp = |resp: ::dioxus_js_interop::RpcResponse<::dioxus_js_interop::serde_json::Value>| -> Result<#ret_type, ::dioxus_js_interop::JsError> {
                if resp.ok {
                    let data_val = resp.data.unwrap_or(::dioxus_js_interop::serde_json::Value::Null);
                    ::dioxus_js_interop::serde_json::from_value::<#ret_type>(data_val)
                        .map_err(|e| ::dioxus_js_interop::JsError::Deserialization(format!("Failed to deserialize return data into {}: {}", stringify!(#ret_type), e)))
                } else {
                    Err(::dioxus_js_interop::JsError::Exception {
                        message: resp.error.unwrap_or_else(|| "Unknown JS error".into()),
                        stack: resp.stack,
                    })
                }
            };

            if let Some(err) = resp.as_error() {
                if err == "MODULE_NOT_FOUND" {
                    #epoch_ident.store(0, ::std::sync::atomic::Ordering::Release);
                    #ensure_fn_ident();

                    let retry_script = format!("{}{}{}", #retry_part1, payload, #retry_part2);
                    let mut retry_eval = ::dioxus::document::eval(&retry_script);

                    let retry_val: ::dioxus_js_interop::serde_json::Value = retry_eval.recv().await
                        .map_err(|e| ::dioxus_js_interop::JsError::Transport(e.to_string()))?;

                    let retry_resp: ::dioxus_js_interop::RpcResponse<::dioxus_js_interop::serde_json::Value> = ::dioxus_js_interop::serde_json::from_value(retry_val)
                        .map_err(|e| ::dioxus_js_interop::JsError::Deserialization(format!("Failed to deserialize retry RPC response frame: {}", e)))?;

                    if let Some(retry_err) = retry_resp.as_error() {
                        if retry_err == "MODULE_UNAVAILABLE" || retry_err == "MODULE_NOT_FOUND" {
                            return Err(::dioxus_js_interop::JsError::ModuleUnavailable(#module_hash.to_string()));
                        }
                    }

                    return decode_resp(retry_resp);
                }
            }

            decode_resp(resp)
        }
    }
}

fn generate_watcher(
    export: &ExportedFunction,
    module_hash: &str,
    rust_name_ident: &Ident,
    ensure_fn_ident: &Ident,
    epoch_ident: &Ident,
    is_raf: bool,
) -> TokenStream {
    let js_name = &export.name;
    let doc = export.doc_comment.as_deref().unwrap_or("");

    let non_callback_params = export.params.iter().filter(|p| !p.is_callback).collect::<Vec<_>>();
    let callback_param = export.params.iter().find(|p| p.is_callback);

    let param_names = non_callback_params.iter().map(|p| {
        Ident::new(&p.name.to_snake_case(), Span::call_site())
    }).collect::<Vec<_>>();

    let param_types = non_callback_params.iter().map(|p| &p.rust_param_type).collect::<Vec<_>>();

    let callback_arg_type = callback_param
        .and_then(|p| p.callback_arg_type.as_ref())
        .cloned()
        .unwrap_or_else(|| quote! { () });

    let payload_tokens = if param_names.is_empty() {
        quote! { "[]".to_string() }
    } else {
        quote! { ::dioxus_js_interop::serde_json::to_string(&(#(#param_names,)*)).expect("Serialization failed in watcher") }
    };

    let template = if is_raf {
        include_str!("../templates/watcher_raf.js")
    } else {
        include_str!("../templates/watcher_immediate.js")
    };

    let js = template
        .replace("__MODULE_HASH__", module_hash)
        .replace("__JS_NAME__", js_name);

    let (part1, rest) = js.split_once("__PAYLOAD__").expect("watcher template missing __PAYLOAD__");
    let (part2, part3) = rest.split_once("__SUB_ID__").expect("watcher template missing __SUB_ID__");

    quote! {
        #[doc = #doc]
        pub fn #rust_name_ident(
            #(#param_names: #param_types,)*
            mut on_event: impl FnMut(#callback_arg_type) + 'static,
        ) -> ::dioxus_js_interop::WatcherGuard {
            #ensure_fn_ident();

            let sub_id = ::dioxus_js_interop::internal::next_subscription_id();
            let payload = #payload_tokens;
            let script = format!("{}{}{}{}{}", #part1, payload, #part2, sub_id, #part3);

            let mut eval = ::dioxus::document::eval(&script);

            let task = ::dioxus::prelude::spawn(async move {
                while let Ok(event) = eval.recv::<::dioxus_js_interop::serde_json::Value>().await {
                    if event.get("__bindgen_err").and_then(|v| v.as_str()) == Some("MODULE_NOT_FOUND") {
                        #epoch_ident.store(0, ::std::sync::atomic::Ordering::Release);
                        break;
                    }
                    match ::dioxus_js_interop::serde_json::from_value::<#callback_arg_type>(event) {
                        Ok(data) => {
                            on_event(data);
                        }
                        Err(err) => {
                            ::dioxus_js_interop::tracing::error!(
                                target: "dioxus_js_interop",
                                "Failed to deserialize watcher event for '{}': {}",
                                #js_name,
                                err
                            );
                        }
                    }
                }
            });

            ::dioxus_js_interop::WatcherGuard::new(stringify!(#rust_name_ident), sub_id, Some(task))
        }
    }
}
