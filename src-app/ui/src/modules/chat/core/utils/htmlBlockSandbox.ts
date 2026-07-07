/**
 * Sandbox posture for the chat HTML-block live preview (`HtmlBlock.tsx`).
 *
 * A fenced ```html code block from an assistant message can be rendered live,
 * but ONLY inside a strictly-sandboxed `<iframe srcdoc>`. This module is the
 * single source of truth for that posture — kept pure + side-effect-free so the
 * security decisions live in one auditable place.
 *
 * Threat model: the HTML is untrusted (LLM- or prompt-injection-authored). The
 * two independent guards, both REQUIRED (see .lifecycle DEC-3 / DEC-5):
 *
 *  1. `SANDBOX` — `allow-scripts` and NOTHING else. Scripts run, but the frame
 *     is a NULL/opaque origin: it cannot reach `window.parent`/`top`, the parent
 *     DOM, cookies, or `localStorage` (Same-Origin Policy). We deliberately do
 *     NOT grant `allow-same-origin` — combined with `allow-scripts` it would let
 *     the frame delete its own sandbox (the canonical bypass). No
 *     top-navigation (anti-clickjacking/phishing), no popups, no forms, no
 *     modals, no downloads.
 *  2. `CSP` — injected as the first `<head>` child of the srcdoc. The sandbox
 *     attribute alone does NOT restrict network; this CSP severs it. It allows
 *     inline script/style + `data:` images/fonts so the HTML renders and is
 *     interactive, but blocks ALL external network (fetch/XHR/WebSocket via
 *     `connect-src`→`default-src 'none'`, external scripts/styles/images/media,
 *     nested external frames, form submission, `<base>` hijack). Conservative
 *     default: no phone-home / no exfiltration.
 */

/**
 * The ONLY sandbox token granted. `allow-scripts` in isolation = scripts run in
 * a null origin. Never add `allow-same-origin` here (see module doc).
 */
export const SANDBOX = 'allow-scripts'

/**
 * Conservative Content-Security-Policy for the preview document. Blocks all
 * external network; permits inline script/style + data: media so the block
 * renders. `default-src 'none'` makes every unlisted directive (connect-src,
 * frame-src, object-src, …) fall back to "nothing".
 */
export const CSP =
  "default-src 'none'; " +
  "script-src 'unsafe-inline'; " +
  "style-src 'unsafe-inline'; " +
  'img-src data:; ' +
  'font-src data:; ' +
  'media-src data:; ' +
  "form-action 'none'; " +
  "base-uri 'none'"

const CSP_META = `<meta http-equiv="Content-Security-Policy" content="${CSP}">`

/**
 * Wrap untrusted HTML into a document string suitable for `<iframe srcDoc>`,
 * with the CSP `<meta>` as the LITERAL first bytes of the document — before any
 * user-supplied markup — so it is in force before any user script/tag is parsed.
 *
 * We do NOT splice the meta into the untrusted HTML (e.g. after a matched
 * `<head>`). String-splicing is defeatable two ways, both of which would let a
 * script run with NO CSP (external network re-opened):
 *   - a `<script>` placed BEFORE the user's `<head>` parses before an
 *     after-`<head>` meta would install;
 *   - a decoy `<!-- <head> -->` comment captures a naive `<head>` regex, trapping
 *     the meta inside a comment (inert).
 * Prepending instead makes both impossible: nothing user-provided can precede
 * our meta. A `<meta http-equiv=CSP>` before `<html>`/`<head>` is parsed into the
 * implicit `<head>` and governs the whole document; a second (user) doctype/head
 * is merged/ignored by the parser. CSP policies are additive, so a looser CSP the
 * user injects LATER cannot relax ours.
 *
 * The result is passed to React's `srcDoc` prop, which HTML-attribute-escapes it
 * — the raw HTML is never concatenated into a live DOM via innerHTML here, so the
 * host page has no HTML-injection surface; all execution is confined to the
 * null-origin frame.
 */
export function buildSandboxedSrcdoc(html: string): string {
  const src = typeof html === 'string' ? html : ''
  return `<!DOCTYPE html>${CSP_META}<meta charset="utf-8">\n${src}`
}
