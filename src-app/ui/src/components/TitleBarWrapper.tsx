import { theme } from 'antd'

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

  return (
    <div
      className={`h-[50px] w-full flex relative border-b px-3 transition-all duration-200 ease-in-out box-border py-0 ${className}`}
      style={{
        paddingLeft: 12,
        paddingRight: 12,
        borderColor: token.colorBorderSecondary,
        ...style,
      }}
    >
      {children}
    </div>
  )
}
