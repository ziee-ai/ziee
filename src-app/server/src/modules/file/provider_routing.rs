//! File → provider-specific ContentBlock conversion.
//!
//! Single source of truth for the per-provider (OpenAI / Anthropic /
//! Gemini) routing block that turns a file row into the wire format
//! the provider expects. Originally located under
//! `chat/extensions/file/processor.rs` — moved here as part of the
//! file/project/mcp bridge extraction (chat knows nothing about
//! files; the file module owns this routing API directly).
//!
//! Has ZERO chat imports: works against `ai_providers`,
//! `modules::file::storage`, and `modules::llm_provider_files`. The
//! "ContentBlock" type comes from the `ai_providers` crate, not chat,
//! so this is properly a file-module concern.
//!
//! Consumers:
//!   - file's chat-extension (per-message file attachments)
//!   - project's chat-extension (knowledge-file batches)
//!   - any future caller that needs a file → provider-routed block

use sqlx::PgPool;
use uuid::Uuid;

use ai_providers::{AIProvider, ContentBlock, DocumentSource, ImageSource};

use crate::common::AppError;
use crate::core::Repos;
use crate::modules::file::storage::manager::get_file_storage;
use crate::modules::llm_provider_files;

/// Process a single file ID into provider-routed ContentBlock(s).
///
/// Ownership is re-validated here as defense in depth: callers
/// already enforce it, but the per-file repository lookup costs
/// nothing extra and fails safe.
pub async fn process_file_blocks(
    pool: &PgPool,
    file_id: Uuid,
    provider_id: Uuid,
    provider_type: &str,
    user_id: Uuid,
) -> Result<Vec<ContentBlock>, AppError> {
    let file = Repos
        .file
        .get_by_id(file_id)
        .await?
        .ok_or_else(|| AppError::not_found("File"))?;
    process_file_blocks_with_file(pool, &file, provider_id, provider_type, user_id).await
}

/// Same as [`process_file_blocks`] but works against an already-fetched file
/// row. Callers that resolve many files at once (e.g. a project's knowledge
/// batch) prefetch via `get_by_ids` and call this to avoid an N+1 `get_by_id`
/// per file. Ownership is still re-validated as defense in depth.
pub async fn process_file_blocks_with_file(
    pool: &PgPool,
    file: &crate::modules::file::models::File,
    provider_id: Uuid,
    provider_type: &str,
    user_id: Uuid,
) -> Result<Vec<ContentBlock>, AppError> {
    let file_id = file.id;

    if file.user_id != user_id {
        return Err(AppError::forbidden(
            "FILE_ACCESS_DENIED",
            "You don't have access to this file",
        ));
    }

    let mime = file
        .mime_type
        .as_deref()
        .unwrap_or("application/octet-stream");

    // `is_real_openai` is only consulted by the openai+pdf branch; resolve it
    // (one DB lookup) only when that branch could fire, so the pure routing
    // decision below stays a function of plain values.
    let is_real_openai = if provider_type == "openai" && mime == "application/pdf" {
        Repos
            .llm_provider
            .get_by_id(provider_id)
            .await?
            .map(|p| p.provider_type == "openai")
            .unwrap_or(false)
    } else {
        false
    };

    let decision = route_decision(
        provider_type,
        mime,
        is_real_openai,
        file.text_page_count,
        crate::modules::file::available_files::is_text_like(mime),
    );

    match decision {
        RouteDecision::ProviderApi => {
            process_via_provider_api(pool, file_id, provider_id, mime, user_id).await
        }
        RouteDecision::InlineText => {
            inline_extracted_text(
                file.blob_version_id,
                &file.filename,
                file.text_page_count as u32,
                user_id,
            )
            .await
        }
        RouteDecision::Base64 => {
            process_via_base64(file.blob_version_id, &file.filename, mime, user_id).await
        }
    }
}

/// Which content-routing path a file takes, as a pure function of the file's
/// MIME + the (mapped) provider type. Extracted so the routing edge cases are
/// unit-testable without a DB / provider / storage. See
/// `process_file_blocks_with_file` for the prose rationale of each branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RouteDecision {
    /// Native image/PDF upload via the provider Files API.
    ProviderApi,
    /// Office/binary doc with extracted per-page text → inline that text.
    InlineText,
    /// Verbatim text / JSON-ish / base64 image fallback.
    Base64,
}

