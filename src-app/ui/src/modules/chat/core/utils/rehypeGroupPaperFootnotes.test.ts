import { test } from 'node:test'
import assert from 'node:assert/strict'
import { parseMarkdownIntoBlocks } from 'streamdown'
import {
  groupFootnoteList,
  markAdjacentFootnoteRefs,
  stampFootnoteRefLabels,
} from './rehypeGroupPaperFootnotes.ts'

// TEST-6 (ziee#167). Same-paper footnote definitions merge into ONE entry with
// nested, labelled excerpts. The two bail-outs are what keep every OTHER
// footnote consumer (citations, knowledge_base) rendering byte-identically —
// they are the load-bearing assertions here, not an afterthought.

interface N {
  type: string
  tagName?: string
  properties?: Record<string, unknown>
  children?: N[]
  value?: string
}

const p = (text: string): N => ({
  type: 'element',
  tagName: 'p',
  children: [{ type: 'text', value: text }],
})

const quote = (text: string): N => ({
  type: 'element',
  tagName: 'blockquote',
  children: [p(text)],
})

/** A GFM footnote definition <li> as rehype-sanitize leaves it (double-prefixed). */
const def = (label: string, header: string, excerpt: string): N => ({
  type: 'element',
  tagName: 'li',
  properties: { id: `user-content-user-content-fn-${label}` },
  children: [p(header), quote(excerpt)],
})

const HEADER = 'Short et al. (2024). Regulated cell death. Cell Death Differ.'

const childrenOf = (n: N, tagName: string) =>
  (n.children ?? []).filter((c) => c.tagName === tagName)

const textOf = (n: N): string =>
  n.type === 'text' ? (n.value ?? '') : (n.children ?? []).map(textOf).join('')

test('groups same-paper definitions into one entry with labelled excerpts', () => {
  const grouped = groupFootnoteList([
    def('1-1', HEADER, 'Caspase-8 is a molecular switch.'),
    def('1-2', HEADER, 'Loss of caspase-8 redirects to necroptosis.'),
    // Flat label: this paper was cited through a single chunk.
    def('2', 'Doe J (2021). PTEN and HR repair. Nature.', 'PTEN loss impairs HR.'),
  ])
  assert.ok(grouped)

  // Two entries, not three: the duplicated paper collapsed.
  assert.equal(grouped.length, 2)

  const [paper, single] = grouped
  assert.deepEqual(paper.properties?.className, ['footnote-paper'])
  assert.equal(paper.properties?.dataPaperLabel, '1')

  // The bibliographic header appears exactly ONCE for the merged paper.
  assert.equal(childrenOf(paper, 'p').filter((n) => textOf(n) === HEADER).length, 1)
  assert.match(textOf(paper), /2 cited excerpts/)

  // Both excerpts survive, nested, each labelled and keeping its own anchor id.
  const excerpts = childrenOf(childrenOf(paper, 'ol')[0], 'li')
  assert.deepEqual(
    excerpts.map((e) => e.properties?.dataExcerptLabel),
    ['1.1', '1.2'],
  )
  assert.deepEqual(
    excerpts.map((e) => e.properties?.id),
    ['user-content-user-content-fn-1-1', 'user-content-user-content-fn-1-2'],
  )
  assert.match(textOf(excerpts[1]), /redirects to necroptosis/)

  // A paper cited through ONE chunk stays FLAT — its own bib entry with its one
  // "Cited excerpt" directly beneath, exactly as it renders today. No nested
  // sub-list, no excerpt-count caption; only the paper NUMBER is stamped so the
  // entry cannot drift from the marker pointing at it.
  assert.equal(single.properties?.dataPaperLabel, '2')
  assert.equal(single.properties?.id, 'user-content-user-content-fn-2')
  assert.doesNotMatch(textOf(single), /cited excerpts/)
  assert.equal(childrenOf(single, 'ol').length, 0)
  assert.equal(childrenOf(single, 'blockquote').length, 1)
})

test('groups by paper key, not adjacency, when papers are interleaved', () => {
  // The <ol> is ordered by first REFERENCE, so alternating citations interleave.
  const grouped = groupFootnoteList([
    def('1-1', HEADER, 'first'),
    def('2-1', 'Other paper.', 'other first'),
    def('1-2', HEADER, 'second'),
    def('2-2', 'Other paper.', 'other second'),
  ])
  assert.ok(grouped)

  assert.equal(grouped.length, 2)
  assert.deepEqual(
    grouped.map((g) => g.properties?.dataPaperLabel),
    ['1', '2'],
  )
  // Paper 1 reclaimed its non-adjacent second chunk.
  const first = childrenOf(childrenOf(grouped[0], 'ol')[0], 'li')
  assert.deepEqual(
    first.map((e) => e.properties?.dataExcerptLabel),
    ['1.1', '1.2'],
  )
})

// --- the degradation guarantees ---------------------------------------------

test('BAILS on an ordinary sequential footnote set (citations / knowledge_base)', () => {
  // No label carries a chunk part, so this is not paper-grouped input.
  const items = [
    def('1', 'A reference body.', 'excerpt one'),
    def('2', 'Another reference.', 'excerpt two'),
    def('3', 'A third.', 'excerpt three'),
  ]
  assert.equal(groupFootnoteList(items), undefined)
})

