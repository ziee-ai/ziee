//! Model file metadata extraction (GGUF + SafeTensors).
//!
//! Reads only the file's header — never tensor weights — to surface
//! capabilities the server can populate `llm_models.capabilities`
//! JSONB with. The validation pipeline calls
//! [`extract_model_capabilities`] in the `validation_status: processing`
//! phase between download/upload commit and `valid`.
//!
//! ## GGUF format reference
//!
//! GGUF is a binary container (KV header + tensor map + payload).
//! We only read the KV header:
//!
//! ```text
//!     u32      magic_le              ("GGUF")
//!     u32      version_le            (3 supported, 2 + 1 tolerated)
//!     u64      tensor_count_le
//!     u64      metadata_kv_count_le
//!     loop metadata_kv_count_le times:
//!         string   key
//!         u32      value_type
//!         <value>  per value_type
//! ```
//!
//! `string` is a u64 length prefix followed by UTF-8 bytes.
//! `value_type` enums:
//!   0=U8, 1=I8, 2=U16, 3=I16, 4=U32, 5=I32, 6=F32,
//!   7=BOOL, 8=STRING, 9=ARRAY, 10=U64, 11=I64, 12=F64
//!
//! We only fully parse the keys we care about, skipping unknown
//! value types by length-walking.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::error::{Result, RuntimeError};
use super::types::EngineType;

// =====================================================================
// Public types
// =====================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelCapabilities {
    pub architecture: Option<String>,
    pub context_length: Option<u32>,
    pub quantization: Option<String>,
    pub embedding_size: Option<u32>,
    pub num_parameters_b: Option<f64>,
    pub chat_template: Option<String>,
    pub bos_token_id: Option<u32>,
    pub eos_token_id: Option<u32>,
    pub supports_chat: bool,
    pub supports_embeddings: bool,
    pub supports_vision: bool,
    pub supports_tool_use: Option<bool>,
    pub auto_detection_failed: Option<bool>,
    pub error: Option<String>,
    pub auto_detected_at: Option<String>,
    /// Per-engine compatibility flags. "ok" / "unsupported" /
    /// "unknown" so the create handler can refuse mismatched picks.
    pub engine_compatibility: Option<serde_json::Value>,
}

impl ModelCapabilities {
    /// Produce a `validation_warning`-shaped capabilities row with
    /// the extraction error captured. The model is still creatable;
    /// operator override fills in the values.
    pub fn detection_failed(reason: impl Into<String>) -> Self {
        Self {
            auto_detection_failed: Some(true),
            error: Some(reason.into()),
            auto_detected_at: Some(chrono::Utc::now().to_rfc3339()),
            ..Default::default()
        }
    }
}

// =====================================================================
// Engine compatibility lookup
// =====================================================================

/// Architectures llama.cpp supports natively as of mid-2026. The
/// list is intentionally permissive — new arches land routinely and
/// we don't want to block a freshly-supported model on stale data.
/// Unknown → "unknown" (not rejected; operator can override).
pub const LLAMACPP_SUPPORTED: &[&str] = &[
    "llama", "mistral", "qwen2", "qwen2_moe", "qwen3", "phi3", "phi2", "gemma", "gemma2",
    "command-r", "stablelm", "starcoder2", "deepseek", "deepseek2", "internlm2", "minicpm",
    "olmo", "rwkv6", "exaone", "gpt2", "falcon", "baichuan", "mpt",
];

pub const MISTRALRS_SUPPORTED: &[&str] = &[
    "llama", "mistral", "qwen2", "qwen3", "phi3", "phi2", "gemma", "gemma2", "starcoder2",
    "deepseek", "deepseek2", "minicpm",
];

pub fn engine_supports(engine: EngineType, architecture: &str) -> &'static str {
    let table = match engine {
        EngineType::Llamacpp => LLAMACPP_SUPPORTED,
        EngineType::Mistralrs => MISTRALRS_SUPPORTED,
    };
    if table.iter().any(|a| a.eq_ignore_ascii_case(architecture)) {
        "ok"
    } else {
        "unsupported"
    }
}

