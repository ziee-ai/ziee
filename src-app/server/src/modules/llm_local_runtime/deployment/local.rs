// Local deployment strategy (same server as chat backend)

use super::{Deployment, DeploymentResult, InstanceStatus};
use crate::common::AppError;
use crate::modules::llm_local_runtime::BinaryManager;
use crate::modules::llm_model::models::{
    DeviceType, LlamaCppSettings, MistralRsCommand, MistralRsSettings, ModelEngineSettings,
};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;

type AppResult<T> = Result<T, AppError>;
use sqlx::types::Uuid;

/// Process-global map of model_id → per-instance bearer token. Chat
/// code calls `get_instance_api_key(model_id)` to retrieve the token
/// for outbound calls. Closes 08-llm-local-runtime F-04 (High) at
/// the runtime layer; the chat-side wiring that actually presents
/// the bearer to the local engine is a follow-up.
static INSTANCE_API_KEYS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<Uuid, String>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Return the bearer token assigned to the engine instance for
/// `model_id`, or None if no instance is running.
pub fn get_instance_api_key(model_id: Uuid) -> Option<String> {
    INSTANCE_API_KEYS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .get(&model_id)
        .cloned()
}

/// Maximum number of log lines retained per engine instance. When the
/// buffer fills, the oldest line is popped (FIFO) in O(1) via
/// VecDeque. The previous Vec::remove(0) was O(n) per push past the
/// cap → O(n²) over the buffer lifetime. Closes 08-llm-local-runtime
/// F-08 (Medium).
const LOG_BUFFER_MAX_LINES: usize = 1000;

/// Maximum bytes per captured log line. Without this, a runaway
/// engine that emits gigabyte-long lines would balloon server memory
/// (each WriteGuard + line allocation). Closes 08-llm-local-runtime
/// F-08's per-line-size sub-finding.
const LOG_LINE_MAX_BYTES: usize = 16 * 1024;

#[derive(Debug)]
struct ProcessInfo {
    child: Child,
    port: i32,
    started_at: std::time::Instant,
    logs: std::collections::VecDeque<String>,
    /// P2: broadcast channel for live log streaming. The capture
    /// loop fans out each line to BOTH the VecDeque (for snapshot)
    /// AND this broadcaster (for SSE). Capacity is small —
    /// `broadcast::Sender::send` drops the oldest on overflow so a
    /// slow subscriber doesn't pin memory.
    log_broadcast: tokio::sync::broadcast::Sender<String>,
}

pub struct LocalDeployment {
    processes: Arc<RwLock<HashMap<Uuid, ProcessInfo>>>,
    binary_manager: Arc<BinaryManager>,
}

impl LocalDeployment {
    pub fn new(binary_manager: Arc<BinaryManager>) -> Self {
        Self {
            processes: Arc::new(RwLock::new(HashMap::new())),
            binary_manager,
        }
    }

    /// Find an available port
    async fn find_available_port() -> AppResult<i32> {
        portpicker::pick_unused_port()
            .map(|p| p as i32)
            .ok_or_else(|| AppError::internal_error("No available ports"))
    }

    /// Validate that a value bound for engine argv is safe. Closes
    /// 08-llm-local-runtime F-02 (High): model.name flows from
    /// admin-uploaded model metadata into `--model VALUE` argv. If
    /// VALUE starts with `-` it could be re-interpreted as another
    /// flag (argument injection); shell metachars (`;`, `&`, `|`,
    /// `\``, `$()`) could enable command injection on engines that
    /// pass through to a shell. We reject either at deploy-time.
    fn validate_argv_value(label: &str, value: &str) -> AppResult<()> {
        if value.is_empty() {
            return Err(AppError::bad_request(
                "INVALID_ARGV",
                format!("{} cannot be empty", label),
            ));
        }
        if value.starts_with('-') {
            return Err(AppError::bad_request(
                "INVALID_ARGV",
                format!(
                    "{} cannot start with '-' (would be parsed as a flag): {:?}",
                    label, value
                ),
            ));
        }
        const BANNED: &[char] = &[';', '&', '|', '`', '$', '\n', '\r', '\0', '<', '>'];
        if value.chars().any(|c| BANNED.contains(&c)) {
            return Err(AppError::bad_request(
                "INVALID_ARGV",
                format!("{} contains shell metacharacters: {:?}", label, value),
            ));
        }
        Ok(())
    }

