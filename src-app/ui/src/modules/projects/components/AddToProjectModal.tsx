import { useEffect, useMemo, useState } from 'react'
import { Dialog, Button, message, Text, Combobox } from '@/components/ui'
import { Stores } from '@/core/stores'

interface AddToProjectModalProps {
  open: boolean
  conversationId: string | null
  onClose: () => void
  onAttached?: (projectId: string) => void
}

/**
 * Project-picker modal. Reads the projects list from
 * `Stores.Projects` (loaded on mount if not already); user picks a
 * project; on confirm, calls `Stores.Projects.attachConversation`.
 */
export function AddToProjectModal({
  open,
  conversationId,
  onClose,
  onAttached,
}: AddToProjectModalProps) {
  const { projects, isInitialized, loading } = Stores.Projects
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [submitting, setSubmitting] = useState(false)

  useEffect(() => {
    if (open && !isInitialized && !loading) {
      void Stores.Projects.loadProjects()
    }
  }, [open, isInitialized, loading])

  // Reset selection when the modal closes so the next open starts fresh.
  useEffect(() => {
    if (!open) {
      setSelectedId(null)
      setSubmitting(false)
    }
  }, [open])

  const options = useMemo(
    () =>
      Array.from(projects.values()).map(p => ({
        value: p.id,
        label: p.name,
      })),
    [projects],
  )

  const handleOk = async () => {
    if (!selectedId || !conversationId) return
    setSubmitting(true)
    try {
      await Stores.Projects.attachConversation(selectedId, conversationId)
      message.success('Added to project')
      onAttached?.(selectedId)
      onClose()
    } catch (err) {
      message.error(
        err instanceof Error ? err.message : 'Failed to add to project',
      )
      setSubmitting(false)
    }
  }

  return (
    <Dialog
      title="Add to project"
      open={open}
      onOpenChange={(v) => { if (!v) onClose() }}
      footer={
        <div className="flex justify-end gap-2">
          <Button variant="outline" onClick={onClose}>Cancel</Button>
          <Button
            onClick={handleOk}
            disabled={!selectedId}
            loading={submitting}
          >
            Add
          </Button>
        </div>
      }
    >
      <div className="flex flex-col gap-3">
        <Text type="secondary">
          Pick a project to file this conversation into. It will start
          receiving the project's instructions, knowledge files, and
          MCP defaults on subsequent sends.
        </Text>
        <Combobox
          placeholder="Pick a project…"
          searchPlaceholder="Search projects…"
          options={options}
          value={selectedId ?? undefined}
          onChange={v => setSelectedId(v ?? null)}
          emptyText={loading ? 'Loading…' : 'No projects — create one first.'}
          className="w-full"
        />
      </div>
    </Dialog>
  )
}