pub(crate) fn route_decision(
    provider_type: &str,
    mime: &str,
    is_real_openai: bool,
    text_page_count: i32,
    is_text_like: bool,
) -> RouteDecision {
    // 1. anthropic/gemini take native image + PDF.
    if matches!(provider_type, "anthropic" | "gemini")
        && (mime == "application/pdf" || mime.starts_with("image/"))
    {
        return RouteDecision::ProviderApi;
    }
    // 2. Only a GENUINE openai provider uploads PDFs (the mapped "openai" type
    //    is shared by groq/deepseek/mistral/custom/local, which don't).
    if provider_type == "openai" && mime == "application/pdf" && is_real_openai {
        return RouteDecision::ProviderApi;
    }
    // 3. Office docs / native-PDF-less PDFs that carry extracted text inline it.
    if text_page_count > 0
        && !mime.starts_with("image/")
        && !mime.starts_with("text/")
        && !is_text_like
    {
        return RouteDecision::InlineText;
    }
    // 4. Everything else: verbatim text / base64.
    RouteDecision::Base64
}

#[cfg(test)]
mod route_decision_tests {
    use super::{route_decision, RouteDecision};

    #[test]
    fn anthropic_gemini_take_native_image_and_pdf() {
        for pt in ["anthropic", "gemini"] {
            assert_eq!(route_decision(pt, "application/pdf", false, 3, false), RouteDecision::ProviderApi);
            assert_eq!(route_decision(pt, "image/png", false, 0, false), RouteDecision::ProviderApi);
        }
    }

    #[test]
    fn only_real_openai_uploads_pdf_others_fall_through() {
        // Mapped-but-not-real openai (groq/deepseek/etc) → NOT provider API.
        // A PDF with extracted text falls to InlineText; without text → Base64.
        assert_eq!(route_decision("openai", "application/pdf", false, 5, false), RouteDecision::InlineText);
        assert_eq!(route_decision("openai", "application/pdf", false, 0, false), RouteDecision::Base64);
        // A genuine openai provider uploads via the Files API.
        assert_eq!(route_decision("openai", "application/pdf", true, 5, false), RouteDecision::ProviderApi);
    }

    #[test]
    fn office_doc_with_text_inlines_but_text_like_and_images_do_not() {
        let docx = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
        assert_eq!(route_decision("openai", docx, false, 4, false), RouteDecision::InlineText);
        // text/* and is_text_like go base64 (verbatim text path), never inline.
        assert_eq!(route_decision("openai", "text/plain", false, 4, false), RouteDecision::Base64);
        assert_eq!(route_decision("openai", "application/json", false, 4, true), RouteDecision::Base64);
        // Images always base64 here (image file_id is provider-API only).
        assert_eq!(route_decision("openai", "image/png", false, 4, false), RouteDecision::Base64);
        // No extracted text → base64 even for an office mime.
        assert_eq!(route_decision("openai", docx, false, 0, false), RouteDecision::Base64);
    }

    #[test]
    fn anthropic_office_doc_with_text_inlines() {
        // anthropic only takes native image/pdf; an office doc still inlines text.
        let docx = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
        assert_eq!(route_decision("anthropic", docx, false, 2, false), RouteDecision::InlineText);
    }
}

async fn process_via_provider_api(
    pool: &PgPool,
    file_id: Uuid,
    provider_id: Uuid,
    mime_type: &str,
    user_id: Uuid,
) -> Result<Vec<ContentBlock>, AppError> {
    let provider = Repos
        .llm_provider
        .get_by_id(provider_id)
        .await?
        .ok_or_else(|| AppError::not_found("Provider"))?;

    let file_storage = get_file_storage();
    let file_repo = &Repos.file;

    let ai_provider: &dyn AIProvider = match provider.provider_type.as_str() {
        "anthropic" => &ai_providers::AnthropicProvider,
        "gemini" => &ai_providers::GeminiProvider,
        _ => &ai_providers::OpenAIProvider,
    };

    // user_id is threaded through for the user-scoped JOIN in
    // get_provider_file_mapping — closes 06-llm-provider F-04.
    let provider_file_id = llm_provider_files::service::get_or_upload_provider_file(
        pool,
        file_repo,
        &file_storage,
        file_id,
        user_id,
        &provider,
        ai_provider,
    )
    .await?;

    if mime_type.starts_with("image/") {
        Ok(vec![ContentBlock::Image {
            source: ImageSource::File {
                file_id: provider_file_id,
                media_type: Some(mime_type.to_string()),
            },
        }])
    } else if mime_type == "application/pdf" {
        Ok(vec![ContentBlock::Document {
            source: DocumentSource::File {
                file_id: provider_file_id,
                media_type: Some(mime_type.to_string()),
            },
        }])
    } else {
        Ok(vec![ContentBlock::Text {
            text: format!("[File: {} ({})]", file_id, mime_type),
        }])
    }
}

