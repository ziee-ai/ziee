import { forwardRef } from 'react'
import {
  OverlayScrollbarsComponent,
  type OverlayScrollbarsComponentProps,
  type OverlayScrollbarsComponentRef,
} from 'overlayscrollbars-react'

export interface DivScrollYProps
  extends Omit<OverlayScrollbarsComponentProps, 'options'> {
  options?: OverlayScrollbarsComponentProps['options']
}

export const DivScrollY = forwardRef<
  OverlayScrollbarsComponentRef,
  DivScrollYProps
>(({ options, className, children, ...restProps }, ref) => {
  const mergedOptions = {
    scrollbars: { autoHide: 'scroll' as const },
    ...options,
  }

  const mergedClassName = ['overflow-y-auto', 'flex', className]
    .filter(Boolean)
    .join(' ')

  return (
    <OverlayScrollbarsComponent
      ref={ref}
      options={mergedOptions}
      className={mergedClassName}
      {...restProps}
    >
      <div className={'flex ' + (className || '')}>{children}</div>
    </OverlayScrollbarsComponent>
  )
})
