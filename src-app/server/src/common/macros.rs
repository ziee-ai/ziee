/// Convert PascalCase to camelCase by lowercasing the first character
/// This is a helper function that will be used at runtime
pub fn pascal_to_camel_case(s: &str) -> String {
    if s.is_empty() {
        return String::new();
    }
    let mut chars: Vec<char> = s.chars().collect();
    chars[0] = chars[0].to_lowercase().next().unwrap_or(chars[0]);
    chars.into_iter().collect()
}

/// Macro to define an SSE event enum with automatic implementation of event helpers and Into<Event> trait
///
/// This macro defines the enum and automatically generates:
/// - event_name() method that converts PascalCase variants to camelCase
/// - data() method that serializes the variant data to JSON
/// - Into<axum::response::sse::Event> implementation
///
/// Usage:
/// ```rust
/// sse_event_enum! {
///     #[derive(Debug, Clone, Serialize, JsonSchema)]
///     #[serde(rename_all = "camelCase")]
///     pub enum SSEMyEvent {
///         Connected(SomeData),
///         Update(OtherData),
///         LogUpdate(String),
///         CreatedBranch(BranchData),
///     }
/// }
/// ```
#[macro_export]
macro_rules! sse_event_enum {
    (
        $(#[$attr:meta])*
        $vis:vis enum $enum_name:ident {
            $($variant:ident($data_type:ty)),+ $(,)?
        }
    ) => {
        $(#[$attr])*
        #[serde(rename_all = "camelCase")]
        $vis enum $enum_name {
            $($variant($data_type),)+
        }

        impl $enum_name {
            pub fn event_name(&self) -> &'static str {
                match self {
                    $(
                        Self::$variant(_) => {
                            // Use a static cache to avoid repeated string operations
                            static EVENT_NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();
                            EVENT_NAME.get_or_init(|| {
                                $crate::common::macros::pascal_to_camel_case(stringify!($variant))
                            })
                        },
                    )+
                }
            }

            pub fn data(&self) -> Result<String, serde_json::Error> {
                match self {
                    $(
                        Self::$variant(data) => serde_json::to_string(data),
                    )+
                }
            }
        }

        impl Into<axum::response::sse::Event> for $enum_name {
            fn into(self) -> axum::response::sse::Event {
                axum::response::sse::Event::default()
                    .event(self.event_name())
                    .data(self.data().unwrap_or_default())
            }
        }
    };
}

/// Implement From<String> for enums that have from_str() method
/// Usage: impl_string_to_enum!(EngineType);
/// This allows SQLx to automatically convert database strings to enum types
#[macro_export]
macro_rules! impl_string_to_enum {
    ($enum_type:ty) => {
        impl From<String> for $enum_type {
            fn from(s: String) -> Self {
                Self::from_str(&s).unwrap_or_else(|| {
                    panic!(
                        "Invalid enum value '{}' for type {}",
                        s,
                        std::any::type_name::<$enum_type>()
                    )
                })
            }
        }

        impl From<&str> for $enum_type {
            fn from(s: &str) -> Self {
                Self::from_str(s).unwrap_or_else(|| {
                    panic!(
                        "Invalid enum value '{}' for type {}",
                        s,
                        std::any::type_name::<$enum_type>()
                    )
                })
            }
        }
    };
}

/// Implement From<serde_json::Value> for types that implement Default and DeserializeOwned
/// Usage: impl_json_from!(MyStruct);
/// This allows automatic JSON value conversion with fallback to default
#[macro_export]
macro_rules! impl_json_from {
    ($struct_type:ty) => {
        impl From<serde_json::Value> for $struct_type {
            fn from(value: serde_json::Value) -> Self {
                serde_json::from_value(value).unwrap_or_default()
            }
        }
    };
}

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