// =====================================================================
// Top-level dispatcher
// =====================================================================

/// Public entry point: file-vs-dir + extension drives which parser.
/// On any error returns a `detection_failed` capabilities row; the
/// model is still creatable.
pub fn extract_model_capabilities(path: &Path, engine_type: EngineType) -> Result<ModelCapabilities> {
    let mut caps = if path.is_dir() {
        extract_from_safetensors_dir(path).unwrap_or_else(ModelCapabilities::detection_failed)
    } else if path.is_file() {
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "gguf" => extract_from_gguf(path).unwrap_or_else(ModelCapabilities::detection_failed),
            other => ModelCapabilities::detection_failed(format!(
                "metadata extraction not supported for .{other}"
            )),
        }
    } else {
        return Err(RuntimeError::internal(format!(
            "path does not exist: {}",
            path.display()
        )));
    };

    // Layer in engine compatibility based on architecture.
    if let Some(arch) = caps.architecture.as_deref() {
        caps.engine_compatibility = Some(serde_json::json!({
            "llamacpp": engine_supports(EngineType::Llamacpp, arch),
            "mistralrs": engine_supports(EngineType::Mistralrs, arch),
            "picked": engine_supports(engine_type, arch),
        }));
    }

    if caps.auto_detected_at.is_none() {
        caps.auto_detected_at = Some(chrono::Utc::now().to_rfc3339());
    }

    Ok(caps)
}

// =====================================================================
// SafeTensors directory (HuggingFace Transformers layout)
// =====================================================================

#[derive(Debug, Deserialize)]
struct HfConfigJson {
    #[serde(default)]
    architectures: Vec<String>,
    #[serde(default)]
    hidden_size: Option<u32>,
    #[serde(default)]
    max_position_embeddings: Option<u32>,
    #[serde(default)]
    torch_dtype: Option<String>,
    #[serde(default)]
    bos_token_id: Option<u32>,
    #[serde(default)]
    eos_token_id: Option<u32>,
}

pub fn extract_from_safetensors_dir(path: &Path) -> std::result::Result<ModelCapabilities, String> {
    let config_path = path.join("config.json");
    if !config_path.exists() {
        return Err(format!("config.json not found at {}", config_path.display()));
    }
    let file = std::fs::File::open(&config_path)
        .map_err(|e| format!("open {}: {e}", config_path.display()))?;
    let cfg: HfConfigJson = serde_json::from_reader(file)
        .map_err(|e| format!("parse {}: {e}", config_path.display()))?;

    let architecture = cfg
        .architectures
        .first()
        .map(|s| hf_architecture_to_engine_arch(s).to_string());

    // Rough parameter estimate from sum of weight shard sizes / dtype.
    let dtype_bytes = match cfg.torch_dtype.as_deref() {
        Some("bfloat16") | Some("float16") | Some("half") => Some(2u64),
        Some("float32") | Some("float") => Some(4),
        Some("int8") | Some("uint8") => Some(1),
        _ => None,
    };
    let num_parameters_b = dtype_bytes.and_then(|bs| {
        let mut total: u64 = 0;
        let dir = std::fs::read_dir(path).ok()?;
        for entry in dir.flatten() {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".safetensors") {
                    if let Ok(m) = entry.metadata() {
                        total += m.len();
                    }
                }
            }
        }
        if total == 0 {
            None
        } else {
            Some((total as f64 / bs as f64) / 1e9)
        }
    });

    Ok(ModelCapabilities {
        architecture,
        context_length: cfg.max_position_embeddings,
        quantization: None, // safetensors are typically full-precision
        embedding_size: cfg.hidden_size,
        num_parameters_b,
        chat_template: None,
        bos_token_id: cfg.bos_token_id,
        eos_token_id: cfg.eos_token_id,
        supports_chat: true,
        supports_embeddings: false,
        supports_vision: false,
        supports_tool_use: None,
        auto_detection_failed: Some(false),
        error: None,
        auto_detected_at: Some(chrono::Utc::now().to_rfc3339()),
        engine_compatibility: None, // filled in by dispatcher
    })
}

