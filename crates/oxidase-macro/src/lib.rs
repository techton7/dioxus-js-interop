mod syntax;
mod analyzer;
mod codegen;

use proc_macro::TokenStream;
use syn::parse_macro_input;

/// `bind_js!` procedural macro for generating zero-build, low-annotation Rust FFI wrappers
/// from local static TS/JS bridge files.
#[proc_macro]
pub fn bind_js(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syntax::BindJsInput);
    let call_span = proc_macro2::Span::call_site();

    let manifest_dir = match std::env::var("CARGO_MANIFEST_DIR") {
        Ok(dir) => dir,
        Err(e) => {
            return syn::Error::new(
                input.file_path.span(),
                format!("Failed to read CARGO_MANIFEST_DIR: {}", e),
            )
            .to_compile_error()
            .into();
        }
    };

    let file_path_str = input.file_path.value();
    let file_path = std::path::Path::new(&file_path_str);
    let resolved_path = if file_path.is_absolute() {
        file_path.to_path_buf()
    } else {
        std::path::Path::new(&manifest_dir).join(file_path)
    };

    let analyzed = match analyzer::analyze_file(&resolved_path, &input.items, input.wildcard, call_span) {
        Ok(a) => a,
        Err(e) => return e.to_compile_error().into(),
    };

    match codegen::generate_bindings(&input, &analyzed, &resolved_path, call_span) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.to_compile_error().into(),
    }
}
