import { Text, Title } from '@/components/ui'
import { ReactNode, useId } from 'react'
import { DivScrollY } from '@/components/common/DivScrollY'

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
  const titleId = useId()
  return (
    // Vertical spacing notes:
    //   pt-3 keeps the title close to the top header bar.
    //   mt-6 on the body section is the gap between title and body
    //   — done as a margin (not flex gap on the DivScrollY) because
    //   DivScrollY wraps its children in an internal
    //   `<div class="flex flex-col">`, so any `gap-*` on DivScrollY
    //   itself lands on the OverlayScrollbars wrapper and never
    //   reaches the title/body siblings.
    <DivScrollY className="h-full" role="region" aria-labelledby={titleId}>
      <div className="w-full flex justify-center pt-3">
        <div className={'max-w-4xl w-full flex flex-col gap-2 px-3'}>
          <Title
            level={4}
            id={titleId}
            data-testid="settings-page-title"
            className="!m-0 !leading-tight"
          >
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
      <div className={'flex w-full flex-1 justify-center pb-3 mt-3'}>
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
