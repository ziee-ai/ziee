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
    stream_event_variants: Vec<String>,
    content_variants: Vec<String>,
}

fn main() {
    // Get the macros crate directory
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let macros_dir = PathBuf::from(&manifest_dir);

    // Get the parent directory (server/)
    let server_dir = macros_dir.parent().unwrap().to_path_buf();

    let modules_dir = server_dir.join("src/modules");
    let chat_dir = modules_dir.join("chat");

    let mut extensions = Vec::new();

    // Walk two locations for `extension.rs` files:
    //   1. modules/chat/**/extension.rs — chat-internal extensions
    //      (assistant, memory, text, title, …) where the qualified
    //      path prefix is `crate::modules::chat::<module_path>::extension::`.
    //   2. modules/*/chat_extension/extension.rs — sibling-module
    //      bridges (file, project, mcp, …) where the qualified path
    //      prefix is `crate::modules::<sibling>::chat_extension::extension::`.
    //
    // Each entry yields a `qualified_path_prefix` used downstream when
    // we need to rewrite tokens that reference per-extension types
    // (only the SSEChatStreamEventVariants case uses this today).
    let mut extension_files: Vec<(PathBuf, String)> = Vec::new();

    if chat_dir.exists() {
        for entry in WalkDir::new(&chat_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.file_name() == Some(std::ffi::OsStr::new("extension.rs")) {
                let module_path = extract_module_path(&chat_dir, path);
                let qualified_prefix =
                    format!("crate::modules::chat::{}::extension", module_path);
                extension_files.push((path.to_path_buf(), qualified_prefix));
            }
        }
    }

    if modules_dir.exists() {
        for entry in fs::read_dir(&modules_dir).into_iter().flatten().flatten() {
            let sibling = entry.path();
            if !sibling.is_dir() {
                continue;
            }
            let ext_file = sibling.join("chat_extension").join("extension.rs");
            if !ext_file.exists() {
                continue;
            }
            let sibling_name = sibling
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            let qualified_prefix =
                format!("crate::modules::{}::chat_extension::extension", sibling_name);
            extension_files.push((ext_file, qualified_prefix));
        }
    }

    for (path, qualified_prefix) in extension_files {
        // module_path is only used downstream for the legacy
        // chat-internal qualified-path builder — see SSEChatStreamEventVariants
        // handling below. For sibling-module bridges the qualified_prefix
        // (computed above) takes precedence.
        let module_path = if path.starts_with(&chat_dir) {
            extract_module_path(&chat_dir, &path)
        } else {
            String::new()
        };

        // Read and parse the extension file
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(parsed) = syn::parse_file(&content) {
                        let mut name = module_path.split("::").last().unwrap_or("unknown").to_string();
                        let mut order = 50; // Default order
                        let mut request_fields = Vec::new();
                        let mut response_fields = Vec::new();
                        let mut delta_variants = Vec::new();
                        let mut stream_event_variants = Vec::new();
                        let mut content_variants = Vec::new();

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
                                // Extract MessageContentDataVariants enum
                                syn::Item::Enum(item_enum) if item_enum.ident == "MessageContentDataVariants" => {
                                    for variant in &item_enum.variants {
                                        let variant_tokens = quote! { #variant };
                                        content_variants.push(variant_tokens.to_string());
                                    }
                                }
                                // Extract SSEChatStreamEventVariants enum
                                syn::Item::Enum(item_enum) if item_enum.ident == "SSEChatStreamEventVariants" => {
                                    for variant in &item_enum.variants {
                                        // Build fully-qualified variant with module path
                                        // e.g., TitleUpdated(SSEChatStreamTitleUpdatedData) -> TitleUpdated(crate::modules::chat::extensions::title::extension::SSEChatStreamTitleUpdatedData)
                                        let variant_name = &variant.ident;

                                        if let syn::Fields::Unnamed(ref fields) = variant.fields {
                                            for field in &fields.unnamed {
                                                if let syn::Type::Path(ref type_path) = field.ty {
                                                    // Get the type name
                                                    let type_name = type_path.path.segments.last().unwrap().ident.to_string();

                                                    // Build fully-qualified path. Uses the
                                                    // qualified_prefix computed during file
                                                    // discovery so chat-internal extensions
                                                    // (`crate::modules::chat::<x>::extension`)
                                                    // and sibling-module bridges
                                                    // (`crate::modules::<x>::chat_extension::extension`)
                                                    // both resolve correctly.
                                                    let full_path = format!(
                                                        "{}::{}",
                                                        qualified_prefix, type_name
                                                    );

                                                    // Store as string with docs if present
                                                    let docs = variant.attrs.iter()
                                                        .filter_map(|attr| {
                                                            if attr.path().is_ident("doc") {
                                                                Some(quote! { #attr }.to_string())
                                                            } else {
                                                                None
                                                            }
                                                        })
                                                        .collect::<Vec<_>>()
                                                        .join("\n    ");

                                                    let variant_str = if docs.is_empty() {
                                                        format!("{}({})", variant_name, full_path)
                                                    } else {
                                                        format!("{}\n    {}({})", docs, variant_name, full_path)
                                                    };

                                                    stream_event_variants.push(variant_str);
                                                }
                                            }
                                        }
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
                            stream_event_variants,
                            content_variants,
                        });
                    } // close `if let Ok(parsed)`
                } // close `if let Ok(content)`
    } // close `for (path, qualified_prefix) in extension_files`

    // Sort extensions by order (for consistent field composition)
    extensions.sort_by_key(|e| e.order);

    // Generate the extensions file (for field composition macros)
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = PathBuf::from(&out_dir).join("chat_extensions.rs");
    generate_extensions_file(&extensions, &dest_path);

    // Note: Extension registration is now handled by linkme distributed slices
    // The auto_register_extensions function in extension_registration.rs
    // iterates the CHAT_EXTENSIONS slice instead of using generated code

    // Tell cargo to rerun if any .rs file under modules/chat (in-chat
    // extensions) OR modules/* (sibling-module bridges) changes.
    // Watching the broader modules dir is fine — cargo's change
    // detection is content-hashed, not just timestamp-based.
    println!("cargo:rerun-if-changed={}", chat_dir.display());
    println!("cargo:rerun-if-changed={}", modules_dir.display());
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

    // Generate enum variants for SSEChatStreamEvent
    let all_stream_event_variants: Vec<String> = extensions
        .iter()
        .flat_map(|ext| ext.stream_event_variants.iter())
        .cloned()
        .collect();

    let stream_event_variants_code = all_stream_event_variants
        .iter()
        .map(|variant| format!("    {},", variant))
        .collect::<Vec<_>>()
        .join("\n");

    // Generate enum variants for MessageContentData
    let all_content_variants: Vec<String> = extensions
        .iter()
        .flat_map(|ext| ext.content_variants.iter())
        .cloned()
        .collect();

    let content_variants_code = all_content_variants
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

// SSEChatStreamEvent enum variants
pub const STREAM_EVENT_VARIANTS: &[&str] = &[
{}
];

// MessageContentData enum variants
pub const CONTENT_VARIANTS: &[&str] = &[
{}
];
"#,
        request_fields_code,
        response_fields_code,
        delta_variants_code,
        stream_event_variants_code,
        content_variants_code
    );

    fs::write(dest_path, generated).unwrap();
}

// Note: generate_auto_registration_module removed - registration now uses linkme
