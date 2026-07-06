/**
 * Shard 2 seeded-surface entries (parallel gap grind).
 *
 * OWNED BY SHARD 2 ONLY — File viewers + project files (`modules/file/**`).
 * Add `SeededSurfaceEntry` objects for your assigned gaps here. Import helpers
 * from './helpers'. Prefix every slug with `seeded-s2-` so slugs never collide
 * across shards. Do NOT edit seededSurfaces.tsx, overlays.tsx, main.tsx,
 * pages.tsx, stories/index.ts, coverage-allowlist.json, or any generated matrix
 * — those are integrator-owned.
 *
 * See /data/pbya/ziee/tmp/gapgrind-shards.md for your assigned gap list.
 */
import type { File as FileEntity } from '@/api-client/types'
import { type SeededSurfaceEntry, holdPatch, lazyProps } from './helpers'

const NOW = '2026-01-01T00:00:00.000Z'

/** Minimal FileEntity for prop-taking file viewers. Overrides let each surface
 *  tweak the fields the branch under test keys on (preview_page_count, etc.). */
const mkFile = (over: Partial<FileEntity> = {}): FileEntity => ({
  id: 'f2000000-0000-0000-0000-000000000001',
  filename: 'sheet.xlsx',
  mime_type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  file_size: 4096,
  blob_version_id: 'fv200000-0000-0000-0000-000000000001',
  current_version_id: 'fv200000-0000-0000-0000-000000000001',
  version: 1,
  has_thumbnail: false,
  preview_page_count: 1,
  text_page_count: 1,
  processing_metadata: {},
  created_by: 'user',
  user_id: 'aaaa0000-0000-0000-0000-000000000001',
  created_at: NOW,
  updated_at: NOW,
  ...over,
})

/** Decode a base64 string to an ArrayBuffer (browser-safe). */
const b64ToBuffer = (b64: string): ArrayBuffer => {
  const bin = atob(b64)
  const u8 = new Uint8Array(bin.length)
  for (let i = 0; i < bin.length; i++) u8[i] = bin.charCodeAt(i)
  return u8.buffer
}

// A corrupt ZIP (valid PK local-file-header magic, then junk). `xlsx.read`
// recognises the ZIP magic, tries to inflate, and throws "Unsupported ZIP file"
// → the viewer's catch sets `loadError` (the error arm). Plain garbage/CSV bytes
// would be silently coerced into a one-sheet CSV workbook, so the magic matters.
const CORRUPT_XLSX = new Uint8Array([
  0x50, 0x4b, 0x03, 0x04, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
  0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff,
  0xff,
]).buffer

