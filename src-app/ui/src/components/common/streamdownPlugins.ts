import { code } from '@streamdown/code'
import { math } from '@streamdown/math'
import { mermaid } from '@streamdown/mermaid'
import type { ComponentProps } from 'react'
import type { Streamdown } from 'streamdown'

/**
 * Streamdown v2 extracted code-highlighting (Shiki), math (KaTeX) and Mermaid
 * out of the core package into optional `@streamdown/*` plugin packages. They
 * only take effect when passed via the `plugins` prop — the `shikiTheme` prop
 * alone just picks the THEME once a code plugin is active, and math/mermaid do
 * nothing at all without their plugin. Missing this wiring is why raw LaTeX,
 * raw ```mermaid source, and unhighlighted code blocks rendered as plain text.
 *
 * Shared here so every Streamdown call site (chat text renderer, file markdown
 * viewer, skill/workflow output) enables the identical plugin set. The matching
 * KaTeX stylesheet + Tailwind `@source` globs for these dists live in
 * `src/index.css`.
 */
export const STREAMDOWN_PLUGINS: NonNullable<
  ComponentProps<typeof Streamdown>['plugins']
> = { code, math, mermaid }
