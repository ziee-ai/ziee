// LLM Provider API response types for chat module

use serde::{Deserialize, Serialize};

use crate::modules::{llm_model::models::LlmModel, llm_provider::models::LlmProvider};

/// Provider with its available models
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ProviderWithModels {
    #[serde(flatten)]
    pub provider: LlmProvider,
    pub llm_models: Vec<LlmModel>,
}

/// Response containing all providers accessible to the user
#[derive(Debug, Serialize, Deserialize, schemars::JsonSchema)]
pub struct GetUserProvidersResponse {
    pub providers: Vec<ProviderWithModels>,
}
