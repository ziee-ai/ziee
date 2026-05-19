import { useMemo } from 'react'
import { theme, Alert } from 'antd'

const MAX_LINES = 100

export function RawCodeView({ text }: { text: string }) {
  const { token } = theme.useToken()
  // Memoize the split — for large files (logs, source code) this is the only
  // expensive work in this component. Without memo, every parent re-render
  // (panel resize, drawer toggle, sibling state change) re-splits the entire
  // file just to throw away everything past MAX_LINES.
  const { lines, truncated } = useMemo(() => {
    const allLines = text.split('\n')
    const wasTruncated = allLines.length > MAX_LINES
    return {
      lines: wasTruncated ? allLines.slice(0, MAX_LINES) : allLines,
      truncated: wasTruncated,
    }
  }, [text])

  return (
    <div className="flex flex-col w-full h-full">
      {truncated && (
        <Alert
          message={`Showing first ${MAX_LINES} lines. Download the file to view all data.`}
          type="warning"
          showIcon
          className="m-2 flex-shrink-0"
        />
      )}
      <div className="flex w-full min-h-0 flex-1 font-mono text-sm overflow-auto">
        <div
          className="flex-shrink-0 text-right pr-3 select-none border-r"
          style={{
            color: token.colorTextQuaternary,
            borderColor: token.colorBorderSecondary,
            minWidth: '3rem',
            lineHeight: '1.6',
          }}
        >
          {lines.map((_, i) => <div key={i}>{i + 1}</div>)}
        </div>
        <div className="pl-3 flex-1 whitespace-pre" style={{ lineHeight: '1.6' }}>
          {lines.map((line, i) => <div key={i}>{line || ' '}</div>)}
        </div>
      </div>
    </div>
  )
}