    /// Apply common security hardening to a spawned engine command:
    ///   - env_clear + minimal whitelisted env (PATH, HOME, LANG, TZ)
    ///   - stdin null (no inherited stdin)
    ///   - stdout/stderr piped (so we can capture)
    ///
    /// Without env_clear, the spawned engine inherits the server's full
    /// environment including DATABASE_URL, JWT_SECRET, upstream-provider
    /// API keys, OAuth secrets, and the HuggingFace token. A compromised
    /// engine binary OR an attacker who exfiltrates env via the engine's
    /// own diagnostics endpoint can then read all of them. Closes
    /// 08-llm-local-runtime F-03 (High).
    fn apply_hardening(cmd: &mut Command) {
        cmd.env_clear();
        // Preserve only the variables the engine genuinely needs to find
        // shared libraries and respect locale / timezone.
        for var in &["PATH", "HOME", "LANG", "LC_ALL", "TZ", "CUDA_VISIBLE_DEVICES"] {
            if let Ok(val) = std::env::var(var) {
                cmd.env(var, val);
            }
        }
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        cmd.kill_on_drop(true);

        // PR_SET_PDEATHSIG makes the engine subprocess (which holds GPU
        // memory) die with the server even on SIGKILL/OOM — otherwise it
        // orphans a GPU-holding llama-server and leaks VRAM. Linux-only
        // (copy of the bio_mcp / code_sandbox squashfuse path).
        #[cfg(target_os = "linux")]
        unsafe {
            cmd.pre_exec(|| {
                let r = libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM, 0, 0, 0);
                if r == 0 {
                    Ok(())
                } else {
                    Err(std::io::Error::last_os_error())
                }
            });
        }
    }

    /// Parse a model's nested `engine_settings` (a `ModelEngineSettings`
    /// — the single source of truth shared with the API/UI) into the
    /// llama.cpp branch. A malformed blob or out-of-range value falls back
    /// to defaults (with a warning) rather than failing the spawn.
    ///
    /// The stored shape is `{ "llamacpp": { ... } }` (per engine), NOT
    /// flat top-level keys; `resolve_model_inputs` additionally injects a
    /// top-level `embeddings: true` for embedder models, which is read at
    /// the call site (the struct has no such field).
    fn parse_llamacpp_settings(config: &serde_json::Value) -> LlamaCppSettings {
        let engine: ModelEngineSettings = serde_json::from_value(config.clone()).unwrap_or_default();
        let s = engine.llamacpp.unwrap_or_default();
        if let Err(e) = s.validate() {
            tracing::warn!("llamacpp: invalid engine_settings ({e}); using defaults");
            return LlamaCppSettings::default();
        }
        s
    }

    fn parse_mistralrs_settings(config: &serde_json::Value) -> MistralRsSettings {
        let engine: ModelEngineSettings = serde_json::from_value(config.clone()).unwrap_or_default();
        let s = engine.mistralrs.unwrap_or_default();
        if let Err(e) = s.validate() {
            tracing::warn!("mistralrs: invalid engine_settings ({e}); using defaults");
            return MistralRsSettings::default();
        }
        s
    }

    /// Build the llama-server argv (everything after the binary), mapping
    /// the full `LlamaCppSettings` vocabulary onto verified flags.
    ///
    /// `--host 127.0.0.1` + `--api-key TOKEN` are forced for the
    /// loopback/bearer hardening (08-llm-local-runtime F-04). Every
    /// user-supplied string flows through `validate_argv_value` (F-02).
    /// `embeddings` is driven by the model's capabilities, not a user
    /// setting. Flag names verified against `llama-server --help`
    /// (ggml-org/llama.cpp): `--flash-attn` takes `on|off|auto`, and
    /// `--device` takes device *IDs* (e.g. `CUDA0`), not backend names.
    fn llamacpp_argv(
        model_path: &str,
        port: i32,
        s: &LlamaCppSettings,
        api_key: &str,
        embeddings: bool,
    ) -> AppResult<Vec<String>> {
        let mut a = vec![
            "--model".to_string(),
            model_path.to_string(),
            "--port".to_string(),
            port.to_string(),
            "--host".to_string(),
            "127.0.0.1".to_string(),
            "--api-key".to_string(),
            api_key.to_string(),
        ];

        // Context / batching / memory.
        push_opt(&mut a, "--ctx-size", s.ctx_size);
        push_opt(&mut a, "--batch-size", s.batch_size);
        push_opt(&mut a, "--ubatch-size", s.ubatch_size);
        push_opt(&mut a, "--parallel", s.parallel);
        push_opt(&mut a, "--keep", s.keep);
        push_bool_flag(&mut a, "--mlock", s.mlock);
        push_bool_flag(&mut a, "--no-mmap", s.no_mmap);

        // Threading + attention.
        push_opt(&mut a, "--threads", s.threads);
        push_opt(&mut a, "--threads-batch", s.threads_batch);
        push_bool_flag(&mut a, "--cont-batching", s.cont_batching);
        if let Some(fa) = s.flash_attn {
            a.push("--flash-attn".to_string());
            a.push(if fa { "on" } else { "off" }.to_string());
        }
        push_bool_flag(&mut a, "--no-kv-offload", s.no_kv_offload);

        // GPU / device. device_type == Cpu forces 0 offloaded layers
        // regardless of n_gpu_layers.
        let cpu_only = matches!(s.device_type, Some(DeviceType::Cpu));
        if cpu_only {
            a.push("--n-gpu-layers".to_string());
            a.push("0".to_string());
        } else {
            push_opt(&mut a, "--n-gpu-layers", s.n_gpu_layers);
        }
        push_opt(&mut a, "--main-gpu", s.main_gpu);
        push_str_arg(&mut a, "--split-mode", "split_mode", s.split_mode.as_ref())?;
        push_str_arg(&mut a, "--tensor-split", "tensor_split", s.tensor_split.as_ref())?;
        // Explicit device IDs → `--device CUDA0,CUDA1`. Only CUDA's
        // `CUDA<n>` naming is emitted here (the one we can name without a
        // live `--list-devices`); other backends rely on n_gpu_layers /
        // main_gpu / tensor_split.
        if !cpu_only
            && matches!(s.device_type, Some(DeviceType::Cuda))
            && let Some(ids) = s.device_ids.as_ref()
            && !ids.is_empty()
        {
            let dev = ids
                .iter()
                .map(|i| format!("CUDA{i}"))
                .collect::<Vec<_>>()
                .join(",");
            a.push("--device".to_string());
            a.push(dev);
        }

        // RoPE / scaling.
        push_opt(&mut a, "--rope-freq-base", s.rope_freq_base);
        push_opt(&mut a, "--rope-freq-scale", s.rope_freq_scale);
        push_str_arg(&mut a, "--rope-scaling", "rope_scaling", s.rope_scaling.as_ref())?;

        // KV cache types + misc.
        push_str_arg(&mut a, "--cache-type-k", "cache_type_k", s.cache_type_k.as_ref())?;
        push_str_arg(&mut a, "--cache-type-v", "cache_type_v", s.cache_type_v.as_ref())?;
        push_opt(&mut a, "--seed", s.seed);
        push_str_arg(&mut a, "--numa", "numa", s.numa.as_ref())?;

        if embeddings {
            a.push("--embeddings".to_string());
        }
        Ok(a)
    }

    /// Build the mistralrs-server argv. mistral.rs uses a subcommand
    /// structure: global flags, then `gguf` / `plain` with the model id,
    /// then the subcommand-scoped `--dtype` / `--arch`.
    ///
    /// SECURITY: `--serve-ip` defaults to `0.0.0.0`; we force
    /// `127.0.0.1` so the engine stays loopback-bound (else
    /// `verify_loopback_bind` would kill it). The engine is
    /// proxy-fronted, so (unlike llama.cpp) there's no `--api-key`.
    /// Global flag names verified against the current
    /// `mistralrs-server` clap source (EricLBuehler/mistral.rs):
    /// PagedAttention is `--pa-*` (not `--paged-attn-*`), ISQ is `--isq`.
    ///
    /// Deferred until verified against a real binary (no artifact
    /// available): `num_device_layers`, `max_seq_len`,
    /// `truncate_sequence`, `prompt_chunksize`, `pa_cache_type`, and the
    /// vision knobs (`max_edge` / `max_num_images` / `max_image_length`).
    /// Excluded by hardening: `serve_ip` (forced), `token_source`,
    /// `interactive_mode`, `log_file`, `chat_template`, `jinja_explicit`,
    /// `tokenizer_json`, `weight_file`, `enable_search`, `search_bert_model`.
    fn mistralrs_argv(model_path: &str, port: i32, s: &MistralRsSettings) -> AppResult<Vec<String>> {
        let mut a = vec![
            "--serve-ip".to_string(),
            "127.0.0.1".to_string(),
            "--port".to_string(),
            port.to_string(),
        ];

        // Global flags.
        push_opt(&mut a, "--max-seqs", s.max_seqs);
        push_opt(&mut a, "--prefix-cache-n", s.prefix_cache_n);
        push_opt(&mut a, "--seed", s.seed);
        push_bool_flag(&mut a, "--no-kv-cache", s.no_kv_cache);
        let cpu = s.cpu == Some(true) || matches!(s.device_type, Some(DeviceType::Cpu));
        if cpu {
            a.push("--cpu".to_string());
        }
        push_str_arg(&mut a, "--isq", "in_situ_quant", s.in_situ_quant.as_ref())?;

        // PagedAttention (current `--pa-*` names).
        push_opt(&mut a, "--pa-gpu-mem", s.paged_attn_gpu_mem);
        push_opt(&mut a, "--pa-gpu-mem-usage", s.paged_attn_gpu_mem_usage);
        push_opt(&mut a, "--pa-ctxt-len", s.paged_ctxt_len);
        push_opt(&mut a, "--pa-blk-size", s.paged_attn_block_size);
        push_bool_flag(&mut a, "--no-paged-attn", s.no_paged_attn);
        push_bool_flag(&mut a, "--paged-attn", s.paged_attn);
        // `--thinking` takes a bool value (clap `Option<bool>`).
        push_opt(&mut a, "--thinking", s.enable_thinking);

        // Subcommand: explicit `command` wins, else auto-detect by
        // extension. Only gguf/plain are wired in v1.
        let path = std::path::Path::new(model_path);
        let use_gguf = match s.command {
            Some(MistralRsCommand::Gguf) => true,
            Some(MistralRsCommand::Plain) => false,
            _ => model_path.ends_with(".gguf"),
        };
        if use_gguf {
            let dir = path
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .filter(|d| !d.is_empty())
                .unwrap_or_else(|| ".".to_string());
            let file = match s.quantized_filename.as_ref() {
                Some(f) => {
                    Self::validate_argv_value("quantized_filename", f)?;
                    f.clone()
                }
                None => path
                    .file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| "*.gguf".to_string()),
            };
            a.push("gguf".to_string());
            a.push("--quantized-model-id".to_string());
            a.push(dir);
            a.push("--quantized-filename".to_string());
            a.push(file);
        } else {
            let model_id = match s.model_id_name.as_ref() {
                Some(m) => {
                    Self::validate_argv_value("model_id_name", m)?;
                    m.clone()
                }
                None => model_path.to_string(),
            };
            a.push("plain".to_string());
            a.push("--model-id".to_string());
            a.push(model_id);
            push_str_arg(&mut a, "--arch", "arch", s.arch.as_ref())?;
        }

        // `--dtype` is subcommand-scoped (valid after gguf/plain).
        push_str_arg(&mut a, "--dtype", "dtype", s.dtype.as_ref())?;
        Ok(a)
    }

    /// Verify after engine start that the listening socket is bound
    /// to 127.0.0.1 (Linux: /proc/<pid>/net/tcp). Returns true on
    /// loopback bind, false on anything else (including parse failure
    /// — be strict since the bind probe is a security boundary).
    /// Non-Linux: returns true (best-effort).
    #[cfg(target_os = "linux")]
    pub(crate) fn verify_loopback_bind(pid: i32, port: i32) -> bool {
        use std::fs;

        let path = format!("/proc/{pid}/net/tcp");
        let contents = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => return false, // strict: can't verify, treat as unsafe
        };
        // Lines look like:
        //   sl  local_address rem_address   st tx_queue rx_queue ...
        //    0: 0100007F:0BB8 00000000:0000 0A ...
        // local_address is host-order hex (little-endian on x86_64);
        // 127.0.0.1 is 7F.00.00.01 so the LE bytes are 0100007F.
        let want_addr = "0100007F"; // 127.0.0.1 little-endian
        // We also accept 7F000001 in case the kernel ever emits big-endian.
        let want_addr_be = "7F000001";
        let want_port_hex = format!("{:04X}", port);
        for line in contents.lines().skip(1) {
            // listening rows have state 0A (LISTEN); column index 3
            // after the sl: prefix.
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 4 {
                continue;
            }
            if parts.get(3) != Some(&"0A") {
                continue;
            }
            let local = parts.get(1).copied().unwrap_or("");
            let mut split = local.split(':');
            let addr = split.next().unwrap_or("");
            let port_h = split.next().unwrap_or("");
            if port_h.eq_ignore_ascii_case(&want_port_hex)
                && (addr == want_addr || addr == want_addr_be)
            {
                return true;
            }
            // Any non-loopback listener on this port is a security
            // violation.
            if port_h.eq_ignore_ascii_case(&want_port_hex) {
                tracing::error!(
                    "engine pid {} bound non-loopback addr {} on port {}",
                    pid,
                    addr,
                    port
                );
                return false;
            }
        }
        // No listener found on the expected port — either still
        // starting or already dead. The caller's /health probe is
        // the authoritative readiness check; we treat absence as
        // "can't verify yet → don't fail validation here".
        true
    }

    /// macOS twin of the Linux impl. There is no `/proc`, so enumerate
    /// the child's open sockets via the same `proc_pidinfo` /
    /// `proc_pidfdinfo` syscalls `lsof` uses and inspect the listening
    /// socket's bound local address. `libc` exposes the syscalls +
    /// `PROC_PIDLISTFDS` but NOT the `socket_fdinfo` layout, so we read
    /// the fields at ABI-fixed byte offsets (verified with clang
    /// `offsetof`/`sizeof` against `<sys/proc_info.h>`; identical on
    /// arm64 + x86_64 — both LP64 with the same field alignment). Falls
    /// back to `lsof -p` if the syscall path can't enumerate.
    ///
    /// Return/error semantics mirror the Linux impl EXACTLY:
    ///   loopback listener on `port`  → true
    ///   non-loopback listener        → false (security violation)
    ///   no listener on `port`        → true  (still starting / already
    ///                                          dead; /health is the
    ///                                          authoritative readiness
    ///                                          probe)
    ///   cannot verify at all         → false (strict — treat as unsafe,
    ///                                          like a failed /proc read)
    #[cfg(target_os = "macos")]
    pub(crate) fn verify_loopback_bind(pid: i32, port: i32) -> bool {
        match Self::macos_proc_listen_verdict(pid, port) {
            Some(v) => v,
            // Syscall path couldn't enumerate the pid's sockets — try lsof.
            None => match Self::macos_lsof_listen_verdict(pid, port) {
                Some(v) => v,
                None => false, // strict: can't verify → unsafe (mirrors Linux read-fail)
            },
        }
    }

    /// Primary macOS path: `proc_pidinfo(PROC_PIDLISTFDS)` to list fds,
    /// then `proc_pidfdinfo(PROC_PIDFDSOCKETINFO)` per socket fd, reading
    /// the listening socket's bound address at fixed offsets.
    ///
    /// `Some(true/false)` = enumerated and decided; `None` = could not
    /// enumerate (dead pid / EPERM / struct-layout drift) → caller falls
    /// back to lsof.
    #[cfg(target_os = "macos")]
    fn macos_proc_listen_verdict(pid: i32, port: i32) -> Option<bool> {
        use std::mem::size_of;

        // sizeof(struct socket_fdinfo). Its `soi_proto` union is sized to
        // its LARGEST member (un_sockinfo, not tcp_sockinfo), so
        // proc_pidfdinfo demands the full buffer or fills nothing.
        const SOCKET_FDINFO_SIZE: usize = 792;
        // Byte offsets into the socket_fdinfo buffer (clang offsetof):
        //   psi @24; socket_info.soi_kind @232, .soi_proto @240. The proto
        //   union begins with in_sockinfo (for SOCKINFO_TCP it is
        //   tcpsi_ini @0 of the union), so the in_sockinfo fields sit at
        //   fixed offsets from the union start (264).
        const OFF_SOI_KIND: usize = 24 + 232; // 256
        const OFF_PROTO: usize = 24 + 240; // 264  (in_sockinfo start)
        const OFF_LPORT: usize = OFF_PROTO + 4; // insi_lport
        const OFF_VFLAG: usize = OFF_PROTO + 24; // insi_vflag
        const OFF_LADDR: usize = OFF_PROTO + 48; // insi_laddr (union)
        const OFF_TCP_STATE: usize = OFF_PROTO + 80; // tcpsi_state (after in_sockinfo=80)
        const SOCKINFO_TCP: i32 = 2;
        const TSI_S_LISTEN: i32 = 1;
        const INI_IPV6: u8 = 0x2;
        const PROC_PIDFDSOCKETINFO: libc::c_int = 3;

        // 1) Size then read the pid's fd table.
        let needed = unsafe {
            libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, std::ptr::null_mut(), 0)
        };
        if needed <= 0 {
            return None; // can't list fds (dead pid / EPERM) → try lsof
        }
        let cap = needed as usize / size_of::<libc::proc_fdinfo>();
        let mut fds: Vec<libc::proc_fdinfo> = vec![unsafe { std::mem::zeroed() }; cap];
        let got = unsafe {
            libc::proc_pidinfo(
                pid,
                libc::PROC_PIDLISTFDS,
                0,
                fds.as_mut_ptr() as *mut libc::c_void,
                needed,
            )
        };
        if got <= 0 {
            return None;
        }
        fds.truncate(got as usize / size_of::<libc::proc_fdinfo>());

        // 2) Inspect each socket fd.
        let mut had_read_error = false;
        let mut buf = [0u8; SOCKET_FDINFO_SIZE];
        for fd in &fds {
            if fd.proc_fdtype != libc::PROX_FDTYPE_SOCKET as u32 {
                continue;
            }
            let rv = unsafe {
                libc::proc_pidfdinfo(
                    pid,
                    fd.proc_fd,
                    PROC_PIDFDSOCKETINFO,
                    buf.as_mut_ptr() as *mut libc::c_void,
                    SOCKET_FDINFO_SIZE as libc::c_int,
                )
            };
            if rv as usize != SOCKET_FDINFO_SIZE {
                // Short/failed read — struct drift or a racing close. Don't
                // silently treat an unreadable socket as "no listener".
                had_read_error = true;
                continue;
            }
            // Only TCP sockets carry a LISTEN state.
            let soi_kind =
                i32::from_ne_bytes(buf[OFF_SOI_KIND..OFF_SOI_KIND + 4].try_into().unwrap());
            if soi_kind != SOCKINFO_TCP {
                continue;
            }
            let tcp_state =
                i32::from_ne_bytes(buf[OFF_TCP_STATE..OFF_TCP_STATE + 4].try_into().unwrap());
            if tcp_state != TSI_S_LISTEN {
                continue;
            }
            // Ports are stored in network byte order in the low 16 bits.
            let lport = u16::from_be_bytes([buf[OFF_LPORT], buf[OFF_LPORT + 1]]) as i32;
            if lport != port {
                continue;
            }
            // A listener on the target port — is it loopback?
            let vflag = buf[OFF_VFLAG];
            let is_loopback = if vflag & INI_IPV6 != 0 {
                // in6_addr @ laddr: ::1 == 15 zero bytes + 0x01.
                let a = &buf[OFF_LADDR..OFF_LADDR + 16];
                a[..15].iter().all(|&b| b == 0) && a[15] == 1
            } else {
                // IPv4 s_addr @ laddr+12 (after in4in6_addr.i46a_pad32[3]),
                // network order. Match 127.0.0.1 exactly, like the Linux
                // impl (which tests 0100007F).
                buf[OFF_LADDR + 12..OFF_LADDR + 16] == [127, 0, 0, 1]
            };
            if is_loopback {
                return Some(true);
            }
            tracing::error!(
                "engine pid {} bound non-loopback listener on port {} (macOS proc)",
                pid,
                port
            );
            return Some(false);
        }

        if had_read_error {
            // Saw socket fds but couldn't read one — inconclusive; let the
            // caller fall back to lsof rather than assume "no listener".
            None
        } else {
            // Clean enumeration, no listener on the port. Absence is not a
            // failure here (mirrors Linux; /health is authoritative).
            Some(true)
        }
    }

    /// Fallback macOS path: parse `lsof` for the pid's LISTEN TCP sockets.
    /// `Some(_)` = ran + decided; `None` = lsof itself couldn't run (not
    /// installed / spawn error) → treated as "can't verify" by the caller.
    #[cfg(target_os = "macos")]
    fn macos_lsof_listen_verdict(pid: i32, port: i32) -> Option<bool> {
        // -nP: numeric host + port (no DNS/service lookup). -Fn: parseable
        // output, one field per line; 'n' rows carry the socket name, e.g.
        //   n127.0.0.1:8080   n*:8080 (0.0.0.0)   n[::1]:8080
        let out = std::process::Command::new("lsof")
            .args(["-nP", "-a", "-p", &pid.to_string(), "-iTCP", "-sTCP:LISTEN", "-Fn"])
            .output()
            .ok()?; // spawn failed (no lsof) → None → caller treats as unverifiable
        let text = String::from_utf8_lossy(&out.stdout);
        let want = format!(":{port}");
        for line in text.lines() {
            let name = match line.strip_prefix('n') {
                Some(n) => n,
                None => continue,
            };
            if !name.ends_with(&want) {
                continue;
            }
            let addr = &name[..name.len() - want.len()];
            // Match loopback exactly (127.0.0.1 / [::1]), like the Linux
            // impl. `*` is lsof's rendering of 0.0.0.0 (wildcard) → reject.
            if addr == "127.0.0.1" || addr == "[::1]" {
                return Some(true);
            }
            tracing::error!(
                "engine pid {} bound non-loopback listener {} on port {} (lsof)",
                pid,
                addr,
                port
            );
            return Some(false);
        }
        // Ran fine, no listener on the port → absence → true (mirrors Linux).
        Some(true)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    /// Windows twin of the Linux impl. There is no `/proc`, so enumerate
    /// the system TCP listener table via `GetExtendedTcpTable`
    /// (`iphlpapi`) with `TCP_TABLE_OWNER_PID_LISTENER` — the same owner-PID
    /// data `netstat -ano` shows — and inspect the rows owned by `pid` on
    /// `port`. Both the IPv4 and IPv6 tables are queried (a listener that
    /// ignored `--host 127.0.0.1` and bound `0.0.0.0`/`::` shows up here).
    ///
    /// Return/error semantics mirror the Linux impl EXACTLY:
    ///   loopback listener on `port`  → true   (127.0.0.1 or ::1)
    ///   non-loopback listener        → false  (security violation)
    ///   no listener on `port`        → true   (still starting / already
    ///                                          dead; /health is the
    ///                                          authoritative readiness
    ///                                          probe)
    ///   cannot enumerate at all      → false  (strict — treat as unsafe,
    ///                                          like a failed /proc read)
    #[cfg(target_os = "windows")]
    pub(crate) fn verify_loopback_bind(pid: i32, port: i32) -> bool {
        // Query both families. Each returns:
        //   Ok(Some(true))  = matching loopback listener found
        //   Ok(Some(false)) = matching NON-loopback listener found
        //   Ok(None)        = enumerated cleanly, no matching row
        //   Err(())         = could not enumerate this family
        let v4 = Self::win_tcp_listener_verdict(pid, port, false);
        let v6 = Self::win_tcp_listener_verdict(pid, port, true);

        // Any non-loopback listener on the port is a hard security failure,
        // regardless of what the other family says.
        if v4 == Ok(Some(false)) || v6 == Ok(Some(false)) {
            return false;
        }
        // A loopback listener on the port → verified.
        if v4 == Ok(Some(true)) || v6 == Ok(Some(true)) {
            return true;
        }
        // No matching row anywhere. If BOTH families failed to enumerate we
        // could not verify → strict false (mirrors a failed /proc read). If
        // at least one enumerated cleanly, absence is not a failure (mirrors
        // Linux; /health is authoritative).
        if v4.is_err() && v6.is_err() {
            return false;
        }
        true
    }

    /// Query one address family's `TCP_TABLE_OWNER_PID_LISTENER` table and
    /// decide whether `pid` owns a listener on `port` and, if so, whether it
    /// is bound to loopback. `ipv6 == false` → IPv4 table (127.0.0.1),
    /// `true` → IPv6 table (::1). See `verify_loopback_bind` for the encoded
    /// verdict.
    #[cfg(target_os = "windows")]
    fn win_tcp_listener_verdict(pid: i32, port: i32, ipv6: bool) -> Result<Option<bool>, ()> {
        use windows_sys::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, NO_ERROR};
        use windows_sys::Win32::NetworkManagement::IpHelper::{
            GetExtendedTcpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCP6TABLE_OWNER_PID,
            MIB_TCPROW_OWNER_PID, MIB_TCPTABLE_OWNER_PID, TCP_TABLE_OWNER_PID_LISTENER,
        };
        use windows_sys::Win32::Networking::WinSock::{AF_INET, AF_INET6};

        let want_pid = pid as u32;
        let af: u32 = if ipv6 { AF_INET6 as u32 } else { AF_INET as u32 };

        // Two-call idiom: size the buffer, then fill it. The table can grow
        // between calls, so loop until the size stops changing.
        let mut size: u32 = 0;
        let mut buf: Vec<u8> = Vec::new();
        // A few attempts is plenty; bail out rather than spin forever.
        for _ in 0..8 {
            let rc = unsafe {
                GetExtendedTcpTable(
                    if buf.is_empty() {
                        std::ptr::null_mut()
                    } else {
                        buf.as_mut_ptr() as *mut core::ffi::c_void
                    },
                    &mut size,
                    0, // bOrder = FALSE (ordering irrelevant to us)
                    af,
                    TCP_TABLE_OWNER_PID_LISTENER,
                    0,
                )
            };
            if rc == NO_ERROR {
                if buf.is_empty() {
                    // Sizing call unexpectedly succeeded with an empty buffer
                    // (size 0) → no rows.
                    return Ok(None);
                }
                break;
            }
            if rc == ERROR_INSUFFICIENT_BUFFER {
                buf = vec![0u8; size as usize];
                continue;
            }
            // Any other error (e.g. ERROR_INVALID_PARAMETER) → cannot verify.
            return Err(());
        }
        if buf.is_empty() {
            // Never got a fillable buffer within the retry budget.
            return Err(());
        }

        // Decode `port` (host order) from a row's `dwLocalPort` (network byte
        // order in the low 16 bits of the DWORD).
        let decode_port = |dw: u32| -> i32 {
            u16::from_be_bytes([(dw & 0xFF) as u8, ((dw >> 8) & 0xFF) as u8]) as i32
        };

        if ipv6 {
            let table = buf.as_ptr() as *const MIB_TCP6TABLE_OWNER_PID;
            let n = unsafe { (*table).dwNumEntries } as isize;
            let rows = unsafe { core::ptr::addr_of!((*table).table) } as *const MIB_TCP6ROW_OWNER_PID;
            for i in 0..n {
                let row = unsafe { &*rows.offset(i) };
                if row.dwOwningPid != want_pid || decode_port(row.dwLocalPort) != port {
                    continue;
                }
                // ::1 == 15 zero bytes followed by 0x01.
                let a = &row.ucLocalAddr;
                let is_loopback = a[..15].iter().all(|&b| b == 0) && a[15] == 1;
                if is_loopback {
                    return Ok(Some(true));
                }
                tracing::error!(
                    "engine pid {} bound non-loopback IPv6 listener on port {} (windows)",
                    pid,
                    port
                );
                return Ok(Some(false));
            }
        } else {
            let table = buf.as_ptr() as *const MIB_TCPTABLE_OWNER_PID;
            let n = unsafe { (*table).dwNumEntries } as isize;
            let rows = unsafe { core::ptr::addr_of!((*table).table) } as *const MIB_TCPROW_OWNER_PID;
            for i in 0..n {
                let row = unsafe { &*rows.offset(i) };
                if row.dwOwningPid != want_pid || decode_port(row.dwLocalPort) != port {
                    continue;
                }
                // dwLocalAddr is the IPv4 s_addr in network byte order; on a
                // little-endian host its native bytes ARE the dotted quad, so
                // 127.0.0.1 → [127, 0, 0, 1]. Matches the Linux impl (exact
                // 127.0.0.1; 0.0.0.0 and any other addr are rejected).
                let is_loopback = row.dwLocalAddr.to_ne_bytes() == [127u8, 0, 0, 1];
                if is_loopback {
                    return Ok(Some(true));
                }
                tracing::error!(
                    "engine pid {} bound non-loopback IPv4 listener {}.{}.{}.{} on port {} (windows)",
                    pid,
                    row.dwLocalAddr.to_ne_bytes()[0],
                    row.dwLocalAddr.to_ne_bytes()[1],
                    row.dwLocalAddr.to_ne_bytes()[2],
                    row.dwLocalAddr.to_ne_bytes()[3],
                    port
                );
                return Ok(Some(false));
            }
        }
        // Enumerated cleanly, no matching listener on the port.
        Ok(None)
    }

    #[cfg(not(any(
        target_os = "linux",
        target_os = "windows",
        target_os = "macos"
    )))]
    pub(crate) fn verify_loopback_bind(_pid: i32, _port: i32) -> bool {
        // Best-effort — the spawn args already force --host 127.0.0.1.
        true
    }

    /// Capture logs from process output
    async fn capture_logs(
        model_id: Uuid,
        child: &mut Child,
        processes: Arc<RwLock<HashMap<Uuid, ProcessInfo>>>,
    ) {
        if let Some(stdout) = child.stdout.take() {
            let processes_clone = processes.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stdout);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let mut procs = processes_clone.write().await;
                    if let Some(proc_info) = procs.get_mut(&model_id) {
                        // P2: fan out to both the snapshot VecDeque
                        // (for /logs) and the broadcaster (for SSE).
                        // broadcast::send is non-blocking and drops
                        // when buffer fills, so a slow subscriber
                        // doesn't backpressure capture.
                        let _ = proc_info.log_broadcast.send(line.clone());
                        push_capped(&mut proc_info.logs, line);
                    }
                }
            });
        }

        if let Some(stderr) = child.stderr.take() {
            let processes_clone = processes.clone();
            tokio::spawn(async move {
                let reader = BufReader::new(stderr);
                let mut lines = reader.lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let line = format!("[stderr] {}", line);
                    let mut procs = processes_clone.write().await;
                    if let Some(proc_info) = procs.get_mut(&model_id) {
                        let _ = proc_info.log_broadcast.send(line.clone());
                        push_capped(&mut proc_info.logs, line);
                    }
                }
            });
        }
    }



}

