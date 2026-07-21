import {
  footnoteLabel,
  formatFootnoteLabel,
} from '@/modules/chat/core/utils/footnoteScope'

/**
 * Merge same-paper footnote definitions into ONE bibliographic entry with its
 * excerpts nested beneath it (ziee#167).
 *
 * The BioGnosia RAG service cites one footnote per retrieved CHUNK, so three
 * chunks of the same paper used to print three identical bibliographic entries
 * that differed only in their "Cited excerpt". It now labels those chunks
 * `P.1`, `P.2`, … (paper index, chunk index) — see `_assign_footnote_labels` in
 * biognosia-mcp's `mcp_route.py`. GFM keeps the label only in the definition
 * `<li>`'s id, which is what this transform groups on.
 *
 * Why a rehype plugin and not React-children surgery in the `section` override:
 * at the React layer the `<ol>`'s children are elements whose `type` is our own
 * override function, so finding the `<li>`s means comparing component identities
 * and reaching into `props.children`. Here the tree is plain data, so this stays
 * a pure function — unit-testable with object fixtures, like `footnoteScope.ts`
 * and `imageSrcPolicy.ts`.
 *
 * SECURITY: this runs LAST, after rehype-raw → rehype-sanitize → rehype-harden
 * (see `chatRehypePlugins`). It therefore only ever sees already-neutralized
 * nodes, it never copies raw markup or attributes out of untrusted input, and
 * the only nodes it creates are `ol`/`li`/`span` elements it authors itself.
 *
 * GRACEFUL DEGRADATION: two independent bail-outs leave the tree untouched
 * unless the input is unambiguously paper-grouped — see `groupFootnoteList`.
 */

// Minimal structural types. The real hast types live in `@types/hast`, which
// this workspace does not depend on; the transform only needs these fields.
interface HastNode {
  type: string
  tagName?: string
  properties?: Record<string, unknown>
  children?: HastNode[]
  value?: string
}

/**
 * `1` / `2-10` → `{ paper: 1, chunk: 10 }`; anything else → undefined.
 *
 * The wire separator is a hyphen (a dot makes Streamdown's block splitter cut
 * the message apart — see `HIERARCHICAL_LABEL_RE` in footnoteScope.ts); the dot
 * the reader sees is applied by `formatFootnoteLabel` at render time.
 */
function parseLabel(
  label: string | undefined,
): { paper: number; chunk?: number } | undefined {
  if (!label) return undefined
  const m = /^(\d+)(?:-(\d+))?$/.exec(label)
  if (!m) return undefined
  return { paper: Number(m[1]), chunk: m[2] ? Number(m[2]) : undefined }
}

const isElement = (n: HastNode, tagName: string) =>
  n.type === 'element' && n.tagName === tagName

/**
 * A footnote definition `<li>` holds the bibliographic header (paragraphs) plus
 * the cited excerpt (a blockquote). Split them so the header can be shown once
 * per paper while every excerpt keeps its own labelled sub-item.
 *
 * The trailing backref-only paragraph GFM appends is dropped with the header of
 * every item but the first — it renders as nothing anyway (the `a` override
 * returns null for `data-footnote-backref`), and keeping N copies would leave N
 * empty paragraphs.
 */
function splitDefinition(li: HastNode): { header: HastNode[]; excerpt: HastNode[] } {
  const header: HastNode[] = []
  const excerpt: HastNode[] = []
  for (const child of li.children ?? []) {
    if (isElement(child, 'blockquote')) excerpt.push(child)
    else header.push(child)
  }
  return { header, excerpt }
}

/**
 * Group a footnotes `<ol>`'s children by paper. Returns the new children, or
 * undefined to signal "leave the list exactly as it is".
 *
 * Exported for unit testing; `rehypeGroupPaperFootnotes` is the plugin wrapper.
 */
export function groupFootnoteList(items: HastNode[]): HastNode[] | undefined {
  const lis = items.filter((n) => isElement(n, 'li'))
  if (lis.length === 0) return undefined

  const parsed = lis.map((li) => ({
    li,
    label: footnoteLabel(li.properties?.id as string | undefined),
  }))

  // BAIL-OUT 1 — any label we cannot parse as `P` or `P.C` (a named footnote,
  // a `1-2` multi-reference suffix, a missing id). Mixed input is not ours.
  const entries = parsed.map((p) => ({ ...p, parts: parseLabel(p.label) }))
  if (entries.some((e) => !e.parts)) return undefined

  // BAIL-OUT 2 — nothing hierarchical: an ordinary sequential footnote set
  // (`1`, `2`, `3`) from the citations / knowledge_base modules. Untouched.
  if (!entries.some((e) => e.parts!.chunk !== undefined)) return undefined

  // Group by paper index, papers in first-appearance order. Grouping is by KEY,
  // not adjacency: the `<ol>` is ordered by first REFERENCE, so two papers cited
  // alternately arrive interleaved (1.1, 2.1, 1.2).
  const groups = new Map<number, typeof entries>()
  for (const entry of entries) {
    const bucket = groups.get(entry.parts!.paper)
    if (bucket) bucket.push(entry)
    else groups.set(entry.parts!.paper, [entry])
  }

  return [...groups.values()].map((group) => {
    const paperLabel = String(group[0].parts!.paper)

    // A paper cited through ONE chunk is already a complete, flat entry — its
    // label is flat too ("2", not "2-1"), so it renders exactly as it does
    // today: bib header, then its single "Cited excerpt", no nested sub-list
    // and no excerpt-count caption. Only the paper NUMBER is stamped, so the
    // entry cannot drift from the inline marker pointing at it.
    if (group.length === 1) {
      const li = group[0].li
      li.properties = { ...li.properties, dataPaperLabel: paperLabel }
      return li
    }

    const { header } = splitDefinition(group[0].li)

    const excerptItems: HastNode[] = group.map((entry) => ({
      type: 'element',
      tagName: 'li',
      properties: {
        // Keep the ORIGINAL id so `#…fn-1.2` still resolves: the `a` override
        // scopes both sides through `scopeFootnoteId`, so the reference click
        // lands on this exact excerpt.
        id: entry.li.properties?.id,
        className: ['footnote-excerpt'],
        // The DISPLAY form ("1.2"); the wire label is "1-2".
        dataExcerptLabel: formatFootnoteLabel(entry.label!),
      },
      children: splitDefinition(entry.li).excerpt,
    }))

    return {
      type: 'element',
      tagName: 'li',
      properties: {
        className: ['footnote-paper'],
        dataPaperLabel: paperLabel,
      },
      children: [
        ...header,
        // The count is what makes the paper→excerpt nesting self-explanatory,
        // but it earns its space only when there is more than one excerpt.
        ...(group.length > 1
          ? [
              {
                type: 'element',
                tagName: 'p',
                properties: { className: ['footnote-excerpt-count'] },
                children: [
                  { type: 'text', value: `${group.length} cited excerpts` },
                ],
              } satisfies HastNode,
            ]
          : []),
        {
          type: 'element',
          tagName: 'ol',
          properties: { className: ['footnote-excerpts'] },
          children: excerptItems,
        },
      ],
    } satisfies HastNode
  })
}

