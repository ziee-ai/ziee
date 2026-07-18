// The `pascal_to_camel_case` helper + the `sse_event_enum!`, `impl_string_to_enum!`,
// and `impl_json_from!` macros moved into `ziee-core` in Chunk B1 of the SDK
// extraction (`ziee_core::macros`). They are re-exported at the ziee crate root
// (see `lib.rs`/`main.rs`) so the existing `crate::sse_event_enum!` /
// `crate::impl_string_to_enum!` / `crate::impl_json_from!` call sites keep
// resolving unchanged (decision N2 — equivalence-preserving re-export shim).
//
// The macros retained below (`make_transparent!`, `impl_json_option_from!`,
// `define_extension_content!`) reference ziee-specific paths
// (`crate::common::types::JsonOption`, chat `MessageContentData`) and therefore
// stay app-side.

/// Create a transparent wrapper with Deref, DerefMut, From, and SQLx implementations
#[macro_export]
macro_rules! make_transparent {
    // Handle generic types like JsonOption<T>
    (
        $(#[$attr:meta])*
        $vis:vis struct $name:ident<$generic:ident>($inner:ty)
    ) => {
        $(#[$attr])*
        $vis struct $name<$generic>($inner);

        impl<$generic> std::ops::Deref for $name<$generic> {
            type Target = $inner;

            fn deref(&self) -> &Self::Target {
                &self.0
            }
        }

        impl<$generic> std::ops::DerefMut for $name<$generic> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.0
            }
        }

        impl<$generic> From<$inner> for $name<$generic> {
            fn from(value: $inner) -> Self {
                Self(value)
            }
        }

        // Implement Default to create empty JsonOption
        impl<$generic> Default for $name<$generic> {
            fn default() -> Self {
                Self(None)
            }
        }

        // Add a custom method for JSON conversion
        impl<$generic> $name<$generic>
        where
            $generic: serde::de::DeserializeOwned,
        {
            pub fn from_json_option(value: Option<serde_json::Value>) -> Self {
                match value {
                    Some(json_value) => {
                        match serde_json::from_value::<$generic>(json_value) {
                            Ok(parsed) => $name(Some(parsed)),
                            Err(_) => $name(None),
                        }
                    }
                    None => $name(None),
                }
            }
        }

        // Implement SQLx traits for seamless database integration
        impl<$generic> sqlx::Decode<'_, sqlx::Postgres> for $name<$generic>
        where
            $generic: serde::de::DeserializeOwned,
        {
            fn decode(
                value: sqlx::postgres::PgValueRef<'_>,
            ) -> Result<Self, Box<dyn std::error::Error + 'static + Send + Sync>> {
                let json_value = sqlx::types::Json::<serde_json::Value>::decode(value)?;

                match json_value.0 {
                    serde_json::Value::Null => Ok($name(None)),
                    other => {
                        match serde_json::from_value::<$generic>(other) {
                            Ok(parsed) => Ok($name(Some(parsed))),
                            Err(_) => Ok($name(None)),
                        }
                    }
                }
            }
        }

        impl<$generic> sqlx::Type<sqlx::Postgres> for $name<$generic> {
            fn type_info() -> sqlx::postgres::PgTypeInfo {
                sqlx::postgres::PgTypeInfo::with_name("jsonb")
            }
        }
    };
}

/// Implement From<Option<JsonValue>> for JsonOption<T> for specific types
#[macro_export]
macro_rules! impl_json_option_from {
    ($concrete_type:ty) => {
        impl From<Option<serde_json::Value>> for $crate::common::types::JsonOption<$concrete_type> {
            fn from(value: Option<serde_json::Value>) -> Self {
                $crate::common::types::JsonOption::from_json_option(value)
            }
        }
    };
}

pub(crate) use make_transparent;

/// Macro to define type-safe extension content types
///
/// This macro generates a type-safe enum for extension-specific content types
/// that can be stored in MessageContentData::Extension variant.
///
/// Usage:
/// ```rust
/// define_extension_content! {
///     extension: "file",
///     name: FileContent,
///
///     Image {
///         source: ImageSource,
///         #[serde(skip_serializing_if = "Option::is_none")]
///         alt_text: Option<String>,
///     } => "image",
///
///     FileAttachment {
///         file_id: Uuid,
///         filename: String,
///         #[serde(skip_serializing_if = "Option::is_none")]
///         mime_type: Option<String>,
///         file_size: i64,
///     } => "file_attachment",
/// }
/// ```
///
/// This generates:
/// - An enum with the specified variants
/// - to_message_content() method to convert to MessageContentData::Extension
/// - from_message_content() method to extract from MessageContentData
/// - content_type() method to get the content type string
#[macro_export]
macro_rules! define_extension_content {
    (
        extension: $ext_name:expr,
        name: $enum_name:ident,
        $(
            $variant:ident {
                $(
                    $(#[$field_attr:meta])*
                    $field:ident : $field_ty:ty
                ),* $(,)?
            } => $type_str:expr
        ),* $(,)?
    ) => {
        #[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
        #[serde(tag = "type", rename_all = "snake_case")]
        pub enum $enum_name {
            $(
                $variant {
                    $(
                        $(#[$field_attr])*
                        $field: $field_ty,
                    )*
                },
            )*
        }

        impl $enum_name {
            /// Convert to MessageContentData via serialization/deserialization
            /// The variants in FileContent map directly to MessageContentData variants
            pub fn to_message_content(&self) -> $crate::modules::chat::core::models::content::MessageContentData {
                // Serialize to JSON and deserialize as MessageContentData
                // This works because both enums have the same variant structure and use #[serde(tag = "type")]
                let json = serde_json::to_value(self).expect("Failed to serialize extension content");
                serde_json::from_value(json).expect("Failed to deserialize as MessageContentData")
            }

            /// Try to extract from MessageContentData
            /// Deserializes and lets serde check type tag
            pub fn from_message_content(
                data: &$crate::modules::chat::core::models::content::MessageContentData
            ) -> Option<Self> {
                // Serialize MessageContentData to JSON and try to deserialize as FileContent
                // This works because both enums have matching variant structures
                let json = serde_json::to_value(data).ok()?;
                serde_json::from_value(json).ok()
            }

            /// Get the content type string
            #[allow(dead_code)]
            pub fn content_type(&self) -> &'static str {
                match self {
                    $(
                        Self::$variant { .. } => $type_str,
                    )*
                }
            }

            /// Get the extension name
            #[allow(dead_code)]
            pub fn extension_name() -> &'static str {
                $ext_name
            }
        }
    };
}
