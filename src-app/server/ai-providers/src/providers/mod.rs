//! AI provider implementations

pub mod anthropic;
pub mod gemini;
pub mod openai;
mod upload_client;

pub use anthropic::AnthropicProvider;
pub use gemini::GeminiProvider;
pub use openai::OpenAIProvider;

pub(crate) use upload_client::upload_http_client;

use reqwest::Client;
use std::sync::OnceLock;
use std::time::Duration;

/// Hard ceiling on a single provider's unparsed SSE buffer. A compromised or
/// MITM'd provider that streams bytes without ever emitting an event delimiter
/// would otherwise grow the buffer until OOM; we abort the stream instead.
pub(crate) const MAX_SSE_BUFFER_BYTES: usize = 16 * 1024 * 1024;

/// Shared HTTP client for every provider call. Built once (TLS session +
/// connection pool reused process-wide) with hardened defaults:
///
/// - `connect_timeout` bounds "host unreachable" so a blackholed endpoint
///   fails fast instead of hanging on connect (reqwest's default is `None`).
/// - `read_timeout` caps the idle gap *between* bytes. This is the correct
///   primitive for SSE: it kills a hung / slow-loris stream without truncating
///   a legitimately long but actively-streaming generation (a total
///   `.timeout()` would cut off long responses). Sized for slow local
///   cold-start first-token.
/// - `redirect(none)`: provider APIs never legitimately 30x, and reqwest only
///   strips `Authorization` on cross-host redirects — the custom `x-api-key` /
///   `x-goog-api-key` headers would otherwise follow a redirect to an attacker
///   host and leak the key.
///
/// Previously each call did `Client::new()`, which has no timeouts at all (a
/// hung stream pinned a task + socket forever) and discarded the pool.
pub(crate) fn http_client() -> Client {
    static CLIENT: OnceLock<Client> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .read_timeout(Duration::from_secs(600))
                .redirect(reqwest::redirect::Policy::none())
                .build()
                // Builder only fails on TLS-backend init; fall back to a plain
                // client so a provider call can still proceed.
                .unwrap_or_else(|_| Client::new())
        })
        .clone()
}

/// Incremental UTF-8 decoder for chunked SSE byte streams.
///
/// Provider responses arrive as arbitrary byte chunks, and a multi-byte UTF-8
/// character (emoji, CJK, accented Latin, smart quotes) can be split across two
/// network chunks. Decoding each raw chunk independently
/// (`str::from_utf8(&chunk)`) errors on that boundary and aborts the whole
/// response mid-stream. This carries the incomplete trailing bytes forward to
/// the next chunk so only complete characters are surfaced.
#[derive(Default)]
pub(crate) struct Utf8StreamDecoder {
    pending: Vec<u8>,
}

impl Utf8StreamDecoder {
    /// Append `chunk` and return the longest decodable valid-UTF-8 prefix,
    /// holding any incomplete trailing multi-byte sequence for the next call.
    pub(crate) fn decode(&mut self, chunk: &[u8]) -> String {
        self.pending.extend_from_slice(chunk);
        match std::str::from_utf8(&self.pending) {
            Ok(_) => {
                // Whole buffer is valid; emit it and reset.
                String::from_utf8(std::mem::take(&mut self.pending)).unwrap_or_default()
            }
            Err(e) => {
                let valid_up_to = e.valid_up_to();
                let decoded =
                    String::from_utf8_lossy(&self.pending[..valid_up_to]).into_owned();
                match e.error_len() {
                    // Incomplete trailing char split across chunks: keep the tail.
                    None => {
                        self.pending.drain(..valid_up_to);
                        decoded
                    }
                    // Genuinely invalid byte(s): emit the valid prefix + a
                    // replacement char and resync past the bad bytes rather than
                    // abort the stream.
                    Some(bad_len) => {
                        self.pending.drain(..valid_up_to + bad_len);
                        format!("{decoded}\u{FFFD}")
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Utf8StreamDecoder;

    /// A multi-byte char split across two chunks must not error — the trailing
    /// bytes are held and completed on the next chunk.
    #[test]
    fn utf8_decoder_handles_split_multibyte_char() {
        let emoji = "😀".as_bytes(); // 4 bytes: F0 9F 98 80
        let mut d = Utf8StreamDecoder::default();
        let first = d.decode(&[b'h', b'i', emoji[0], emoji[1]]);
        assert_eq!(first, "hi"); // emoji withheld (incomplete)
        let second = d.decode(&[emoji[2], emoji[3], b'!']);
        assert_eq!(second, "😀!");
    }

    /// CJK split mid-character across three chunks (1 byte at a time).
    #[test]
    fn utf8_decoder_handles_byte_at_a_time() {
        let cjk = "中".as_bytes(); // 3 bytes
        let mut d = Utf8StreamDecoder::default();
        assert_eq!(d.decode(&[cjk[0]]), "");
        assert_eq!(d.decode(&[cjk[1]]), "");
        assert_eq!(d.decode(&[cjk[2]]), "中");
    }

    /// Pure ASCII passes straight through.
    #[test]
    fn utf8_decoder_passes_ascii() {
        let mut d = Utf8StreamDecoder::default();
        assert_eq!(d.decode(b"data: {}\n\n"), "data: {}\n\n");
    }
}
