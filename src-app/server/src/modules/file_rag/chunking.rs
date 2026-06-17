//! Page-aware text chunking for Document RAG.
//!
//! Chunks never cross a page boundary (PDFs are per-page; text/CSV/code is a
//! single page = whole file). Each chunk is a sliding char-window with overlap
//! and a soft word boundary. `char_start`/`char_end` are **byte** offsets into
//! the page's UTF-8 text, always aligned to a char boundary — so the citation
//! layer recovers the exact span with a native slice
//! `&page_text[char_start..char_end]`, and the unit invariant
//! `&page[char_start..char_end] == content` holds for non-ASCII text too.
//!
//! The chunker is pure; the per-file `max_chunks_per_file` cap is enforced by
//! the caller (`ingest`) across pages.

use super::models::{ChunkDraft, FileRagAdminSettings};

/// Fraction of the window (from the end) within which we look back for a
/// whitespace boundary before falling back to a hard cut. 15%.
const SOFT_BOUNDARY_NUM: usize = 15;
const SOFT_BOUNDARY_DEN: usize = 100;

#[derive(Debug, Clone, Copy)]
pub struct ChunkParams {
    /// Target window size, in characters.
    pub chunk_chars: usize,
    /// Overlap between consecutive windows, in characters (< chunk_chars).
    pub overlap_chars: usize,
}

impl ChunkParams {
    pub fn from_settings(s: &FileRagAdminSettings) -> Self {
        let chunk_chars = (s.chunk_chars.max(1)) as usize;
        // DB CHECK guarantees overlap < chunk_chars, but clamp defensively so
        // the chunker can never stall on a bad row.
        let overlap_chars = (s.chunk_overlap_chars.max(0) as usize).min(chunk_chars - 1);
        Self {
            chunk_chars,
            overlap_chars,
        }
    }
}

