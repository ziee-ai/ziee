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
