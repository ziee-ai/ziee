/**
 * Project Extensions Auto-Discovery
 *
 * Sibling-module extensions register knowledge kinds (and any future
 * project slots) by exporting a default `register(...)` callback from
 *
 *   `modules/<name>/project-extension/extension.tsx`
 *
 * Mirrors the chat-extension auto-discovery pattern at
 * `modules/chat/extensions/index.ts:50-58`. The glob is evaluated
 * eagerly at app boot so each extension's `register(...)` call runs
 * before any project page renders.
 *
 * Bootstrapped by `modules/projects/module.tsx` via
 * `import '@/modules/projects/extensions'`.
 *
 * Acid-test invariant: if no sibling module exposes a
 * `project-extension/extension.tsx`, the glob returns zero modules and
 * the project knowledge area renders empty (each slot host treats
 * missing contributions as "no knowledge of this kind yet"). The
 * projects module itself never imports any sibling-extension code —
 * delete `modules/file/` and the projects module still works.
 */

const extensions = import.meta.glob<unknown>(
  '../../*/project-extension/extension.tsx',
  { eager: true },
)

console.log(
  `[Project Extensions] Auto-discovered ${Object.keys(extensions).length} sibling-module project-extension(s)`,
)
for (const path of Object.keys(extensions)) {
  console.log(`[Project Extensions] Loaded: ${path}`)
}
