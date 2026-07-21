import type { PluggableList } from 'unified'
import { type PluginConfig, defaultRehypePlugins } from 'streamdown'
import { HtmlBlock } from './HtmlBlock'
import { STREAMDOWN_PLUGINS } from '@/components/common/streamdownPlugins'
import { rehypeGroupPaperFootnotes } from './rehypeGroupPaperFootnotes'

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

/**
 * Streamdown `rehypePlugins` for the CHAT markdown renderers.
 *
 * ⚠ Passing `rehypePlugins` REPLACES Streamdown's default chain outright — it is
 * `props.rehypePlugins || defaults`, not a merge. Dropping the defaults would
 * silently disable HTML sanitization, so `defaultRehypePlugins`
 * (rehype-raw → rehype-sanitize → rehype-harden) is spread back in FIRST and the
 * footnote grouper appended LAST. It therefore only ever sees already-sanitized
 * nodes. Anything added here must keep that order.
 *
 * Exported as ONE constant consumed by BOTH chat <Streamdown> call sites, for
 * the same reason `chatMarkdownPlugins` above is: the two render paths must not
 * drift. Non-chat renderers (file viewer, skill/workflow output) pass no
 * `rehypePlugins` and keep Streamdown's defaults untouched.
 */
export const chatRehypePlugins: PluggableList = [
  ...Object.values(defaultRehypePlugins),
  rehypeGroupPaperFootnotes,
]
