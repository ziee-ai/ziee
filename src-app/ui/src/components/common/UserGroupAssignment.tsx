import { useEffect, useState } from 'react'
import { Pencil, ChevronDown, ChevronRight } from 'lucide-react'
import { message, Button, MultiSelect, Space, Spin, Tag, Text } from '@/components/ui'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'

export interface UserGroupOption {
  id: string
  name: string
}

export interface UserGroupAssignmentProps {
  /** Groups currently assigned (resolved to id + name for the tags). Empty = all users. */
  assignedGroups: UserGroupOption[]
  loading?: boolean
  canAssign: boolean
  /** Line shown when no groups are assigned (e.g. "Available to all users"). */
  emptyText: string
  /** Optional hint shown above the tags when groups ARE assigned. */
  description?: string
  /** Testid prefix; sub-parts derive `${testid}-{toggle,assign,empty,…}`. */
  'data-testid': string
  /**
   * Inline editor. When provided, Assign opens a Drawer with a group MultiSelect +
   * Save/Cancel. When omitted, Assign calls `onAssign` (e.g. open a bespoke drawer,
   * as the MCP card does).
   */
  editor?: {
    loadAllGroups: () => Promise<UserGroupOption[]>
    save: (ids: string[]) => Promise<void>
  }
  /** Title of the editor Drawer (default "Assign User Groups"). */
  drawerTitle?: string
  onAssign?: () => void
}

/**
 * Shared "User Groups" assignment section used by System MCP servers, System
 * Skills and System Workflows. A chevron disclosure (User Groups toggle + Assign
 * action) with a collapsible body showing the assigned-group tags or an empty
 * line. Clicking Assign opens a Drawer (either the shared editor Drawer when an
 * `editor` is supplied, or a caller-owned one via `onAssign`).
 */
export function UserGroupAssignment({
  assignedGroups,
  loading,
  canAssign,
  emptyText,
  description,
  editor,
  drawerTitle = 'Assign User Groups',
  onAssign,
  'data-testid': testid,
}: UserGroupAssignmentProps) {
  const [open, setOpen] = useState(false)
  const [editing, setEditing] = useState(false)
  const [allGroups, setAllGroups] = useState<UserGroupOption[]>([])
  const [draft, setDraft] = useState<string[]>([])
  const [saving, setSaving] = useState(false)

  // On expand, load the full group list so the assigned tags can show names
  // (stores that only keep ids pass name = id until this resolves). MCP has no
  // inline editor and its assignedGroups already carry names, so this is a no-op.
  useEffect(() => {
    if (!open || !editor || allGroups.length > 0) return
    let cancelled = false
    void editor.loadAllGroups().then(groups => {
      if (!cancelled) setAllGroups(groups)
    }).catch(() => {})
    return () => { cancelled = true }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open])

  const tid = (part: string) => `${testid}-${part}`

  const startAssign = async () => {
    if (!editor) {
      onAssign?.()
      return
    }
    setDraft(assignedGroups.map(g => g.id))
    try {
      setAllGroups(await editor.loadAllGroups())
      setEditing(true)
    } catch {
      message.error('Failed to load groups')
    }
  }

  const save = async () => {
    if (!editor) return
    setSaving(true)
    try {
      await editor.save(draft)
      setEditing(false)
      setOpen(true)
    } catch {
      message.error('Failed to save group assignments')
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="pb-3">
      <div className="flex items-center gap-2">
        <Button
          variant="ghost"
          size="default"
          data-testid={tid('toggle')}
          onClick={() => setOpen(o => !o)}
          aria-expanded={open}
          aria-label={open ? 'Collapse user groups' : 'Expand user groups'}
          icon={open ? <ChevronDown aria-hidden="true" /> : <ChevronRight aria-hidden="true" />}
        >
          <Text className="font-medium text-sm">User Groups</Text>
        </Button>
        <div className="ml-auto">
          {canAssign ? (
            <Button
              variant="ghost"
              size="default"
              data-testid={tid('assign')}
              icon={<Pencil aria-hidden="true" />}
              onClick={() => void startAssign()}
              aria-label="Manage user groups"
            >
              Assign
            </Button>
          ) : null}
        </div>
      </div>
      {open && (
        <div className="pt-2">
          {loading ? (
            <Spin size="sm" label="Loading" />
          ) : assignedGroups.length === 0 ? (
            <Text type="secondary" className="text-xs" data-testid={tid('empty')}>
              {emptyText}
            </Text>
          ) : (
            <Space direction="vertical" className="w-full">
              {description && (
                <Text type="secondary" className="text-xs">
                  {description}
                </Text>
              )}
              <Space wrap size="small">
                {assignedGroups.map(g => (
                  <Tag variant="outline" key={g.id} tone="info" data-testid={`${testid}-tag-${g.id}`}>
                    {/* Prefer a name from the loaded group list (stores that only
                        keep ids pass name = id); fall back to whatever was given. */}
                    {allGroups.find(x => x.id === g.id)?.name ?? g.name}
                  </Tag>
                ))}
              </Space>
            </Space>
          )}
        </div>
      )}

      {editor && (
        <Drawer
          open={editing}
          onClose={() => setEditing(false)}
          title={drawerTitle}
          size={600}
          data-testid={tid('drawer')}
          footer={
            <div className="flex justify-end gap-2">
              <Button variant="outline" data-testid={tid('cancel')} onClick={() => setEditing(false)}>
                Cancel
              </Button>
              <Button loading={saving} data-testid={tid('save')} onClick={save}>
                Save
              </Button>
            </div>
          }
        >
          <MultiSelect
            className="w-full"
            data-testid={tid('multiselect')}
            placeholder="Restrict to specific groups (empty = all users)"
            searchPlaceholder="Search groups"
            emptyText="No groups found"
            removeLabel={label => `Remove ${label}`}
            value={draft}
            onChange={setDraft}
            options={allGroups.map(g => ({ label: g.name, value: g.id }))}
            aria-label="Select groups"
          />
        </Drawer>
      )}
    </div>
  )
}
