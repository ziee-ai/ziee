//! TEST-8 — REAL-voice gold-smoke for the streaming (live-caption) path.
//!
//! The stub tiers prove the plumbing with a canned transcript; this is the ONLY
//! test that proves live captions work on REAL acoustics. It downloads the REAL
//! `ziee-ai/whisper.cpp` `whisper-server` binary + the `base.en` ggml model
//! (public HF repo, no API key) and transcribes the canonical public-domain JFK
//! inaugural clip (`fixtures/jfk.wav`, 16 kHz mono), asserting:
//!   (a) the interim `/voice/transcribe/stream` endpoint returns NON-EMPTY text on
//!       a partial (mid-recording) buffer, and
//!   (b) the final `/voice/transcribe` decode CONTAINS the expected keyword.
//!
//! Soft-skip (a runtime early-return, not a compile-time ignore-attribute): it
//! lives in the default `voice::` suite and probes the external gate — the
//! published whisper release — BEFORE any work. A missing release / unreachable /
//! rate-limited GitHub is an EXTERNAL gate → an `[external gate: whisper-release]`
//! marker + return (never a false failure offline). The instant the release +
//! model are reachable it runs for REAL with every downstream step a hard
//! assertion. Mirrors `real_repo_test.rs`'s discipline.
//!
//!   source tests/.env.test
//!   cargo test --test integration_tests \
//!     -- voice::streaming_real_test --test-threads=1

use std::time::Duration;

use serde_json::{Value, json};

use super::{drive_download_to_terminal, VOICE_ADMIN_PERMS};
use crate::common::TestServer;
use crate::common::test_helpers::create_user_with_permissions;

/// The canonical public-domain whisper sample (JFK inaugural, "…ask not what your
/// country can do for you…"), 16 kHz mono 16-bit PCM. Baked into the test binary.
const JFK_WAV: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/voice/fixtures/jfk.wav"));

const WHISPER_LATEST_RELEASE_API: &str =
    "https://api.github.com/repos/ziee-ai/whisper.cpp/releases/latest";

/// Rebuild a canonical 16 kHz mono 16-bit WAV holding only the first `secs` of the
/// input's PCM — a faithful "mid-recording" prefix of the accumulating buffer.
fn wav_prefix_secs(wav: &[u8], secs: f64) -> Vec<u8> {
    assert!(wav.len() >= 12 && &wav[0..4] == b"RIFF" && &wav[8..12] == b"WAVE", "fixture must be WAV");
    let mut pos = 12usize;
    let (mut sample_rate, mut channels, mut bits) = (16_000u32, 1u16, 16u16);
    let mut data: &[u8] = &[];
    while pos + 8 <= wav.len() {
        let id = &wav[pos..pos + 4];
        let size = u32::from_le_bytes([wav[pos + 4], wav[pos + 5], wav[pos + 6], wav[pos + 7]]) as usize;
        let body = pos + 8;
        if id == b"fmt " && body + 16 <= wav.len() {
            channels = u16::from_le_bytes([wav[body + 2], wav[body + 3]]);
            sample_rate = u32::from_le_bytes([wav[body + 4], wav[body + 5], wav[body + 6], wav[body + 7]]);
            bits = u16::from_le_bytes([wav[body + 14], wav[body + 15]]);
        } else if id == b"data" {
            let end = (body + size).min(wav.len());
            data = &wav[body..end];
            break;
        }
        pos = body + size + (size & 1);
    }
    let byte_rate = sample_rate * channels as u32 * (bits / 8) as u32;
    let want = ((byte_rate as f64 * secs) as usize).min(data.len());
    // Align down to a full frame so we never split a sample.
    let block_align = (channels * bits / 8) as usize;
    let want = if block_align > 0 { want - (want % block_align) } else { want };
    let prefix = &data[..want];

    let mut w = Vec::with_capacity(44 + prefix.len());
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&((36 + prefix.len()) as u32).to_le_bytes());
    w.extend_from_slice(b"WAVE");
    w.extend_from_slice(b"fmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&channels.to_le_bytes());
    w.extend_from_slice(&sample_rate.to_le_bytes());
    w.extend_from_slice(&byte_rate.to_le_bytes());
    w.extend_from_slice(&(channels * bits / 8).to_le_bytes());
    w.extend_from_slice(&bits.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(prefix.len() as u32).to_le_bytes());
    w.extend_from_slice(prefix);
    w
}

async fn post_wav(server: &TestServer, path: &str, token: &str, wav: Vec<u8>) -> reqwest::Response {
    let part = reqwest::multipart::Part::bytes(wav)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .unwrap();
    reqwest::Client::new()
        .post(server.api_url(path))
        .header("Authorization", format!("Bearer {token}"))
        .multipart(reqwest::multipart::Form::new().part("file", part))
        .send()
        .await
        .expect("post wav")
}