// A VALID xlsx zip whose workbook.xml has an empty `<sheets/>` element →
// `wb.SheetNames === []` (parses cleanly, zero sheets). This is the only input
// that reaches the "No data found" empty arm; real files always carry ≥1 sheet,
// and garbage bytes coerce to a synthetic "Sheet1". Prebuilt via jszip.
const ZERO_SHEET_XLSX_B64 =
  'UEsDBAoAAAAAAORq5lyOS3DJqgEAAKoBAAATAAAAW0NvbnRlbnRfVHlwZXNdLnhtbDw/eG1sIHZlcnNpb249IjEuMCIgZW5jb2Rpbmc9IlVURi04IiBzdGFuZGFsb25lPSJ5ZXMiPz4KPFR5cGVzIHhtbG5zPSJodHRwOi8vc2NoZW1hcy5vcGVueG1sZm9ybWF0cy5vcmcvcGFja2FnZS8yMDA2L2NvbnRlbnQtdHlwZXMiPgo8RGVmYXVsdCBFeHRlbnNpb249InJlbHMiIENvbnRlbnRUeXBlPSJhcHBsaWNhdGlvbi92bmQub3BlbnhtbGZvcm1hdHMtcGFja2FnZS5yZWxhdGlvbnNoaXBzK3htbCIvPgo8RGVmYXVsdCBFeHRlbnNpb249InhtbCIgQ29udGVudFR5cGU9ImFwcGxpY2F0aW9uL3htbCIvPgo8T3ZlcnJpZGUgUGFydE5hbWU9Ii94bC93b3JrYm9vay54bWwiIENvbnRlbnRUeXBlPSJhcHBsaWNhdGlvbi92bmQub3BlbnhtbGZvcm1hdHMtb2ZmaWNlZG9jdW1lbnQuc3ByZWFkc2hlZXRtbC5zaGVldC5tYWluK3htbCIvPgo8L1R5cGVzPlBLAwQKAAAAAADkauZcAAAAAAAAAAAAAAAABgAAAF9yZWxzL1BLAwQKAAAAAADkauZcfm/AhSoBAAAqAQAACwAAAF9yZWxzLy5yZWxzPD94bWwgdmVyc2lvbj0iMS4wIiBlbmNvZGluZz0iVVRGLTgiIHN0YW5kYWxvbmU9InllcyI/Pgo8UmVsYXRpb25zaGlwcyB4bWxucz0iaHR0cDovL3NjaGVtYXMub3BlbnhtbGZvcm1hdHMub3JnL3BhY2thZ2UvMjAwNi9yZWxhdGlvbnNoaXBzIj4KPFJlbGF0aW9uc2hpcCBJZD0icklkMSIgVHlwZT0iaHR0cDovL3NjaGVtYXMub3BlbnhtbGZvcm1hdHMub3JnL29mZmljZURvY3VtZW50LzIwMDYvcmVsYXRpb25zaGlwcy9vZmZpY2VEb2N1bWVudCIgVGFyZ2V0PSJ4bC93b3JrYm9vay54bWwiLz4KPC9SZWxhdGlvbnNoaXBzPlBLAwQKAAAAAADkauZcAAAAAAAAAAAAAAAAAwAAAHhsL1BLAwQKAAAAAADkauZcoERoGegAAADoAAAADwAAAHhsL3dvcmtib29rLnhtbDw/eG1sIHZlcnNpb249IjEuMCIgZW5jb2Rpbmc9IlVURi04IiBzdGFuZGFsb25lPSJ5ZXMiPz4KPHdvcmtib29rIHhtbG5zPSJodHRwOi8vc2NoZW1hcy5vcGVueG1sZm9ybWF0cy5vcmcvc3ByZWFkc2hlZXRtbC8yMDA2L21haW4iIHhtbG5zOnI9Imh0dHA6Ly9zY2hlbWFzLm9wZW54bWxmb3JtYXRzLm9yZy9vZmZpY2VEb2N1bWVudC8yMDA2L3JlbGF0aW9uc2hpcHMiPgo8c2hlZXRzLz4KPC93b3JrYm9vaz5QSwMECgAAAAAA5GrmXAAAAAAAAAAAAAAAAAkAAAB4bC9fcmVscy9QSwMECgAAAAAA5GrmXChGJbCcAAAAnAAAABoAAAB4bC9fcmVscy93b3JrYm9vay54bWwucmVsczw/eG1sIHZlcnNpb249IjEuMCIgZW5jb2Rpbmc9IlVURi04IiBzdGFuZGFsb25lPSJ5ZXMiPz4KPFJlbGF0aW9uc2hpcHMgeG1sbnM9Imh0dHA6Ly9zY2hlbWFzLm9wZW54bWxmb3JtYXRzLm9yZy9wYWNrYWdlLzIwMDYvcmVsYXRpb25zaGlwcyI+PC9SZWxhdGlvbnNoaXBzPlBLAQIUAAoAAAAAAORq5lyOS3DJqgEAAKoBAAATAAAAAAAAAAAAAAAAAAAAAABbQ29udGVudF9UeXBlc10ueG1sUEsBAhQACgAAAAAA5GrmXAAAAAAAAAAAAAAAAAYAAAAAAAAAAAAQAAAA2wEAAF9yZWxzL1BLAQIUAAoAAAAAAORq5lx+b8CFKgEAACoBAAALAAAAAAAAAAAAAAAAAP8BAABfcmVscy8ucmVsc1BLAQIUAAoAAAAAAORq5lwAAAAAAAAAAAAAAAADAAAAAAAAAAAAEAAAAFIDAAB4bC9QSwECFAAKAAAAAADkauZcoERoGegAAADoAAAADwAAAAAAAAAAAAAAAABzAwAAeGwvd29ya2Jvb2sueG1sUEsBAhQACgAAAAAA5GrmXAAAAAAAAAAAAAAAAAkAAAAAAAAAAAAQAAAAiAQAAHhsL19yZWxzL1BLAQIUAAoAAAAAAORq5lwoRiWwnAAAAJwAAAAaAAAAAAAAAAAAAAAAAK8EAAB4bC9fcmVscy93b3JrYm9vay54bWwucmVsc1BLBQYAAAAABwAHAJsBAACDBQAAAAA='