/// Map a HuggingFace `architectures[0]` ("LlamaForCausalLM") to the
/// short architecture key used in GGUF + our engine_compatibility
/// tables.
fn hf_architecture_to_engine_arch(s: &str) -> &str {
    let lower = s.to_ascii_lowercase();
    if lower.starts_with("llama") {
        "llama"
    } else if lower.starts_with("mistral") {
        "mistral"
    } else if lower.starts_with("qwen3") {
        "qwen3"
    } else if lower.starts_with("qwen2") {
        "qwen2"
    } else if lower.starts_with("phi3") {
        "phi3"
    } else if lower.starts_with("phi") {
        "phi2"
    } else if lower.starts_with("gemma2") {
        "gemma2"
    } else if lower.starts_with("gemma") {
        "gemma"
    } else if lower.starts_with("starcoder2") {
        "starcoder2"
    } else if lower.starts_with("deepseek") {
        "deepseek"
    } else {
        s
    }
}

// =====================================================================
// GGUF header parser
// =====================================================================

/// GGUF metadata value type. Names + numbers from the spec.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GgufVt {
    U8 = 0,
    I8 = 1,
    U16 = 2,
    I16 = 3,
    U32 = 4,
    I32 = 5,
    F32 = 6,
    Bool = 7,
    String = 8,
    Array = 9,
    U64 = 10,
    I64 = 11,
    F64 = 12,
}

impl GgufVt {
    fn from_u32(v: u32) -> Option<Self> {
        Some(match v {
            0 => Self::U8,
            1 => Self::I8,
            2 => Self::U16,
            3 => Self::I16,
            4 => Self::U32,
            5 => Self::I32,
            6 => Self::F32,
            7 => Self::Bool,
            8 => Self::String,
            9 => Self::Array,
            10 => Self::U64,
            11 => Self::I64,
            12 => Self::F64,
            _ => return None,
        })
    }

    fn scalar_size(self) -> Option<usize> {
        Some(match self {
            Self::U8 | Self::I8 | Self::Bool => 1,
            Self::U16 | Self::I16 => 2,
            Self::U32 | Self::I32 | Self::F32 => 4,
            Self::U64 | Self::I64 | Self::F64 => 8,
            _ => return None,
        })
    }
}

/// Decoded GGUF metadata value. We only read U32/U64/Str in the
/// header walk, but the full set of scalar variants is kept so the
/// parser can decode + skip every value type correctly — the inner
/// payloads of F32/F64/Bool are intentionally unread.
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum GgufValue {
    U32(u32),
    U64(u64),
    F32(f32),
    F64(f64),
    Bool(bool),
    Str(String),
    /// Unhandled (Array, etc.) — kept opaque.
    Other,
}

