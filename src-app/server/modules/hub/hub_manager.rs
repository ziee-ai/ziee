use std::path::PathBuf;
use std::fs;
use tokio::fs as async_fs;
use serde_json;
use include_dir::{include_dir, Dir};

use crate::common::AppError;
use super::models::{HubData, HubModel, HubAssistant, HubMCPServer};

const GITHUB_HUB_REPO: &str = "https://raw.githubusercontent.com/YOUR_ORG/ziee-hub/main";
const CURRENT_HUB_VERSION: &str = "1.0.0";

/// Embedded hub directories (compiled into binary)
static HUB_MODELS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/resources/hub/llm-models/1.0.0");
static HUB_ASSISTANTS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/resources/hub/assistants/1.0.0");
static HUB_MCP_SERVERS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/resources/hub/mcp-servers/1.0.0");

pub struct HubManager {
    app_data_dir: PathBuf,
}

impl HubManager {
    pub fn new(app_data_dir: impl Into<PathBuf>) -> Result<Self, AppError> {
        Ok(Self {
            app_data_dir: app_data_dir.into(),
        })
    }

    /// Initialize hub data on app startup
    pub fn initialize(&self) -> Result<(), AppError> {
        // Create hub directories if they don't exist
        let hub_dir = self.app_data_dir.join("hub");
        fs::create_dir_all(hub_dir.join("llm-models").join(CURRENT_HUB_VERSION))
            .map_err(|e| AppError::internal_error(format!("Failed to create hub directories: {}", e)))?;
        fs::create_dir_all(hub_dir.join("assistants").join(CURRENT_HUB_VERSION))
            .map_err(|e| AppError::internal_error(format!("Failed to create hub directories: {}", e)))?;
        fs::create_dir_all(hub_dir.join("mcp-servers").join(CURRENT_HUB_VERSION))
            .map_err(|e| AppError::internal_error(format!("Failed to create hub directories: {}", e)))?;

        // Copy embedded files if not already present
        self.copy_embedded_hub_files()?;

        Ok(())
    }

    /// Copy embedded hub files to app_data directory (only if not exists)
    fn copy_embedded_hub_files(&self) -> Result<(), AppError> {
        let hub_dir = self.app_data_dir.join("hub");

        // Copy all files from embedded directories
        self.copy_embedded_dir(&HUB_MODELS_DIR, hub_dir.join("llm-models").join(CURRENT_HUB_VERSION))?;
        self.copy_embedded_dir(&HUB_ASSISTANTS_DIR, hub_dir.join("assistants").join(CURRENT_HUB_VERSION))?;
        self.copy_embedded_dir(&HUB_MCP_SERVERS_DIR, hub_dir.join("mcp-servers").join(CURRENT_HUB_VERSION))?;

        // Write version files
        self.write_version_file("llm-models", CURRENT_HUB_VERSION)?;
        self.write_version_file("assistants", CURRENT_HUB_VERSION)?;
        self.write_version_file("mcp-servers", CURRENT_HUB_VERSION)?;

        Ok(())
    }

    /// Copy all files from an embedded directory to target directory
    fn copy_embedded_dir(&self, embedded_dir: &Dir, target_dir: PathBuf) -> Result<(), AppError> {
        for file in embedded_dir.files() {
            let file_path = target_dir.join(file.path());
            if !file_path.exists() {
                fs::write(&file_path, file.contents())
                    .map_err(|e| AppError::internal_error(format!("Failed to write file {:?}: {}", file_path, e)))?;
                tracing::debug!("Copied embedded file: {:?}", file_path);
            }
        }
        Ok(())
    }

    /// Write version file
    fn write_version_file(&self, category: &str, version: &str) -> Result<(), AppError> {
        let version_path = self.app_data_dir.join("hub").join(category).join("version.json");
        if !version_path.exists() {
            let version_data = serde_json::json!({ "version": version });
            let version_json = serde_json::to_string_pretty(&version_data)
                .map_err(|e| AppError::internal_error(format!("Failed to serialize version: {}", e)))?;
            fs::write(version_path, version_json)
                .map_err(|e| AppError::internal_error(format!("Failed to write version file: {}", e)))?;
        }
        Ok(())
    }

