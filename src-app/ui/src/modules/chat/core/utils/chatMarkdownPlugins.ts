import type { PluginConfig } from 'streamdown'
import { HtmlBlock } from './HtmlBlock'
import { STREAMDOWN_PLUGINS } from '@/components/common/streamdownPlugins'

/**
 * Streamdown `plugins` config for the CHAT markdown renderers. Starts from the
 * shared STREAMDOWN_PLUGINS (Shiki code highlighting + KaTeX math + the mermaid
 * code⇄render toggle) and adds the chat-only HTML sandboxed-iframe renderer
 * (fenced ```html / ```htm → HtmlBlock, with its own code⇄render toggle). The
 * languages are disjoint (mermaid vs html/htm), so the `renderers` arrays
 * concatenate cleanly; every other fence still falls through to Streamdown's
 * Shiki CodeBlock via the `code` plugin.
 *
 * Imported by BOTH chat <Streamdown> call sites so the two render paths stay in
 * lockstep. Non-chat renderers (file viewer, skill/workflow output) use
 * STREAMDOWN_PLUGINS directly — no HTML iframe there.
 */
export const chatMarkdownPlugins: PluginConfig = {
  ...STREAMDOWN_PLUGINS,
  renderers: [
    ...(STREAMDOWN_PLUGINS.renderers ?? []),
    { component: HtmlBlock, language: ['html', 'htm'] },
  ],
}