/** Depth-first walk; the footnotes section is always a direct child of root in
 * practice, but Streamdown may wrap blocks, so do not assume a depth. */
function visit(node: HastNode, fn: (n: HastNode) => void): void {
  fn(node)
  for (const child of node.children ?? []) visit(child, fn)
}

const isFootnoteRefSup = (n: HastNode | undefined) =>
  !!n &&
  isElement(n, 'sup') &&
  (n.children ?? []).some(
    (c) => isElement(c, 'a') && c.properties?.dataFootnoteRef !== undefined,
  )

/**
 * Mark every footnote-reference `<sup>` that DIRECTLY follows another one, so
 * CSS can put a comma between them: `[^1][^2][^3]` renders as one run-together
 * blob ("123") otherwise (ziee#167).
 *
 * Why not a pure CSS `sup + sup` rule: the adjacent-sibling combinator ignores
 * intervening TEXT, so a later, unrelated citation in the same paragraph
 * ("…of these[^1]. Bypass signalling…[^2]") is also a `sup + sup` match and
 * would get a spurious leading comma. Here the previous SIBLING NODE is
 * checked, text nodes included, so only a true `[^1][^2]` run is marked.
 *
 * Applies to every footnote consumer, not just paper-grouped ones — the blob is
 * the same bug for the citations and knowledge_base modules.
 */
export function markAdjacentFootnoteRefs(tree: HastNode): void {
  visit(tree, (node) => {
    const children = node.children
    if (!children) return
    for (let i = 1; i < children.length; i++) {
      if (isFootnoteRefSup(children[i]) && isFootnoteRefSup(children[i - 1])) {
        const props = (children[i].properties ??= {})
        const existing = props.className
        props.className = [
          ...(Array.isArray(existing) ? existing : existing ? [existing] : []),
          'footnote-ref-adjacent',
        ]
      }
    }
  })
}

/**
 * On a paper-grouped answer, show every inline marker's own label rather than
 * GFM's sequential ordinal — including a FLAT one.
 *
 * A paper cited through a single chunk keeps a flat label ("2"), because the
 * hierarchy should only appear where it carries information. But GFM numbers
 * footnotes by first-reference order, so that "2" sits at ordinal position 3
 * (after "1-1" and "1-2") and would render as "3" — disagreeing with its own
 * reference entry. Stamping the label on every ref fixes flat and hierarchical
 * markers together.
 *
 * Only runs once a grouped set has been detected, so an ordinary sequential
 * footnote set is never touched and keeps GFM's numbering exactly.
 */
export function stampFootnoteRefLabels(tree: HastNode): void {
  visit(tree, (node) => {
    if (
      node.type !== 'element' ||
      node.tagName !== 'a' ||
      node.properties?.dataFootnoteRef === undefined
    ) {
      return
    }
    // The href carries the definition's label; the ref's own id may carry a
    // re-reference suffix (`fnref-1-1-2`) that is not the label.
    const label = footnoteLabel(node.properties.href as string | undefined)
    if (!label || !/^\d+(?:-\d+)?$/.test(label)) return
    node.properties.dataFootnoteDisplay = formatFootnoteLabel(label)
  })
}

export function rehypeGroupPaperFootnotes() {
  return (tree: HastNode) => {
    // Independent of grouping: every footnote consumer gets comma-separated
    // co-located citations.
    markAdjacentFootnoteRefs(tree)
    let grouped = false
    visit(tree, (node) => {
      if (node.type !== 'element' || node.tagName !== 'section') return
      if (node.properties?.dataFootnotes === undefined) return
      const ol = (node.children ?? []).find((c) => isElement(c, 'ol'))
      if (!ol) return
      const regrouped = groupFootnoteList(ol.children ?? [])
      if (regrouped) {
        ol.children = regrouped
        grouped = true
      }
    })
    if (grouped) stampFootnoteRefLabels(tree)
  }
}
