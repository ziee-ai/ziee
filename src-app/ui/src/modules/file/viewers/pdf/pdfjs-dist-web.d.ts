// pdfjs-dist v6 ships no `exports` map and places its `.d.ts` files under
// `types/` (not next to the runtime `web/pdf_viewer.mjs`), so a bare
// `import … from 'pdfjs-dist/web/pdf_viewer.mjs'` resolves the runtime module
// but not its types. Re-point the subpath at the bundled barrel declaration
// (`types/web/pdf_viewer.component.d.ts`, which re-exports PDFViewer /
// EventBus / PDFLinkService / PDFFindController / FindState / …).
//
// This file lives under `ui/src/**`, which BOTH the core-ui and desktop-ui
// tsconfigs include (`desktop/ui/tsconfig.json` has `"../../ui/src"` in
// `include`), so the ambient declaration is visible in both typecheck passes.
declare module 'pdfjs-dist/web/pdf_viewer.mjs' {
  export * from 'pdfjs-dist/types/web/pdf_viewer.component'
}
