//! Dynamic proof that the embedded sandbox-runtime extracts and boots
//! a libkrun microVM end-to-end with NO brew packages on the runtime
//! path. We poison `DYLD_LIBRARY_PATH` / `DYLD_FALLBACK_LIBRARY_PATH`
//! to a nonexistent dir so any accidental dlopen of a brew dylib would
//! fail loudly.
//!
//! Mirrors the contract proven by `sandbox-vm-launcher/tests/boot_smoke.rs`
//! but exercises the embedded-bundle extraction path instead of the
//! dev build. `#[ignore]`'d by default: it boots a libkrun VM (~3s) and
//! shells out to Docker for the dummy sandbox squashfs assembly is NOT
//! needed (we reuse the dev cache).
//!
//! Run:
//!   cargo test --release --target aarch64-apple-darwin --test macos_brewless_boot -- --ignored --nocapture

#![cfg(all(target_os = "macos", target_arch = "aarch64"))]

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use ziee::code_sandbox_embedded as embedded;

const SOCK_TIMEOUT: Duration = Duration::from_secs(30);
const READ_TIMEOUT: Duration = Duration::from_secs(15);
const EXEC_TIMEOUT_MS: u64 = 10_000;

// --- frame encoding (kept inline to avoid pulling sandbox-vm-protocol
//     into the server's dep graph just for one test) ---
const TAG_EXEC: u8 = 1;
const TAG_STDOUT: u8 = 2;
const TAG_STDERR: u8 = 3;
const TAG_EXIT: u8 = 4;

fn encode_exec(payload: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(5 + payload.len());
    frame.push(TAG_EXEC);
    frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    frame.extend_from_slice(payload);
    frame
}

