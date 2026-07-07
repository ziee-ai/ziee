import {
  ArrowRight,
  Copy,
  Download,
  ExternalLink,
  Maximize2,
  Star,
} from 'lucide-react'
import { Tag, Button } from '@/components/ui'

/**
 * DEFECT-REPRO — the detection system's living KNOWN-POSITIVE fixture suite.
 *
 * ROOT CAUSE this closes (the "trust the instrument" gap): a geometry/runtime
 * detector that returns 0 findings is indistinguishable from a detector that is
 * silently broken/mis-scoped. Several taxonomy `[G]` classes were reported as
 * "0 findings app-wide" not because the app is clean but because (a) the bad
 * STATE was never rendered anywhere the audit could scan, and (b) the detector
 * itself was a no-op / mis-scoped. This surface renders EVERY geometrically /
 * runtime-expressible taxonomy miss (`docs/DEFECT_TAXONOMY.md`, user misses
 * #1-21) as an INTENTIONALLY-defective, individually-`data-testid`'d cell, so
 * `scripts/detector-acceptance.mjs` can prove each detector actually FIRES on
 * its known-bad instance (RED if it does not).
 *
 * Every cell is intentionally defective. Its findings are allow-listed for the
 * geometry `--gate` (see geometry-allowlist.json) but still REPORTED — that is
 * the acceptance proof. The source-lint variants (A1 adjacency, C11 icon-action)
 * opt out with `data-allow-*` so `npm run check` stays green while the RUNTIME
 * detectors still measure the defect.
 *
 * Cell testid convention: `repro-<class>-<slug>`. The acceptance harness matches
 * a finding to a cell by its taxonomy class + this surface + (where the detector
 * names an element) the `repro-<class>` testid substring.
 *
 * This renders `fullHeight` (see SeededSurfaceFrame) — a natural-height column,
 * NOT the standard 720px overflow-hidden frame — so no cell below the fold is
 * spuriously "clipped by the gallery frame" (which would pollute A11/G7).
 */

/** A titled cell wrapper. `data-gallery-chrome` keeps the heading out of the
 *  detector pool so the cell's own content is what gets measured. */
function Cell({
  id,
  title,
  children,
}: {
  id: string
  title: string
  children: React.ReactNode
}) {
  return (
    <section
      data-testid={`repro-cell-${id}`}
      className="flex flex-col gap-2 rounded-lg border border-border p-4"
    >
      <div data-gallery-chrome className="text-sm font-medium text-muted-foreground">
        {title}
      </div>
      {children}
    </section>
  )
}

// Raw TeX / mermaid sources kept as constants so the JSX stays literal.
const RAW_TEX = '$$\\frac{a}{b} = \\sqrt{c}$$'
const RAW_MERMAID = 'graph TD;\n  A[Start] --> B[End]'
const PLAIN_CODE = 'const answer = 42\nfunction go() { return answer }'

