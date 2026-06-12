use uuid::Uuid;

use crate::define_extension_content;

/// Image source (URL or base64)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    Url { url: String },
    Base64 { media_type: String, data: String },
    File { file_id: String },
}

// Define type-safe file content types using the macro
define_extension_content! {
    extension: "file",
    name: FileContent,

    Image {
        source: ImageSource,
        #[serde(skip_serializing_if = "Option::is_none")]
        alt_text: Option<String>,
    } => "image",

    FileAttachment {
        file_id: Uuid,
        filename: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
        file_size: i64,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        version_id: Option<Uuid>,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        version: Option<i32>,
    } => "file_attachment",
}
