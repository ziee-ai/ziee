import React, { useEffect } from 'react'

interface BlankLayoutProps {
  children: React.ReactNode
}

export function BlankLayout({ children }: BlankLayoutProps) {
  useEffect(() => {
    // set root document background color based on theme
    const root = document.documentElement
    root.style.backgroundColor = 'hsl(var(--background))'
  }, [])

  return <>{children}</>
}
