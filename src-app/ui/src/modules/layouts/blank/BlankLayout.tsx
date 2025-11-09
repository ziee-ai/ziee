import React, { useEffect } from 'react'
import { theme } from 'antd'

interface BlankLayoutProps {
  children: React.ReactNode
}

export function BlankLayout({ children }: BlankLayoutProps) {
  const { token } = theme.useToken()

  useEffect(() => {
    //set root document background color based on theme
    const root = document.documentElement
    root.style.backgroundColor = token.colorBgLayout
  }, [token.colorBgLayout])

  return <>{children}</>
}
