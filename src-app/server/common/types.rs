use crate::common::macros::make_transparent;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Custom wrapper for optional JSON fields that handles null values properly
make_transparent!(
    #[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
    pub struct JsonOption<T>(Option<T>)
);

impl<T> JsonOption<T> {
    /// Convert JsonOption<T> to Option<T>
    pub fn into_option(self) -> Option<T> {
        self.0
    }

    /// Convert &JsonOption<T> to Option<&T>
    pub fn as_option(&self) -> Option<&T> {
        self.0.as_ref()
    }

    /// Convert &JsonOption<T> to Option<T> by cloning
    pub fn to_option(&self) -> Option<T>
    where
        T: Clone,
    {
        self.0.clone()
    }
}