/// Inline a doc's already-extracted per-page text as a single Text block. Used
/// for office docs and (non-native) PDFs so their content reaches the model
/// instead of a `[File: x]` placeholder.
async fn inline_extracted_text(
    blob_version_id: Uuid,
    filename: &str,
    pages: u32,
    user_id: Uuid,
) -> Result<Vec<ContentBlock>, AppError> {
    let storage = get_file_storage();
    let mut text = format!("[File: {filename}]\n");
    for p in 1..=pages {
        // HEAD blob key — file_id keys v1's text pages (stale for edited files).
        if let Ok(page) = storage.load_text_page(user_id, blob_version_id, p).await {
            text.push_str(&page);
            if !page.ends_with('\n') {
                text.push('\n');
            }
        }
    }
    Ok(vec![ContentBlock::Text { text }])
}

async fn process_via_base64(
    blob_version_id: Uuid,
    filename: &str,
    mime_type: &str,
    user_id: Uuid,
) -> Result<Vec<ContentBlock>, AppError> {
    let file_storage = get_file_storage();
    // Canonical extension (matches upload's blob naming); blob_version_id is the
    // HEAD blob key — file_id would load v1's stale bytes for an edited file.
    let extension = crate::modules::file::utils::extension_of(filename);
    let file_data = file_storage
        .load_original(user_id, blob_version_id, &extension)
        .await?;

    if mime_type.starts_with("image/") {
        use base64::Engine;
        let base64_data = base64::engine::general_purpose::STANDARD.encode(&file_data);
        Ok(vec![ContentBlock::Image {
            source: ImageSource::Base64 {
                media_type: mime_type.to_string(),
                data: base64_data,
            },
        }])
    } else if mime_type == "application/pdf" {
        use base64::Engine;
        let base64_data = base64::engine::general_purpose::STANDARD.encode(&file_data);
        Ok(vec![ContentBlock::Document {
            source: DocumentSource::Base64 {
                media_type: mime_type.to_string(),
                data: base64_data,
            },
        }])
    } else if mime_type.starts_with("text/")
        || matches!(
            mime_type,
            "application/json"
                | "application/xml"
                | "application/javascript"
                | "application/x-yaml"
                | "application/yaml"
        )
    {
        // Text-like files: inline the content verbatim as a Text
        // ContentBlock. The pre-R4 implementation returned only the
        // filename (`[File: foo.txt]`) which meant attached
        // knowledge files reached the LLM with no body — silently
        // misleading any user attaching .txt/.md/.json content.
        //
        // Best-effort UTF-8: if the file isn't valid UTF-8 we fall
        // back to a labeled placeholder so a binary-disguised-as-
        // text upload doesn't crash the chat send.
        match String::from_utf8(file_data) {
            Ok(text) => Ok(vec![ContentBlock::Text {
                text: format!("[File: {filename}]\n{text}"),
            }]),
            Err(_) => Ok(vec![ContentBlock::Text {
                text: format!(
                    "[File: {filename} ({mime_type}) — non-UTF8 content omitted]"
                ),
            }]),
        }
    } else {
        // Unsupported binary type — surface as a labeled placeholder
        // so the LLM at least knows the file existed.
        Ok(vec![ContentBlock::Text {
            text: format!("[File: {filename} ({mime_type})]"),
        }])
    }
}

#[cfg(test)]
mod tests {
    //! Edge-case coverage for the file → provider ContentBlock routing decision.
    //!
    //! `tests/file/file_attachments_real_providers_test.rs` constructs
    //! `ContentBlock`s by hand and calls the provider API directly — it never
    //! drives `process_file_blocks_with_file`, so the routing branches below
    //! (ownership guard, text-like inline, image→base64, unsupported-binary /
    //! non-UTF8 / missing-mime placeholders, office-doc extracted-text inline)
    //! had no coverage.
    //!
    //! These exercise the REAL routing fn with `provider_type = "openai"` (a
    //! non-real-OpenAI mapped type), whose non-PDF paths never touch the DB pool
    //! or a provider upstream — only the global file store — so the tests are
    //! deterministic and DB-free. The pool is a never-connected lazy handle
    //! because these branches never read it.

    // `use super::*` already brings the parent's imports (ContentBlock,
    // ImageSource, DocumentSource, AppError, Uuid) into scope — re-importing
    // them here would be an E0252 duplicate-import error.
    use super::*;
    use crate::modules::file::models::File;
    use base64::Engine;
    use chrono::Utc;
    use std::sync::{Arc, OnceLock};
    use tempfile::TempDir;

