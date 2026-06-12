import { Alert, Spin, Typography } from 'antd'
import { FileOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import type { FileViewerSlotProps } from '../../types/viewer'

const { Text } = Typography

export function PdfBody(props: FileViewerSlotProps) {
  // PDF viewer is not inline-capable (its module declares no `inline:`).
  // The chat dispatcher won't call this in source context, but guard for
  // type-narrowing safety.
  if (!('file' in props)) return null
  const { file } = props

  // Subscribe to previewPageUrls Map directly so we re-render as each
  // page slot loads. Calling the `getPreviewPageUrls()` action instead
  // would only subscribe to the function reference (whose identity never
  // changes), so the body would freeze at the initial placeholder array.
  const previewPageUrls = Stores.File.previewPageUrls
  const cachedUrls = previewPageUrls.get(file.id)
  const pageUrls = cachedUrls ?? Stores.File.getPreviewPageUrls(file)

  // Total page count of the source document, when the backend was able
  // to compute it (PDF / DOCX-via-PDF). May be undefined for
  // never-paged formats or for legacy rows uploaded before
  // `processing_metadata.page_count` was added — in that case we
  // assume `preview_page_count` is the true total (no truncation
  // banner). `processing_metadata` is typed as `any` in the OpenAPI
  // surface, hence the cast + the explicit null/undefined check.
  const meta = (file.processing_metadata ?? {}) as { page_count?: number }
  const totalPages =
    typeof meta.page_count === 'number' ? meta.page_count : undefined
  const truncated =
    typeof totalPages === 'number' && totalPages > file.preview_page_count

  if (file.preview_page_count === 0) {
    return (
      <div className="flex flex-col items-center gap-2 py-8">
        <FileOutlined style={{ fontSize: 48 }} />
        <Text type="secondary">Preview not available for this file</Text>
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-6 p-4 overflow-auto h-full">
      {truncated && (
        <Alert
          type="info"
          showIcon
          title={
            `Showing first ${file.preview_page_count} of ${totalPages} pages. ` +
            `Download the file to view all pages.`
          }
          className="flex-shrink-0"
        />
      )}
      {pageUrls.map((url, i) => (
        <div
          key={i}
          className="flex flex-col items-center gap-1"
          // Browser-native virtualization: skip painting / image
          // decode for pages that are offscreen. Each page is
          // reserved as ~800px (rough A4-at-display height) so the
          // scrollbar geometry stays accurate without measuring
          // every page. Cuts initial render cost for 50+-page PDFs
          // from all-pages-decoded to viewport-worth-of-pages.
          // `loading="lazy"` on the img is a defense-in-depth — even
          // when content-visibility doesn't apply (older browsers),
          // the image network fetch + decode is deferred.
          style={{
            contentVisibility: 'auto',
            containIntrinsicSize: 'auto 800px',
          }}
        >
          <Text type="secondary" className="!text-xs">
            Page {i + 1} of {totalPages ?? file.preview_page_count}
          </Text>
          {url ? (
            <img
              src={url}
              alt={`Page ${i + 1}`}
              className="w-full object-contain rounded shadow"
              loading="lazy"
            />
          ) : (
            <div className="w-full flex items-center justify-center py-16">
              <Spin />
            </div>
          )}
        </div>
      ))}
    </div>
  )
}
