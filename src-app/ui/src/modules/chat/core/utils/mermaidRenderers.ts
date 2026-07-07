import type { PluginConfig } from 'streamdown'
import { MermaidBlock } from '@/components/common/MermaidBlock'

/**
 * Streamdown plugin config that registers our mermaid custom renderer. Streamdown
 * resolves a per-language `plugins.renderers` entry BEFORE its built-in mermaid
 * path, so this fully replaces the library's mermaid rendering with the
 * code⇄render toggle (+ copy-source + download-svg) in `MermaidBlock`.
 *
 * Shared by every chat `<Streamdown>` instance (both `TextContent` render paths)
 * so the affordance is identical everywhere chat markdown renders.
 */
export const mermaidRenderers: PluginConfig = {
  renderers: [{ language: 'mermaid', component: MermaidBlock }],
}