    // One process-stable temp dir for the whole module: `init_file_storage`
    // wins the global `OnceCell` only once, so a per-test dir that drops at fn
    // exit would leave `get_file_storage()` pointing at a deleted path. A
    // single static dir kept for the test process avoids that.
    static STORAGE_DIR: OnceLock<TempDir> = OnceLock::new();

    fn storage() -> Arc<dyn crate::modules::file::storage::FileStorage> {
        let dir = STORAGE_DIR.get_or_init(|| tempfile::tempdir().unwrap());
        crate::modules::file::storage::manager::init_file_storage(dir.path());
        crate::modules::file::storage::manager::get_file_storage()
    }

    fn lazy_pool() -> sqlx::PgPool {
        // Never connects: these routing branches don't read the pool. A valid
        // URL is all `connect_lazy` needs (it parses, it does not dial).
        sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://u:p@127.0.0.1:1/never")
            .unwrap()
    }

    fn make_file(user_id: Uuid, filename: &str, mime: Option<&str>, text_pages: i32) -> File {
        let blob = Uuid::new_v4();
        File {
            id: Uuid::new_v4(),
            user_id,
            filename: filename.to_string(),
            file_size: 0,
            mime_type: mime.map(|m| m.to_string()),
            checksum: None,
            has_thumbnail: false,
            preview_page_count: 0,
            text_page_count: text_pages,
            processing_metadata: serde_json::json!({}),
            created_by: "user".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            version: 1,
            current_version_id: blob,
            blob_version_id: blob,
        }
    }

    async fn route(file: &File, user_id: Uuid) -> Result<Vec<ContentBlock>, AppError> {
        process_file_blocks_with_file(&lazy_pool(), file, Uuid::new_v4(), "openai", user_id).await
    }

    #[tokio::test]
    async fn ownership_mismatch_is_forbidden() {
        let owner = Uuid::new_v4();
        let file = make_file(owner, "secret.txt", Some("text/plain"), 0);
        // A different user must be denied before any storage read.
        let err = route(&file, Uuid::new_v4()).await.unwrap_err();
        assert_eq!(err.status_code(), 403);
        assert_eq!(err.error_code(), "FILE_ACCESS_DENIED");
    }

