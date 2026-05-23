//! Host ↔ guest control protocol for the code-sandbox microVM backend
//! (macOS libkrun, and later WSL2). The host (server) builds the **full bwrap
//! argv** with `sandbox::build_bwrap_argv` using *guest* paths and sends it to
//! the in-VM guest agent, which simply execs it and streams the output back —
//! so `build_bwrap_argv` stays the single source of truth for hardening across
//! every backend; the guest agent is a dumb executor.
//!
//! ## Wire format
//!
//! A stream of length-prefixed frames, each: `[u8 tag][u32 BE len][payload]`.
//! Structured frames (`Exec`, `Exit`) carry a JSON payload; output frames
//! (`Stdout`, `Stderr`) carry raw bytes. Tags are unique across both
//! directions so a single decoder serves host and guest.
//!
//! This crate is **pure** (no IO): each side does its own (async) socket IO and
//! uses [`encode`] / [`Decoder`] for framing. That keeps the contract trivially
//! unit-testable and dependency-light.

use serde::{Deserialize, Serialize};

/// Wire-protocol version. Bumped any time the on-wire shape of `Frame` or
/// `ExecRequest` changes in a way that would break an older peer. Host + agent
/// ship together from the same release, so a mismatch always indicates an
/// operator running a stale agent binary against a fresh server (or vice
/// versa). Defense-in-depth — surfaces the mismatch loudly instead of running
/// against undefined semantics.
pub const PROTOCOL_VERSION: u32 = 1;

/// Request to run one command in the sandbox. `argv` is the complete bwrap
/// argv produced by the host (already pointing at *guest* paths for the rootfs
/// mount + workspace); the agent execs `bwrap_path` with it verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecRequest {
    /// Must equal [`PROTOCOL_VERSION`] of the agent that receives it. Older
    /// peers send no field → serde defaults to `0` → mismatch → agent rejects.
    #[serde(default)]
    pub protocol_version: u32,
    /// Correlates the response frames with this request (the agent handles
    /// concurrent requests on separate connections, so this is mostly for logs).
    pub request_id: u64,
    /// Absolute path to `bwrap` inside the guest.
    pub bwrap_path: String,
    /// Full bwrap argv (the output of `build_bwrap_argv`, guest paths). When
    /// `seccomp_fd` is set, the argv already contains `--seccomp <that fd>`.
    pub argv: Vec<String>,
    /// Wall-clock budget; the agent SIGKILLs the bwrap process group on expiry.
    pub timeout_ms: u64,
    /// If set, the agent builds the shared seccomp BPF and pipes it to this fd
    /// in the bwrap child (the argv references it via `--seccomp <fd>`). `None`
    /// → no seccomp (e.g. a host that can't build the guest-arch filter). The
    /// macOS/Windows backends set this so the guest applies the same filter the
    /// Linux host does.
    #[serde(default)]
    pub seccomp_fd: Option<i32>,
    /// If set, the agent creates an in-guest cgroup v2 scope with these limits
    /// and attaches the bwrap process to it (defense-in-depth; the in-argv
    /// prlimit is the always-on backstop). `None` → rely on prlimit only. The
    /// host owns the policy so it stays single-source (and config-driven later).
    #[serde(default)]
    pub cgroup: Option<CgroupLimits>,
}

/// cgroup v2 resource limits the guest agent applies per exec. Values mirror
/// the Linux host's `cgroup::CgroupScope` defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CgroupLimits {
    /// `memory.max` (bytes).
    pub memory_max_bytes: u64,
    /// `memory.swap.max` (bytes; 0 disables swap).
    pub memory_swap_max_bytes: u64,
    /// `pids.max`.
    pub pids_max: u64,
    /// `cpu.max` ("<quota> <period>" in µs; "100000 100000" = 1 CPU).
    pub cpu_max: String,
}

impl CgroupLimits {
    /// The default policy — must match `cgroup::CgroupScope` on the Linux host
    /// (512 MiB / no swap / 256 PIDs / 1 CPU). Will become config-driven with
    /// Plan 1 §6 (runtime-configurable limits).
    pub fn default_policy() -> Self {
        Self {
            memory_max_bytes: 512 * 1024 * 1024,
            memory_swap_max_bytes: 0,
            pids_max: 256,
            cpu_max: "100000 100000".to_string(),
        }
    }
}

