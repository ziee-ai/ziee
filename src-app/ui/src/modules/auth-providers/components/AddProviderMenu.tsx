import { Button, Dropdown } from 'antd'
import { DownOutlined, PlusOutlined } from '@ant-design/icons'
import type { MenuProps } from 'antd'
import { PROVIDER_TEMPLATES, type ProviderTemplate } from '../types'

interface Props {
  onPick: (template: ProviderTemplate) => void
  /// Provider names already present in the table. Templates that
  /// would collide (their `key` matches an existing name — i.e.
  /// google/microsoft/apple seeded by migration 47, or a previous
  /// add) are disabled with a hint to edit the existing row.
  existingNames?: string[]
  disabled?: boolean
}

/**
 * "Add provider" dropdown. Each menu item is a pre-filled template
 * (issuer URL, default scopes, attribute mapping) so the admin only
 * has to paste client_id + client_secret.
 *
 * Migration 47 pre-seeds disabled `google`, `microsoft`, and `apple`
 * rows so they're discoverable from the table. To prevent the
 * admin from accidentally creating a second Google row (which would
 * fail with a unique-name constraint error), the corresponding
 * template menu items are disabled when a row with that name
 * already exists — the admin's natural path is "Edit" on the
 * existing row instead.
 */
export function AddProviderMenu({ onPick, existingNames, disabled }: Props) {
  const taken = new Set((existingNames ?? []).map(n => n.toLowerCase()))
  const items: MenuProps['items'] = PROVIDER_TEMPLATES.map(t => {
    const isTaken = taken.has(t.key.toLowerCase())
    return {
      key: t.key,
      label: isTaken ? `${t.label} (already added — edit existing)` : t.label,
      disabled: isTaken,
      onClick: () => {
        if (!isTaken) onPick(t)
      },
    }
  })

  return (
    <Dropdown menu={{ items }} disabled={disabled}>
      <Button type="primary" icon={<PlusOutlined />} disabled={disabled}>
        Add provider <DownOutlined />
      </Button>
    </Dropdown>
  )
}
