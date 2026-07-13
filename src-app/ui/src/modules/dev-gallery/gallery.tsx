/**
 * Dev-gallery seed for the `dev-gallery` module itself — the imperative
 * DialogHost open-states + the gallery-local demo surfaces (artifact-canvas
 * editors, the defect-repro fixture, the kit-Table / tabular-viewer demos, and
 * the long message-list virtualizer surface). Auto-discovered by the gallery's
 * runtime registry (`@/dev/gallery/support`); never imported by `module.tsx`, so
 * it is dev-only and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { lazyNamed, lazyProps } from '@/dev/gallery/support'
import { dialog } from '@/components/ui'

export const gallery: ModuleGallery = {
  overlays: [
    {
      // <DialogHost/> singleton with a DESCRIBED alert → the open AlertDialog (:94)
      // + the `description != null` arm of the aria-describedby spread (:95).
      slug: 'overlay-dialog-host-described',
      surface: 'components/ui/kit/dialog-host',
      title: 'Imperative dialog — described',
      component: lazyNamed(() => import('@/components/ui'), 'DialogHost'),
      open: () => {
        void dialog.info({
          title: 'Heads up',
          description: 'A described alert dialog.',
          okText: 'OK',
          testid: 'gallery-dialog-with-desc',
        })
      },
    },
    {
      // Bare alert (no description) → the `description == null` arm of :95
      // (aria-describedby explicitly undefined). Separate frame: two simultaneously
      // -open Radix AlertDialogs don't both mount.
      slug: 'overlay-dialog-host-bare',
      surface: 'components/ui/kit/dialog-host',
      title: 'Imperative dialog — bare (no description)',
      component: lazyNamed(() => import('@/components/ui'), 'DialogHost'),
      open: () => {
        void dialog.warning({
          title: 'Bare alert (no description)',
          okText: 'OK',
          testid: 'gallery-dialog-no-desc',
        })
      },
    },
  ],
  seeded: [
    // ── Artifact canvas EDITORS — the deliverable-editing surfaces. Rendered with
    //    fixed sample content (prop-taking components) so runtime-health drives the
    //    real Plate WYSIWYG + toolbar, the CodeMirror editor, and the editable CSV
    //    grid in a real browser (console errors / AA-contrast / a11y-name). ─────────
    {
      slug: 'seeded-artifact-canvas-markdown',
      title: 'Artifact canvas — markdown editor (Plate + toolbar)',
      note: 'the WYSIWYG deliverable editor: formatting toolbar + rendered GFM content',
      path: '/gallery/artifact-md',
      initialPath: '/gallery/artifact-md',
      component: lazyProps(
        () => import('@/components/kit/editor/KitMarkdownEditor'),
        'KitMarkdownEditor',
        {
          initialMarkdown:
            '# Assay Methods\n\nSamples were prepared with **care** and *precision* using `buffer A`.\n\n- RNA extraction\n- Reverse transcription\n\n> Keep samples on ice.\n',
        },
      ),
    },
    {
      slug: 'seeded-artifact-canvas-image',
      title: 'Artifact canvas — markdown editor with an embedded image',
      note: 'the WYSIWYG editor rendering a pasted image node (ITEM-21) — a data-URL src so the cell needs no network',
      path: '/gallery/artifact-image',
      initialPath: '/gallery/artifact-image',
      component: lazyProps(
        () => import('@/components/kit/editor/KitMarkdownEditor'),
        'KitMarkdownEditor',
        {
          initialMarkdown:
            '# Figure 1\n\nThe assay result:\n\n![result](data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==)\n\nCaption below.\n',
        },
      ),
    },
    {
      slug: 'seeded-artifact-canvas-code',
      title: 'Artifact canvas — code editor (CodeMirror)',
      note: 'plain-text code editor for a code deliverable',
      path: '/gallery/artifact-code',
      initialPath: '/gallery/artifact-code',
      component: lazyProps(
        () => import('@/components/kit/editor/KitCodeEditor'),
        'KitCodeEditor',
        {
          initialText: 'def hello(name):\n    print(f"hi {name}")\n\nhello("world")\n',
        },
      ),
    },
    // ── DEFECT-REPRO: the detection system's own known-positive fixture. Renders
    // the canonical human-caught geometry misses (starting with A1 zero-gap: the
    // hardware "Disconnected"/"Connect" pair) so the Layer-1 detectors have a
    // permanent regression target. INTENTIONALLY defective — allow-listed for the
    // gate, still reported (see docs/DEFECT_TAXONOMY.md + geometry-allowlist.json).
    {
      slug: 'seeded-defect-repro',
      title: 'Defect repro — detection-system known positives',
      note: 'every geometric/runtime taxonomy miss (#1-21) as an intentional fixture cell',
      path: '/',
      initialPath: '/',
      fullHeight: true,
      component: lazyNamed(() => import('@/dev/gallery/DefectRepro'), 'DefectRepro'),
    },
    // ── kit-Table + tabular-viewer demos as ISOLATED single surfaces (no overlay
    //    backdrops) so the interactive F1 e2e can click sort/filter/resize/columns/
    //    selection/expand. Store-free → no setup. ──────────────────────────────
    {
      slug: 'seeded-kit-table-actions',
      title: 'kit Table — actions (interactive)',
      note: 'sortable/filterable/resizable/columnChooser/selection demo',
      path: '/',
      initialPath: '/',
      component: lazyNamed(() => import('@/dev/gallery/TableDemos'), 'TableActionsDemo'),
    },
    {
      slug: 'seeded-kit-table-scroll',
      title: 'kit Table — scroll-to-index (interactive)',
      note: 'virtualized jump-to-row demo',
      path: '/',
      initialPath: '/',
      component: lazyNamed(() => import('@/dev/gallery/TableDemos'), 'TableScrollDemo'),
    },
    {
      slug: 'seeded-delimited-viewer',
      title: 'Tabular viewer — CSV (interactive)',
      note: 'real DelimitedTable with sort/filter/jump/expand',
      path: '/',
      initialPath: '/',
      component: lazyNamed(() => import('@/dev/gallery/TableDemos'), 'DelimitedViewerDemo'),
    },
    {
      slug: 'seeded-delimited-viewer-shell',
      title: 'Tabular viewer — CSV with header actions',
      note: 'DelimitedHeader (view-aware Export / Copy-selection) over the real DelimitedTable',
      path: '/',
      initialPath: '/',
      component: lazyNamed(() => import('@/dev/gallery/TableDemos'), 'DelimitedViewerWithHeaderDemo'),
    },
    {
      slug: 'seeded-xlsx-viewer',
      title: 'Tabular viewer — XLSX sheet (interactive)',
      note: 'real XlsxSheet with sort/filter',
      path: '/',
      initialPath: '/',
      component: lazyNamed(() => import('@/dev/gallery/TableDemos'), 'XlsxViewerDemo'),
    },
    {
      slug: 'seeded-delimited-viewer-large',
      title: 'Tabular viewer — large CSV (interactive)',
      note: 'DelimitedTable, >10k rows: row-virtualized, whole-set sort/filter, no truncation',
      path: '/',
      initialPath: '/',
      component: lazyNamed(() => import('@/dev/gallery/TableDemos'), 'LargeDelimitedViewerDemo'),
    },
    {
      slug: 'seeded-rawcode-large',
      title: 'Text/code viewer — large file (interactive)',
      note: 'RawCodeView, thousands of lines: chunk-on-demand Shiki highlight, lifted line cap',
      path: '/',
      initialPath: '/',
      component: lazyNamed(() => import('@/dev/gallery/TableDemos'), 'LargeRawCodeViewDemo'),
    },
    // ── MessageList: a ~500-message MIXED conversation driving the REAL
    //    virtualizer in a scroll box. Reproduces the jitter root cause (variable-
    //    height inline content) so the scroll-stability e2e can measure the
    //    correction counter + assert show-more/resize persistence
    //    (message-scroll-stability ITEM-1). ───────────────────────────────────────
    {
      slug: 'seeded-message-list-long',
      title: 'Message list — long mixed conversation (interactive)',
      note: '500 mixed messages (long collapsible, tables, images, inline files) → virtualizer scroll-stability surface',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () => import('@/dev/gallery/MessageListLongDemo'),
        'MessageListLongDemo',
      ),
    },
  ],
}