#[test]
#[ignore = "boots a libkrun VM; requires the embedded bundle"]
fn embedded_bundle_boots_under_no_brew() {
    assert!(
        embedded::is_supported(),
        "embedded bundle empty — was ZIEE_SKIP_SANDBOX_BUNDLE=1 set during build?"
    );

    // Trigger extraction up front so DYLD env pollution can't affect the
    // codesign/codesign-validation pass inside ensure().
    let extracted = embedded::ensure().expect("extract bundle");
    let launcher = extracted.launcher.clone();
    let guest_root = extracted.guest_root.clone();
    assert!(launcher.exists(), "launcher path missing: {}", launcher.display());
    assert!(guest_root.exists(), "guest-root path missing: {}", guest_root.display());

    // Reuse the dev-test sandbox squashfs + workspace dir if present;
    // otherwise create them. These live under .ziee-cache (gitignored).
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo = std::fs::canonicalize(manifest_dir.join("../..")).expect("repo root");
    let cache_dir = repo.join(".ziee-cache").join("test-validation");
    std::fs::create_dir_all(cache_dir.join("workspace")).expect("mkdir workspace");
    let sandbox_disk = cache_dir.join("dummy-sandbox.sqfs");
    assert!(
        sandbox_disk.exists(),
        "missing test sandbox squashfs at {}. Build it with the dev recipe (see boot_smoke.rs).",
        sandbox_disk.display()
    );

    let sock_path = cache_dir.join("vm-brewless.sock");
    let _ = std::fs::remove_file(&sock_path);
    let cfg_path = cache_dir.join("launch-brewless.json");
    let cfg = format!(
        r#"{{
  "num_vcpus": 1,
  "ram_mib": 256,
  "root_path": {root:?},
  "sandbox_disk_path": {disk:?},
  "workspace_host_path": {ws:?},
  "vsock_socket_path": {sock:?},
  "vsock_port": 1024,
  "agent_exec_path": "/usr/bin/ziee-sandbox-agent"
}}"#,
        root = guest_root.to_string_lossy(),
        disk = sandbox_disk.to_string_lossy(),
        ws = cache_dir.join("workspace").to_string_lossy(),
        sock = sock_path.to_string_lossy(),
    );
    std::fs::write(&cfg_path, cfg).expect("write launch.json");

    // Spawn launcher with brew poisoned. If the bundling didn't fully
    // self-contain things, libkrun/libkrunfw will fail to load.
    let mut child = std::process::Command::new(&launcher)
        .arg(&cfg_path)
        .env("DYLD_LIBRARY_PATH", "/nonexistent")
        .env("DYLD_FALLBACK_LIBRARY_PATH", "/nonexistent")
        .env_remove("HOMEBREW_PREFIX")
        .env_remove("HOMEBREW_CELLAR")
        .env_remove("HOMEBREW_REPOSITORY")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("spawn launcher");

    // Reader thread mirrors agent stderr to ours AND signals readiness.
    let child_stderr = child.stderr.take().expect("piped stderr");
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        use std::io::{BufRead, BufReader};
        let mut signaled = false;
        for line in BufReader::new(child_stderr).lines().map_while(Result::ok) {
            eprintln!("{line}");
            if !signaled && line.contains("listening on vsock port") {
                let _ = ready_tx.send(());
                signaled = true;
            }
        }
    });

    let guard = ChildGuard(child);

    let deadline = Instant::now() + SOCK_TIMEOUT;
    while !sock_path.exists() {
        assert!(
            Instant::now() < deadline,
            "vsock socket {} did not appear within {SOCK_TIMEOUT:?}",
            sock_path.display()
        );
        std::thread::sleep(Duration::from_millis(100));
    }
    ready_rx
        .recv_timeout(SOCK_TIMEOUT)
        .expect("agent did not log 'listening on vsock port' in time");

    let mut stream = UnixStream::connect(&sock_path).expect("connect vsock bridge");
    stream.set_read_timeout(Some(READ_TIMEOUT)).expect("set read timeout");

    let payload = format!(
        r#"{{"protocol_version":1,"request_id":1,"bwrap_path":"/bin/busybox","argv":["echo","hello self-contained"],"timeout_ms":{EXEC_TIMEOUT_MS},"seccomp_fd":null,"cgroup":null}}"#
    );
    stream
        .write_all(&encode_exec(payload.as_bytes()))
        .expect("write Exec");

    let mut buf = [0u8; 64 * 1024];
    let mut accum = Vec::new();
    let mut stdout_buf = Vec::new();
    let mut exit_code: Option<i32> = None;
    while exit_code.is_none() {
        let n = stream.read(&mut buf).expect("read");
        if n == 0 {
            break;
        }
        accum.extend_from_slice(&buf[..n]);
        while accum.len() >= 5 {
            let tag = accum[0];
            let len = u32::from_be_bytes([accum[1], accum[2], accum[3], accum[4]]) as usize;
            if accum.len() < 5 + len {
                break;
            }
            let body = accum[5..5 + len].to_vec();
            accum.drain(..5 + len);
            match tag {
                TAG_STDOUT => stdout_buf.extend_from_slice(&body),
                TAG_STDERR => eprintln!("agent-stderr: {}", String::from_utf8_lossy(&body)),
                TAG_EXIT => {
                    // ExitStatus JSON: {"code": N, "timed_out": bool}
                    let json = String::from_utf8_lossy(&body);
                    exit_code = json
                        .split("\"code\":")
                        .nth(1)
                        .and_then(|s| s.trim_start().split(',').next())
                        .and_then(|s| s.trim().parse().ok());
                }
                other => panic!("unexpected tag {other}"),
            }
        }
    }
    drop(guard); // explicit teardown before assertions

    let stdout = String::from_utf8_lossy(&stdout_buf);
    assert_eq!(exit_code, Some(0), "expected exit 0, got {exit_code:?} (stdout={stdout:?})");
    assert!(
        stdout.contains("hello self-contained"),
        "missing expected stdout. got: {stdout:?}"
    );
}

struct ChildGuard(std::process::Child);
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
