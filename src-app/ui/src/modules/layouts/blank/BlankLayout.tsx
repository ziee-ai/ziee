import React, { useEffect } from 'react'
import { theme } from 'antd'

interface BlankLayoutProps {
  children: React.ReactNode
}

export function BlankLayout({ children }: BlankLayoutProps) {
  const { token } = theme.useToken()

  useEffect(() => {
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
