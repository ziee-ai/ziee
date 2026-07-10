// Conversation export: render a conversation's active-branch transcript to
// markdown, then optionally to docx/pdf/odt/rtf/html via the shared pandoc
// helper, streamed as a download attachment.
//
// The markdown serializer is a FAITHFUL renderer of every `MessageContentData`
// variant (unlike `summarizer::message_to_summarizable`, which keeps only text):
// text as prose, tool_use/tool_result/thinking/code as fenced blocks,
// file_attachment/image as links — under `## Role` headers, in message order.

use aide::transform::TransformOperation;
use axum::extract::{Path, Query};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use schemars::JsonSchema;
use serde::Deserialize;
use uuid::Uuid;

use crate::common::{ApiResult, AppError};
use crate::core::Repos;
use crate::modules::chat::core::permissions::MessagesRead;
use crate::modules::chat::core::types::MessageWithContent;
use crate::modules::file::handlers::download::content_disposition;
use crate::modules::file::utils::export::{export_mime, render_to_format};
use crate::modules::permissions::extractors::RequirePermissions;
use crate::modules::permissions::with_permission;

/// `?format=` for `GET /conversations/{id}/export`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ConversationExportQuery {
    /// Target format: `md | docx | pdf | odt | rtf | html`.
    pub format: String,
}

/// Render a conversation's messages to a single markdown string. Pure +
/// unit-tested per content-block variant.
pub fn conversation_to_markdown(msgs: &[MessageWithContent]) -> String {
    let mut out = String::new();
    for m in msgs {
        let role = match m.message.role.as_str() {
            "user" => "User",
            "assistant" => "Assistant",
            "system" => "System",
            other => other,
        };
        out.push_str("## ");
        out.push_str(role);
        out.push_str("\n\n");

        for c in &m.contents {
            let Ok(data) = c.parse_content() else { continue };
            let Ok(v) = serde_json::to_value(&data) else { continue };
            let ty = v.get("type").and_then(|t| t.as_str()).unwrap_or("");
            match ty {
                "text" => {
                    if let Some(t) = v.get("text").and_then(|x| x.as_str()) {
                        out.push_str(t.trim_end());
                        out.push_str("\n\n");
                    }
                }
                "thinking" => {
                    let t = v
                        .get("thinking")
                        .and_then(|x| x.as_str())
                        .or_else(|| v.get("text").and_then(|x| x.as_str()))
                        .unwrap_or("");
                    out.push_str("```thinking\n");
                    out.push_str(t.trim_end());
                    out.push_str("\n```\n\n");
                }
                "tool_use" => {
                    let name = v.get("name").and_then(|x| x.as_str()).unwrap_or("tool");
                    let input = v
                        .get("input")
                        .map(|i| serde_json::to_string_pretty(i).unwrap_or_default())
                        .unwrap_or_default();
                    out.push_str(&format!("```tool_use ({name})\n{}\n```\n\n", input.trim_end()));
                }
                "tool_result" => {
                    let body = v
                        .get("content")
                        .map(|c| serde_json::to_string_pretty(c).unwrap_or_default())
                        .or_else(|| {
                            v.get("text").and_then(|x| x.as_str()).map(|s| s.to_string())
                        })
                        .unwrap_or_default();
                    out.push_str("```tool_result\n");
                    out.push_str(body.trim_end());
                    out.push_str("\n```\n\n");
                }
                "file_attachment" => {
                    let name = v
                        .get("filename")
                        .and_then(|x| x.as_str())
                        .or_else(|| v.get("name").and_then(|x| x.as_str()))
                        .unwrap_or("file");
                    let fid = v
                        .get("file_id")
                        .and_then(|x| x.as_str())
                        .or_else(|| {
                            v.get("source")
                                .and_then(|s| s.get("file_id"))
                                .and_then(|x| x.as_str())
                        });
                    match fid {
                        Some(id) => out.push_str(&format!("[{name}](/api/files/{id})\n\n")),
                        None => out.push_str(&format!("{name}\n\n")),
                    }
                }
                "image" => {
                    out.push_str("_[image]_\n\n");
                }
                _ => {}
            }
        }
    }
    out
}

/// Export the conversation's active-branch transcript as a downloadable file.
/// Gated on `MessagesRead` (reading message content); ownership-scoped (another
/// user's conversation id → 404).
pub async fn export_conversation(
    auth: RequirePermissions<(MessagesRead,)>,
    Path(conversation_id): Path<Uuid>,
    Query(q): Query<ConversationExportQuery>,
) -> ApiResult<Response> {
    let user_id = auth.user.id;
    let format = q.format.to_lowercase();
    let mime = export_mime(&format).ok_or_else(|| {
        AppError::bad_request(
            "INVALID_FORMAT",
            format!("unsupported export format '{}'", format),
        )
    })?;

    let conversation = Repos
        .chat
        .core
        .get_conversation(conversation_id, user_id)
        .await?
        .ok_or_else(|| AppError::not_found("Conversation"))?;
    let branch_id = conversation
        .active_branch_id
        .ok_or_else(|| AppError::internal_error("Conversation has no active branch"))?;

    let msgs = Repos.chat.core.get_conversation_history(branch_id).await?;
    let markdown = conversation_to_markdown(&msgs);

    let title = conversation
        .title
        .clone()
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| "conversation".to_string());
    let out_name = format!("{title}.{format}");

    let out_bytes = render_to_format(markdown.as_bytes(), "md", &format).await?;

    let headers = [
        (header::CONTENT_TYPE, mime.to_string()),
        (header::CONTENT_DISPOSITION, content_disposition(&out_name)),
        (header::CONTENT_LENGTH, out_bytes.len().to_string()),
    ];
    Ok((StatusCode::OK, (headers, out_bytes).into_response()))
}

pub fn export_conversation_docs(op: TransformOperation) -> TransformOperation {
    with_permission::<(MessagesRead,)>(op)
        .id("Chat.exportConversation")
        .tag("Chat")
        .summary("Export a conversation")
        .description(
            "Download the conversation's active-branch transcript rendered to \
             md/docx/pdf/odt/rtf/html.",
        )
}