struct GgufReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> GgufReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn read(&mut self, n: usize) -> std::result::Result<&'a [u8], String> {
        if self.pos + n > self.bytes.len() {
            return Err(format!(
                "gguf: short read ({}/{}+{})",
                self.bytes.len(),
                self.pos,
                n
            ));
        }
        let s = &self.bytes[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn u32_le(&mut self) -> std::result::Result<u32, String> {
        Ok(u32::from_le_bytes(self.read(4)?.try_into().unwrap()))
    }

    fn u64_le(&mut self) -> std::result::Result<u64, String> {
        Ok(u64::from_le_bytes(self.read(8)?.try_into().unwrap()))
    }

    fn f32_le(&mut self) -> std::result::Result<f32, String> {
        Ok(f32::from_le_bytes(self.read(4)?.try_into().unwrap()))
    }

    fn f64_le(&mut self) -> std::result::Result<f64, String> {
        Ok(f64::from_le_bytes(self.read(8)?.try_into().unwrap()))
    }

    fn string(&mut self) -> std::result::Result<String, String> {
        let len = self.u64_le()? as usize;
        // Bound — refuse to allocate gigabytes for a stray u64.
        if len > 64 * 1024 {
            return Err(format!("gguf: implausible string length {len}"));
        }
        let bytes = self.read(len)?;
        String::from_utf8(bytes.to_vec()).map_err(|e| format!("gguf: non-utf8 string: {e}"))
    }

    fn value(&mut self, vt: GgufVt) -> std::result::Result<GgufValue, String> {
        match vt {
            GgufVt::U8 => Ok(GgufValue::U32(self.read(1)?[0] as u32)),
            GgufVt::I8 => Ok(GgufValue::U32(self.read(1)?[0] as u32)),
            GgufVt::U16 => Ok(GgufValue::U32(
                u16::from_le_bytes(self.read(2)?.try_into().unwrap()) as u32,
            )),
            GgufVt::I16 => Ok(GgufValue::U32(
                u16::from_le_bytes(self.read(2)?.try_into().unwrap()) as u32,
            )),
            GgufVt::U32 | GgufVt::I32 => Ok(GgufValue::U32(self.u32_le()?)),
            GgufVt::F32 => Ok(GgufValue::F32(self.f32_le()?)),
            GgufVt::Bool => Ok(GgufValue::Bool(self.read(1)?[0] != 0)),
            GgufVt::String => Ok(GgufValue::Str(self.string()?)),
            GgufVt::U64 | GgufVt::I64 => Ok(GgufValue::U64(self.u64_le()?)),
            GgufVt::F64 => Ok(GgufValue::F64(self.f64_le()?)),
            GgufVt::Array => {
                let inner_raw = self.u32_le()?;
                let inner = GgufVt::from_u32(inner_raw)
                    .ok_or_else(|| format!("gguf: unknown array element type {inner_raw}"))?;
                let len = self.u64_le()? as usize;
                // Walk past the array body. Strings have variable length;
                // scalars have fixed length.
                match inner {
                    GgufVt::String => {
                        for _ in 0..len {
                            let _ = self.string()?;
                        }
                    }
                    GgufVt::Array => {
                        return Err("gguf: nested arrays not supported".into());
                    }
                    _ => {
                        let s = inner
                            .scalar_size()
                            .ok_or_else(|| "gguf: array of non-scalar".to_string())?;
                        self.read(s * len)?;
                    }
                }
                Ok(GgufValue::Other)
            }
        }
    }
}