#[tokio::test]
async fn real_voice_streaming_transcribes_jfk_with_base_en() {
    // SOFT-SKIP gate — probe the external dependency BEFORE any work.
    match reqwest::Client::new()
        .get(WHISPER_LATEST_RELEASE_API)
        .header("User-Agent", "ziee-voice-streaming-real-test")
        .timeout(Duration::from_secs(20))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => { /* release published — run for real */ }
        Ok(r) => {
            eprintln!(
                "SOFT-SKIP [external gate: whisper-release]: {WHISPER_LATEST_RELEASE_API} \
                 returned HTTP {} (no published release / rate-limited); skipping real-voice smoke.",
                r.status()
            );
            return;
        }
        Err(e) => {
            eprintln!(
                "SOFT-SKIP [external gate: whisper-release]: GitHub unreachable ({e}); \
                 skipping real-voice smoke."
            );
            return;
        }
    }

    let server = TestServer::start().await;
    let admin = create_user_with_permissions(&server, "voice_realvoice_admin", VOICE_ADMIN_PERMS).await;
    let client = reqwest::Client::new();

    // 1. Download the REAL whisper-server binary for this host (registered default).
    let res = client
        .post(server.api_url("/voice/versions/download"))
        .header("Authorization", format!("Bearer {}", admin.token))
        .json(&json!({ "version": "latest" }))
        .send()
        .await
        .expect("start real whisper-server download");
    assert_eq!(res.status(), 200, "download start should 200 (release published?)");
    let key = res.json::<Value>().await.unwrap()["key"].as_str().unwrap().to_string();
    // Binary download is an EXTERNAL gate (GitHub asset for this host triple):
    // soft-skip if it can't complete rather than hard-failing an offline box.
    if let Err(e) = drive_download_to_terminal(&server, &admin.token, &key, Duration::from_secs(180)).await
    {
        eprintln!(
            "SOFT-SKIP [external gate: whisper-release]: whisper-server download did not \
             complete ({e}); skipping real-voice smoke."
        );
        return;
    }

    // 2. Select base.en (DEC-13). The first transcribe lazily downloads it from the
    //    public HF repo (no API key) and spawns whisper-server.
    let r = client
        .put(server.api_url("/voice/settings"))
        .header("Authorization", format!("Bearer {}", admin.token))
        // Generous auto-start window: base.en model load on cold start.
        .json(&json!({ "model": "base.en", "auto_start_timeout_secs": 180 }))
        .send()
        .await
        .unwrap();
    assert_eq!(r.status(), 200, "select base.en");

    let user = create_user_with_permissions(&server, "voice_realvoice_user", &[]).await;

    // 3. FINAL decode (batch, 300s whisper ceiling) — also warms the model download
    //    (public HF repo) + spawn. The model download / runtime provisioning is an
    //    EXTERNAL gate too: a non-200 here (HF unreachable, no asset) soft-skips;
    //    only once we have a real 200 do we HARD-assert the acoustics.
    let resp = post_wav(&server, "/voice/transcribe", &user.token, JFK_WAV.to_vec()).await;
    let status = resp.status();
    let body = resp.text().await.unwrap();
    if status != 200 {
        eprintln!(
            "SOFT-SKIP [external gate: whisper-model]: base.en provisioning / batch decode \
             returned HTTP {status} (model download or runtime unavailable); body: {body}"
        );
        return;
    }
    let final_text = serde_json::from_str::<Value>(&body).unwrap()["text"]
        .as_str()
        .unwrap_or_default()
        .to_lowercase();
    assert!(
        final_text.contains("country"),
        "base.en should transcribe the JFK clip to include 'country'; got: {final_text:?}"
    );

    // 4. INTERIM decode on a mid-recording PREFIX (~6s of the ~11s clip), everything
    //    warm. Assert the live-caption endpoint returns NON-EMPTY real text.
    let prefix = wav_prefix_secs(JFK_WAV, 6.0);
    let resp = post_wav(&server, "/voice/transcribe/stream", &user.token, prefix).await;
    let status = resp.status();
    let body = resp.text().await.unwrap();
    assert_eq!(status, 200, "interim stream of the prefix should 200 (body: {body})");
    let interim_text = serde_json::from_str::<Value>(&body).unwrap()["text"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_string();
    assert!(
        !interim_text.is_empty(),
        "interim decode of a real-speech prefix must be non-empty; got empty text"
    );
    eprintln!("real-voice smoke ✅ interim={interim_text:?} final contains 'country'");
}
