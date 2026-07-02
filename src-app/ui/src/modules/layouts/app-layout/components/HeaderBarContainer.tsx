import { Stores } from '@/core/stores'

interface HeaderBarContainerProps {
  children?: React.ReactNode
  className?: string
  style?: React.CSSProperties
}

export const HeaderBarContainer = ({
  children,
  className = '',
  style = {},
}: HeaderBarContainerProps) => {
  const { isSidebarCollapsed } = Stores.AppLayout

  return (
    <div
      className={`h-[50px] w-full flex relative px-3 border-b border-border transition-all duration-200 ease-in-out box-border py-0 ${className}`}
      style={{
        paddingLeft: isSidebarCollapsed ? 48 : 12,
        paddingRight: 12,
        zIndex: 2,
        ...style,
      }}
    >
      {children}
    </div>
  )
}
