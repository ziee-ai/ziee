//! Configuration tests

use llm_runtime::{
    DeviceType, EngineSettings, EngineType, GlobalSettings, InstanceConfig, LlamaCppSettings,
    MistralRsSettings, RuntimeConfig,
};
use std::path::PathBuf;

#[test]
fn test_parse_yaml_config() {
    let yaml = r#"
global:
  log_dir: ./logs
  health_check_interval_secs: 30
  auto_restart: true
  max_restart_attempts: 3

instances:
  - id: test-model
    engine: llamacpp
    model_path: /models/test.gguf
    device: cuda
    settings:
      port: 8080
      llamacpp:
        ctx_size: 8192
        n_gpu_layers: 35
        batch_size: 512
"#;

    let config: RuntimeConfig = serde_yaml::from_str(yaml).unwrap();

    assert_eq!(config.instances.len(), 1);
    assert_eq!(config.instances[0].id, "test-model");
    assert_eq!(config.instances[0].engine, EngineType::Llamacpp);
    assert_eq!(config.instances[0].device, DeviceType::Cuda);
    assert_eq!(config.instances[0].settings.llamacpp.ctx_size, 8192);
    assert_eq!(config.instances[0].settings.llamacpp.n_gpu_layers, 35);
}

#[test]
fn test_engine_type_aliases() {
    // Test that engine type aliases work
    let yaml_llama = "engine: llama";
    let yaml_llamacpp = "engine: llamacpp";
    let yaml_llama_cpp = "engine: llama-cpp";

    let instance: serde_json::Value = serde_yaml::from_str(&format!(
        "{}\nmodel_path: /test\ndevice: cpu",
        yaml_llama
    ))
    .unwrap();
    assert_eq!(instance["engine"], "llama");

    let instance: serde_json::Value = serde_yaml::from_str(&format!(
        "{}\nmodel_path: /test\ndevice: cpu",
        yaml_llamacpp
    ))
    .unwrap();
    assert_eq!(instance["engine"], "llamacpp");
}

#[test]
fn test_device_types() {
    let devices = vec![
        "cpu", "cuda", "metal", "rocm", "vulkan", "opencl",
    ];

    for device in devices {
        let yaml = format!(
            "id: test\nengine: llamacpp\nmodel_path: /test\ndevice: {}",
            device
        );
        let config: InstanceConfig = serde_yaml::from_str(&yaml).unwrap();
        assert!(matches!(
            config.device,
            DeviceType::Cpu
                | DeviceType::Cuda
                | DeviceType::Metal
                | DeviceType::Rocm
                | DeviceType::Vulkan
                | DeviceType::Opencl
        ));
    }
}

#[test]
fn test_default_global_settings() {
    let settings = GlobalSettings::default();

    assert_eq!(settings.health_check_interval_secs, 30);
    assert_eq!(settings.startup_timeout_secs, 300);
    assert_eq!(settings.shutdown_timeout_secs, 10);
    assert_eq!(settings.auto_restart, false);
    assert_eq!(settings.max_restart_attempts, 3);
}

#[test]
fn test_default_llamacpp_settings() {
    let settings = LlamaCppSettings::default();

    assert_eq!(settings.ctx_size, 8192);
    assert_eq!(settings.n_gpu_layers, 0);
    assert_eq!(settings.batch_size, 512);
    assert_eq!(settings.embeddings, false);
}

#[test]
fn test_default_mistralrs_settings() {
    let settings = MistralRsSettings::default();

    assert_eq!(settings.max_seqs, 64);
    assert_eq!(settings.prefix_cache_n, 32);
    assert_eq!(settings.dtype, "f16");
    assert_eq!(settings.model_format, "auto");
}

#[test]
fn test_llamacpp_settings_validation() {
    let mut settings = LlamaCppSettings::default();

    // Valid settings should pass
    assert!(settings.validate().is_ok());

    // Invalid ctx_size
    settings.ctx_size = 0;
    assert!(settings.validate().is_err());

    settings.ctx_size = 200000;
    assert!(settings.validate().is_err());

    // Invalid batch_size
    settings.ctx_size = 8192;
    settings.batch_size = 0;
    assert!(settings.validate().is_err());

    settings.batch_size = 10000;
    assert!(settings.validate().is_err());
}

#[test]
fn test_mistralrs_settings_validation() {
    let mut settings = MistralRsSettings::default();

    // Valid settings should pass
    assert!(settings.validate().is_ok());

    // Invalid max_seqs
    settings.max_seqs = 0;
    assert!(settings.validate().is_err());

    settings.max_seqs = 300;
    assert!(settings.validate().is_err());

    // Invalid dtype
    settings.max_seqs = 64;
    settings.dtype = "invalid".to_string();
    assert!(settings.validate().is_err());

    // Valid dtypes
    for dtype in &["f16", "f32", "bf16", "auto"] {
        settings.dtype = dtype.to_string();
        assert!(settings.validate().is_ok());
    }

    // Invalid model_format
    settings.model_format = "invalid".to_string();
    assert!(settings.validate().is_err());

    // Valid formats
    for format in &["auto", "gguf", "safetensors", "pytorch"] {
        settings.model_format = format.to_string();
        assert!(settings.validate().is_ok());
    }
}

#[test]
fn test_runtime_config_validation() {
    let yaml = r#"
global:
  log_dir: ./logs

instances:
  - id: model1
    engine: llamacpp
    model_path: /tmp/test.gguf
    device: cpu

  - id: model1
    engine: llamacpp
    model_path: /tmp/test2.gguf
    device: cpu
"#;

    // Create temp files for validation
    std::fs::write("/tmp/test.gguf", "test").ok();
    std::fs::write("/tmp/test2.gguf", "test").ok();

    // Duplicate IDs should fail
    let result = RuntimeConfig::from_yaml(yaml);
    assert!(result.is_err());

    // Cleanup
    std::fs::remove_file("/tmp/test.gguf").ok();
    std::fs::remove_file("/tmp/test2.gguf").ok();
}

#[test]
fn test_engine_settings_default() {
    let settings = EngineSettings::default();

    assert_eq!(settings.port, None);
    assert_eq!(settings.llamacpp.ctx_size, 8192);
    assert_eq!(settings.mistralrs.max_seqs, 64);
}
