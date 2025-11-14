use std::env;
use std::fs;
use std::path::PathBuf;
use walkdir::WalkDir;
use quote::quote;

#[derive(Debug)]
#[allow(dead_code)]
struct ExtensionInfo {
    module_path: String,
    name: String,
    order: i32,
    request_fields: Vec<String>,
    response_fields: Vec<String>,
    delta_variants: Vec<String>,
}

fn main() {
    // Get the macros crate directory
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let macros_dir = PathBuf::from(&manifest_dir);

    // Get the parent directory (server/)
    let server_dir = macros_dir.parent().unwrap().to_path_buf();

    // Scan for extension.rs files in modules/chat/**/
    let chat_dir = server_dir.join("src/modules/chat");

    let mut extensions = Vec::new();

    if chat_dir.exists() {
        for entry in WalkDir::new(&chat_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();

            // Look for extension.rs files
            if path.file_name() == Some(std::ffi::OsStr::new("extension.rs")) {

                // Extract module path from file path
                let module_path = extract_module_path(&chat_dir, path);

                // Read and parse the extension file
                if let Ok(content) = fs::read_to_string(path) {
                    if let Ok(parsed) = syn::parse_file(&content) {
                        let mut name = module_path.split("::").last().unwrap_or("unknown").to_string();
                        let mut order = 50; // Default order
                        let mut request_fields = Vec::new();
                        let mut response_fields = Vec::new();
                        let mut delta_variants = Vec::new();

                        // Extract metadata and fields
                        for item in parsed.items {
                            match &item {
                                // Extract METADATA constant
                                syn::Item::Const(const_item) if const_item.ident == "METADATA" => {
                                    // Try to extract order from the const expression
                                    if let syn::Expr::Struct(expr_struct) = &*const_item.expr {
                                        for field in &expr_struct.fields {
                                            if let syn::Member::Named(ident) = &field.member {
                                                if ident == "name" {
                                                    if let syn::Expr::Lit(syn::ExprLit {
                                                        lit: syn::Lit::Str(lit_str), ..
                                                    }) = &field.expr {
                                                        name = lit_str.value();
                                                    }
                                                } else if ident == "order" {
                                                    if let syn::Expr::Lit(syn::ExprLit {
                                                        lit: syn::Lit::Int(lit_int), ..
                                                    }) = &field.expr {
                                                        order = lit_int.base10_parse().unwrap_or(50);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                // Extract SendMessageRequestFields struct
                                syn::Item::Struct(item_struct) if item_struct.ident == "SendMessageRequestFields" => {
                                    if let syn::Fields::Named(fields_named) = &item_struct.fields {
                                        for field in &fields_named.named {
                                            let field_tokens = quote! { #field };
                                            request_fields.push(field_tokens.to_string());
                                        }
                                    }
                                }
                                // Extract ChatStreamChunkFields struct
                                syn::Item::Struct(item_struct) if item_struct.ident == "ChatStreamChunkFields" => {
                                    if let syn::Fields::Named(fields_named) = &item_struct.fields {
                                        for field in &fields_named.named {
                                            let field_tokens = quote! { #field };
                                            response_fields.push(field_tokens.to_string());
                                        }
                                    }
                                }
                                // Extract ContentBlockDeltaVariants enum
                                syn::Item::Enum(item_enum) if item_enum.ident == "ContentBlockDeltaVariants" => {
                                    for variant in &item_enum.variants {
                                        let variant_tokens = quote! { #variant };
                                        delta_variants.push(variant_tokens.to_string());
                                    }
                                }
                                _ => {}
                            }
                        }

                        extensions.push(ExtensionInfo {
                            module_path,
                            name,
                            order,
                            request_fields,
                            response_fields,
                            delta_variants,
                        });
                    }
                }
            }
        }
    }

    // Sort extensions by order (for consistent field composition)
    extensions.sort_by_key(|e| e.order);

    // Generate the extensions file (for field composition macros)
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = PathBuf::from(&out_dir).join("chat_extensions.rs");
    generate_extensions_file(&extensions, &dest_path);

    // Note: Extension registration is now handled by linkme distributed slices
    // The auto_register_extensions function in extension_registration.rs
    // iterates the CHAT_EXTENSIONS slice instead of using generated code

    // Tell cargo to rerun if any .rs file in modules/chat changes
    println!("cargo:rerun-if-changed={}", chat_dir.display());
}

fn extract_module_path(base: &PathBuf, path: &std::path::Path) -> String {
    path.strip_prefix(base)
        .ok()
        .and_then(|p| p.parent())
        .map(|p| {
            p.components()
                .filter_map(|c| c.as_os_str().to_str())
                .collect::<Vec<_>>()
                .join("::")
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn generate_extensions_file(extensions: &[ExtensionInfo], dest_path: &PathBuf) {
    // Generate request field definitions for the proc macro
    let all_request_fields: Vec<String> = extensions
        .iter()
        .flat_map(|ext| ext.request_fields.iter())
        .cloned()
        .collect();

    let request_fields_code = all_request_fields
        .iter()
        .map(|field| format!("    {},", field))
        .collect::<Vec<_>>()
        .join("\n");

    // Generate response field definitions for the proc macro
    let all_response_fields: Vec<String> = extensions
        .iter()
        .flat_map(|ext| ext.response_fields.iter())
        .cloned()
        .collect();

    let response_fields_code = all_response_fields
        .iter()
        .map(|field| format!("    {},", field))
        .collect::<Vec<_>>()
        .join("\n");

    // Generate enum variants for ContentBlockDelta
    let all_delta_variants: Vec<String> = extensions
        .iter()
        .flat_map(|ext| ext.delta_variants.iter())
        .cloned()
        .collect();

    let delta_variants_code = all_delta_variants
        .iter()
        .map(|variant| format!("    {},", variant))
        .collect::<Vec<_>>()
        .join("\n");

    let generated = format!(
        r#"// Auto-generated list of chat extension fields
// DO NOT EDIT - Generated by build.rs

// Request fields for SendMessageRequest
pub const REQUEST_FIELDS: &[&str] = &[
{}
];

// Response fields for ChatStreamChunk
pub const RESPONSE_FIELDS: &[&str] = &[
{}
];

// ContentBlockDelta enum variants
pub const DELTA_VARIANTS: &[&str] = &[
{}
];
"#,
        request_fields_code,
        response_fields_code,
        delta_variants_code
    );

    fs::write(dest_path, generated).unwrap();
}

// Note: generate_auto_registration_module removed - registration now uses linkme