    /// Load hub data with locale support
    pub async fn load_hub_data_with_locale(&self, locale: &str) -> Result<HubData, AppError> {
        let hub_dir = self.app_data_dir.join("hub");
        let version = self.get_current_version("llm-models").await?;

        // Load base data (English)
        let models_base: Vec<HubModel> = self.load_json_file(
            hub_dir.join("llm-models").join(&version).join("base.json")
        ).await?;
        let assistants_base: Vec<HubAssistant> = self.load_json_file(
            hub_dir.join("assistants").join(&version).join("base.json")
        ).await?;
        let mcp_servers_base: Vec<HubMCPServer> = self.load_json_file(
            hub_dir.join("mcp-servers").join(&version).join("base.json")
        ).await?;

        // If locale is not English, merge with locale-specific overrides
        let (models, assistants, mcp_servers) = if locale != "en" {
            let models_override: Option<Vec<serde_json::Value>> = self.load_json_file_optional(
                hub_dir.join("llm-models").join(&version).join(format!("{}.json", locale))
            ).await?;
            let assistants_override: Option<Vec<serde_json::Value>> = self.load_json_file_optional(
                hub_dir.join("assistants").join(&version).join(format!("{}.json", locale))
            ).await?;
            let mcp_servers_override: Option<Vec<serde_json::Value>> = self.load_json_file_optional(
                hub_dir.join("mcp-servers").join(&version).join(format!("{}.json", locale))
            ).await?;

            (
                self.merge_models_with_overrides(models_base, models_override),
                self.merge_assistants_with_overrides(assistants_base, assistants_override),
                self.merge_mcp_servers_with_overrides(mcp_servers_base, mcp_servers_override),
            )
        } else {
            (models_base, assistants_base, mcp_servers_base)
        };

        Ok(HubData {
            version,
            models,
            assistants,
            mcp_servers,
        })
    }

    /// Merge base models with locale-specific overrides
    fn merge_models_with_overrides(&self, mut base: Vec<HubModel>, overrides: Option<Vec<serde_json::Value>>) -> Vec<HubModel> {
        if let Some(overrides) = overrides {
            for item in base.iter_mut() {
                if let Some(override_item) = overrides.iter().find(|o| o["id"].as_str() == Some(&item.id)) {
                    if let Some(display_name) = override_item["display_name"].as_str() {
                        item.display_name = display_name.to_string();
                    }
                    if let Some(description) = override_item["description"].as_str() {
                        item.description = Some(description.to_string());
                    }
                }
            }
        }
        base
    }

    /// Merge base assistants with locale-specific overrides
    fn merge_assistants_with_overrides(&self, mut base: Vec<HubAssistant>, overrides: Option<Vec<serde_json::Value>>) -> Vec<HubAssistant> {
        if let Some(overrides) = overrides {
            for item in base.iter_mut() {
                if let Some(override_item) = overrides.iter().find(|o| o["id"].as_str() == Some(&item.id)) {
                    if let Some(name) = override_item["name"].as_str() {
                        item.name = name.to_string();
                    }
                    if let Some(description) = override_item["description"].as_str() {
                        item.description = Some(description.to_string());
                    }
                    if let Some(instructions) = override_item["instructions"].as_str() {
                        item.instructions = Some(instructions.to_string());
                    }
                    if let Some(use_cases) = override_item["use_cases"].as_array() {
                        item.use_cases = Some(use_cases.iter().filter_map(|v| v.as_str().map(String::from)).collect());
                    }
                    if let Some(example_prompts) = override_item["example_prompts"].as_array() {
                        item.example_prompts = Some(example_prompts.iter().filter_map(|v| v.as_str().map(String::from)).collect());
                    }
                }
            }
        }
        base
    }

