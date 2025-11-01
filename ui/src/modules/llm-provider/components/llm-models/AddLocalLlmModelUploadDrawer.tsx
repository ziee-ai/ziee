import { Alert } from 'antd'
import { Drawer } from '@/components/common/Drawer'
import {
  closeAddLocalLlmModelUploadDrawer,
  useAddLocalLlmModelUploadDrawerStore,
} from '@/modules/llm-provider/store'

/**
 * TODO: Implement full local model upload functionality
 * This drawer needs:
 * - File upload with progress tracking
 * - Model file validation (safetensors, pytorch, gguf)
 * - Integration with ApiClient.LlmModel.upload()
 * - Proper form handling for model metadata
 *
 * See react-test reference implementation for full feature set
 */
export function AddLocalLlmModelUploadDrawer() {
  const { open } = useAddLocalLlmModelUploadDrawerStore()

  return (
    <Drawer
      title="Upload Local Model"
      placement="right"
      size="large"
      open={open}
      onClose={closeAddLocalLlmModelUploadDrawer}
    >
      <Alert
        type="info"
        message="Upload Feature Coming Soon"
        description={
          <div>
            <p>
              Local model upload functionality will be available once the
              following backend features are implemented:
            </p>
            <ul>
              <li>Model file upload endpoint with progress tracking</li>
              <li>File validation for different model formats</li>
              <li>Model metadata extraction</li>
            </ul>
            <p>
              In the meantime, models can be added directly via the API or
              downloaded from repositories.
            </p>
          </div>
        }
      />
    </Drawer>
  )
}
