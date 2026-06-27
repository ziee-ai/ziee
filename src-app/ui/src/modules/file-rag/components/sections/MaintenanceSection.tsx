import { Alert, Button, Card, Paragraph } from '@/components/ui'
import { message } from '@/components/ui'
import { DatabaseOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

const READ_PERM = Permissions.FileRagAdminRead
const MANAGE_PERM = Permissions.FileRagAdminManage

/**
 * Maintenance: index files that pre-date Document RAG (or that failed to
 * index). The bounded, idempotent backfill also runs automatically on each
 * server boot; this is the manual trigger.
 */
export function MaintenanceSection() {
  const canRead = usePermission(READ_PERM) || usePermission(MANAGE_PERM)
  const canManage = usePermission(MANAGE_PERM)
  const { settings, triggeringBackfill } = Stores.FileRagAdmin

  if (!canRead) {
    return (
      <Card title="Maintenance">
        <Alert
          tone="warning"
          title="You don't have permission to view Document RAG admin settings."
        />
      </Card>
    )
  }
  if (!settings) return null

  const handleBackfill = async () => {
    try {
      await Stores.FileRagAdmin.triggerBackfill()
      message.info('Backfill dispatched in the background.')
    } catch (error) {
      message.error(
        error instanceof Error ? error.message : 'Failed to start backfill.',
      )
    }
  }

  return (
    <Card title="Maintenance">
      <Paragraph type="secondary" className="!mb-3 text-sm">
        Backfill indexes files that have extracted text but no chunks yet —
        anything uploaded before Document RAG was enabled, or that failed to
        index. It's bounded and idempotent (safe to run repeatedly) and also
        runs on every server boot.
      </Paragraph>
      <div className="flex flex-col gap-2 opacity-[inherit]" aria-disabled={!canManage}>
        <div className="flex items-center gap-4">
          <span className="text-sm font-medium w-40">Backfill existing files</span>
          <Button
            icon={<DatabaseOutlined />}
            loading={triggeringBackfill}
            disabled={!settings.enabled || !canManage}
            onClick={handleBackfill}
          >
            Run backfill
          </Button>
        </div>
      </div>
    </Card>
  )
}
