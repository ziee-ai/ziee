import { Typography } from 'antd'
import { ReactNode } from 'react'
import { DivScrollY } from '@/components/common/DivScrollY'

const { Title, Text } = Typography

interface SettingsPageContainerProps {
  title: string | ReactNode
  subtitle?: string
  children: ReactNode
}

export function SettingsPageContainer({
  title,
  subtitle,
  children,
}: SettingsPageContainerProps) {
  return (
    <DivScrollY className="flex flex-col gap-3 h-full">
      <div className="w-full flex justify-center pt-3">
        <div className={'max-w-4xl w-full flex flex-col gap-2 px-3'}>
          <Title level={4} className="!m-0 !leading-tight">
            {title}
          </Title>
          {subtitle && (
            <Text
              type="secondary"
              className=" !m-0 !p-0 text-base !leading-tight"
            >
              {subtitle}
            </Text>
          )}
        </div>
      </div>
      <div className={'flex w-full flex-1 justify-center pb-3'}>
        <div
          className={
            'max-w-4xl w-full h-full flex flex-col gap-3 px-3 self-center'
          }
        >
          {children}
        </div>
      </div>
    </DivScrollY>
  )
}
