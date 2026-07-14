import { RotateCw, Star, Trash2 } from 'lucide-react'
import { Fragment, useState } from 'react'
import { Permissions, type VoiceModel } from '@/api-client/types'
import { ListPagination } from '@/components/common/ListPagination'
import {
  Button,
  Card,
  Checkbox,
  Confirm,
  Descriptions,
  Empty,
  ErrorState,
  Flex,
  message,
  Separator,
  Spin,
  Tag,
  Text,
  Tooltip,
} from '@ziee/kit'
import { usePermission } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { formatBytes } from '@/utils/downloadUtils'

const PAGE_SIZE = 10

/**
 * The installed whisper-model library. Each row shows the model metadata plus
 * set-active / delete actions. Mirrors the sibling InstalledVersionsCard.
 */
export function InstalledModelsCard() {
  const { installed, loadingInstalled, error } = Stores.VoiceModel
  const [page, setPage] = useState(1)

  const handleRefresh = () => {
    Stores.VoiceModel.loadInstalled().catch(() =>
      message.error('Failed to refresh installed models'),
    )
  }

  const total = installed.length
  const pageItems = installed.slice((page - 1) * PAGE_SIZE, page * PAGE_SIZE)

  return (
    <Card
      title="Installed models"
      data-testid="voice-installed-models-card"
      extra={
        <Button
          icon={<RotateCw />}
          loading={loadingInstalled}
          onClick={handleRefresh}
          data-testid="voice-installed-models-refresh"
          aria-label="Refresh installed models"
        >
          Refresh
        </Button>
      }
    >
      {loadingInstalled && installed.length === 0 ? (
        <Spin label="Loading" />
      ) : error && installed.length === 0 ? (
        <ErrorState
          resource="installed models"
          description="The installed whisper models couldn't be loaded."
          details={error}
          onRetry={handleRefresh}
          data-testid="voice-installed-models-error"
        />
      ) : installed.length === 0 ? (
        <Empty
          description="No models installed yet — download one above."
          data-testid="voice-installed-models-empty"
        />
      ) : (
        <>
          <div>
            {pageItems.map((m, i) => (
              <Fragment key={m.id}>
                {i > 0 && <Separator className="!my-4" />}
                <InstalledModelRow model={m} />
              </Fragment>
            ))}
          </div>
          {total > PAGE_SIZE && (
            <ListPagination
              current={page}
              total={total}
              pageSize={PAGE_SIZE}
              onChange={setPage}
              onPageSizeChange={() => setPage(1)}
              itemNoun="models"
              data-testid="voice-installed-models-pagination"
              aria-label="Installed models pagination"
            />
          )}
        </>
      )}
    </Card>
  )
}

function InstalledModelRow({ model }: { model: VoiceModel }) {
  const { activating, deleting } = Stores.VoiceModel
  const canManage = usePermission(Permissions.VoiceAdminManage)
  const isActivating = activating.get(model.id) || false
  const isDeleting = deleting.get(model.id) || false
  const [ackActive, setAckActive] = useState(false)

  const handleActivate = async () => {
    try {
      await Stores.VoiceModel.activate(model.id)
      message.success(`Activated ${model.name}`)
    } catch {
      /* error surfaced in store */
    }
  }

  const handleDelete = async () => {
    try {
      await Stores.VoiceModel.remove(model.id, ackActive)
      setAckActive(false)
    } catch (error) {
      message.error(
        error instanceof Error ? error.message : 'Failed to delete model',
      )
    }
  }

  return (
    <div data-testid={`voice-installed-model-row-${model.name}`}>
      <div className="flex items-center gap-2 mb-2 flex-wrap">
        <div className="flex-1 min-w-48">
          <Flex align="center" gap="small" wrap>
            <Text className="font-medium">{model.name}</Text>
            {model.is_active && (
              <Tag
                tone="success"
                variant="outline"
                data-testid={`voice-installed-model-active-tag-${model.name}`}
              >
                active
              </Tag>
            )}
            <Tag
              variant="outline"
              data-testid={`voice-installed-model-source-${model.name}`}
            >
              {model.source}
            </Tag>
            <Tag
              tone={model.verified ? 'success' : 'warning'}
              variant="outline"
              data-testid={`voice-installed-model-verified-${model.name}`}
            >
              {model.verified ? 'verified' : 'unverified'}
            </Tag>
            {model.update_available && (
              <Tag
                tone="info"
                variant="outline"
                data-testid={`voice-installed-model-update-${model.name}`}
              >
                update available
              </Tag>
            )}
          </Flex>
        </div>
        <div className="flex flex-wrap gap-1 items-center justify-end">
          {canManage && !model.is_active && (
            <Tooltip content="Make this the active model the whisper-server serves">
              <Button
                variant="ghost"
                icon={<Star />}
                loading={isActivating}
                onClick={handleActivate}
                data-testid={`voice-installed-model-activate-${model.name}`}
                aria-label={`Set ${model.name} as active`}
              >
                Set active
              </Button>
            </Tooltip>
          )}
          {canManage && (
            <Confirm
              data-testid={`voice-installed-model-delete-confirm-${model.name}`}
              title="Delete model"
              description={
                <Flex direction="column" gap="small" className="[&_*]:!m-0">
                  <Text>Are you sure you want to delete {model.name}?</Text>
                  {model.is_active && (
                    <>
                      <Text type="danger">
                        Warning: This is the active model. Transcription will
                        fail until another model is activated.
                      </Text>
                      <Checkbox
                        checked={ackActive}
                        onChange={(e: boolean) => setAckActive(e)}
                        label="I understand this is the active model"
                        data-testid={`voice-installed-model-delete-ackactive-${model.name}`}
                      />
                    </>
                  )}
                </Flex>
              }
              onConfirm={handleDelete}
              onOpenChange={open => {
                if (!open) setAckActive(false)
              }}
              okText="Delete"
              cancelText="Cancel"
              okButtonProps={{
                danger: true,
                disabled: model.is_active && !ackActive,
              }}
            >
              <Button
                variant="ghost"
                icon={<Trash2 />}
                loading={isDeleting}
                data-testid={`voice-installed-model-delete-${model.name}`}
                aria-label={`Delete ${model.name}`}
              >
                Delete
              </Button>
            </Confirm>
          )}
        </div>
      </div>

      <Descriptions
        size="sm"
        data-testid={`voice-installed-model-desc-${model.name}`}
        items={[
          { key: 'filename', label: 'File', children: model.filename },
          {
            key: 'size',
            label: 'Size',
            children: formatBytes(model.size_bytes),
          },
          ...(model.source_url
            ? [{ key: 'url', label: 'Source URL', children: model.source_url }]
            : []),
          {
            key: 'installed',
            label: 'Installed',
            children: new Date(model.created_at).toLocaleString(),
          },
        ]}
      />
    </div>
  )
}