/// Terminal status of an `ExecRequest`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExitStatus {
    /// Process exit code (`-1` if killed by signal / timeout).
    pub code: i32,
    /// True if the agent killed it because `timeout_ms` elapsed.
    pub timed_out: bool,
}

/// A single protocol frame, either direction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    /// host → guest: run this command.
    Exec(ExecRequest),
    /// guest → host: a chunk of stdout.
    Stdout(Vec<u8>),
    /// guest → host: a chunk of stderr.
    Stderr(Vec<u8>),
    /// guest → host: the command finished.
    Exit(ExitStatus),
    /// host → guest: clean-shutdown request. The agent acknowledges by exiting
    /// its own process; any in-flight bwrap children then die via the argv's
    /// `--die-with-parent`. Used by the WSL2 backend so a `wsl --terminate`
    /// can be preceded by a clean in-distro stop (without it, the agent can
    /// outlive the Windows-side `wsl.exe` relay because there is no
    /// `PR_SET_PDEATHSIG` across the WSL boundary; [microsoft/WSL#1037]).
    Shutdown,
}

const TAG_EXEC: u8 = 1;
const TAG_STDOUT: u8 = 2;
const TAG_STDERR: u8 = 3;
const TAG_EXIT: u8 = 4;
const TAG_SHUTDOWN: u8 = 5;

/// Header bytes (tag + u32 length) before each payload.
const HEADER_LEN: usize = 1 + 4;

/// Hard cap on a single frame's payload so a corrupt/hostile length prefix
/// can't make a peer allocate unbounded memory. 8 MiB comfortably exceeds any
/// realistic output chunk (the host re-caps total output separately).
pub const MAX_FRAME_PAYLOAD: usize = 8 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum ProtocolError {
    UnknownTag(u8),
    FrameTooLarge(usize),
    BadJson,
}

impl std::fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolError::UnknownTag(t) => write!(f, "unknown frame tag {t}"),
            ProtocolError::FrameTooLarge(n) => write!(f, "frame payload too large ({n} bytes)"),
            ProtocolError::BadJson => write!(f, "malformed JSON frame payload"),
        }
    }
}
impl std::error::Error for ProtocolError {}

/// Encode a frame to its wire bytes.
pub fn encode(frame: &Frame) -> Vec<u8> {
    let (tag, payload): (u8, Vec<u8>) = match frame {
        Frame::Exec(req) => (TAG_EXEC, serde_json::to_vec(req).expect("ExecRequest serializes")),
        Frame::Stdout(b) => (TAG_STDOUT, b.clone()),
        Frame::Stderr(b) => (TAG_STDERR, b.clone()),
        Frame::Exit(s) => (TAG_EXIT, serde_json::to_vec(s).expect("ExitStatus serializes")),
        Frame::Shutdown => (TAG_SHUTDOWN, Vec::new()),
    };
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.push(tag);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(&payload);
    out
}

/// Incremental frame decoder: feed it bytes as they arrive on the socket and
/// pull complete frames out. Holds a buffer of not-yet-consumed bytes.
#[derive(Default)]
pub struct Decoder {
    buf: Vec<u8>,
}