export function DefectRepro() {
  return (
    <div
      className="flex flex-col gap-6 p-6"
      data-testid="defect-repro-root"
    >
      {/* ── #1 · A1 zero-gap adjacency (hardware Disconnected/Connect) ─────── */}
      <Cell id="a1-zero-gap" title="A1 · zero-gap adjacency (Disconnected/Connect)">
        {/* `data-allow-adjacent` opts the SOURCE lint out; the RUNTIME geometry
            audit still measures the touching boxes and flags A1. */}
        <div className="flex items-center flex-wrap" data-allow-adjacent data-testid="repro-a1-zero-gap">
          <Tag variant="outline" tone="error" data-testid="repro-a1-tag">
            Disconnected
          </Tag>
          <Button variant="default" data-testid="repro-a1-button">
            Connect
          </Button>
        </div>
      </Cell>

      {/* ── #2/#3 · B1 premature wrap (header + button fit on one row) ─────── */}
      <Cell id="b1-premature-wrap" title="B1 · premature wrap (fits on one row)">
        <div
          data-testid="repro-b1-row"
          className="flex flex-wrap items-center gap-2"
          style={{ width: 420 }}
        >
          <span className="text-base font-semibold" data-testid="repro-b1-title">
            My Assistants
          </span>
          {/* zero-height full-width flex item forces a wrap break even though the
              two real children fit side-by-side (Σwidths ≪ container). */}
          <div className="basis-full" style={{ height: 0 }} aria-hidden />
          <Button variant="outline" data-testid="repro-b1-action">
            New
          </Button>
        </div>
      </Cell>

      {/* ── #4 · C1 status badge ordered BEFORE its label ─────────────────── */}
      <Cell id="c1-badge-before-label" title="C1 · badge before its label ((verified) key)">
        <div className="flex items-center gap-2" data-testid="repro-c1-row">
          <span
            data-slot="badge"
            data-testid="repro-c1-badge"
            className="inline-flex items-center rounded bg-muted px-1.5 py-0.5 text-xs"
          >
            verified
          </span>
          <span data-testid="repro-c1-label">vaswani2017attention</span>
        </div>
      </Cell>

      {/* ── #5 · G7 focus-ring / elevation clipped by overflow ancestor ───── */}
      <Cell id="g7-clipped-ring" title="G7 · focus ring clipped (flush in overflow-hidden)">
        <div className="overflow-hidden" style={{ width: 160 }} data-testid="repro-g7-clip">
          {/* persistent ring (box-shadow) + flush-left inside an overflow-hidden
              box → the left of the ring is cut. measureRing reads it at rest. */}
          <button
            type="button"
            data-testid="repro-g7-btn"
            data-allow-custom-color
            className="rounded bg-primary px-3 py-1 text-primary-foreground"
            style={{ boxShadow: '0 0 0 4px rgba(220,20,60,0.9)' }}
          >
            Focus me
          </button>
        </div>
      </Cell>

      {/* ── #6 · C7 indistinguishable roles (user vs assistant identical) ──── */}
      <Cell id="c7-indistinct-roles" title="C7 · user vs assistant look identical">
        <div className="flex flex-col gap-2">
          <div
            data-role="repro-usr"
            data-testid="repro-c7-user"
            className="rounded bg-background p-2 text-foreground"
          >
            A user message.
          </div>
          <div
            data-role="repro-asst"
            data-testid="repro-c7-assistant"
            className="rounded bg-background p-2 text-foreground"
          >
            An assistant message.
          </div>
        </div>
      </Cell>

      {/* ── #7a · C9 icon/label split across lines (fits on one) ───────────── */}
      <Cell id="c9-icon-label-split" title="C9 · icon + label on different lines">
        <div
          data-testid="repro-c9-row"
          className="flex flex-wrap items-center gap-1 rounded bg-warning/10 p-2 text-warning"
          style={{ width: 260 }}
        >
          <Star size={16} aria-hidden />
          <div className="basis-full" style={{ height: 0 }} aria-hidden />
          <span data-testid="repro-c9-label">Tool Approval Required</span>
        </div>
      </Cell>

      {/* ── #7b · C10 icon disproportionate to adjacent text ──────────────── */}
      <Cell id="c10-icon-oversized" title="C10 · icon 2.4× its text line-height">
        <div className="flex items-center gap-2">
          <Star data-testid="repro-c10-icon" width={48} height={48} aria-hidden />
          <span className="text-sm leading-5">Tool Approval Required</span>
        </div>
      </Cell>

      {/* ── #8 · K1 persistent context inside a scroll container ───────────── */}
      <Cell id="k1-context-in-scroll" title="K1 · context chip scrolls out of view">
        <div
          className="overflow-y-auto rounded border border-border"
          style={{ height: 96 }}
          data-testid="repro-k1-scroller"
        >
          <div style={{ height: 400 }} className="p-2">
            <div data-testid="conversation-title" className="text-sm font-medium">
              In project: Attention Research
            </div>
            <p className="text-xs text-muted-foreground">…long conversation…</p>
          </div>
        </div>
      </Cell>

      {/* ── #9b · I5 horizontal strip scrolls VERTICALLY ──────────────────── */}
      <Cell id="i5-vertical-strip" title="I5 · tablist scrolls vertically">
        <div
          role="tablist"
          data-testid="repro-i5-tablist"
          className="flex flex-col overflow-y-auto rounded border border-border"
          style={{ height: 28 }}
        >
          <button role="tab" className="px-2 py-1 text-sm">Files</button>
          <button role="tab" className="px-2 py-1 text-sm">Literature</button>
          <button role="tab" className="px-2 py-1 text-sm">Memory</button>
        </div>
      </Cell>

      {/* ── #9c · A8 strip children not vertically centered ────────────────── */}
      <Cell id="a8-misaligned-strip" title="A8 · tab off the row center line">
        <div
          role="tablist"
          data-testid="repro-a8-tablist"
          className="flex items-center rounded border border-border px-2"
          style={{ height: 44 }}
        >
          <button role="tab" className="px-2 text-sm">Files</button>
          <button role="tab" className="px-2 text-sm">Literature</button>
          {/* pushed off the shared center line */}
          <button role="tab" className="px-2 text-sm" style={{ marginTop: 16 }}>
            Memory
          </button>
        </div>
      </Cell>

      {/* ── #10a · J6 mixed button variants in one action group ────────────── */}
      <Cell id="j6-mixed-variants" title="J6 · outline + ghost icon peers">
        <div data-testid="repro-j6-group" className="flex items-center gap-1">
          <button
            type="button"
            aria-label="download"
            className="flex h-8 w-8 items-center justify-center rounded border border-border bg-background"
          >
            <Download size={16} aria-hidden />
          </button>
          <button
            type="button"
            aria-label="open"
            className="flex h-8 w-8 items-center justify-center rounded bg-transparent hover:bg-accent"
          >
            <ExternalLink size={16} aria-hidden />
          </button>
        </div>
      </Cell>

      {/* ── #11a · L1 math fell back to raw TeX ────────────────────────────── */}
      <Cell id="l1-raw-math" title="L1 · raw $$…$$ (no KaTeX)">
        <div className="prose" data-testid="repro-l1-prose">
          <p>{RAW_TEX}</p>
        </div>
      </Cell>

      {/* ── #11b · L2 mermaid fell back to raw source ──────────────────────── */}
      <Cell id="l2-raw-mermaid" title="L2 · raw mermaid (no <svg>)">
        <div className="prose" data-testid="repro-l2-prose">
          <pre>
            <code data-testid="repro-l2-code">{RAW_MERMAID}</code>
          </pre>
        </div>
      </Cell>

      {/* ── #11c · L3 syntax highlighting absent ───────────────────────────── */}
      <Cell id="l3-no-highlight" title="L3 · language-tagged code, single color">
        <div className="prose">
          <pre className="language-js" data-testid="repro-l3-pre">
            <code className="language-js">{PLAIN_CODE}</code>
          </pre>
        </div>
      </Cell>

      {/* ── #12 · J7 same action on opposite sides across containers ───────── */}
      <Cell id="j7-inconsistent-side" title="J7 · copy on LEFT here, RIGHT there">
        <div className="flex flex-col gap-2">
          <div data-slot="card" className="flex justify-start rounded border border-border p-2">
            <button
              type="button"
              data-testid="repro-j7-copy-left"
              aria-label="copy"
              className="flex h-7 w-7 items-center justify-center rounded"
            >
              <Copy size={15} aria-hidden />
            </button>
          </div>
          <div data-slot="card" className="flex justify-end rounded border border-border p-2">
            <button
              type="button"
              data-testid="repro-j7-copy-right"
              aria-label="copy"
              className="flex h-7 w-7 items-center justify-center rounded"
            >
              <Copy size={15} aria-hidden />
            </button>
          </div>
        </div>
      </Cell>

      {/* ── #13a · C12 bare placeholder avatar circle ─────────────────────── */}
      <Cell id="c12-bare-avatar" title="C12 · avatar circle with no content">
        <div className="flex items-center gap-2">
          <div
            data-testid="repro-c12-avatar"
            className="rounded-full bg-muted"
            style={{ width: 40, height: 40 }}
          />
          <span className="text-sm">A user message.</span>
        </div>
      </Cell>

      {/* ── #15 · A9 peer chips with mismatched icon sizes ─────────────────── */}
      <Cell id="a9-chip-icon-mismatch" title="A9 · footer chips, unequal icon sizes">
        <div data-testid="repro-a9-strip" className="flex items-center gap-2">
          <span
            data-testid="repro-a9-chip-1"
            className="chip inline-flex h-6 items-center gap-1 rounded bg-muted px-2 text-xs"
          >
            <Star width={14} height={14} aria-hidden />
            Memory: auto
          </span>
          <span
            data-testid="repro-a9-chip-2"
            className="chip inline-flex h-6 items-center gap-1 rounded bg-muted px-2 text-xs"
          >
            <Star width={14} height={14} aria-hidden />
            Summary: auto
          </span>
          {/* oversized icon → peer metric mismatch */}
          <span
            data-testid="repro-a9-chip-3"
            className="chip inline-flex h-6 items-center gap-1 rounded bg-muted px-2 text-xs"
          >
            <Star width={22} height={22} aria-hidden />
            Tools: auto
          </span>
        </div>
      </Cell>

      {/* ── #16 · A10 edit-form input collapsed to zero width ──────────────── */}
      <Cell id="a10-collapsed-input" title="A10 · inline rename input collapsed">
        <form data-testid="repro-a10-form" className="flex items-center gap-2">
          {/* an open edit form whose input renders at zero width (the "input
              disappears" / vertical-form bug family). display is NOT none — the
              control is meant to show but has no usable size. */}
          <input
            data-testid="repro-a10-input"
            defaultValue="rename me"
            className="rounded border border-border"
            style={{ width: 0, height: 32, padding: 0, borderWidth: 1 }}
          />
          <Button variant="default" data-testid="repro-a10-save">
            Save
          </Button>
        </form>
      </Cell>

      {/* ── #18 · A11 card border clipped by an overflow ancestor ──────────── */}
      <Cell id="a11-border-clipped" title="A11 · tool-call card border clipped">
        <div className="overflow-hidden rounded" style={{ width: 200 }} data-testid="repro-a11-clip">
          {/* wider-than-parent bordered card inside overflow-hidden → its right
              border is cut. */}
          <div
            data-testid="repro-a11-card"
            className="border border-border p-2"
            style={{ width: 260, borderWidth: 1 }}
          >
            <span className="text-sm">execute_command</span>
          </div>
        </div>
      </Cell>

      {/* ── #21c · A12 outline button crammed against a bordered container ─── */}
      <Cell id="a12-cramped-border" title="A12 · outline button double-border cramp">
        <div
          data-testid="repro-a12-container"
          className="border border-border"
          style={{ padding: 2, borderWidth: 1, width: 160 }}
        >
          <button
            type="button"
            data-testid="repro-a12-btn"
            className="w-full rounded border border-border px-2 py-1 text-sm"
            style={{ borderWidth: 1 }}
          >
            Edit title
          </button>
        </div>
      </Cell>

      {/* ── #21a · G9 hover-only controls shift a persistent sibling ───────── */}
      <Cell id="g9-hover-shift" title="G9 · hover-only actions reserve no space">
        <div data-testid="repro-g9-row" className="flex items-center gap-1">
          <span data-testid="repro-g9-persistent" className="text-sm">
            Branch 2/3
          </span>
          {/* display:none at rest (Tailwind `hidden`) + hover-reveal signature →
              when shown it takes layout and shifts the persistent branch label. */}
          <button
            type="button"
            data-hover-reveal
            className="hidden group-hover:inline-flex h-6 w-6 items-center justify-center rounded"
            aria-label="copy message"
          >
            <Copy size={14} aria-hidden />
          </button>
          <button
            type="button"
            data-hover-reveal
            className="hidden group-hover:inline-flex h-6 w-6 items-center justify-center rounded"
            aria-label="edit message"
          >
            <Maximize2 size={14} aria-hidden />
          </button>
        </div>
      </Cell>

      {/* ── #20 · H7 empty model-select renders nothing ───────────────────── */}
      <Cell id="h7-empty-select" title="H7 · model select shows nothing (no models)">
        <div className="flex items-center gap-3">
          {/* a combobox trigger with no selected value AND no options — the model
              picker when zero providers are configured. Shows literally nothing. */}
          <button
            type="button"
            role="combobox"
            aria-label="model"
            data-slot="select-trigger"
            data-testid="repro-h7-combobox"
            className="h-8 min-w-[140px] rounded border border-border px-2 text-sm"
          />
          {/* a native select with zero <option>s — same empty class */}
          <select
            data-testid="repro-h7-select"
            className="h-8 rounded border border-border px-2 text-sm"
          />
        </div>
      </Cell>

      {/* ── #10b · C11 icon-action mismatch (open-in-new-tab → wrong glyph) ──
          Runtime cue only; the source-lint acceptance uses the fixture under
          __detector_fixtures__. Here the glyph (ArrowRight) does not read as
          "open in new tab" — a vision/lint concern, not a geometric one. */}
      <Cell id="c11-icon-action" title="C11 · open-in-new-tab renders a bare arrow">
        <button
          type="button"
          data-allow-icon
          aria-label="open in new tab"
          data-testid="repro-c11-btn"
          className="flex h-7 w-7 items-center justify-center rounded"
        >
          <ArrowRight size={15} aria-hidden />
        </button>
      </Cell>

      {/* ── #22 · A13 child block breaks the parent's right-alignment axis ───── */}
      <Cell id="a13-align-break" title="A13 · left-packed attachment under a right-aligned message">
        {/* flex COLUMN → cross axis is horizontal; the message uses self-end to
            right-align (matching the real chat bubble). Its attachment block packs
            LEFT, so the file card floats at the far left of the message's width. */}
        <div className="flex flex-col">
          <div className="self-end w-[300px] rounded-lg bg-muted p-2" data-testid="repro-a13-message">
            <div className="text-end text-sm">Here's the spreadsheet</div>
            <div className="flex" data-testid="repro-a13-attachments">
              <div className="w-[90px] rounded border border-border p-2 text-xs" data-testid="repro-a13-file">
                data.csv
              </div>
            </div>
          </div>
        </div>
      </Cell>

      {/* ── #23 · A14 dead space from an over-tall fixed/min height ──────────── */}
      <Cell id="a14-dead-space" title="A14 · small table in a tall fixed-height viewer body">
        {/* mirrors the inline file-viewer body (`h-[min(360px,55vh)]`): a fixed 300px
            box holding a 2-row table leaves a large blank band below the content. */}
        <div className="overflow-auto h-[300px] rounded border border-border" data-testid="repro-a14-deadspace">
          <table className="text-sm">
            <thead>
              <tr><th className="px-2 py-1 text-start">Name</th><th className="px-2 py-1 text-start">Value</th></tr>
            </thead>
            <tbody>
              <tr><td className="px-2 py-1">alpha</td><td className="px-2 py-1">1</td></tr>
              <tr><td className="px-2 py-1">beta</td><td className="px-2 py-1">2</td></tr>
            </tbody>
          </table>
        </div>
      </Cell>
    </div>
  )
}
