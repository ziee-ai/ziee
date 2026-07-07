import type { PluginConfig } from 'streamdown'
import { mermaidRenderers } from './mermaidRenderers'
import { streamdownPlugins } from './streamdownPlugins'

/**
 * Combined Streamdown plugin config for chat markdown: unions the custom
 * renderers from both the mermaid toggle (language `mermaid`) and the HTML
 * sandboxed-iframe block (languages `html`/`htm`). Disjoint languages, so the
 * `renderers` arrays concatenate cleanly.
 */
export const chatMarkdownPlugins: PluginConfig = {
  ...streamdownPlugins,
  ...mermaidRenderers,
  renderers: [
    ...(mermaidRenderers.renderers ?? []),
    ...(streamdownPlugins.renderers ?? []),
  ],
}