const XLSX_FILE_ID = 'f2000000-0000-0000-0000-000000000001'
const CHROME_FILE_ID = 'f2000000-0000-0000-0000-0000000000ff'

/** Seed one binary-content entry into the File store and hold it. */
const seedBinary = async (id: string, buf: ArrayBuffer) => {
  const { File: FileStoreDef } = await import('@/modules/file/stores/File.store')
  await holdPatch(() => {
    const b = new Map(FileStoreDef.store.getState().fileBinaryContents)
    b.set(id, buf)
    FileStoreDef.store.setState({ fileBinaryContents: b } as any)
  })
}

/** Seed a non-null project + a ProjectFiles state, held. */
const seedProjectFiles = async (patch: Record<string, unknown>) => {
  const { ProjectDetail } = await import(
    '@/modules/projects/stores/ProjectDetail.store'
  )
  const { ProjectFiles } = await import(
    '@/modules/file/project-extension/stores/ProjectFiles.store'
  )
  await holdPatch(() => {
    ProjectDetail.store.setState({
      project: { id: 'proj-s2-0001', name: 'Gallery Project' },
    } as any)
    ProjectFiles.store.setState(patch as any)
  })
}

export const shard2Seeded: SeededSurfaceEntry[] = [
  // ── XlsxBody: parse error (loaded bytes that fail to parse). ────────────────
  {
    slug: 'seeded-s2-xlsx-error',
    title: 'Xlsx viewer — parse error',
    note: 'corrupt workbook bytes → xlsx.read throws → loadError arm (file-xlsx-error)',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () => import('@/modules/file/viewers/tabular/XlsxBody'),
      'XlsxBody',
      { file: mkFile() },
    ),
    setup: () => seedBinary(XLSX_FILE_ID, CORRUPT_XLSX),
  },
  // ── XlsxBody: stuck loading (binary content never resolves). ────────────────
  {
    slug: 'seeded-s2-xlsx-loading',
    title: 'Xlsx viewer — loading',
    note: '!fileBinaryContent → the Loading spinner (id parked in fileBinaryLoadingSet)',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () => import('@/modules/file/viewers/tabular/XlsxBody'),
      'XlsxBody',
      { file: mkFile() },
    ),
    setup: async () => {
      const { File: FileStoreDef } = await import(
        '@/modules/file/stores/File.store'
      )
      // Park the id in the loading set (and out of the content map) so
      // getFileBinaryContent neither returns content nor schedules a load —
      // the viewer holds at the `!fileBinaryContent || loading` spinner.
      await holdPatch(() => {
        const loading = new Set(
          FileStoreDef.store.getState().fileBinaryLoadingSet,
        )
        loading.add(XLSX_FILE_ID)
        const b = new Map(FileStoreDef.store.getState().fileBinaryContents)
        b.delete(XLSX_FILE_ID)
        FileStoreDef.store.setState({
          fileBinaryLoadingSet: loading,
          fileBinaryContents: b,
        } as any)
      })
    },
  },
  // ── XlsxBody: parsed OK but zero sheets → "No data found" empty. ────────────
  {
    slug: 'seeded-s2-xlsx-empty',
    title: 'Xlsx viewer — no sheets',
    note: 'valid workbook with empty <sheets/> → sheets.length===0 → "No data found"',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () => import('@/modules/file/viewers/tabular/XlsxBody'),
      'XlsxBody',
      { file: mkFile() },
    ),
    setup: () => seedBinary(XLSX_FILE_ID, b64ToBuffer(ZERO_SHEET_XLSX_B64)),
  },
  // ── PdfBody: a file with no preview pages → the "Preview not available" arm
  //    (also exercises the effect's `!root || preview_page_count===0` guard). ──
  {
    slug: 'seeded-s2-pdf-empty',
    title: 'PDF viewer — no preview',
    note: 'preview_page_count===0 → "Preview not available for this file"',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () => import('@/modules/file/viewers/pdf/body'),
      'PdfBody',
      {
        file: mkFile({
          filename: 'scan.pdf',
          mime_type: 'application/pdf',
          preview_page_count: 0,
        }),
      },
    ),
  },
  // ── chrome RawToggle: no fileViewModes entry → the `?? 'compiled'` fallback. ─
  {
    slug: 'seeded-s2-chrome-viewmode-fallback',
    title: 'Viewer chrome — view-mode fallback',
    note: 'text_page_count>0 & no fileViewModes entry → mode falls back to compiled',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () => import('@/modules/file/viewers/shared/chrome'),
      'RawToggle',
      {
        file: mkFile({
          id: CHROME_FILE_ID,
          filename: 'notes.md',
          mime_type: 'text/markdown',
          text_page_count: 2,
        }),
      },
    ),
    setup: async () => {
      // Ensure this file has NO fileViewModes entry so the `?? 'compiled'`
      // fallback executes rather than a seeded value.
      const { File: FileStoreDef } = await import(
        '@/modules/file/stores/File.store'
      )
      await holdPatch(() => {
        const m = new Map(FileStoreDef.store.getState().fileViewModes)
        m.delete(CHROME_FILE_ID)
        FileStoreDef.store.setState({ fileViewModes: m } as any)
      })
    },
  },
  // ── FileCard: row upload-error with retry (ERROR badge + retry action). ─────
  {
    slug: 'seeded-s2-filecard-row-error',
    title: 'File card (row) — upload error + retry',
    note: "uploadProgress.status==='error' && onRetry → ERROR badge + retry button",
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () => import('@/modules/file/components/FileCard'),
      'FileCard',
      {
        variant: 'row',
        uploadProgress: {
          id: 'up-s2-row',
          filename: 'report.csv',
          progress: 40,
          status: 'error',
          error: 'Network error while uploading',
          size: 20480,
        },
        onRetry: () => undefined,
        onRemove: () => undefined,
      },
    ),
  },
  // ── FileCard: square upload-error with retry (square retry button). ─────────
  {
    slug: 'seeded-s2-filecard-square-error',
    title: 'File card (square) — upload error + retry',
    note: 'square uploadProgress error && onRetry → the square retry button',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () => import('@/modules/file/components/FileCard'),
      'FileCard',
      {
        variant: 'square',
        uploadProgress: {
          id: 'up-s2-sq',
          filename: 'photo.png',
          progress: 12,
          status: 'error',
          error: 'Upload timed out',
          size: 131072,
        },
        onRetry: () => undefined,
        onRemove: () => undefined,
      },
    ),
  },
  // ── ProjectFilesInlinePreview: first-load spinner (loading & no files). ─────
  {
    slug: 'seeded-s2-project-files-inline-loading',
    title: 'Project knowledge files (inline) — loading',
    note: 'filesLoading && files.length===0 → the load spinner',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () =>
        import(
          '@/modules/file/project-extension/components/ProjectFilesInlinePreview'
        ),
      'ProjectFilesInlinePreview',
      {},
    ),
    setup: () => seedProjectFiles({ files: [], filesLoading: true }),
  },
  // ── ProjectFilesInlinePreview: empty (loaded, no files) → the manage link. ──
  {
    slug: 'seeded-s2-project-files-inline-empty',
    title: 'Project knowledge files (inline) — empty',
    note: '!loading && files.length===0 → "No knowledge files yet" manage link',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () =>
        import(
          '@/modules/file/project-extension/components/ProjectFilesInlinePreview'
        ),
      'ProjectFilesInlinePreview',
      {},
    ),
    setup: () => seedProjectFiles({ files: [], filesLoading: false }),
  },
  // ── ProjectFilesManagePanel: empty (no uploads, no files) → the Empty arm. ──
  {
    slug: 'seeded-s2-project-files-manage-empty',
    title: 'Project knowledge files (manage) — empty',
    note: 'uploadingRows empty (null) + files.length===0 → the Empty state',
    path: '/',
    initialPath: '/',
    component: lazyProps(
      () =>
        import(
          '@/modules/file/project-extension/components/ProjectFilesManagePanel'
        ),
      'ProjectFilesManagePanel',
      {},
    ),
    setup: () =>
      seedProjectFiles({
        files: [],
        filesLoading: false,
        uploadingFiles: new Map(),
        selectedFileIds: new Set(),
      }),
  },
]
