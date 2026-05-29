import { App } from 'antd'
import type { TestMcpConnectionResponse } from '@/api-client/types'

type MessageApi = ReturnType<typeof App.useApp>['message']

// Backend connection errors can be long (timeout details, a 401 body, a
// command-not-found dump). Rendered as a raw antd toast they grow into a single
// centered line that runs off both screen edges and looks clipped. Wrapping the
// text in a width-capped, word-breaking span makes long messages wrap instead.
const toastContent = (text: string) => (
  <span
    style={{
      display: 'inline-block',
      maxWidth: 'min(640px, 90vw)',
      textAlign: 'left',
      whiteSpace: 'normal',
      wordBreak: 'break-word',
    }}
  >
    {text}
  </span>
)

/** Show a connection-test result as a success/error toast (width-capped). */
export const showConnectionTestResult = (
  message: MessageApi,
  result: TestMcpConnectionResponse,
) => {
  if (result.success) {
    message.success(toastContent(result.message || 'Connection successful'))
  } else {
    message.error(toastContent(result.message || 'Connection failed'))
  }
}

/** Show a thrown error (network/unexpected) as a width-capped error toast. */
export const showConnectionTestError = (message: MessageApi, error: unknown) => {
  message.error(
    toastContent(
      error instanceof Error ? error.message : 'Connection test failed',
    ),
  )
}
