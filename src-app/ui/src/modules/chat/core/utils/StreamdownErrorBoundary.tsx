import { Component, type ReactNode } from 'react'

/**
 * Catches errors thrown by the lazy chunks Streamdown loads at runtime
 * (Shiki's `highlighted-body-*.js`, the mermaid plugin, etc.) and either:
 *
 *  1. Auto-retries once on dynamic-import failures. In dev/test this
 *     unblocks the "first chat code-block render" → Vite-504 race
 *     where Streamdown's lazy chunk hits Vite's on-the-fly optimizer
 *     and crashes the React tree before Vite finishes bundling.
 *  2. Falls back to plain pre-formatted text if the retry also fails
 *     — protects production against deploy-cycle stale-chunk crashes
 *     (upstream: https://github.com/vercel/streamdown/issues/343).
 *
 * Usage: wrap each `<Streamdown>` instance.
 *
 *   <StreamdownErrorBoundary fallbackText={text}>
 *     <Streamdown ...>{text}</Streamdown>
 *   </StreamdownErrorBoundary>
 */
interface Props {
  /** The text passed to `<Streamdown>` — rendered as plain `<pre>` if
   *  retry fails. */
  fallbackText: string
  children: ReactNode
}

interface State {
  error: Error | null
  retryAttempt: number
}

const isDynamicImportError = (err: unknown): boolean => {
  if (!(err instanceof Error)) return false
  const m = err.message ?? ''
  return (
    m.includes('Failed to fetch dynamically imported module') ||
    m.includes('Importing a module script failed') ||
    m.includes('Outdated Optimize Dep')
  )
}

export class StreamdownErrorBoundary extends Component<Props, State> {
  state: State = { error: null, retryAttempt: 0 }
  private retryTimer: ReturnType<typeof setTimeout> | null = null

  static getDerivedStateFromError(error: Error): Partial<State> {
    return { error }
  }

  componentDidUpdate(_prevProps: Props, prevState: State) {
    if (
      this.state.error &&
      !prevState.error &&
      this.state.retryAttempt === 0 &&
      isDynamicImportError(this.state.error)
    ) {
      // Auto-retry after a delay long enough for Vite to finish
      // re-bundling the lazy chunk (the `?v=` hash gets updated when
      // Vite's optimizer settles). 1.5s is a balance between fast
      // recovery in production (where the chunk is already cached
      // post-deploy) and giving Vite enough time to rebuild in dev/
      // test (which can take >500ms for the highlighted-body shiki
      // chunk on cold start).
      this.retryTimer = setTimeout(() => {
        this.setState({ error: null, retryAttempt: 1 })
      }, 1500)
    }
  }

  componentWillUnmount() {
    if (this.retryTimer) clearTimeout(this.retryTimer)
  }

  render() {
    if (this.state.error) {
      // Retry exhausted (or non-recoverable error): fall back to plain
      // text. Keeps the file content readable instead of a broken UI.
      return (
        <pre
          className="whitespace-pre-wrap break-words p-2 text-sm opacity-80"
          data-testid="streamdown-fallback"
        >
          {this.props.fallbackText}
        </pre>
      )
    }
    return this.props.children
  }
}
