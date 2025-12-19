//! State persistence for LLM runtime
//!
//! Tracks running instances and their configurations using SQLite.

use crate::config::{DeviceType, EngineSettings, EngineType, InstanceConfig};
use crate::error::{Result, RuntimeError};
use rusqlite::{params, Connection};
use std::path::PathBuf;

/// Runtime state manager
pub struct StateManager {
    conn: Connection,
}

impl StateManager {
    /// Create a new state manager with default database location
    pub fn new() -> Result<Self> {
        let db_path = Self::default_db_path()?;
        Self::with_path(db_path)
    }

    /// Create a new state manager with custom database path
    pub fn with_path(db_path: PathBuf) -> Result<Self> {
        // Create parent directory if it doesn't exist
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&db_path)?;
        let mut manager = Self { conn };
        manager.initialize_schema()?;
        Ok(manager)
    }

    /// Get the default database path
    /// Returns `~/.llm-runtime/state.db`
    fn default_db_path() -> Result<PathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| RuntimeError::internal("Could not determine home directory"))?;

        Ok(home.join(".llm-runtime").join("state.db"))
    }

    /// Initialize database schema
    fn initialize_schema(&mut self) -> Result<()> {
        self.conn.execute(
            "CREATE TABLE IF NOT EXISTS instances (
                id TEXT PRIMARY KEY,
                engine TEXT NOT NULL,
                model_path TEXT NOT NULL,
                device TEXT NOT NULL,
                settings_json TEXT NOT NULL,
                pid INTEGER,
                port INTEGER,
                base_url TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
            [],
        )?;

        Ok(())
    }

    /// Save instance configuration
    pub fn save_instance(&self, instance: &InstanceConfig, pid: u32, port: u16, base_url: &str) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        let engine_str = instance.engine.to_string();
        let device_str = instance.device.to_string();
        let settings_json = serde_json::to_string(&instance.settings)?;

        self.conn.execute(
            "INSERT OR REPLACE INTO instances (id, engine, model_path, device, settings_json, pid, port, base_url, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                &instance.id,
                engine_str,
                instance.model_path.to_string_lossy().to_string(),
                device_str,
                settings_json,
                pid as i64,
                port as i64,
                base_url,
                now,
                now,
            ],
        )?;

        Ok(())
    }

    /// Get instance configuration by ID
    pub fn get_instance(&self, instance_id: &str) -> Result<Option<(InstanceConfig, u32, u16, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT engine, model_path, device, settings_json, pid, port, base_url
             FROM instances WHERE id = ?1"
        )?;

        let result = stmt.query_row(params![instance_id], |row| {
            let engine_str: String = row.get(0)?;
            let model_path_str: String = row.get(1)?;
            let device_str: String = row.get(2)?;
            let settings_json: String = row.get(3)?;
            let pid: i64 = row.get(4)?;
            let port: i64 = row.get(5)?;
            let base_url: String = row.get(6)?;

            Ok((engine_str, model_path_str, device_str, settings_json, pid, port, base_url))
        });

        match result {
            Ok((engine_str, model_path_str, device_str, settings_json, pid, port, base_url)) => {
                let engine = match engine_str.as_str() {
                    "llamacpp" => EngineType::Llamacpp,
                    "mistralrs" => EngineType::Mistralrs,
                    _ => return Err(RuntimeError::internal(format!("Unknown engine type: {}", engine_str))),
                };

                let device = match device_str.as_str() {
                    "cpu" => DeviceType::Cpu,
                    "cuda" => DeviceType::Cuda,
                    "metal" => DeviceType::Metal,
                    "rocm" => DeviceType::Rocm,
                    "vulkan" => DeviceType::Vulkan,
                    "opencl" => DeviceType::Opencl,
                    _ => return Err(RuntimeError::internal(format!("Unknown device type: {}", device_str))),
                };

                let settings: EngineSettings = serde_json::from_str(&settings_json)?;

                let config = InstanceConfig {
                    id: instance_id.to_string(),
                    engine,
                    model_path: PathBuf::from(model_path_str),
                    device,
                    settings,
                };

                Ok(Some((config, pid as u32, port as u16, base_url)))
            }
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// List all stored instances
    pub fn list_instances(&self) -> Result<Vec<(String, InstanceConfig)>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, engine, model_path, device, settings_json FROM instances ORDER BY id"
        )?;

        let rows = stmt.query_map([], |row| {
            let id: String = row.get(0)?;
            let engine_str: String = row.get(1)?;
            let model_path_str: String = row.get(2)?;
            let device_str: String = row.get(3)?;
            let settings_json: String = row.get(4)?;

            Ok((id, engine_str, model_path_str, device_str, settings_json))
        })?;

        let mut instances = Vec::new();

        for row in rows {
            let (id, engine_str, model_path_str, device_str, settings_json) = row?;

            let engine = match engine_str.as_str() {
                "llamacpp" => EngineType::Llamacpp,
                "mistralrs" => EngineType::Mistralrs,
                _ => continue,
            };

            let device = match device_str.as_str() {
                "cpu" => DeviceType::Cpu,
                "cuda" => DeviceType::Cuda,
                "metal" => DeviceType::Metal,
                "rocm" => DeviceType::Rocm,
                "vulkan" => DeviceType::Vulkan,
                "opencl" => DeviceType::Opencl,
                _ => continue,
            };

            let settings: EngineSettings = match serde_json::from_str(&settings_json) {
                Ok(s) => s,
                Err(_) => continue,
            };

            let config = InstanceConfig {
                id: id.clone(),
                engine,
                model_path: PathBuf::from(model_path_str),
                device,
                settings,
            };

            instances.push((id, config));
        }

        Ok(instances)
    }

    /// Delete instance from state
    pub fn delete_instance(&self, instance_id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM instances WHERE id = ?1",
            params![instance_id],
        )?;

        Ok(())
    }

    /// Clear all instances
    pub fn clear_all(&self) -> Result<()> {
        self.conn.execute("DELETE FROM instances", [])?;
        Ok(())
    }
}

impl Default for StateManager {
    fn default() -> Self {
        Self::new().expect("Failed to create default state manager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_state_manager_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let state = StateManager::with_path(db_path.clone()).unwrap();

        // Create test instance
        let mut settings = EngineSettings::default();
        settings.llamacpp.ctx_size = 4096;

        let config = InstanceConfig {
            id: "test-model".to_string(),
            engine: EngineType::Llamacpp,
            model_path: PathBuf::from("/tmp/model.gguf"),
            device: DeviceType::Cuda,
            settings,
        };

        // Save instance
        state.save_instance(&config, 12345, 8080, "http://127.0.0.1:8080").unwrap();

        // Retrieve instance
        let result = state.get_instance("test-model").unwrap();
        assert!(result.is_some());

        let (retrieved_config, pid, port, base_url) = result.unwrap();
        assert_eq!(retrieved_config.id, "test-model");
        assert_eq!(retrieved_config.engine, EngineType::Llamacpp);
        assert_eq!(pid, 12345);
        assert_eq!(port, 8080);
        assert_eq!(base_url, "http://127.0.0.1:8080");

        // List instances
        let instances = state.list_instances().unwrap();
        assert_eq!(instances.len(), 1);

        // Delete instance
        state.delete_instance("test-model").unwrap();

        let result = state.get_instance("test-model").unwrap();
        assert!(result.is_none());
    }
}
