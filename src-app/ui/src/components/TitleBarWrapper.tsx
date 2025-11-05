import { theme } from 'antd'
import { Stores } from '@/core/stores'

interface TitleBarWrapperProps {
  children?: React.ReactNode
  className?: string
  style?: React.CSSProperties
}

export const TitleBarWrapper = ({
  children,
  className = '',
  style = {},
}: TitleBarWrapperProps) => {
  const { token } = theme.useToken()
  const { isSidebarCollapsed } = Stores.AppLayout

  return (
    <div
      className={`h-[50px] w-full flex relative border-b px-3 transition-all duration-200 ease-in-out box-border py-0 ${className}`}
      style={{
        paddingLeft: isSidebarCollapsed ? 48 : 12,
        paddingRight: 12,
        borderColor: token.colorBorderSecondary,
        ...style,
      }}
    >
      {children}
    </div>
  )
}