/// Chunk one page's extracted text. `chunk_index_start` is the file-global
/// running index for the first emitted chunk; emitted chunks are numbered
/// sequentially from there. Whitespace-only windows are skipped (no value to
/// FTS or embeddings) but still advance the cursor.
pub fn chunk_page(
    page_text: &str,
    page_number: i32,
    chunk_index_start: i32,
    params: &ChunkParams,
) -> Vec<ChunkDraft> {
    // (byte_offset, char) for every char, plus a sentinel end offset so a
    // char position `p` maps to a byte offset via `byte_at(p)`.
    let chars: Vec<(usize, char)> = page_text.char_indices().collect();
    let total = chars.len();
    if total == 0 {
        return Vec::new();
    }
    let byte_at = |p: usize| -> usize {
        if p < total {
            chars[p].0
        } else {
            page_text.len()
        }
    };

    let soft = (params.chunk_chars * SOFT_BOUNDARY_NUM / SOFT_BOUNDARY_DEN).max(1);

    let mut out = Vec::new();
    let mut idx = chunk_index_start;
    let mut start = 0usize; // char position
    while start < total {
        let nominal_end = (start + params.chunk_chars).min(total);

        // Soft boundary: on a non-final window, back off to the last
        // whitespace within the trailing `soft` chars so chunks don't split
        // mid-word. `floor` keeps at least one char in the window.
        let mut end = nominal_end;
        if nominal_end < total {
            let floor = nominal_end.saturating_sub(soft).max(start + 1);
            let mut w = nominal_end; // exclusive
            while w > floor {
                if chars[w - 1].1.is_whitespace() {
                    end = w; // include the whitespace; next window starts after it
                    break;
                }
                w -= 1;
            }
        }

        let byte_start = byte_at(start);
        let byte_end = byte_at(end);
        let slice = &page_text[byte_start..byte_end];
        if !slice.trim().is_empty() {
            out.push(ChunkDraft {
                page_number,
                chunk_index: idx,
                char_start: byte_start as i32,
                char_end: byte_end as i32,
                content: slice.to_string(),
            });
            idx += 1;
        }

        if end >= total {
            break;
        }
        // Advance with overlap, but always make progress (`end > start`).
        let advance_to = end.saturating_sub(params.overlap_chars);
        start = if advance_to > start { advance_to } else { end };
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(chunk: usize, overlap: usize) -> ChunkParams {
        ChunkParams {
            chunk_chars: chunk,
            overlap_chars: overlap,
        }
    }

    /// The defining invariant: every chunk's recorded span re-slices to its
    /// exact content. Guards the citation re-load contract.
    fn assert_spans_exact(page: &str, drafts: &[ChunkDraft]) {
        for d in drafts {
            assert_eq!(
                &page[d.char_start as usize..d.char_end as usize],
                d.content,
                "span [{}..{}] must re-slice to content",
                d.char_start,
                d.char_end
            );
        }
    }

    #[test]
    fn empty_and_whitespace_pages_yield_nothing() {
        assert!(chunk_page("", 1, 0, &params(100, 10)).is_empty());
        assert!(chunk_page("   \n\t  ", 1, 0, &params(100, 10)).is_empty());
    }

    #[test]
    fn short_text_is_one_chunk_spanning_whole_page() {
        let page = "Hello world.";
        let drafts = chunk_page(page, 3, 0, &params(100, 10));
        assert_eq!(drafts.len(), 1);
        assert_eq!(drafts[0].page_number, 3);
        assert_eq!(drafts[0].chunk_index, 0);
        assert_eq!(drafts[0].char_start, 0);
        assert_eq!(drafts[0].char_end, page.len() as i32);
        assert_eq!(drafts[0].content, page);
        assert_spans_exact(page, &drafts);
    }

    #[test]
    fn long_text_chunks_overlap_and_cover() {
        // 600 chars of words; window 100, overlap 20.
        let page: String = (0..120).map(|i| format!("word{i} ")).collect();
        let drafts = chunk_page(&page, 1, 0, &params(100, 20));
        assert!(drafts.len() > 1, "should produce multiple chunks");
        assert_spans_exact(&page, &drafts);
        // chunk_index is sequential from the start value.
        for (i, d) in drafts.iter().enumerate() {
            assert_eq!(d.chunk_index, i as i32);
        }
        // Consecutive chunks overlap (next start < prev end) thanks to overlap.
        for pair in drafts.windows(2) {
            assert!(
                pair[1].char_start < pair[0].char_end,
                "expected overlap: next.start {} < prev.end {}",
                pair[1].char_start,
                pair[0].char_end
            );
        }
    }

    #[test]
    fn soft_boundary_avoids_mid_word_cut_when_possible() {
        // Window 100 (soft lookback ≈15 chars) over space-separated words:
        // every non-final chunk should end at a whitespace, never mid-word.
        // (A tiny window would shrink the 15% lookback below one word and the
        // feature can't help — realistic windows are hundreds of chars.)
        let page: String = (0..60).map(|i| format!("word{i} ")).collect();
        let drafts = chunk_page(&page, 1, 0, &params(100, 10));
        assert!(drafts.len() > 1, "need multiple chunks to test boundaries");
        assert_spans_exact(&page, &drafts);
        // No emitted chunk (except possibly the last) should end in the middle
        // of a word — i.e. it ends at a whitespace or at end-of-page.
        for d in &drafts {
            let ends_clean = d.char_end as usize == page.len()
                || page[..d.char_end as usize]
                    .chars()
                    .last()
                    .map(|c| c.is_whitespace())
                    .unwrap_or(false);
            assert!(ends_clean, "chunk {:?} cut mid-word", d.content);
        }
    }

    #[test]
    fn utf8_multibyte_spans_are_char_aligned() {
        // Accents + emoji: byte offsets must land on char boundaries so the
        // native slice never panics and the invariant holds.
        let page = "café façade naïve — 🍓🍓🍓 Привет мир, теста данных здесь.";
        let drafts = chunk_page(page, 2, 5, &params(12, 3));
        assert!(!drafts.is_empty());
        assert_eq!(drafts[0].chunk_index, 5);
        assert_spans_exact(page, &drafts);
    }

    #[test]
    fn chunk_index_continues_across_pages() {
        let p1 = chunk_page("first page text here", 1, 0, &params(8, 2));
        let next = p1.last().unwrap().chunk_index + 1;
        let p2 = chunk_page("second page text here", 2, next, &params(8, 2));
        assert_eq!(p2[0].chunk_index, next);
    }

    #[test]
    fn overlap_clamped_when_exceeding_window_makes_progress() {
        // Pathological: overlap clamped to chunk-1 by from_settings; here we
        // pass overlap == chunk-1 directly and assert termination + coverage.
        let page = "abcdefghijklmnopqrstuvwxyz";
        let drafts = chunk_page(page, 1, 0, &params(5, 4));
        assert_spans_exact(page, &drafts);
        assert!(drafts.len() >= 2);
    }
}
