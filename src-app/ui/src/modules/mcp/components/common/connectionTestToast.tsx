import type { TestMcpConnectionResponse } from '@/api-client/types'

// Minimal structural shape so any toast API (kit `message` or a legacy antd
// MessageInstance from a not-yet-migrated caller) satisfies it.
type MessageApi = {
  success: (content: string) => unknown
  error: (content: string) => unknown
}

// Backend connection errors can be long (timeout details, a 401 body, a
// command-not-found dump). The kit toast renders the message as plain text and
// wraps long content itself, so we pass the raw string through.

/** Show a connection-test result as a success/error toast. */
export const showConnectionTestResult = (
  message: MessageApi,
  result: TestMcpConnectionResponse,
) => {
  if (result.success) {
    message.success(result.message || 'Connection successful')
  } else {
    message.error(result.message || 'Connection failed')
  }
}

/** Show a thrown error (network/unexpected) as an error toast. */
export const showConnectionTestError = (message: MessageApi, error: unknown) => {
  message.error(error instanceof Error ? error.message : 'Connection test failed')
}
