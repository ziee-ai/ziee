import { Alert } from 'antd'
import { Drawer } from '@/components/common/Drawer'
import {
  closeAddLocalLlmModelDownloadDrawer,
  useAddLocalLlmModelDownloadDrawerStore,
} from '@/modules/llm-provider/llm-model-drawer-store'

/**
 * TODO: Implement full model download functionality
 * This drawer needs:
 * - Repository selection (HuggingFace, etc.)
 * - Model browser/search
 * - Download progress tracking via SSE
 * - Download management (pause/resume/cancel)
 *
 * Backend requirements:
 * - GET /api/llm-repositories - List available repositories
 * - POST /api/llm-models/download - Initiate download
 * - GET /api/llm-models/downloads - List active downloads
 * - GET /api/llm-models/downloads/progress - SSE endpoint for progress
 * - DELETE /api/llm-models/downloads/{id}/cancel - Cancel download
 *
 * See react-test reference implementation for full feature set
 */
export function AddLocalLlmModelDownloadDrawer() {
  const { open } = useAddLocalLlmModelDownloadDrawerStore()

  return (
    <Drawer
      title="Download Model from Repository"
      placement="right"
      size="large"
      open={open}
      onClose={closeAddLocalLlmModelDownloadDrawer}
    >
      <Alert
        type="info"
        message="Download Feature Coming Soon"
        description={
          <div>
            <p>
              Model download functionality from repositories (HuggingFace, etc.)
              will be available once the following backend features are
              implemented:
            </p>
            <ul>
              <li>Repository management API</li>
              <li>Model download endpoint with progress tracking</li>
              <li>SSE endpoint for real-time progress updates</li>
              <li>Download pause/resume/cancel operations</li>
            </ul>
            <p>
              In the meantime, models can be added via direct upload or API.
            </p>
          </div>
        }
      />
    </Drawer>
  )
}
