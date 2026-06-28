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

  return <>{children}</>
}