impl Decoder {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    /// Append freshly-read bytes.
    pub fn feed(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Pop the next complete frame, if one is fully buffered. Returns
    /// `Ok(None)` when more bytes are needed.
    pub fn next_frame(&mut self) -> Result<Option<Frame>, ProtocolError> {
        if self.buf.len() < HEADER_LEN {
            return Ok(None);
        }
        let tag = self.buf[0];
        let len = u32::from_be_bytes([self.buf[1], self.buf[2], self.buf[3], self.buf[4]]) as usize;
        if len > MAX_FRAME_PAYLOAD {
            return Err(ProtocolError::FrameTooLarge(len));
        }
        if self.buf.len() < HEADER_LEN + len {
            return Ok(None); // payload not fully arrived yet
        }
        let payload = self.buf[HEADER_LEN..HEADER_LEN + len].to_vec();
        // Drop the consumed header + payload.
        self.buf.drain(..HEADER_LEN + len);

        let frame = match tag {
            TAG_EXEC => Frame::Exec(
                serde_json::from_slice(&payload).map_err(|_| ProtocolError::BadJson)?,
            ),
            TAG_STDOUT => Frame::Stdout(payload),
            TAG_STDERR => Frame::Stderr(payload),
            TAG_EXIT => {
                Frame::Exit(serde_json::from_slice(&payload).map_err(|_| ProtocolError::BadJson)?)
            }
            TAG_SHUTDOWN => Frame::Shutdown,
            other => return Err(ProtocolError::UnknownTag(other)),
        };
        Ok(Some(frame))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_exec() -> Frame {
        Frame::Exec(ExecRequest {
            protocol_version: PROTOCOL_VERSION,
            request_id: 42,
            bwrap_path: "/usr/bin/bwrap".to_string(),
            argv: vec!["--clearenv".into(), "--".into(), "/bin/bash".into(), "-lc".into(), "echo hi".into()],
            timeout_ms: 600_000,
            seccomp_fd: Some(10),
            cgroup: Some(CgroupLimits::default_policy()),
        })
    }

    #[test]
    fn roundtrips_every_frame() {
        for frame in [
            sample_exec(),
            Frame::Stdout(b"hello world".to_vec()),
            Frame::Stderr(vec![0, 1, 2, 255, 254]),
            Frame::Exit(ExitStatus { code: 0, timed_out: false }),
            Frame::Exit(ExitStatus { code: -1, timed_out: true }),
            Frame::Shutdown,
        ] {
            let mut d = Decoder::new();
            d.feed(&encode(&frame));
            let decoded = d.next_frame().unwrap().expect("a full frame");
            assert_eq!(decoded, frame);
            assert!(d.next_frame().unwrap().is_none(), "no trailing frame");
        }
    }

    #[test]
    fn decodes_multiple_frames_from_one_buffer() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&encode(&Frame::Stdout(b"a".to_vec())));
        bytes.extend_from_slice(&encode(&Frame::Stderr(b"b".to_vec())));
        bytes.extend_from_slice(&encode(&Frame::Exit(ExitStatus { code: 7, timed_out: false })));

        let mut d = Decoder::new();
        d.feed(&bytes);
        assert_eq!(d.next_frame().unwrap(), Some(Frame::Stdout(b"a".to_vec())));
        assert_eq!(d.next_frame().unwrap(), Some(Frame::Stderr(b"b".to_vec())));
        assert_eq!(d.next_frame().unwrap(), Some(Frame::Exit(ExitStatus { code: 7, timed_out: false })));
        assert_eq!(d.next_frame().unwrap(), None);
    }

    #[test]
    fn handles_partial_then_completed_frame() {
        let full = encode(&Frame::Stdout(b"chunky".to_vec()));
        let (head, tail) = full.split_at(3); // split mid-header/payload
        let mut d = Decoder::new();
        d.feed(head);
        assert_eq!(d.next_frame().unwrap(), None, "incomplete → None");
        d.feed(tail);
        assert_eq!(d.next_frame().unwrap(), Some(Frame::Stdout(b"chunky".to_vec())));
    }

    #[test]
    fn rejects_oversized_frame() {
        // Craft a header claiming a payload bigger than the cap.
        let mut bytes = vec![TAG_STDOUT];
        bytes.extend_from_slice(&((MAX_FRAME_PAYLOAD as u32) + 1).to_be_bytes());
        let mut d = Decoder::new();
        d.feed(&bytes);
        assert_eq!(d.next_frame(), Err(ProtocolError::FrameTooLarge(MAX_FRAME_PAYLOAD + 1)));
    }

    #[test]
    fn rejects_unknown_tag() {
        let mut bytes = vec![99u8];
        bytes.extend_from_slice(&0u32.to_be_bytes());
        let mut d = Decoder::new();
        d.feed(&bytes);
        assert_eq!(d.next_frame(), Err(ProtocolError::UnknownTag(99)));
    }

    #[test]
    fn legacy_exec_request_decodes_with_version_zero() {
        // A peer that predates PROTOCOL_VERSION sends an ExecRequest with no
        // `protocol_version` field. With `#[serde(default)]` this deserializes
        // to 0 (≠ PROTOCOL_VERSION) so the agent can reject it loudly.
        let legacy_json = serde_json::json!({
            "request_id": 1,
            "bwrap_path": "/usr/bin/bwrap",
            "argv": [],
            "timeout_ms": 0,
        });
        let payload = serde_json::to_vec(&legacy_json).unwrap();
        let mut bytes = vec![TAG_EXEC];
        bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&payload);
        let mut d = Decoder::new();
        d.feed(&bytes);
        let frame = d.next_frame().unwrap().expect("legacy ExecRequest decodes");
        match frame {
            Frame::Exec(req) => assert_eq!(req.protocol_version, 0),
            _ => panic!("expected Exec"),
        }
    }
}
