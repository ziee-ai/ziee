import { Button, Dropdown, Tooltip } from 'antd'
import { PlusOutlined } from '@ant-design/icons'
import type { MenuProps } from 'antd'
import { PROVIDER_TEMPLATES, type ProviderTemplate } from '../types'

interface Props {
  onPick: (template: ProviderTemplate) => void
  /// Provider names already present in the list. Templates that
  /// would collide on the unique-name constraint (google/microsoft/
  /// apple seeded by migration 47, or a previous add) are filtered
  /// OUT of the dropdown entirely — the admin's path for those is
  /// to use the "Edit" action on the existing row.
  existingNames?: string[]
  disabled?: boolean
}

/**
 * "Add provider" dropdown. Each menu item is a pre-filled template
 * (issuer URL, default scopes, attribute mapping) so the admin only
 * has to paste client_id + client_secret. Hidden when every template
 * is already present (the trigger button is disabled in that case).
 */
export function AddProviderMenu({ onPick, existingNames, disabled }: Props) {
  const taken = new Set((existingNames ?? []).map(n => n.toLowerCase()))
  const available = PROVIDER_TEMPLATES.filter(
    t => !taken.has(t.key.toLowerCase()),
  )
  const items: MenuProps['items'] = available.map(t => ({
    key: t.key,
    label: t.label,
    onClick: () => onPick(t),
  }))

  const allTaken = available.length === 0
  const isDisabled = disabled || allTaken

  return (
    <Dropdown menu={{ items }} disabled={isDisabled}>
      <Tooltip title={allTaken ? 'All providers taken' : 'Add authentication provider'}>
        <Button
          type="text"
          icon={<PlusOutlined />}
          disabled={isDisabled}
          aria-label="Add authentication provider"
        />
      </Tooltip>
    </Dropdown>
  )
}