pub fn extract_from_gguf(path: &Path) -> std::result::Result<ModelCapabilities, String> {
    // Read first 128 KB — plenty for the KV header in practice.
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(|e| format!("open: {e}"))?;
    let mut buf = vec![0u8; 256 * 1024];
    let n = file.read(&mut buf).map_err(|e| format!("read: {e}"))?;
    buf.truncate(n);

    let mut r = GgufReader::new(&buf);

    let magic = r.read(4)?;
    if magic != b"GGUF" {
        return Err(format!(
            "not a GGUF file (magic = {:02X?})",
            magic
        ));
    }
    let version = r.u32_le()?;
    if !(1..=3).contains(&version) {
        return Err(format!("unsupported GGUF version {version}"));
    }
    let _tensor_count = r.u64_le()?;
    let kv_count = r.u64_le()?;

    let mut caps = ModelCapabilities {
        supports_chat: true,
        auto_detection_failed: Some(false),
        auto_detected_at: Some(chrono::Utc::now().to_rfc3339()),
        ..Default::default()
    };

    let mut architecture: Option<String> = None;
    let mut quantization_idx: Option<u32> = None;

    for _ in 0..kv_count {
        let key = r.string()?;
        let vt_raw = r.u32_le()?;
        let vt = match GgufVt::from_u32(vt_raw) {
            Some(v) => v,
            None => {
                return Err(format!("gguf: unknown value type {vt_raw} at key '{key}'"));
            }
        };
        let value = r.value(vt)?;

        match (key.as_str(), &value) {
            ("general.architecture", GgufValue::Str(s)) => {
                architecture = Some(s.clone());
            }
            ("general.file_type", GgufValue::U32(n)) => {
                quantization_idx = Some(*n);
            }
            ("tokenizer.chat_template", GgufValue::Str(s)) => {
                caps.chat_template = Some(s.clone());
            }
            ("tokenizer.ggml.bos_token_id", GgufValue::U32(n)) => {
                caps.bos_token_id = Some(*n);
            }
            ("tokenizer.ggml.eos_token_id", GgufValue::U32(n)) => {
                caps.eos_token_id = Some(*n);
            }
            _ => {}
        }
    }

    // arch-prefixed fields require knowing the architecture first.
    if let Some(ref arch) = architecture {
        let mut r2 = GgufReader::new(&buf);
        // Skip past the header again.
        let _ = r2.read(4 + 4 + 8 + 8);
        for _ in 0..kv_count {
            let key = r2.string()?;
            let vt_raw = r2.u32_le()?;
            let vt = match GgufVt::from_u32(vt_raw) {
                Some(v) => v,
                None => break,
            };
            let value = r2.value(vt)?;
            let ctx_key = format!("{arch}.context_length");
            let emb_key = format!("{arch}.embedding_length");
            let block_key = format!("{arch}.block_count");
            match (key.as_str(), &value) {
                (k, GgufValue::U32(n)) if k == ctx_key => caps.context_length = Some(*n),
                (k, GgufValue::U64(n)) if k == ctx_key => caps.context_length = Some(*n as u32),
                (k, GgufValue::U32(n)) if k == emb_key => caps.embedding_size = Some(*n),
                (k, GgufValue::U32(n)) if k == block_key => {
                    if let Some(emb) = caps.embedding_size {
                        // Very rough param estimate: 2 * blocks * emb^2
                        // (ignores FFN expansion etc.).
                        let est = 2.0 * (*n as f64) * (emb as f64).powi(2);
                        caps.num_parameters_b = Some(est / 1e9);
                    }
                }
                _ => {}
            }
        }
        caps.architecture = Some(arch.clone());
    }

    caps.quantization = quantization_idx.map(gguf_file_type_to_quant);

    Ok(caps)
}

fn gguf_file_type_to_quant(idx: u32) -> String {
    // Subset of the file-type enum used widely in the wild.
    // (See llama.cpp's `ggml.h::ggml_ftype` enum for the full list.)
    match idx {
        0 => "F32",
        1 => "F16",
        2 => "Q4_0",
        3 => "Q4_1",
        7 => "Q8_0",
        8 => "Q5_0",
        9 => "Q5_1",
        10 => "Q2_K",
        11 => "Q3_K_S",
        12 => "Q3_K_M",
        13 => "Q3_K_L",
        14 => "Q4_K_S",
        15 => "Q4_K_M",
        16 => "Q5_K_S",
        17 => "Q5_K_M",
        18 => "Q6_K",
        24 => "IQ2_XS",
        25 => "IQ3_XXS",
        26 => "IQ1_S",
        27 => "IQ4_NL",
        28 => "IQ3_S",
        29 => "IQ3_M",
        30 => "IQ2_S",
        31 => "IQ2_M",
        32 => "IQ4_XS",
        _ => return format!("UNKNOWN_FT_{idx}"),
    }
    .to_string()
}

