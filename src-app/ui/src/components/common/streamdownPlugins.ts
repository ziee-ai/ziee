import { code } from '@streamdown/code'
import { math } from '@streamdown/math'
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
  code,
  math,
  renderers: [{ language: 'mermaid', component: MermaidBlock }],
}
