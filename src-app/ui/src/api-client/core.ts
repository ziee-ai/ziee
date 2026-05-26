import {
  ApiEndpointUrl,
  ParameterByUrl,
  ResponseByUrl,
} from '@/api-client/types'
import type { SSECallback } from '@/api-client/sse-types'
import { createSSEHandler } from '@/api-client/sse-types'
import { getBaseUrl } from '@/api-client/getBaseURL'

export const getAuthToken = () => {
  // eslint-disable-next-line no-undef
  const authData = localStorage.getItem('auth-storage')
  if (!authData) return null
  try {
    const parsed = JSON.parse(authData)
    return parsed.state?.token || null
  } catch (error) {
    // Corrupt localStorage (manual edit, power-loss mid-write, schema
    // change from an older app version). Don't crash every API call —
    // treat as logged-out; user will be redirected to login.
    console.error('[getAuthToken] Failed to parse auth-storage:', error)
    return null
  }
}

export { getBaseUrl }

// Files upload progress callback type
export interface FileUploadProgressCallback {
  __init?: (xhr: XMLHttpRequest) => void
  onProgress?: (
    progress: number,
    fileIndex: number,
    overallProgress: number,
  ) => void
  onComplete?: (response: any) => void
  onError?: (error: string, fileName?: string) => void
}