test('BAILS on a label it cannot parse, rather than renumbering it', () => {
  // A named footnote.
  assert.equal(
    groupFootnoteList([def('1-1', HEADER, 'a'), def('note', 'Named.', 'b')]),
    undefined,
  )
  // A three-part label is not the P-C shape this owns.
  assert.equal(
    groupFootnoteList([def('1-1', HEADER, 'a'), def('1-2-3', HEADER, 'b')]),
    undefined,
  )
  // An <li> with no id at all.
  assert.equal(
    groupFootnoteList([
      def('1-1', HEADER, 'a'),
      { type: 'element', tagName: 'li', children: [p('no id')] },
    ]),
    undefined,
  )
})

test('BAILS on an empty list rather than emitting an empty <ol>', () => {
  assert.equal(groupFootnoteList([]), undefined)
  assert.equal(groupFootnoteList([{ type: 'text', value: '\n' }]), undefined)
})

// --- flat labels inside a grouped answer -------------------------------------

test('stamps EVERY ref with its own label, so a flat one is not shown as its ordinal', () => {
  // The reason this exists: a paper cited through one chunk keeps the flat
  // label "2", but GFM numbers by first-reference order, so that ref sits at
  // ordinal position 3 (after 1-1 and 1-2) and would DISPLAY as "3" — against
  // a reference entry numbered 2. Stamping fixes flat and hierarchical alike.
  const ref = (label: string, ordinal: string): N => ({
    type: 'element',
    tagName: 'a',
    properties: {
      dataFootnoteRef: true,
      href: `#user-content-fn-${label}`,
      id: `user-content-user-content-fnref-${label}`,
    },
    children: [{ type: 'text', value: ordinal }],
  })
  const tree: N = {
    type: 'element',
    tagName: 'div',
    children: [ref('1-1', '1'), ref('1-2', '2'), ref('2', '3')],
  }
  stampFootnoteRefLabels(tree)
  assert.deepEqual(
    (tree.children ?? []).map(c => c.properties?.dataFootnoteDisplay),
    ['1.1', '1.2', '2'],
  )
})

// --- comma between co-located citations --------------------------------------

const refSup = (): N => ({
  type: 'element',
  tagName: 'sup',
  children: [
    {
      type: 'element',
      tagName: 'a',
      properties: { dataFootnoteRef: true },
      children: [{ type: 'text', value: '1' }],
    },
  ],
})
const plainSup = (): N => ({
  type: 'element',
  tagName: 'sup',
  children: [{ type: 'text', value: '2' }],
})
const marks = (para: N) =>
  (para.children ?? []).map(c =>
    ((c.properties?.className as string[]) ?? []).includes('footnote-ref-adjacent'),
  )

test('marks only TRULY adjacent citation superscripts', () => {
  // [^1][^2][^3] — a real run: 2nd and 3rd get the comma, the 1st does not.
  const run: N = { type: 'element', tagName: 'p', children: [refSup(), refSup(), refSup()] }
  markAdjacentFootnoteRefs(run)
  assert.deepEqual(marks(run), [false, true, true])
})

test('does NOT mark a later citation separated by text (the CSS sibling trap)', () => {
  // "…of these[^1]. Bypass signalling…[^2]" — `sup + sup` in CSS WOULD match
  // these and print a spurious leading comma before the second citation.
  const para: N = {
    type: 'element',
    tagName: 'p',
    children: [
      { type: 'text', value: 'of these' },
      refSup(),
      { type: 'text', value: '. Bypass signalling is a second route ' },
      refSup(),
      { type: 'text', value: '.' },
    ],
  }
  markAdjacentFootnoteRefs(para)
  assert.deepEqual(marks(para), [false, false, false, false, false])
})

test('never marks a real exponent, and leaves a lone citation alone', () => {
  const para: N = {
    type: 'element',
    tagName: 'p',
    children: [{ type: 'text', value: 'E = mc' }, plainSup(), plainSup(), refSup()],
  }
  markAdjacentFootnoteRefs(para)
  assert.deepEqual(marks(para), [false, false, false, false])
})

// The trap that cost a rewrite: Streamdown splits a message into blocks BEFORE
// parsing, and a footnote definition labelled `[^1.1]:` makes the splitter cut
// the message apart — the definitions land in different blocks from the markers
// referencing them, GFM binds nothing, and every marker renders as literal
// `[^1.1]` text with no References section at all. A hyphen keeps it whole.
// This is why the wire label is `1-1` and only the DISPLAY uses a dot.
test('the wire label must not contain a dot — it breaks Streamdown block splitting', () => {
  const answer = (labels: string[]) =>
    `Text ${labels.map(l => `[^${l}]`).join('')}.\n\n` +
    labels.map(l => `[^${l}]: Author (2020). Title.\n    > An excerpt.`).join('\n\n')

  // What we ship: one block, so refs and definitions parse together.
  assert.equal(parseMarkdownIntoBlocks(answer(['1-1', '1-2', '2-1'])).length, 1)
  // The ordinary sequential case is unaffected either way.
  assert.equal(parseMarkdownIntoBlocks(answer(['1', '2'])).length, 1)
  // The dotted form the design originally called for: shattered.
  assert.ok(parseMarkdownIntoBlocks(answer(['1.1', '1.2', '2.1'])).length > 1)
})

test('a definition with no blockquote yields an empty excerpt, not a crash', () => {
  const grouped = groupFootnoteList([
    { ...def('1-1', HEADER, 'x'), children: [p(HEADER)] },
    def('1-2', HEADER, 'has an excerpt'),
  ])
  assert.ok(grouped)
  const excerpts = childrenOf(childrenOf(grouped[0], 'ol')[0], 'li')
  assert.equal(excerpts.length, 2)
  assert.deepEqual(excerpts[0].children, [])
})
