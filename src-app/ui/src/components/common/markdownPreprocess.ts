import { normalizeMathDelimiters } from '@/components/common/normalizeMathDelimiters'

// Link reference definition:  [id]: url "optional title"   (indented ≤3 spaces)
//
// The id may NOT start with `^`: in a GFM document `[^id]:` is always a FOOTNOTE
// definition, never a link reference. Without this exclusion a footnote whose
// body happens to be a single token — `[^2]: Two.` — is collected as a link
// definition with url `Two.`, and the reference pass below then rewrites the
// co-located citation run `[^1][^2]` (which looks exactly like the reference
// link `[text][id]`) into `[^1](Two.)`, silently destroying the second citation
// of every run. Long definition bodies happen not to match, so this only ever
// bit short ones — see markdownPreprocess.test.ts.
const DEF_RE =
  /^ {0,3}\[([^^\]\r\n][^\]\r\n]*)\]:[ \t]*(<[^>\r\n]*>|\S+)(?:[ \t]+(?:"([^"]*)"|'([^']*)'|\(([^)]*)\)))?[ \t]*$/gm

// Inline image:  ![alt](url "title")
const IMG_RE =
  /!\[([^\]\r\n]*)\]\(\s*(<[^>\r\n]*>|[^)\s]+)(?:\s+(?:"[^"]*"|'[^']*'|\([^)]*\)))?\s*\)/g

// A `$$…$$` or `$…$` math span. The bracket-bearing passes below must not reach
// inside one: `$$ a[1] $$` alongside a `[1]: url` definition would otherwise have
// its `[1]` rewritten into a link, corrupting the equation.
//
// This is an APPROXIMATION of micromark-extension-math's tokenizer, deliberately
// kept conservative. The `\n(?!\s*\n)` clause is the important part: math can span
// lines but NOT a blank line, so without it two unrelated `$$` occurrences
// paragraphs apart would swallow everything between them and silently disable the
// reference-link and image passes over that whole region. Known remaining
// divergences from the real parser: a single-`$` span never spans a newline (the
// real one may), and a `$$$x$$$` run is matched by delimiter text rather than by
// matched run length.
const MATH_SPAN_RE = /(\$\$(?:[^\n]|\n(?!\s*\n))*?\$\$|\$[^$\n]*\$)/

const normId = (s: string) => s.trim().replace(/\s+/g, ' ').toLowerCase()

function isSameOriginImage(url: string): boolean {
  if (url.startsWith('/')) return true
  if (url.startsWith('data:')) return false
  try {
    return new URL(url, window.location.origin).origin === window.location.origin
  } catch {
    return false
  }
}

/**
 * One-pass markdown pre-processing applied before the text reaches Streamdown
 * (which parses block-by-block for streaming). Three fixes, all operating outside
 * code spans/fences:
 *
 * 1. **Reference links** — Streamdown resolves `[text][id]` only within a single
 *    block, so a `[id]: url` definition in another paragraph is invisible and
 *    renders as raw text. We collect every definition in the full document and
 *    rewrite the usages to inline `[text](url)`.
 *
 * 2. **Blocked images** — external / `data:` image `src`s are blocked for
 *    exfil-safety, and Streamdown's own image component just hides the (never-
 *    loaded) image, leaving a broken-looking dangling caption. We rewrite those
 *    to a `🖼 alt` placeholder — a link out for a standalone external image
 *    (opens only on an explicit click; nothing auto-loads), or plain text when
 *    it's a `data:` URI or already wrapped in a link. Same-origin images are
 *    left intact.
 *
 * 3. **LaTeX math delimiters** — models write math with LaTeX's own delimiters,
 *    but remark-math only understands `$`. Markdown eats the `\[` / `\(` as a
 *    character escape, so the equation leaks through as raw LaTeX (issue #177).
 *    `normalizeMathDelimiters` rewrites display `\[ … \]` into the `$$ … $$`
 *    block form and inline `\( … \)` into `$ … $`. It runs FIRST, and its output
 *    is then split back out so passes (1) and (2) never reach inside a math span.
 *    Running it here — inside the code-fence/inline-code split — is what keeps a
 *    real `sed 's/\(foo\)/bar/'` in a code block literal.
 */
export function preprocessMarkdown(md: string): string {
  // The reference/image passes need a `[`, but the math pass does NOT: `\[`
  // contains one, `\(` does not. Without the second clause a message whose only
  // markup is inline math — `Energy \( E = mc^2 \) is nice.` — returns here
  // untouched and the math pass never runs at all.
  if (typeof md !== 'string') return md
  if (md.indexOf('[') === -1 && md.indexOf('\\(') === -1) return md

  DEF_RE.lastIndex = 0
  const defs = new Map<string, { url: string; title?: string }>()
  let m: RegExpExecArray | null
  while ((m = DEF_RE.exec(md)) !== null) {
    const id = normId(m[1])
    if (defs.has(id)) continue
    let url = m[2]
    if (url.startsWith('<') && url.endsWith('>')) url = url.slice(1, -1)
    defs.set(id, { url, title: m[3] ?? m[4] ?? m[5] })
  }

  const inlineLink = (label: string, d: { url: string; title?: string }, bang: string) =>
    `${bang}[${label}](${d.url}${d.title ? ` "${d.title.replace(/"/g, '\\"')}"` : ''})`

  // Split on code fences + inline code so we never rewrite inside code. The
  // capture group keeps the delimiters at ODD indices — rewrite EVEN only.
  const parts = md.split(/(```[\s\S]*?```|~~~[\s\S]*?~~~|`[^`\r\n]+`)/)
  for (let i = 0; i < parts.length; i += 2) {
    // (3) Math delimiters first, so the spans it produces are protected below.
    // Splitting on math spans keeps the ODD indices (the spans themselves)
    // untouched — for input with no math this is a 1-element array, so the loop
    // and the re-join are byte-for-byte identity.
    const sub = normalizeMathDelimiters(parts[i]).split(MATH_SPAN_RE)
    for (let j = 0; j < sub.length; j += 2) {
      let s = sub[j]

      // (2) Blocked images → placeholder. Done before the reference pass so the
      // rewritten `[🖼 alt](url)` link isn't itself re-touched.
      s = s.replace(IMG_RE, (whole, alt: string, rawUrl: string, offset: number, str: string) => {
        let url = rawUrl
        if (url.startsWith('<') && url.endsWith('>')) url = url.slice(1, -1)
        if (isSameOriginImage(url)) return whole
        const label = `🖼 ${alt?.trim() || 'image'}`
        // `![…]` preceded by `[` is an image-as-link (`[![…](…)](…)`); keep it as
        // text so we don't create an invalid nested link.
        const insideLink = offset > 0 && str[offset - 1] === '['
        return !insideLink && /^https?:\/\//i.test(url) ? `[${label}](${url})` : label
      })

      // (1) Reference links (only when the doc has definitions).
      if (defs.size > 0) {
        // Full + collapsed:  [label][id]  and  [label][]
        s = s.replace(/(!?)\[([^\]\r\n]+)\]\[([^\]\r\n]*)\]/g, (whole, bang, label, id) => {
          const d = defs.get(normId(id.trim() === '' ? label : id))
          return d ? inlineLink(label, d, bang) : whole
        })
        // Shortcut:  [label]  (not `[](`, `[][`, `[]:`, or a footnote `[^…]`)
        s = s.replace(/(!?)\[([^\]\r\n^][^\]\r\n]*)\](?![[(:])/g, (whole, bang, label) => {
          const d = defs.get(normId(label))
          return d ? inlineLink(label, d, bang) : whole
        })
      }

      sub[j] = s
    }

    parts[i] = sub.join('')
  }
  return parts.join('')
}