    #[tokio::test]
    async fn text_like_json_inlines_verbatim_text() {
        let user = Uuid::new_v4();
        let file = make_file(user, "data.json", Some("application/json"), 0);
        let body = b"{\"k\":\"v\"}";
        storage()
            .save_original(user, file.blob_version_id, "json", body)
            .await
            .unwrap();

        let blocks = route(&file, user).await.unwrap();
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            ContentBlock::Text { text } => {
                assert_eq!(text, "[File: data.json]\n{\"k\":\"v\"}");
            }
            other => panic!("expected verbatim Text, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn image_on_non_native_provider_routes_to_base64_image() {
        let user = Uuid::new_v4();
        let file = make_file(user, "pic.png", Some("image/png"), 0);
        let bytes = b"\x89PNG\r\n\x1a\nIMG";
        storage()
            .save_original(user, file.blob_version_id, "png", bytes)
            .await
            .unwrap();

        let blocks = route(&file, user).await.unwrap();
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            ContentBlock::Image {
                source: ImageSource::Base64 { media_type, data },
            } => {
                assert_eq!(media_type, "image/png");
                assert_eq!(
                    data,
                    &base64::engine::general_purpose::STANDARD.encode(bytes)
                );
            }
            other => panic!("expected base64 Image, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn unsupported_binary_routes_to_labeled_placeholder() {
        let user = Uuid::new_v4();
        let file = make_file(user, "blob.bin", Some("application/octet-stream"), 0);
        storage()
            .save_original(user, file.blob_version_id, "bin", &[0u8, 1, 2, 3])
            .await
            .unwrap();

        let blocks = route(&file, user).await.unwrap();
        match &blocks[0] {
            ContentBlock::Text { text } => {
                assert_eq!(text, "[File: blob.bin (application/octet-stream)]");
            }
            other => panic!("expected placeholder Text, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn missing_mime_defaults_to_octet_stream_placeholder() {
        let user = Uuid::new_v4();
        // mime_type None → defaults to application/octet-stream → placeholder.
        let file = make_file(user, "mystery.dat", None, 0);
        storage()
            .save_original(user, file.blob_version_id, "dat", &[9u8, 9, 9])
            .await
            .unwrap();

        let blocks = route(&file, user).await.unwrap();
        match &blocks[0] {
            ContentBlock::Text { text } => {
                assert_eq!(text, "[File: mystery.dat (application/octet-stream)]");
            }
            other => panic!("expected placeholder Text, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn non_utf8_text_routes_to_omitted_placeholder() {
        let user = Uuid::new_v4();
        let file = make_file(user, "bad.txt", Some("text/plain"), 0);
        // Invalid UTF-8 → the verbatim-text branch's from_utf8 fallback fires.
        storage()
            .save_original(user, file.blob_version_id, "txt", &[0xFFu8, 0xFE, 0x00])
            .await
            .unwrap();

        let blocks = route(&file, user).await.unwrap();
        match &blocks[0] {
            ContentBlock::Text { text } => {
                assert_eq!(
                    text,
                    "[File: bad.txt (text/plain) — non-UTF8 content omitted]"
                );
            }
            other => panic!("expected non-UTF8 placeholder Text, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn office_doc_with_extracted_text_inlines_pages() {
        let user = Uuid::new_v4();
        // A docx (not image/text/text-like) with extracted pages → inline them
        // rather than dropping to a useless `[File: x]` placeholder.
        let file = make_file(
            user,
            "report.docx",
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
            2,
        );
        let st = storage();
        st.save_text_page(user, file.blob_version_id, 1, "PAGE ONE\n")
            .await
            .unwrap();
        st.save_text_page(user, file.blob_version_id, 2, "PAGE TWO\n")
            .await
            .unwrap();

        let blocks = route(&file, user).await.unwrap();
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            ContentBlock::Text { text } => {
                assert_eq!(text, "[File: report.docx]\nPAGE ONE\nPAGE TWO\n");
            }
            other => panic!("expected inlined extracted Text, got {other:?}"),
        }
    }

    // ---- Provider-SPECIFIC routing branches (lines 76-77 / 82-87) ----------
    // The tests above all pin `provider_type = "openai"`, so the
    // `matches!(provider_type, "anthropic" | "gemini")` native-branch guard at
    // line 76 is only ever evaluated FALSE. These drive the same routing fn
    // with the native-capable provider types so that guard is evaluated TRUE,
    // proving it is correctly *mime-gated*: a non-image/non-PDF file on
    // anthropic/gemini is NOT diverted to the native provider-API path — it
    // falls through to the exact same extracted-text / verbatim-text handling
    // as openai. (The native image/PDF divergence at line 79 →
    // `process_via_provider_api` reads `Repos.llm_provider` + uploads, so it is
    // integration-tier — `tests/file/file_attachments_real_providers_test.rs`;
    // these stay DB-free.)
    async fn route_as(
        provider_type: &str,
        file: &File,
        user_id: Uuid,
    ) -> Result<Vec<ContentBlock>, AppError> {
        process_file_blocks_with_file(&lazy_pool(), file, Uuid::new_v4(), provider_type, user_id)
            .await
    }

    #[tokio::test]
    async fn anthropic_non_native_doc_falls_through_to_extracted_text() {
        let user = Uuid::new_v4();
        // A docx is neither image nor PDF, so even on anthropic the native
        // guard (line 76-77) does NOT capture it → extracted-text inline,
        // identical to the openai path. Exercises the guard evaluated TRUE for
        // the provider but FALSE on the mime sub-condition.
        let file = make_file(
            user,
            "brief.docx",
            Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
            1,
        );
        storage()
            .save_text_page(user, file.blob_version_id, 1, "ANTHROPIC PAGE\n")
            .await
            .unwrap();

        let blocks = route_as("anthropic", &file, user).await.unwrap();
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            ContentBlock::Text { text } => {
                assert_eq!(text, "[File: brief.docx]\nANTHROPIC PAGE\n");
            }
            other => panic!("expected inlined extracted Text on anthropic, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn gemini_text_like_falls_through_to_verbatim_text() {
        let user = Uuid::new_v4();
        // A text-like (JSON) file on gemini skips the native image/PDF guard and
        // is inlined verbatim — same as openai — proving the gemini arm of the
        // native-branch guard is mime-gated, not a blanket provider divert.
        let file = make_file(user, "cfg.json", Some("application/json"), 0);
        let body = b"{\"provider\":\"gemini\"}";
        storage()
            .save_original(user, file.blob_version_id, "json", body)
            .await
            .unwrap();

        let blocks = route_as("gemini", &file, user).await.unwrap();
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            ContentBlock::Text { text } => {
                assert_eq!(text, "[File: cfg.json]\n{\"provider\":\"gemini\"}");
            }
            other => panic!("expected verbatim Text on gemini, got {other:?}"),
        }
    }
}
