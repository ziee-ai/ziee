import { useState } from 'react'
import { Download, Save } from 'lucide-react'
import { Button, message } from '@ziee/kit'
import { ApiClient, getAuthToken } from '@/api-client'
import { getBaseUrl } from '@/api-client/getBaseURL'
import { Permissions } from '@/api-client/permissions'
import type { MessageContentDataToolResult, MessageContent } from '@/api-client/types'
import { usePermission } from '@/core/permissions'
import type { ContentRendererProps } from '@/modules/chat/core/extensions'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'
import { MessageFilesView } from '@/modules/file/chat-extension/components/MessageFilesView'
import { Chat } from '@/modules/chat/core/stores/chatBridge'

/** The tool name a `run_from_workspace` result carries. */
const RUN_FROM_WORKSPACE = 'run_from_workspace'

function toolResultBlock(content: MessageContent): MessageContentDataToolResult | null {
  if (content.content_type !== 'tool_result') return null
  return content.content as MessageContentDataToolResult
}

/**
 * Inline card for a `run_from_workspace` tool result: renders the default
 * file/resource view, plus — on a SUCCESSFUL run that reported its authored
 * `workspace_dir` — a "Save to my workflows" + "Download .tar.gz" affordance so
 * the user can graduate the ephemeral workflow.
 *
 * Uses the registry's `contentMatch` seam to claim ONLY its own blocks; every
 * other `tool_result` falls through to the next renderer (file / literature).
 */
export function WorkflowWorkspaceRunCard(props: ContentRendererProps) {
  const block = toolResultBlock(props.content)
  const sc = (block?.structured_content ?? null) as { workspace_dir?: string } | null
  const dir = sc?.workspace_dir
  // Resolve THIS pane's conversation (ITEM-38) — a `.$` snapshot on the bridge
  // would export the FOCUSED pane's workspace, not the one this card renders in.
  const chat = (useChatPaneOrNull()?.store ?? Chat) as typeof Chat
  const conversationId = chat.$.conversation?.id
  const [saving, setSaving] = useState(false)
  const [downloading, setDownloading] = useState(false)
  const [saved, setSaved] = useState(false)
  // Each affordance is gated on the perm its endpoint requires:
  //   Save   → workspace-save  → workflows::install
  //   Download → workspace-export → workflows::execute
  // (see server workflow/handlers/dev.rs). A viewer who can see the run
  // result but lacks these must not see a button that 403s.
  const canSave = usePermission(Permissions.WorkflowsInstall)
  const canDownload = usePermission(Permissions.WorkflowsExecute)

  const canGraduate =
    !block?.is_error && !!dir && !!conversationId && (canSave || canDownload)

  const onSave = async () => {
    if (!dir || !conversationId) return
    setSaving(true)
    try {
      await ApiClient.Workflow.workspaceSave({ conversation_id: conversationId, dir, scope: 'user' })
      setSaved(true)
      message.success('Saved to your workflows')
    } catch (e) {
      message.error(`Save failed: ${e instanceof Error ? e.message : String(e)}`)
    } finally {
      setSaving(false)
    }
  }

  const onDownload = async () => {
    if (!dir || !conversationId) return
    setDownloading(true)
    try {
      const base = await getBaseUrl()
      const url = `${base}/api/workflows/workspace-export?conversation_id=${encodeURIComponent(
        conversationId,
      )}&dir=${encodeURIComponent(dir)}`
      const token = getAuthToken()
      const resp = await fetch(url, {
        headers: token ? { Authorization: `Bearer ${token}` } : {},
      })
      if (!resp.ok) throw new Error(`export failed (${resp.status})`)
      const blob = await resp.blob()
      const objectUrl = URL.createObjectURL(blob)
      const a = document.createElement('a')
      a.href = objectUrl
      a.download = `${dir.replace(/[^a-zA-Z0-9._-]/g, '_')}.tar.gz`
      document.body.appendChild(a)
      a.click()
      a.remove()
      URL.revokeObjectURL(objectUrl)
    } catch (e) {
      message.error(`Download failed: ${e instanceof Error ? e.message : String(e)}`)
    } finally {
      setDownloading(false)
    }
  }

  return (
    <>
      <MessageFilesView {...props} />
      {canGraduate && (
        <div className="my-2 flex gap-2" data-testid="workflow-workspace-run-actions">
          {canSave && (
            <Button
              size="default"
              variant="outline"
              icon={<Save />}
              loading={saving}
              disabled={saved}
              onClick={onSave}
              data-testid="workflow-save-to-mine"
            >
              {saved ? 'Saved' : 'Save to my workflows'}
            </Button>
          )}
          {canDownload && (
            <Button
              size="default"
              variant="outline"
              icon={<Download />}
              loading={downloading}
              onClick={onDownload}
              data-testid="workflow-download-targz"
            >
              Download .tar.gz
            </Button>
          )}
        </div>
      )}
    </>
  )
}

/** Claim only `run_from_workspace` tool results; everything else falls through. */
WorkflowWorkspaceRunCard.contentMatch = (content: MessageContent): boolean => {
  const block = toolResultBlock(content)
  return block?.name === RUN_FROM_WORKSPACE
}
