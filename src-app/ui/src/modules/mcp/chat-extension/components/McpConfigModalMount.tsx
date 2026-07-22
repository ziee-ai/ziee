import { Suspense, lazy } from 'react'
import { McpComposer } from '@/modules/mcp/stores/mcpComposer'
import { useDelayedFalse } from '@/hooks/useDelayedFalse'

// Lazy body: the 578-line MCP config surface is opened on demand from the
// composer, so it must NOT ride the chat bundle. The McpComposer store is
// already loaded by the chat-extension, so reading `configModalVisible` here is
// free — we mount the heavy body only once the user actually opens the dialog.
const McpConfigModal = lazy(() =>
  import('@/modules/mcp/components/McpConfigModal').then(m => ({
    default: m.McpConfigModal,
  })),
)

/**
 * Thin, always-mounted slot wrapper (input_area_suffix). Renders nothing until
 * the MCP config dialog is opened; keeps the body mounted briefly after close
 * (useDelayedFalse) for the exit animation, then unmounts it. This keeps
 * McpConfigModal out of the initial chat-home chunk while preserving the
 * store-driven open/close behavior.
 */
export function McpConfigModalMount() {
  const mounted = useDelayedFalse(() => McpComposer.configModalVisible)
  if (!mounted) return null
  return (
    <Suspense fallback={null}>
      <McpConfigModal />
    </Suspense>
  )
}
