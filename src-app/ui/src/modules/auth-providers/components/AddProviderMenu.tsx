import { Button, Dropdown } from 'antd'
import { DownOutlined, PlusOutlined } from '@ant-design/icons'
import type { MenuProps } from 'antd'
import { PROVIDER_TEMPLATES, type ProviderTemplate } from '../types'

interface Props {
  onPick: (template: ProviderTemplate) => void
  disabled?: boolean
}

/**
 * "Add provider" dropdown. Each menu item is a pre-filled template
 * (issuer URL, default scopes, attribute mapping) so the admin only
 * has to paste client_id + client_secret.
 */
export function AddProviderMenu({ onPick, disabled }: Props) {
  const items: MenuProps['items'] = PROVIDER_TEMPLATES.map(t => ({
    key: t.key,
    label: t.label,
    onClick: () => onPick(t),
  }))

  return (
    <Dropdown menu={{ items }} disabled={disabled}>
      <Button type="primary" icon={<PlusOutlined />} disabled={disabled}>
        Add provider <DownOutlined />
      </Button>
    </Dropdown>
  )
}
