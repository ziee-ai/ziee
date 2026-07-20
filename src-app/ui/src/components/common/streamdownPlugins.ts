// These prebuilt stylesheets ride the LAZY Streamdown chunk (this module is
// only ever imported dynamically, from LazyStreamdown's variant loaders), so
// ~24 KB of KaTeX CSS + the Streamdown keyframes stay OFF the critical path and
// load only when a markdown surface actually mounts. Moved here out of
// `src/index.css` (they used to load eagerly on every page).
import 'streamdown/styles.css'
import 'katex/dist/katex.min.css'
import { createCodePlugin } from '@streamdown/code'
import { createMathPlugin } from '@streamdown/math'
import { MermaidBlock } from '@/components/common/MermaidBlock'
import type { ComponentProps } from 'react'
import type { Streamdown } from 'streamdown'

/**
 * Streamdown v2 extracted code-highlighting (Shiki) and math (KaTeX) out of the
 * core package into optional `@streamdown/*` plugin packages that only take
 * effect when passed via the `plugins` prop — the `shikiTheme` prop alone just
 * picks the THEME once a code plugin is active. Missing this wiring is why raw
 * LaTeX and unhighlighted code blocks rendered as plain text.
 *
 * Mermaid is handled by a custom `renderers` entry (MermaidBlock) rather than the
 * @streamdown/mermaid plugin, so every diagram gets the code⇄render toggle plus
 * copy-source / download-svg affordances. Streamdown resolves a per-language
 * `renderers` entry BEFORE its built-in mermaid path, so this fully owns mermaid.
 *
 * Shared here so every Streamdown call site (chat text renderer, file markdown
 * viewer, skill/workflow output) enables the identical set. The KaTeX stylesheet
 * + Tailwind `@source` globs for these dists live in `src/index.css`.
 */
export const STREAMDOWN_PLUGINS: NonNullable<
  ComponentProps<typeof Streamdown>['plugins']
> = {
  // The @streamdown/code plugin carries its OWN theme pair and IGNORES Streamdown's
  // `shikiTheme` prop, so the themes must be set HERE. github-light/dark's default
  // tokens (e.g. the #E36209 orange on white, the #6A737D comment on near-black)
  // fail WCAG AA; GitHub's high-contrast variants are the accessible equivalents.
  code: createCodePlugin({
    themes: ['github-light-high-contrast', 'github-dark-high-contrast'],
  }),
  // singleDollarTextMath: enable INLINE math with single `$…$` (default off, which
  // rendered `$E = mc^2$` as literal text). Block `$$…$$` already worked.
  math: createMathPlugin({ singleDollarTextMath: true }),
  renderers: [{ language: 'mermaid', component: MermaidBlock }],
}