// Type-safe callAsync function that maps URL to exact parameter and response types
export const callAsync = async <U extends ApiEndpointUrl>(
  endpointUrl: U,
  params: ParameterByUrl<U>,
  callbacks?: {
    SSE?: SSECallback<ResponseByUrl<U>>
    fileUploadProgress?: FileUploadProgressCallback
  },
): Promise<ResponseByUrl<U>> => {
  let bUrl = await getBaseUrl()

  let { SSE: sseCallbacks, fileUploadProgress } = callbacks || {}

  // If SSE is an object (handlers), wrap it with createSSEHandler
  let sseFunction: ((event: string, data: any) => void) | undefined = undefined
  if (sseCallbacks) {
    if (typeof sseCallbacks === 'function') {
      sseFunction = sseCallbacks as (event: string, data: any) => void
    } else {
      sseFunction = createSSEHandler(sseCallbacks as any)
    }
  }

  try {
    // Check if params is FormData for file uploads
    const isFormData = params instanceof FormData

    let headers: Record<string, string> = {}

    // Don't set Content-Type for FormData - let browser set it with boundary
    if (!isFormData) {
      headers['Content-Type'] = 'application/json'
    }

    // Add auth token if available
    const token = getAuthToken()
    if (token) {
      headers['Authorization'] = `Bearer ${token}`
    }

    const method = endpointUrl.split(' ')[0] as
      | 'POST'
      | 'GET'
      | 'PUT'
      | 'DELETE'
      | 'PATCH'
    let endpointPath = endpointUrl.replace(/^[A-Z]+\s+/, '').trim()
    //get {capture} from endpointPath
    const captureMatches = (endpointPath.match(/{([^}]+)}/g) || []).map(match =>
      match.slice(1, -1),
    )

    // For FormData, we need to handle path parameters differently
    if (isFormData) {
      // Replace {capture} with actual values from FormData entries
      captureMatches.forEach(capture => {
        const value = (params as FormData).get(capture.trim())
        if (value !== null) {
          endpointPath = endpointPath.replace(`{${capture}}`, value.toString())
        } else {
          throw new Error(`Missing required parameter: ${capture}`)
        }
      })
    } else {
      // Replace {capture} with actual values from params object
      captureMatches.forEach(capture => {
        let c = capture.trim() as keyof typeof params
        if (params[c] !== undefined) {
          //@ts-ignore
          endpointPath = endpointPath.replace(`{${capture}}`, params[c])
        } else {
          throw new Error(`Missing required parameter: ${capture}`)
        }
      })
    }

    if (method === 'GET') {
      //add query parameters to the URL for GET requests
      const queryParams: string[] = []
      if (params && typeof params === 'object') {
        for (const [key, value] of Object.entries(params)) {
          if (value !== undefined && !captureMatches.includes(key)) {
            // Encode the key and value to ensure they are URL-safe
            queryParams.push(
              `${encodeURIComponent(key)}=${encodeURIComponent(value)}`,
            )
          }
        }
        if (queryParams.length > 0) {
          endpointPath += `?${queryParams.join('&')}`
        }
      }
    }

    // Prepare the request body. Strip path-captured params from the
    // body — sending them as body fields (e.g. `provider_id`) trips
    // the backend's serde deny-unknown-fields validators (introduced
    // in the 2026-05 security pass), which return 422.
    let body: any = undefined
    if (['POST', 'PUT', 'PATCH'].includes(method) && params !== undefined) {
      if (isFormData) {
        body = params as FormData
      } else if (captureMatches.length > 0 && typeof params === 'object') {
        const bodyParams: Record<string, unknown> = {}
        for (const [key, value] of Object.entries(params)) {
          if (!captureMatches.includes(key)) {
            bodyParams[key] = value
          }
        }
        body = JSON.stringify(bodyParams)
      } else {
        body = JSON.stringify(params)
      }
    }

    let response: Response
    let abortController: AbortController | undefined = undefined

    // Use XMLHttpRequest for FormData uploads with progress tracking
    if (isFormData && fileUploadProgress && body) {
      response = await new Promise<Response>((resolve, reject) => {
        const xhr = new XMLHttpRequest()

        // Call __init callback if provided to give caller access to XHR for cancellation
        fileUploadProgress.__init?.(xhr)

        // Get file information from FormData
        const formData = body as FormData
        const files: { name: string; file: File; size: number }[] = []

        // Extract files from FormData for progress tracking
        for (const [key, value] of formData.entries()) {
          if (value instanceof File) {
            files.push({ name: key, file: value, size: value.size })
          }
        }

        // Calculate cumulative file sizes for progress tracking
        const fileCumsums: {
          name: string
          startByte: number
          endByte: number
          size: number
        }[] = []
        let cumulativeSize = 0

        files.forEach(fileInfo => {
          const startByte = cumulativeSize
          const endByte = cumulativeSize + fileInfo.size
          fileCumsums.push({
            name: fileInfo.name,
            startByte,
            endByte,
            size: fileInfo.size,
          })
          cumulativeSize += fileInfo.size
        })

        let lastReportedFileIndex = -1

        xhr.upload.addEventListener('progress', event => {
          if (event.lengthComputable) {
            const bytesUploaded = event.loaded
            const totalBytes = event.total
            const overallProgress = Math.round(
              (bytesUploaded / totalBytes) * 100,
            )

            // Find which file is currently being uploaded
            let currentFileIndex = 0
            let fileProgress = 0

            for (let i = 0; i < fileCumsums.length; i++) {
              const fileInfo = fileCumsums[i]

              if (
                bytesUploaded >= fileInfo.startByte &&
                bytesUploaded <= fileInfo.endByte
              ) {
                currentFileIndex = i

                // Calculate progress within this specific file
                const bytesUploadedInFile = bytesUploaded - fileInfo.startByte
                fileProgress = Math.round(
                  (bytesUploadedInFile / fileInfo.size) * 100,
                )
                break
              } else if (bytesUploaded > fileInfo.endByte) {
                // This file is completely uploaded
                currentFileIndex = i
                fileProgress = 100
              }
            }

            // Report progress for all completed files that weren't reported yet
            for (let i = lastReportedFileIndex + 1; i < currentFileIndex; i++) {
              fileUploadProgress.onProgress?.(100, i, overallProgress)
            }

            // Report progress for the current file being uploaded
            if (currentFileIndex >= lastReportedFileIndex) {
              fileUploadProgress.onProgress?.(
                fileProgress,
                currentFileIndex,
                overallProgress,
              )
              lastReportedFileIndex = currentFileIndex
            }
          }
        })

        xhr.addEventListener('load', () => {
          if (xhr.status >= 200 && xhr.status < 300) {
            // Call onComplete callback if provided
            try {
              const responseData = JSON.parse(xhr.responseText)
              fileUploadProgress.onComplete?.(responseData)
            } catch {
              // If response is not JSON, call onComplete with the text
              fileUploadProgress.onComplete?.(xhr.responseText)
            }

            // Create a Response-like object
            const responseHeaders = new Headers()
            xhr
              .getAllResponseHeaders()
              .split('\r\n')
              .forEach(header => {
                const [key, value] = header.split(': ')
                if (key && value) {
                  responseHeaders.set(key, value)
                }
              })

            const mockResponse = {
              ok: xhr.status >= 200 && xhr.status < 300,
              status: xhr.status,
              statusText: xhr.statusText,
              headers: responseHeaders,
              text: () => Promise.resolve(xhr.responseText),
              json: () => Promise.resolve(JSON.parse(xhr.responseText)),
            } as Response

            resolve(mockResponse)
          } else {
            reject(new Error(`HTTP error! status: ${xhr.status}`))
          }
        })

        xhr.addEventListener('error', () => {
          fileUploadProgress.onError?.('Network error during file upload')
          reject(new Error('Network error during file upload'))
        })

        xhr.addEventListener('abort', () => {
          fileUploadProgress.onError?.('Upload cancelled')
          reject(new Error('Upload cancelled'))
        })

        xhr.open(method, `${bUrl}${endpointPath}`)

        // Set headers (excluding Content-Type for FormData)
        Object.entries(headers).forEach(([key, value]) => {
          if (key !== 'Content-Type') {
            xhr.setRequestHeader(key, value)
          }
        })

        xhr.send(body)
      })
    } else {
      // Create AbortController for SSE stream management if SSE callbacks are provided
      abortController = sseFunction ? new AbortController() : undefined

      // Use fetch for non-FormData requests or when no progress tracking is needed
      response = await fetch(`${bUrl}${endpointPath}`, {
        method,
        headers,
        body,
        signal: abortController?.signal,
      })

      // Send initial __init event with abortController for SSE streams
      if (abortController && sseFunction) {
        sseFunction('__init' as any, { abortController })
      }
    }

    // Handle SSE streaming if callbacks are provided and response is text/event-stream
    if (
      sseFunction &&
      response.headers.get('Content-Type')?.includes('text/event-stream')
    ) {
      if (!response.ok) {
        const errorMessage = `HTTP error! status: ${response.status}`
        throw new Error(errorMessage)
      }

      const reader = response.body?.getReader()
      if (!reader) {
        const error = 'No response body reader available'
        throw new Error(error)
      }

      const decoder = new globalThis.TextDecoder()
      let buffer = ''

      try {
        let currentEvent = ''

        while (true) {
          // Check if abort was requested
          if (abortController?.signal.aborted) {
            reader.releaseLock()
            break
          }

          const { done, value } = await reader.read()
          if (done) break

          buffer += decoder.decode(value, { stream: true })
          const lines = buffer.split('\n')
          buffer = lines.pop() || '' // Keep incomplete line in buffer

          for (const line of lines) {
            if (line.trim() === '') {
              // Empty line indicates end of event, reset current event
              currentEvent = ''
              continue
            }

            if (line.startsWith('event: ')) {
              currentEvent = line.slice(7).trim()
            } else if (line.startsWith('data: ')) {
              const data = line.slice(6)
              let parsed = data

              try {
                parsed = JSON.parse(data)
              } catch {
                //do nothing, keep as string
              }

              const result = sseFunction?.(currentEvent as any, parsed as any)
              if ((result as unknown) instanceof Promise) await result
            }
          }
        }
      } catch (error) {
        reader.releaseLock()
        throw error
      }

      // For SSE streaming, return empty response since data is handled via callbacks
      return {} as ResponseByUrl<U>
    }

    // Parse the response as JSON
    if (!response.ok) {
      let errorMessage = `HTTP error! status: ${response.status}`

      // Handle 403 Forbidden specifically
      if (response.status === 403) {
        try {
          // Try to extract specific error message from response body
          const errorResponse = await response.json()
          if (errorResponse.error) {
            errorMessage = errorResponse.error
          } else {
            errorMessage = 'Permission denied'
          }
        } catch {
          // If we can't parse the error response, use default permission denied message
          errorMessage = 'Permission denied'
        }
      } else {
        let textResponse: string | undefined
        try {
          textResponse = await response.text()
        } catch {
          textResponse = ''
        }
        try {
          // Try to extract error message from response body for other errors
          const errorResponse = JSON.parse(textResponse)
          if (errorResponse.error) {
            errorMessage = errorResponse.error
          }
        } catch {
          // If we can't parse the error response, use the default message
          // get the reponse text instead
          if (textResponse) {
            errorMessage = `HTTP error! status: ${response.status} - ${textResponse}`
          }
        }
      }

      throw new Error(errorMessage)
    }

    //try to parse the response based on content type
    const contentType = response.headers.get('Content-Type') || ''

    if (contentType.includes('application/json')) {
      return (await response.json()) as ResponseByUrl<U>
    } else if (
      contentType.startsWith('text/') ||
      contentType.includes('application/xml') ||
      contentType.includes('application/javascript') ||
      contentType.includes('application/json')
    ) {
      // Return text for text-like content types
      const textResponse = await response.text()
      return textResponse as unknown as ResponseByUrl<U>
    } else if (
      contentType.startsWith('image/') ||
      contentType.startsWith('video/') ||
      contentType.startsWith('audio/') ||
      contentType.includes('application/pdf') ||
      contentType.includes('application/octet-stream')
    ) {
      // Return blob for binary content types
      const blobResponse = await response.blob()
      return blobResponse as unknown as ResponseByUrl<U>
    } else {
      // Fallback to text for unknown content types
      const textResponse = await response.text()
      return textResponse as unknown as ResponseByUrl<U>
    }
  } catch (error) {
    // Handle AbortErrors more gracefully - they're expected during cleanup
    if (error instanceof Error && error.name === 'AbortError') {
      console.log(`Request to ${endpointUrl} was aborted`)
    } else {
      console.error(`Error calling endpoint ${endpointUrl}:`, error)
    }
    throw error // Re-throw to allow caller to handle it
  }
}
