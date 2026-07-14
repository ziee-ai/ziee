import { ZoomIn, ZoomOut } from 'lucide-react'
import { Button, Segmented, Space } from '@ziee/kit'
import { Stores } from '@/core/stores'
import { DEFAULT_IMAGE_VIEW } from './zoom'
import type { FileViewerSlotProps } from '../../types/viewer'

export function ImageHeader(props: FileViewerSlotProps) {
  // Inline-context headers are owned by the chat-side InlineFilePreview
  // (which renders icon + filename + open-in-new-tab link). The
  // FileStore-coupled chrome below only works with a real FileEntity,
  // so for inline context we render nothing extra.
  if (!('file' in props)) return null
  const { file } = props
  // Read the map directly (reactive subscription) so the Segmented reflects
  // scale-driven mode flips (a zoom switches mode → 'actual').
  const view = Stores.File.imageViewStates.get(file.id) ?? DEFAULT_IMAGE_VIEW
  return (
    <Space size="small" wrap={false}>
      <Button
        variant="ghost"
        size="icon"
        tooltip="Zoom out"
        aria-label="Zoom out"
        icon={<ZoomOut />}
        onClick={() => Stores.File.zoomImage(file.id, 0.8)}
        data-testid="file-viewer-zoom-out-btn"
      />
      <Segmented
        value={view.mode}
        onChange={(v: string) =>
          Stores.File.setImageViewMode(file.id, v as 'fit' | 'actual')
        }
        data-testid="file-viewer-image-fit-segmented"
        options={[
          { value: 'fit', label: 'Fit', 'aria-label': 'Fit to window' },
          { value: 'actual', label: '100%', 'aria-label': 'Actual size' },
        ]}
      />
      <Button
        variant="ghost"
        size="icon"
        tooltip="Zoom in"
        aria-label="Zoom in"
        icon={<ZoomIn />}
        onClick={() => Stores.File.zoomImage(file.id, 1.25)}
        data-testid="file-viewer-zoom-in-btn"
      />
    </Space>
  )
}