    /// Merge base MCP servers with locale-specific overrides
    fn merge_mcp_servers_with_overrides(&self, mut base: Vec<HubMCPServer>, overrides: Option<Vec<serde_json::Value>>) -> Vec<HubMCPServer> {
        if let Some(overrides) = overrides {
            for item in base.iter_mut() {
                if let Some(override_item) = overrides.iter().find(|o| o["id"].as_str() == Some(&item.id)) {
                    if let Some(display_name) = override_item["display_name"].as_str() {
                        item.display_name = display_name.to_string();
                    }
                    if let Some(description) = override_item["description"].as_str() {
                        item.description = Some(description.to_string());
                    }
                }
            }
        }
        base
    }

    /// Get current hub version for a specific category
    pub async fn get_current_version(&self, category: &str) -> Result<String, AppError> {
        let version_path = self.app_data_dir.join("hub").join(category).join("version.json");
        if version_path.exists() {
            let version_data: serde_json::Value = self.load_json_file(version_path).await?;
            Ok(version_data["version"].as_str().unwrap_or(CURRENT_HUB_VERSION).to_string())
        } else {
            Ok(CURRENT_HUB_VERSION.to_string())
        }
    }

    /// Refresh hub data for a specific category from GitHub
    pub async fn refresh_hub_category(&self, category: &str) -> Result<(), AppError> {
        tracing::info!("Refreshing hub category '{}' from GitHub", category);

        // Download latest version info for this category
        let version_url = format!("{}/{}/version.json", GITHUB_HUB_REPO, category);
        let latest_version: serde_json::Value = self.fetch_json(&version_url).await?;
        let latest_version_str = latest_version["version"].as_str()
            .ok_or_else(|| AppError::internal_error("Invalid version format"))?;

        // Update hub files for this category
        self.update_category_files_from_github(category, latest_version_str).await?;

        Ok(())
    }

    /// Update hub files for a specific category from GitHub
    async fn update_category_files_from_github(&self, category: &str, version: &str) -> Result<(), AppError> {
        let hub_dir = self.app_data_dir.join("hub");

        // Download base.json for this category
        self.download_hub_file(
            &format!("{}/{}/{}/base.json", GITHUB_HUB_REPO, category, version),
            hub_dir.join(category).join(version).join("base.json"),
        ).await?;

        // Update version file
        self.write_version_file(category, version)?;

        Ok(())
    }

    /// Download file from URL and save to path
    async fn download_hub_file(&self, url: &str, path: PathBuf) -> Result<(), AppError> {
        let response = reqwest::get(url).await
            .map_err(|e| AppError::internal_error(format!("Failed to download from GitHub: {}", e)))?;

        let content = response.bytes().await
            .map_err(|e| AppError::internal_error(format!("Failed to read response: {}", e)))?;

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            async_fs::create_dir_all(parent).await
                .map_err(|e| AppError::internal_error(format!("Failed to create directories: {}", e)))?;
        }

        async_fs::write(path, content).await
            .map_err(|e| AppError::internal_error(format!("Failed to write file: {}", e)))?;
        Ok(())
    }

    /// Fetch JSON from URL
    async fn fetch_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T, AppError> {
        let response = reqwest::get(url).await
            .map_err(|e| AppError::internal_error(format!("Failed to fetch from GitHub: {}", e)))?;

        response.json::<T>().await
            .map_err(|e| AppError::internal_error(format!("Failed to parse JSON: {}", e)))
    }

    /// Load JSON file
    async fn load_json_file<T: serde::de::DeserializeOwned>(&self, path: PathBuf) -> Result<T, AppError> {
        let content = async_fs::read_to_string(&path).await
            .map_err(|e| AppError::internal_error(format!("Failed to read file {:?}: {}", path, e)))?;

        serde_json::from_str(&content)
            .map_err(|e| AppError::internal_error(format!("Failed to parse JSON from {:?}: {}", path, e)))
    }

    /// Load JSON file (returns None if file doesn't exist)
    async fn load_json_file_optional<T: serde::de::DeserializeOwned>(&self, path: PathBuf) -> Result<Option<T>, AppError> {
        if !path.exists() {
            return Ok(None);
        }

        self.load_json_file(path).await.map(Some)
    }
}
