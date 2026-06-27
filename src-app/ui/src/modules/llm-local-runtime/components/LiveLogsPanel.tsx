import { useEffect, useRef, useState } from 'react'
import { Card, Empty } from '@/components/ui'
import { ApiClient } from '@/api-client'
import type { SSELogLineData, SSELogLagData } from '@/api-client/types'

const MAX_LINES = 1000

/**
 * P2: live engine-log tail. Subscribes to the SSE endpoint
 * `GET /api/local-runtime/models/{id}/logs/stream` which replays the
 * existing buffer then streams new lines. Transient UI state, so
 * local `useState` (no store). Aborts the SSE connection on unmount.
 */
export function LiveLogsPanel({ modelId }: { modelId: string }) {
  const [lines, setLines] = useState<string[]>([])
  const abortRef = useRef<AbortController | null>(null)
  const scrollRef = useRef<HTMLDivElement | null>(null)

  useEffect(() => {
    let cancelled = false

    const append = (line: string) => {
      if (cancelled) return
      setLines(prev => {
        const next = [...prev, line]
        return next.length > MAX_LINES ? next.slice(next.length - MAX_LINES) : next
      })
    }

    // Typed SSE: the `streamLogs` response is the `SSELogEvent` map
    // ({ log: SSELogLineData; lag: SSELogLagData }), so each handler's
    // `data` is fully typed — no casts.
    ApiClient.LocalRuntime.streamLogs(
      { model_id: modelId },
      {
        SSE: {
          __init: data => {
            abortRef.current = data.abortController
          },
          log: (data: SSELogLineData) => {
            append(data.line)
          },
          lag: (data: SSELogLagData) => {
            append(`--- ${data.message} ---`)
          },
        },
      },
    ).catch(() => {
      // SSE connection closed / errored — leave the buffered lines.
    })

    return () => {
      cancelled = true
      abortRef.current?.abort()
    }
  }, [modelId])

  // Auto-scroll to bottom on new lines.
  useEffect(() => {
    if (scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight
    }
  }, [lines])

  return (
    <Card title="Live logs">
      {lines.length === 0 ? (
        <Empty description="No log output yet" />
      ) : (
        <div
          ref={scrollRef}
          className="max-h-[360px] overflow-y-auto font-mono text-xs whitespace-pre-wrap break-all"
        >
          {lines.map((line, i) => (
            <div key={i}>{line}</div>
          ))}
        </div>
      )}
    </Card>
  )
}
