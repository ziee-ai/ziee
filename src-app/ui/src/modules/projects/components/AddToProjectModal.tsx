import { useEffect, useMemo, useState } from 'react'
import { Dialog, Button, message, Text, Combobox } from '@ziee/kit'
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
  const [error, setError] = useState<string | null>(null)

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
      setError(null)
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
      const msg =
        err instanceof Error ? err.message : 'Failed to add to project'
      setError(msg)
      message.error(msg)
      setSubmitting(false)
    }
  }

  return (
    <Dialog
      data-testid="project-add-to-project-dialog"
      title="Add to project"
      open={open}
      onOpenChange={(v) => { if (!v) onClose() }}
      footer={
        <div className="flex justify-end gap-2">
          <Button data-testid="project-add-to-project-cancel-button" variant="outline" onClick={onClose}>Cancel</Button>
          <Button
            data-testid="project-add-to-project-confirm-button"
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
          data-testid="project-add-to-project-combobox"
          placeholder="Pick a project…"
          searchPlaceholder="Search projects…"
          options={options}
          value={selectedId ?? undefined}
          onChange={v => {
            setSelectedId(v ?? null)
            setError(null)
          }}
          emptyText={loading ? 'Loading…' : 'No projects — create one first.'}
          className="w-full"
        />
        {error && (
          <Text type="danger" className="text-sm">
            {error}
          </Text>
        )}
      </div>
    </Dialog>
  )
}
