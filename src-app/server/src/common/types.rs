use crate::common::macros::make_transparent;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Custom wrapper for optional JSON fields that handles null values properly
make_transparent!(
    #[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
    pub struct JsonOption<T>(Option<T>)
);