/// Push `flag VALUE` onto the argv when `v` is `Some` (numeric/float
/// settings — `Display` renders the value).
fn push_opt<T: ToString>(a: &mut Vec<String>, flag: &str, v: Option<T>) {
    if let Some(v) = v {
        a.push(flag.to_string());
        a.push(v.to_string());
    }
}

/// Push a valueless `flag` when the bool setting is `Some(true)`.
fn push_bool_flag(a: &mut Vec<String>, flag: &str, on: Option<bool>) {
    if on == Some(true) {
        a.push(flag.to_string());
    }
}

/// Push `flag VALUE` for a user-supplied string after argv-injection
/// validation (08-llm-local-runtime F-02). No-op when `None`.
fn push_str_arg(
    a: &mut Vec<String>,
    flag: &str,
    label: &str,
    v: Option<&String>,
) -> AppResult<()> {
    if let Some(v) = v {
        LocalDeployment::validate_argv_value(label, v)?;
        a.push(flag.to_string());
        a.push(v.clone());
    }
    Ok(())
}

/// Push a log line with both line-size and ring-buffer caps. Closes
/// 08-llm-local-runtime F-08 (Medium).
fn push_capped(buf: &mut std::collections::VecDeque<String>, mut line: String) {
    if line.len() > LOG_LINE_MAX_BYTES {
        // Truncate at a UTF-8 boundary just under the cap.
        let mut end = LOG_LINE_MAX_BYTES;
        while !line.is_char_boundary(end) && end > 0 {
            end -= 1;
        }
        line.truncate(end);
        line.push_str("…[truncated]");
    }
    while buf.len() >= LOG_BUFFER_MAX_LINES {
        buf.pop_front();
    }
    buf.push_back(line);
}

