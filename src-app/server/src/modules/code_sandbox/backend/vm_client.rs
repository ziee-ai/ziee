//! Host-side client for the in-guest `ziee-sandbox-agent`, shared by the macOS
//! (libkrun → vsock-bridged unix socket) and Windows (WSL2 → localhost TCP)
//! backends. Sends one `ExecRequest` over an already-connected stream and
//! collects the streamed `Stdout`/`Stderr`/`Exit` frames into a
//! `SandboxRunResult`, applying the same 1 MiB output cap as the Linux backend
//! and a host-side read-timeout backstop (a wedged agent can't hang the turn).

use std::time::{Duration, Instant};

use sandbox_vm_protocol::{encode, Decoder, ExecRequest, Frame};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::common::AppError;
use crate::modules::code_sandbox::sandbox::{SandboxRunResult, OUTPUT_CAP_BYTES};

/// Run one command on a connected control stream. Transport-agnostic: the
/// caller (mac unix socket / WSL2 TCP) connects and hands the stream in.
pub async fn run_on_stream<S>(
    mut stream: S,
    req: ExecRequest,
    timeout_secs: u64,
) -> Result<SandboxRunResult, AppError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let started = Instant::now();
    stream
        .write_all(&encode(&Frame::Exec(req)))
        .await
        .map_err(|e| AppError::internal_error(format!("send exec to guest: {e}")))?;

    let mut decoder = Decoder::new();
    let mut buf = vec![0u8; 64 * 1024];
    let mut stdout: Vec<u8> = Vec::new();
    let mut stderr: Vec<u8> = Vec::new();
    let mut stdout_truncated = false;
    let mut stderr_truncated = false;
    let mut exit_code = -1;
    let mut timed_out = false;

    // The agent enforces the per-exec timeout in-guest and should always send
    // Exit, but if it wedges, bound the host wait at the exec budget + grace.
    let read_budget = Duration::from_secs(timeout_secs + 30);
    loop {
        let n = match tokio::time::timeout(read_budget, stream.read(&mut buf)).await {
            Ok(Ok(n)) => n,
            Ok(Err(e)) => return Err(AppError::internal_error(format!("read guest stream: {e}"))),
            Err(_) => {
                timed_out = true;
                break;
            }
        };
        if n == 0 {
            break; // stream closed
        }
        decoder.feed(&buf[..n]);
        let mut done = false;
        loop {
            match decoder.next_frame() {
                Ok(Some(Frame::Stdout(b))) => append_capped(&mut stdout, &b, &mut stdout_truncated),
                Ok(Some(Frame::Stderr(b))) => append_capped(&mut stderr, &b, &mut stderr_truncated),
                Ok(Some(Frame::Exit(s))) => {
                    exit_code = s.code;
                    timed_out = s.timed_out;
                    done = true;
                    break;
                }
                Ok(Some(Frame::Exec(_))) => {} // not expected from the guest
                Ok(None) => break,
                Err(e) => return Err(AppError::internal_error(format!("guest protocol error: {e}"))),
            }
        }
        if done {
            break;
        }
    }

    Ok(SandboxRunResult {
        exit_code,
        stdout: lossy(stdout, stdout_truncated),
        stderr: lossy(stderr, stderr_truncated),
        stdout_truncated,
        stderr_truncated,
        duration_ms: started.elapsed().as_millis() as u64,
        timed_out,
    })
}

fn append_capped(buf: &mut Vec<u8>, chunk: &[u8], truncated: &mut bool) {
    if *truncated {
        return;
    }
    if buf.len() + chunk.len() > OUTPUT_CAP_BYTES {
        let remain = OUTPUT_CAP_BYTES - buf.len();
        buf.extend_from_slice(&chunk[..remain]);
        *truncated = true;
    } else {
        buf.extend_from_slice(chunk);
    }
}

fn lossy(buf: Vec<u8>, truncated: bool) -> String {
    let mut s = String::from_utf8_lossy(&buf).into_owned();
    if truncated {
        s.push_str(&format!("\n[output truncated at {OUTPUT_CAP_BYTES} bytes]\n"));
    }
    s
}
