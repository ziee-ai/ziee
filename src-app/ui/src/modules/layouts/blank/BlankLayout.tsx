import React, { useLayoutEffect } from 'react'
import { theme } from 'antd'

interface BlankLayoutProps {
  children: React.ReactNode
}

export function BlankLayout({ children }: BlankLayoutProps) {
  const { token } = theme.useToken()

  // useLayoutEffect (not useEffect) so the background color is applied AND
  // restored synchronously before the browser paints — with useEffect the
  // change lands a frame late, producing a visible white/blank flash on mount
  // and on teardown (e.g. closing a popup).
  useLayoutEffect(() => {
    // set root document background color based on theme, restore on teardown
    const root = document.documentElement
    const prev = root.style.backgroundColor
    root.style.backgroundColor = token.colorBgLayout
    return () => {
      root.style.backgroundColor = prev
    }
  }, [token.colorBgLayout])

  // Provide a top-level `main` landmark so assistive tech has a primary
  // content region on the auth/blank pages (these render outside the app
  // shell that normally supplies it). `display: contents` keeps the element
  // out of the box tree so it has zero layout impact while still exposing the
  // landmark role.
  return <main style={{ display: 'contents' }}>{children}</main>
}