#[async_trait::async_trait]
impl Deployment for LocalDeployment {
    async fn start(
        &self,
        model_id: Uuid,
        engine_type: &str,
        model_path: &str,
        config: &serde_json::Value,
    ) -> AppResult<DeploymentResult> {
        // Check if already running
        {
            let processes = self.processes.read().await;
            if processes.contains_key(&model_id) {
                return Err(AppError::conflict("Model instance already running"));
            }
        }

        // Validate model_path before it flows into engine argv.
        // Closes 08-llm-local-runtime F-02 (High): model.name (which
        // becomes model_path in handlers.rs) is admin-uploaded and
        // unvalidated; without this check, a name like `--exec ...`
        // is parsed by some engines as an additional flag.
        Self::validate_argv_value("model_path", model_path)?;

        // Concurrent-engine quota. Closes 08-llm-local-runtime F-07
        // (Medium): without this, an admin (or an automated client
        // hitting the start endpoint in a loop) can spin up dozens of
        // local engines and OOM the host. 8 matches a typical
        // workstation's GPU count + the per-model VRAM ceiling.
        // Operators with bigger boxes can raise this via a future
        // config; the hardcoded value below is the safe ceiling.
        const MAX_CONCURRENT_ENGINES: usize = 8;
        {
            let processes = self.processes.read().await;
            if processes.len() >= MAX_CONCURRENT_ENGINES {
                return Err(AppError::bad_request(
                    "TOO_MANY_INSTANCES",
                    format!(
                        "{} engine instances are already running (cap {}); stop one before starting another",
                        processes.len(),
                        MAX_CONCURRENT_ENGINES
                    ),
                ));
            }
        }

        // Find available port
        let port = Self::find_available_port().await?;
        let base_url = format!("http://127.0.0.1:{}", port);

        // Normalize engine type
        let normalized_engine = match engine_type.to_lowercase().as_str() {
            "llamacpp" | "llama.cpp" => "llamacpp",
            "mistralrs" | "mistral.rs" => "mistralrs",
            _ => {
                return Err(AppError::bad_request(
                    "UNSUPPORTED_ENGINE",
                    format!("Unsupported engine type: {}", engine_type),
                ))
            }
        };

        // Resolve binary version: try system default, fall back to latest
        let runtime_version = self
            .binary_manager
            .get_system_default(normalized_engine)
            .await
            .map_err(|e| AppError::internal_with_id(e))?
            .or_else(|| {
                // Fallback: try to get latest version (blocking)
                let binary_manager = self.binary_manager.clone();
                let engine = normalized_engine.to_string();
                tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current().block_on(async {
                        binary_manager.get_latest_version(&engine).await.ok().flatten()
                    })
                })
            })
            .ok_or_else(|| {
                AppError::internal_error(format!(
                    "No runtime version available for engine '{}'. Please download a version first.",
                    normalized_engine
                ))
            })?;

        // Get binary path
        let binary_path = self
            .binary_manager
            .get_binary_path(runtime_version.id)
            .await
            .map_err(|e| AppError::internal_with_id(e))?;

        tracing::info!(
            "Using runtime version: {} {} ({})",
            runtime_version.engine,
            runtime_version.version,
            runtime_version.id
        );

        // Mint a per-instance bearer token. Stored in the
        // process-global INSTANCE_API_KEYS map so chat-side code can
        // look it up via get_instance_api_key(model_id) when
        // dispatching to the local engine. Closes
        // 08-llm-local-runtime F-04 (High) for llama.cpp; chat-side
        // wiring is the follow-up that actually presents the bearer.
        // 256-bit CSPRNG token (matches the proxy's PROXY_TOKEN), not
        // a 122-bit UUID — the bearer gates the engine's HTTP surface.
        let api_key = crate::modules::llm_local_runtime::proxy::generate_proxy_token();
        // NOTE: the token is registered in INSTANCE_API_KEYS only AFTER a
        // successful spawn (below) — registering it here would leave a stale
        // bearer in the process-global map whenever argv-building or spawn
        // fails.

        // Build the engine argv from the model's typed engine_settings,
        // then assemble + harden the command.
        let args = match normalized_engine {
            "llamacpp" => {
                let s = Self::parse_llamacpp_settings(config);
                // Embedder models inject a top-level `embeddings: true`
                // (from capabilities) into the config in resolve_model_inputs.
                let embeddings = config
                    .get("embeddings")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                Self::llamacpp_argv(model_path, port, &s, &api_key, embeddings)?
            }
            "mistralrs" => {
                let s = Self::parse_mistralrs_settings(config);
                Self::mistralrs_argv(model_path, port, &s)?
            }
            _ => unreachable!(), // Already validated above
        };
        let mut cmd = Command::new(&binary_path);
        cmd.args(&args);
        Self::apply_hardening(&mut cmd);

        // Spawn the process
        let mut child = cmd.spawn().map_err(|e| {
            AppError::internal_with_id(e)
        })?;

        // Register the per-instance bearer only after the spawn succeeds, so an
        // argv-build or spawn failure above doesn't leave a stale token mapped
        // for a model that isn't actually running. Chat-side code looks it up
        // via get_instance_api_key(model_id).
        INSTANCE_API_KEYS
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(model_id, api_key.clone());

        let pid = child
            .id()
            .ok_or_else(|| AppError::internal_error("Failed to get process ID"))?
            as i32;

        // Start log capture
        Self::capture_logs(model_id, &mut child, self.processes.clone()).await;

        // Store process info. The broadcaster capacity 256 is plenty
        // for live tail UIs; messages are dropped (not blocked) when
        // a subscriber falls behind.
        let (log_broadcast, _) = tokio::sync::broadcast::channel::<String>(256);
        let proc_info = ProcessInfo {
            child,
            port,
            started_at: std::time::Instant::now(),
            logs: std::collections::VecDeque::new(),
            log_broadcast,
        };

        {
            let mut processes = self.processes.write().await;
            processes.insert(model_id, proc_info);
        }

        Ok(DeploymentResult {
            pid,
            port,
            base_url,
        })
    }

    async fn stop(&self, model_id: Uuid) -> AppResult<()> {
        // Drop the per-instance bearer token. Closes
        // 08-llm-local-runtime F-04 (High) — keeping the token alive
        // past process death would let a future model_id collision
        // accidentally reuse it.
        INSTANCE_API_KEYS
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&model_id);

        let mut processes = self.processes.write().await;

        let mut proc_info = processes
            .remove(&model_id)
            .ok_or_else(|| AppError::not_found("Process not found"))?;

        // Try graceful shutdown first
        if let Err(e) = proc_info.child.kill().await {
            tracing::warn!("Failed to kill process for model {}: {}", model_id, e);
        }

        // Wait for process to exit (with timeout)
        match tokio::time::timeout(
            std::time::Duration::from_secs(10),
            proc_info.child.wait(),
        )
        .await
        {
            Ok(Ok(_)) => {
                tracing::info!("Process for model {} stopped gracefully", model_id);
            }
            Ok(Err(e)) => {
                tracing::warn!("Error waiting for process {}: {}", model_id, e);
            }
            Err(_) => {
                tracing::warn!("Process {} did not stop within timeout", model_id);
            }
        }

        Ok(())
    }

    async fn status(&self, model_id: Uuid) -> AppResult<InstanceStatus> {
        let processes = self.processes.read().await;

        if let Some(proc_info) = processes.get(&model_id) {
            let uptime = proc_info.started_at.elapsed().as_secs() as i64;

            // Try to get actual PID (may have changed or process may have died)
            let pid = proc_info.child.id().map(|id| id as i32);

            Ok(InstanceStatus {
                running: pid.is_some(),
                pid,
                port: Some(proc_info.port),
                uptime_seconds: Some(uptime),
            })
        } else {
            Ok(InstanceStatus {
                running: false,
                pid: None,
                port: None,
                uptime_seconds: None,
            })
        }
    }

    async fn health_check(&self, base_url: &str) -> AppResult<bool> {
        // Try to make a health check request to the server
        let health_url = format!("{}/health", base_url);

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .map_err(|e| AppError::internal_with_id(e))?;

        match client.get(&health_url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => {
                // Try root endpoint as fallback
                match client.get(base_url).send().await {
                    Ok(response) => Ok(response.status().is_success()),
                    Err(_) => Ok(false),
                }
            }
        }
    }

    async fn get_logs(&self, model_id: Uuid, lines: usize) -> AppResult<Vec<String>> {
        let processes = self.processes.read().await;

        if let Some(proc_info) = processes.get(&model_id) {
            let total_lines = proc_info.logs.len();
            let start_index = total_lines.saturating_sub(lines);
            Ok(proc_info.logs.iter().skip(start_index).cloned().collect())
        } else {
            Err(AppError::not_found("Process not found"))
        }
    }

    /// P2: Subscribe to live logs. Unlike the trait default (which hands
    /// back a closed receiver), this returns a live subscription to the
    /// running process's `log_broadcast` sender plus a snapshot of the
    /// already-captured buffer for initial replay, so the SSE endpoint
    /// streams new lines as `capture_logs` emits them.
    async fn subscribe_logs(
        &self,
        model_id: Uuid,
    ) -> AppResult<(tokio::sync::broadcast::Receiver<String>, Vec<String>)> {
        let processes = self.processes.read().await;

        if let Some(proc_info) = processes.get(&model_id) {
            let snapshot: Vec<String> = proc_info.logs.iter().cloned().collect();
            let rx = proc_info.log_broadcast.subscribe();
            Ok((rx, snapshot))
        } else {
            Err(AppError::not_found("Process not found"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(a: &[String], flag: &str, val: &str) -> bool {
        a.windows(2).any(|w| w[0] == flag && w[1] == val)
    }
    fn has(a: &[String], flag: &str) -> bool {
        a.iter().any(|x| x == flag)
    }

    #[test]
    fn llamacpp_argv_maps_full_vocabulary() {
        let s = LlamaCppSettings {
            ctx_size: Some(4096),
            batch_size: Some(256),
            ubatch_size: Some(128),
            parallel: Some(4),
            keep: Some(64),
            mlock: Some(true),
            no_mmap: Some(true),
            threads: Some(8),
            threads_batch: Some(6),
            cont_batching: Some(true),
            no_kv_offload: Some(true),
            n_gpu_layers: Some(33),
            main_gpu: Some(1),
            cache_type_k: Some("q8_0".into()),
            cache_type_v: Some("q8_0".into()),
            rope_freq_base: Some(10000.0),
            seed: Some(42),
            numa: Some("distribute".into()),
            ..Default::default()
        };
        let a = LocalDeployment::llamacpp_argv("/m/x.gguf", 18080, &s, "tok", false).unwrap();
        // Forced hardening flags.
        assert!(pair(&a, "--model", "/m/x.gguf"));
        assert!(pair(&a, "--host", "127.0.0.1"));
        assert!(pair(&a, "--api-key", "tok"));
        // Newly-wired flags.
        assert!(pair(&a, "--ctx-size", "4096"));
        assert!(pair(&a, "--batch-size", "256"));
        assert!(pair(&a, "--ubatch-size", "128"));
        assert!(pair(&a, "--parallel", "4"));
        assert!(pair(&a, "--keep", "64"));
        assert!(has(&a, "--mlock"));
        assert!(has(&a, "--no-mmap"));
        assert!(pair(&a, "--threads", "8"));
        assert!(pair(&a, "--threads-batch", "6"));
        assert!(has(&a, "--cont-batching"));
        assert!(has(&a, "--no-kv-offload"));
        assert!(pair(&a, "--n-gpu-layers", "33"));
        assert!(pair(&a, "--main-gpu", "1"));
        assert!(pair(&a, "--cache-type-k", "q8_0"));
        assert!(pair(&a, "--cache-type-v", "q8_0"));
        assert!(pair(&a, "--rope-freq-base", "10000"));
        assert!(pair(&a, "--seed", "42"));
        assert!(pair(&a, "--numa", "distribute"));
        // Unset optionals are absent.
        let joined = a.join(" ");
        assert!(!joined.contains("--rope-freq-scale"));
        assert!(!joined.contains("--embeddings"));
        assert!(!joined.contains("--flash-attn"));
    }

    #[test]
    fn llamacpp_argv_flash_attn_on_off_and_embeddings() {
        let on = LlamaCppSettings { flash_attn: Some(true), ..Default::default() };
        let a = LocalDeployment::llamacpp_argv("/m/x.gguf", 1, &on, "t", true).unwrap();
        assert!(pair(&a, "--flash-attn", "on"));
        assert!(has(&a, "--embeddings")); // driven by the capabilities arg
        let off = LlamaCppSettings { flash_attn: Some(false), ..Default::default() };
        let b = LocalDeployment::llamacpp_argv("/m/x.gguf", 1, &off, "t", false).unwrap();
        assert!(pair(&b, "--flash-attn", "off"));
        assert!(!b.join(" ").contains("--embeddings"));
    }

    #[test]
    fn llamacpp_argv_cpu_device_forces_zero_gpu_layers() {
        // device_type=Cpu must win even if n_gpu_layers is set.
        let s = LlamaCppSettings {
            device_type: Some(DeviceType::Cpu),
            n_gpu_layers: Some(99),
            ..Default::default()
        };
        let a = LocalDeployment::llamacpp_argv("/m/x.gguf", 1, &s, "t", false).unwrap();
        assert!(pair(&a, "--n-gpu-layers", "0"));
        assert!(!pair(&a, "--n-gpu-layers", "99"));
        assert!(!a.join(" ").contains("--device"));
    }

    #[test]
    fn llamacpp_argv_cuda_device_ids() {
        let s = LlamaCppSettings {
            device_type: Some(DeviceType::Cuda),
            device_ids: Some(vec![0, 1]),
            ..Default::default()
        };
        let a = LocalDeployment::llamacpp_argv("/m/x.gguf", 1, &s, "t", false).unwrap();
        assert!(pair(&a, "--device", "CUDA0,CUDA1"));
    }

    #[test]
    fn llamacpp_argv_rejects_argv_injection() {
        let s = LlamaCppSettings {
            cache_type_k: Some("-malicious".into()),
            ..Default::default()
        };
        assert!(LocalDeployment::llamacpp_argv("/m/x.gguf", 1, &s, "t", false).is_err());
    }

    #[test]
    fn mistralrs_argv_always_loopback_and_gguf_subcommand() {
        let s = MistralRsSettings {
            max_seqs: Some(32),
            prefix_cache_n: Some(16),
            paged_attn_gpu_mem: Some(4096),
            ..Default::default()
        };
        let a = LocalDeployment::mistralrs_argv("/models/qwen/q.gguf", 18081, &s).unwrap();
        let joined = a.join(" ");
        // SECURITY: loopback always forced.
        assert!(pair(&a, "--serve-ip", "127.0.0.1"));
        assert!(pair(&a, "--port", "18081"));
        assert!(pair(&a, "--max-seqs", "32"));
        assert!(pair(&a, "--prefix-cache-n", "16"));
        // PagedAttention uses the current `--pa-*` names, NOT `--paged-attn-*`.
        assert!(pair(&a, "--pa-gpu-mem", "4096"));
        assert!(!joined.contains("--paged-attn-gpu-mem"));
        assert!(!has(&a, "--cpu"));
        // gguf subcommand, path decomposed.
        assert!(has(&a, "gguf"));
        assert!(pair(&a, "--quantized-model-id", "/models/qwen"));
        assert!(pair(&a, "--quantized-filename", "q.gguf"));
    }

    #[test]
    fn mistralrs_argv_plain_subcommand_cpu_dtype_arch() {
        let s = MistralRsSettings {
            command: Some(MistralRsCommand::Plain),
            cpu: Some(true),
            dtype: Some("bf16".into()),
            arch: Some("llama".into()),
            no_kv_cache: Some(true),
            ..Default::default()
        };
        let a = LocalDeployment::mistralrs_argv("/models/llama-dir", 1, &s).unwrap();
        let joined = a.join(" ");
        assert!(has(&a, "--cpu"));
        assert!(has(&a, "--no-kv-cache"));
        assert!(has(&a, "plain"));
        assert!(pair(&a, "--model-id", "/models/llama-dir"));
        assert!(pair(&a, "--dtype", "bf16"));
        assert!(pair(&a, "--arch", "llama"));
        assert!(!joined.contains("gguf"));
    }

    #[test]
    fn mistralrs_argv_excludes_hardening_and_deferred_fields() {
        let s = MistralRsSettings {
            serve_ip: Some("0.0.0.0".into()),
            log_file: Some("/tmp/x.log".into()),
            token_source: Some("env".into()),
            chat_template: Some("/t.jinja".into()),
            interactive_mode: Some(true),
            max_seq_len: Some(4096),
            truncate_sequence: Some(true),
            prompt_chunksize: Some(256),
            num_device_layers: Some(vec!["0:16".into()]),
            ..Default::default()
        };
        let a = LocalDeployment::mistralrs_argv("/m/q.gguf", 1, &s).unwrap();
        let joined = a.join(" ");
        // serve_ip is forced to loopback, never the user value.
        assert!(pair(&a, "--serve-ip", "127.0.0.1"));
        assert!(!joined.contains("0.0.0.0"));
        // Hardening-excluded flags never appear.
        assert!(!joined.contains("--log"));
        assert!(!joined.contains("--token-source"));
        assert!(!joined.contains("--chat-template"));
        assert!(!joined.contains("--interactive-mode"));
        // Deferred (verify-before-enabling) flags stay unmapped.
        assert!(!joined.contains("--max-seq-len"));
        assert!(!joined.contains("--truncate-sequence"));
        assert!(!joined.contains("--prompt-chunksize"));
        assert!(!joined.contains("--num-device-layers"));
    }

    #[test]
    fn parse_llamacpp_settings_reads_nested_shape() {
        let cfg = serde_json::json!({ "llamacpp": { "ctx_size": 2048, "n_gpu_layers": 10 } });
        let s = LocalDeployment::parse_llamacpp_settings(&cfg);
        assert_eq!(s.ctx_size, Some(2048));
        assert_eq!(s.n_gpu_layers, Some(10));
    }

    #[test]
    fn parse_llamacpp_settings_falls_back_on_out_of_range() {
        let cfg = serde_json::json!({ "llamacpp": { "ctx_size": 999_999_999i64 } });
        let s = LocalDeployment::parse_llamacpp_settings(&cfg);
        // validate() fails (>131072) → defaults (all None).
        assert_eq!(s.ctx_size, None);
    }

    #[test]
    fn parse_mistralrs_settings_reads_nested_shape() {
        let cfg = serde_json::json!({ "mistralrs": { "max_seqs": 128, "dtype": "bf16" } });
        let s = LocalDeployment::parse_mistralrs_settings(&cfg);
        assert_eq!(s.max_seqs, Some(128));
        assert_eq!(s.dtype.as_deref(), Some("bf16"));
    }

    #[test]
    fn parse_ignores_flat_legacy_keys() {
        // The pre-unification flat shape no longer parses into a branch;
        // such rows degrade to defaults (documents the shape change).
        let cfg = serde_json::json!({ "ctx_size": 2048 });
        let s = LocalDeployment::parse_llamacpp_settings(&cfg);
        assert_eq!(s.ctx_size, None);
    }

    /// Exercises the real macOS `proc_pidfdinfo` socket enumeration in
    /// this test process: a `127.0.0.1` listener must verify, a `0.0.0.0`
    /// listener must be rejected, and a port nobody listens on must pass
    /// (absence → true, mirroring the Linux impl). This also validates the
    /// hand-computed struct byte offsets on the running kernel — a wrong
    /// offset/byte-order makes the wildcard socket read as "no listener"
    /// and fails the rejection assertion below.
    #[cfg(target_os = "macos")]
    /// Exercises the real Windows `GetExtendedTcpTable` enumeration in this
    /// test process: a `127.0.0.1` listener must verify, a `0.0.0.0` listener
    /// must be rejected as non-loopback, and a port nobody listens on must
    /// pass (absence → true, mirroring the Linux impl). Because the row is
    /// filtered by owning PID, the in-process listeners (owned by this test)
    /// are exactly the ones the function inspects.
    #[cfg(target_os = "windows")]
    #[test]
    fn verify_loopback_bind_detects_loopback_vs_wildcard() {
        use std::net::TcpListener;
        let pid = std::process::id() as i32;

        // 127.0.0.1 listener → must be accepted (held open across the call).
        let lo = TcpListener::bind("127.0.0.1:0").expect("bind loopback");
        let lo_port = lo.local_addr().unwrap().port() as i32;
        assert!(
            LocalDeployment::verify_loopback_bind(pid, lo_port),
            "loopback listener on port {lo_port} should verify as bound to 127.0.0.1"
        );

        // 0.0.0.0 (wildcard) listener → must be rejected as non-loopback.
        let any = TcpListener::bind("0.0.0.0:0").expect("bind wildcard");
        let any_port = any.local_addr().unwrap().port() as i32;
        assert!(
            !LocalDeployment::verify_loopback_bind(pid, any_port),
            "wildcard 0.0.0.0 listener on port {any_port} must be rejected as non-loopback"
        );

        // Absence: no listener in this process on this port → true (the
        // /health probe is authoritative; we don't fail validation here).
        drop(lo);
        drop(any);
        assert!(
            LocalDeployment::verify_loopback_bind(pid, 9),
            "no listener on port 9 in this process → should return true (absence)"
        );
    }
}