// =====================================================================
// Tests
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn synth_gguf_minimal() -> Vec<u8> {
        // Build a minimal valid GGUF v3 header with just
        // general.architecture = "llama" and llama.context_length = 4096.
        let mut buf = Vec::new();
        buf.extend_from_slice(b"GGUF");
        buf.extend_from_slice(&3u32.to_le_bytes()); // version
        buf.extend_from_slice(&0u64.to_le_bytes()); // tensor_count
        buf.extend_from_slice(&2u64.to_le_bytes()); // kv_count

        // KV1: general.architecture = "llama"
        let key1 = b"general.architecture";
        buf.extend_from_slice(&(key1.len() as u64).to_le_bytes());
        buf.extend_from_slice(key1);
        buf.extend_from_slice(&(GgufVt::String as u32).to_le_bytes());
        let val1 = b"llama";
        buf.extend_from_slice(&(val1.len() as u64).to_le_bytes());
        buf.extend_from_slice(val1);

        // KV2: llama.context_length = 4096 (u32)
        let key2 = b"llama.context_length";
        buf.extend_from_slice(&(key2.len() as u64).to_le_bytes());
        buf.extend_from_slice(key2);
        buf.extend_from_slice(&(GgufVt::U32 as u32).to_le_bytes());
        buf.extend_from_slice(&4096u32.to_le_bytes());

        buf
    }

    #[test]
    fn gguf_minimal_parses() {
        let bytes = synth_gguf_minimal();
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), &bytes).unwrap();

        let caps = extract_from_gguf(tmp.path()).unwrap();
        assert_eq!(caps.architecture.as_deref(), Some("llama"));
        assert_eq!(caps.context_length, Some(4096));
    }

    #[test]
    fn gguf_wrong_magic_errors() {
        let mut bytes = synth_gguf_minimal();
        bytes[0] = b'X';
        let tmp = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(tmp.path(), &bytes).unwrap();
        let r = extract_from_gguf(tmp.path());
        assert!(r.is_err());
        assert!(format!("{}", r.unwrap_err()).contains("magic"));
    }

    #[test]
    fn engine_supports_known_arches() {
        assert_eq!(engine_supports(EngineType::Llamacpp, "llama"), "ok");
        assert_eq!(engine_supports(EngineType::Mistralrs, "llama"), "ok");
        assert_eq!(engine_supports(EngineType::Llamacpp, "rwkv6"), "ok");
        assert_eq!(engine_supports(EngineType::Mistralrs, "rwkv6"), "unsupported");
        assert_eq!(engine_supports(EngineType::Llamacpp, "random-arch"), "unsupported");
    }

    #[test]
    fn detection_failed_shape() {
        let c = ModelCapabilities::detection_failed("test reason");
        assert_eq!(c.auto_detection_failed, Some(true));
        assert_eq!(c.error.as_deref(), Some("test reason"));
        assert!(c.auto_detected_at.is_some());
    }

    #[test]
    fn hf_arch_to_engine_arch_normalizes() {
        assert_eq!(hf_architecture_to_engine_arch("LlamaForCausalLM"), "llama");
        assert_eq!(hf_architecture_to_engine_arch("Qwen2ForCausalLM"), "qwen2");
        assert_eq!(hf_architecture_to_engine_arch("Qwen3ForCausalLM"), "qwen3");
        assert_eq!(hf_architecture_to_engine_arch("Phi3ForCausalLM"), "phi3");
        assert_eq!(hf_architecture_to_engine_arch("Gemma2ForCausalLM"), "gemma2");
        assert_eq!(hf_architecture_to_engine_arch("PhiForCausalLM"), "phi2");
        assert_eq!(hf_architecture_to_engine_arch("RandomModel"), "RandomModel");
    }

    #[test]
    fn safetensors_dir_parses_config() {
        let tmp = tempfile::tempdir().unwrap();
        let cfg = serde_json::json!({
            "architectures": ["LlamaForCausalLM"],
            "hidden_size": 4096,
            "max_position_embeddings": 32768,
            "torch_dtype": "bfloat16",
            "num_hidden_layers": 32,
            "bos_token_id": 1,
            "eos_token_id": 2,
        });
        std::fs::write(tmp.path().join("config.json"), cfg.to_string()).unwrap();
        let caps = extract_from_safetensors_dir(tmp.path()).unwrap();
        assert_eq!(caps.architecture.as_deref(), Some("llama"));
        assert_eq!(caps.context_length, Some(32768));
        assert_eq!(caps.embedding_size, Some(4096));
        assert_eq!(caps.bos_token_id, Some(1));
        assert_eq!(caps.eos_token_id, Some(2));
    }

    #[test]
    fn safetensors_dir_missing_config_errors() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(extract_from_safetensors_dir(tmp.path()).is_err());
    }
}
