import { slugifyHeading, safeDecode } from '@/components/common/markdownHeadings'

/**
 * Prefix-count-agnostic scoping for GFM footnote DOM ids/hrefs.
 *
 * Why this exists: Streamdown v2's default rehype pipeline applies the
 * `user-content-` clobber prefix to footnote ids TWICE — once in
 * `mdast-util-to-hast` (its default `clobberPrefix`) and again in
 * `rehype-sanitize` (`hast-util-sanitize` unconditionally re-prepends
 * `clobberPrefix` to every `id`/`name`). So a footnote definition arrives as
 * `<li id="user-content-user-content-fn-1">` while the reference's `href`
 * (which is NOT clobbered by sanitize) stays single-prefixed
 * `#user-content-fn-1`. The old overrides matched a single `user-content-fn-`
 * prefix, so the double-prefixed `<li>` id was never re-scoped and
 * `getElementById` for the click target returned null → clicking a reference
 * no-oped.
 *
 * These helpers strip ANY number (0, 1, 2+) of leading `user-content-`
 * prefixes and re-scope the footnote to the current message's `contentId`, so
 * the ref href and the definition id resolve to the SAME element regardless of
 * how many times Streamdown prefixed them — and it stays correct if a future
 * Streamdown drops back to single/zero prefixing.
 *
 * Kept as a pure, DOM-free module so it is unit-testable with node:test
 * (mirrors `components/common/imageSrcPolicy.ts`).
 */

// `fn-<suffix>` / `fnref-<suffix>` with any number of leading `user-content-`
// clobber prefixes. Suffix captures the footnote identifier (`1`, `note`,
// `1-2` for a multiply-referenced footnote, …).
const FOOTNOTE_ID_RE = /^(?:user-content-)*(fn|fnref)-(.+)$/
const FOOTNOTE_HREF_RE = /^#(?:user-content-)*(fn|fnref)-(.+)$/
const FOOTNOTE_LABEL_RE = /^(?:user-content-)*footnote-label$/

/**
 * Scope a footnote element id (`fn-`/`fnref-`, however many `user-content-`
 * prefixes precede it) to this message: `${contentId}-<kind>-<suffix>`. Any
 * non-footnote id (or undefined) is returned unchanged.
 */
export function scopeFootnoteId(
  id: string | undefined,
  contentId: string,
): string | undefined {
  if (!id) return id
  const m = FOOTNOTE_ID_RE.exec(id)
  return m ? `${contentId}-${m[1]}-${m[2]}` : id
}

/**
 * Scope an anchor `href` to this message:
 *  - a footnote hash (`#…fn-N` / `#…fnref-N`) → `#${contentId}-<kind>-<suffix>`
 *    (matches what `scopeFootnoteId` produces for the target element);
 *  - any other in-page hash (`#Some Section`) → this message's slugged heading
 *    id `#${contentId}-h-${slug}` (same slugify as the heading override), so an
 *    in-markdown `[Section](#section)` link scrolls to THIS message's heading;
 *  - anything else (external URL, undefined) → returned unchanged.
 */
export function scopeHref(
  href: string | undefined,
  contentId: string,
): string | undefined {
  if (!href) return href
  const fn = FOOTNOTE_HREF_RE.exec(href)
  if (fn) return `#${contentId}-${fn[1]}-${fn[2]}`
  if (href.startsWith('#')) {
    return `#${contentId}-h-${slugifyHeading(safeDecode(href.slice(1)))}`
  }
  return href
}

/**
 * True when `id` is the GFM footnotes-section label heading
 * (`footnote-label`), tolerating any number of `user-content-` prefixes — used
 * to suppress the sr-only "Footnotes" `<h2>` in favor of the `<summary>`.
 */
export function isFootnoteLabel(id: string | undefined): boolean {
  return !!id && FOOTNOTE_LABEL_RE.test(id)
}

/**
 * A hierarchical `paper-chunk` footnote label as it travels on the wire —
 * `1-1`, `2-10`. Shown to the reader as `1.1` (see `formatFootnoteLabel`).
 *
 * The separator is a HYPHEN, not a dot, and that is load-bearing: Streamdown
 * splits a message into blocks before parsing, and a definition labelled
 * `[^1.1]:` makes that splitter cut the message apart — the definitions land in
 * different blocks from the markers referencing them, so GFM binds nothing and
 * every marker renders as literal `[^1.1]` text. A hyphen keeps the message
 * whole. Covered by a test; do not switch it back to a dot.
 */
const HIERARCHICAL_LABEL_RE = /^(\d+)-(\d+)$/

/**
 * The raw footnote identifier carried by an id or href, with any number of
 * `user-content-` prefixes and the `fn-`/`fnref-` kind stripped: the suffix
 * only, e.g. `user-content-fn-1-1` → `1-1`. Undefined for a non-footnote value.
 */
export function footnoteLabel(
  idOrHref: string | undefined,
): string | undefined {
  if (!idOrHref) return undefined
  const m = idOrHref.startsWith('#')
    ? FOOTNOTE_HREF_RE.exec(idOrHref)
    : FOOTNOTE_ID_RE.exec(idOrHref)
  return m ? m[2] : undefined
}

/** `1-2` → `1.2`. The dot is presentation only; the wire uses a hyphen. */
export function formatFootnoteLabel(label: string): string {
  return label.replace('-', '.')
}

/**
 * The `P.C` label to DISPLAY for a footnote reference, or undefined to keep
 * whatever GFM rendered.
 *
 * Why this exists: remark-gfm renumbers footnotes sequentially by first
 * reference — `mdast-util-to-hast` emits `String(counter)` as the anchor's text
 * and NEVER the label — so a `[^1.1]` written by the RAG service still displays
 * as `1`. The label survives only in the anchor's id/href, so the hierarchy is
 * read back from there (ziee#167).
 *
 * DELIBERATELY NARROW: only a `<digits>-<digits>` label is honored. An ordinary
 * sequential footnote set (`1`, `2`, …) and a named one (`note`, `alpha`) both
 * return undefined, so every other footnote consumer — the citations and
 * knowledge_base modules — renders byte-identically to before.
 *
 * Reads the `fn-` form in preference to `fnref-`. A footnote referenced more
 * than once gets a DISAMBIGUATED ref id (`fnref-1-1-2` for the 2nd use of
 * `1-1`), which is not the label; the ref's href (`#…fn-1-1`) and the
 * definition's id (`…fn-1-1`) both carry the label verbatim.
 */
export function hierarchicalFootnoteLabel(
  id: string | undefined,
  href?: string | undefined,
): string | undefined {
  const fromFn = (v: string | undefined) =>
    v && !/(?:^|-)fnref-/.test(v) ? footnoteLabel(v) : undefined
  const label = fromFn(href) ?? fromFn(id)
  return label && HIERARCHICAL_LABEL_RE.test(label) ? label : undefined
}
