//! Provider registry for mapping provider types to implementations

use crate::{
    providers::{AnthropicProvider, GeminiProvider, OpenAIProvider},
    traits::AIProvider,
};
use std::collections::HashMap;
use std::sync::Arc;

/// Registry of AI providers
///
/// Maps provider type strings (e.g., "openai", "anthropic") to their implementations.
/// Multiple provider types can share the same implementation (e.g., "openai", "groq",
/// "deepseek" all use `OpenAIProvider`).
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn AIProvider>>,
}

impl ProviderRegistry {
    /// Creates a new provider registry with all supported providers
    pub fn new() -> Self {
        let mut providers: HashMap<String, Arc<dyn AIProvider>> = HashMap::new();

        // OpenAI provider used for all OpenAI-API-compatible providers
        let openai: Arc<dyn AIProvider> = Arc::new(OpenAIProvider);
        providers.insert("openai".to_string(), Arc::clone(&openai));
        providers.insert("groq".to_string(), Arc::clone(&openai));
        providers.insert("deepseek".to_string(), Arc::clone(&openai));
        providers.insert("mistral".to_string(), Arc::clone(&openai));
        providers.insert("huggingface".to_string(), Arc::clone(&openai));
        providers.insert("local".to_string(), Arc::clone(&openai));
        providers.insert("custom".to_string(), openai);

        // Custom API providers
        providers.insert("anthropic".to_string(), Arc::new(AnthropicProvider));
        providers.insert("gemini".to_string(), Arc::new(GeminiProvider));

        Self { providers }
    }

    /// Gets a provider by type
    ///
    /// Returns `None` if the provider type is not registered.
    pub fn get(&self, provider_type: &str) -> Option<Arc<dyn AIProvider>> {
        self.providers.get(provider_type).cloned()
    }

    /// Lists all registered provider types
    pub fn list_types(&self) -> Vec<String> {
        self.providers.keys().cloned().collect()
    }

    /// Checks if a provider type is registered
    pub fn has(&self, provider_type: &str) -> bool {
        self.providers.contains_key(provider_type)
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let registry = ProviderRegistry::new();
        assert!(registry.has("openai"));
        assert!(registry.has("anthropic"));
        assert!(registry.has("gemini"));
        assert!(registry.has("groq"));
    }

    #[test]
    fn test_shared_implementation() {
        let registry = ProviderRegistry::new();

        // OpenAI, Groq, DeepSeek should all use the same provider instance
        let openai = registry.get("openai").unwrap();
        let groq = registry.get("groq").unwrap();
        let deepseek = registry.get("deepseek").unwrap();

        assert_eq!(openai.name(), "OpenAI");
        assert_eq!(groq.name(), "OpenAI");
        assert_eq!(deepseek.name(), "OpenAI");
    }

    #[test]
    fn test_different_implementations() {
        let registry = ProviderRegistry::new();

        let openai = registry.get("openai").unwrap();
        let anthropic = registry.get("anthropic").unwrap();
        let gemini = registry.get("gemini").unwrap();

        assert_eq!(openai.name(), "OpenAI");
        assert_eq!(anthropic.name(), "Anthropic");
        assert_eq!(gemini.name(), "Gemini");
    }

    #[test]
    fn test_list_types() {
        let registry = ProviderRegistry::new();
        let types = registry.list_types();

        assert!(types.contains(&"openai".to_string()));
        assert!(types.contains(&"anthropic".to_string()));
        assert!(types.contains(&"gemini".to_string()));
        assert!(types.contains(&"groq".to_string()));
        assert!(types.contains(&"deepseek".to_string()));
        assert!(types.contains(&"mistral".to_string()));
        assert!(types.contains(&"huggingface".to_string()));
        assert!(types.contains(&"local".to_string()));
        assert!(types.contains(&"custom".to_string()));
    }
}
