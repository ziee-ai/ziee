//! Wire protocol for the sandbox-helper named pipe.
//!
//! Length-prefixed JSON: a 4-byte little-endian `u32` byte count, then that
//! many bytes of `serde_json`. One request, one response, per connection —
//! no multiplexing needed (calls are rare: once per WSL session for VmId,
//! once at install for registration).

use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

/// Cap on a single framed message. The largest payload is an error string;
/// 64 KiB is comfortably more than any legitimate message and bounds a
/// malformed/hostile length prefix.
const MAX_FRAME: u32 = 64 * 1024;

#[derive(Debug, Serialize, Deserialize)]
pub enum Request {
    /// Liveness probe — used by the client to distinguish "service not
    /// installed" from "service wedged".
    Ping,
    /// Resolve the WSL utility VM's VmId (the privileged `hcsdiag`/HCS call).
    ResolveVmId,
    /// Ensure the `GuestCommunicationServices` GUIDs for
    /// `port_start..port_start+count` are registered. Returns how many were
    /// newly added (callers can decide whether a `wsl --shutdown` is needed).
    EnsureRegistered { port_start: u32, count: u32 },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Response {
    Pong,
    /// VmId in canonical `aabbccdd-eeff-...` form.
    VmId(String),
    /// Count of newly-registered GUIDs (0 = all were already present).
    Registered { added: u32 },
    /// Operation failed; message is human-readable (surfaced in server logs).
    Error(String),
}

/// Write a length-prefixed JSON frame.
pub fn write_frame<W: Write, T: Serialize>(w: &mut W, msg: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec(msg)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let len = u32::try_from(bytes.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "frame too large"))?;
    if len > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame exceeds MAX_FRAME"));
    }
    w.write_all(&len.to_le_bytes())?;
    w.write_all(&bytes)?;
    w.flush()
}

/// Read a length-prefixed JSON frame.
pub fn read_frame<R: Read, T: for<'de> Deserialize<'de>>(r: &mut R) -> io::Result<T> {
    let mut len_buf = [0u8; 4];
    r.read_exact(&mut len_buf)?;
    let len = u32::from_le_bytes(len_buf);
    if len > MAX_FRAME {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "frame exceeds MAX_FRAME"));
    }
    let mut buf = vec![0u8; len as usize];
    r.read_exact(&mut buf)?;
    serde_json::from_slice(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
