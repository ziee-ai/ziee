import type { PluginConfig } from 'streamdown'
import { HtmlBlock } from './HtmlBlock'

/**
 * Shared Streamdown `plugins` config for the chat markdown renderers.
 *
 * Registers `HtmlBlock` as the custom renderer for fenced ```html code blocks
 * (Streamdown's `plugins.renderers` seam — the same hook its mermaid plugin
 * uses). Only the `html` language is intercepted; every other fence still falls
 * through to Streamdown's built-in Shiki `CodeBlock` (driven by `shikiTheme`),
 * so we do NOT set `plugins.code` and highlighting is unaffected.
 *
 * Imported by BOTH chat `<Streamdown>` call sites
 * (`components/TextContent.tsx` + `extensions/text/components/TextContent.tsx`)
 * so the two render paths stay in lockstep.
 */
export const streamdownPlugins: PluginConfig = {
  renderers: [{ component: HtmlBlock, language: 'html' }],
}
