import { message } from '@/components/ui'
import { useEffect } from 'react'
import { Stores } from '@/core/stores'

/**
 * Globally-mounted listener that converts the
 * `llm_model.download_completed` / `.download_failed` EventBus events into
 * antd toast notifications. Rendered once at the app shell so the
 * user gets the toast no matter what page they've navigated to while
 * the download was in flight — the hub model card alone wouldn't
 * cover that case.
 *
 * Mirrors the mount pattern used by `LlmRepositoryDrawer` (registered
 * under `components` in its module — always present in the React
 * tree regardless of the active route).
 *
 * Returns `null` — purely a side-effect component.
 */
export function LlmModelDownloadNotifications() {
  // App.useApp() is the only way to get `message` outside a component
  // context. Mounting this at the module level means we share the
  // top-level `<App>` provider that ConfigProvider sets up.
  // const { message } = App.useApp()

  useEffect(() => {
    const GROUP = 'LlmModelDownloadNotifications'

    Stores.EventBus.on(
      'llm_model.download_completed',
      async event => {
        const { modelDisplayName } = event.data
        // 5s duration on success — matches the visual weight of a
        // "happy path" toast elsewhere in the app.
        message.success(`Downloaded ${modelDisplayName}`)
      },
      GROUP
    )

    Stores.EventBus.on(
      'llm_model.download_failed',
      async event => {
        const { modelDisplayName, errorMessage } = event.data
        // 8s on failure — matches the duration used by the
        // enable-toggle-probe-failed toast in MCP / LLM-repo drawers,
        // giving the user time to read the reason before it dismisses.
        message.error(
          errorMessage
            ? `Download failed: ${modelDisplayName} — ${errorMessage}`
            : `Download failed: ${modelDisplayName}`
        )
      },
      GROUP
    )

    return () => {
      Stores.EventBus.removeGroupListeners(GROUP)
    }
    // `message` is a stable module-level import — no need to re-subscribe.
  }, [])

  return null
}
