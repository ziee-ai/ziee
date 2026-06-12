import {
  Button,
  Drawer as AntDrawer,
  DrawerProps as AntDrawerProps,
  theme,
  Typography,
} from 'antd'
import React, { useRef } from 'react'
import { ResizeHandle } from '@/modules/layouts/app-layout/components/ResizeHandle'
import tinycolor from 'tinycolor2'
import { useWindowMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'
import { IoIosArrowBack } from 'react-icons/io'
import { DivScrollY } from '@/components/common/DivScrollY'

export interface DrawerProps extends AntDrawerProps {
  children?: React.ReactNode
  /**
   * When true, render `children` directly in the drawer body instead
   * of wrapping them in the `<DivScrollY>` (vertical OverlayScrollbars)
   * scroll layer. Use this for content that owns its own scrolling
   * (e.g. file preview, where the body's `<pre>` needs both vertical
   * AND horizontal scroll with scrollbars anchored to the viewport
   * edge — the DivScrollY wrapper collapses the inner flex/height
   * chain and forces both scrollbars to the bottom of the unbounded
   * content box). Defaults to false; existing drawer callers keep
   * the wrapped behavior unchanged.
   */
  noBodyScrollWrap?: boolean
}

export const Drawer: React.FC<DrawerProps> = props => {
  const { token } = theme.useToken()
  const windowMinSize = useWindowMinSize()

  const drawerDivRef = useRef<HTMLDivElement>(null)

  const {
    placement = 'right',
    size = 520,
    children,
    styles: propsStyles,
    noBodyScrollWrap = false,
    ...restProps
  } = props

  // Resolve styles if it's a function
  const resolvedPropsStyles =
    typeof propsStyles === 'function' ? propsStyles({ props }) : propsStyles

  // antd 6 `size` accepts number | 'default' | 'large' | string.
  // On the smallest breakpoint we want the panel to fill the
  // viewport — antd's `size` doesn't accept '100%', so route through
  // `width` only in that case (still a supported antd prop, not
  // deprecated; just less convenient than `size` for the common case).
  const useSizeProp =
    typeof size === 'number' || size === 'default' || size === 'large'

  if (Array.isArray(restProps.footer)) {
    restProps.footer = (
      <div className="flex gap-2">
        {restProps.footer.map((item, index) => (
          <React.Fragment key={index}>{item}</React.Fragment>
        ))}
      </div>
    )
  }

  return (
    <AntDrawer
      placement={placement}
      {...(useSizeProp
        ? { size: size as number | 'default' | 'large' }
        : { width: windowMinSize.xs ? '100%' : size })}
      {...restProps}
      closable={false}
      classNames={{
        body: `!pl-3 !pr-0 !pt-0 overflow-x-visible`,
        wrapper: '!overflow-hidden !bg-transparent',
        ...(restProps.classNames || {}),
      }}
      title={
        props.title ? (
          <div
            className={
              'flex w-full items-center gap-1 py-2 pt-[10px] px-1 relative'
            }
          >
            <Button
              type={'text'}
              onClick={props.onClose}
              aria-label="Close drawer"
              style={{
                width: 30,
              }}
            >
              <div className={'text-xl'}>
                <IoIosArrowBack aria-hidden="true" />
              </div>
            </Button>
            {typeof props.title === 'string' ? (
              <Typography.Title level={5} className={'!m-0'}>
                {props.title}
              </Typography.Title>
            ) : (
              props.title
            )}
          </div>
        ) : null
      }
      styles={{
        header: {
          borderBottom: 'none',
          padding: 0,
          backgroundColor: token.colorBgLayout,
          ...(resolvedPropsStyles?.header || {}),
        },
        footer: {
          borderTop: 'none',
          padding: '6px 12px 12px 12px',
          backgroundColor: token.colorBgLayout,
          ...(resolvedPropsStyles?.footer || {}),
        },
        mask: {
          backdropFilter: 'brightness(0.75)',
          backgroundColor: tinycolor(token.colorBgLayout)
            .setAlpha(0.75)
            .toString(),
          ...(resolvedPropsStyles?.mask || {}),
        },
        wrapper: {
          border: windowMinSize.xs
            ? 'none'
            : `1px solid ${token.colorBorderSecondary}`,
          borderRadius: windowMinSize.xs ? 0 : 8,
          maxWidth: `calc(100vw - ${windowMinSize.xs ? 0 : 24}px)`,
          boxShadow: 'none',
          // 8px inset on top/right/bottom matches the LeftSidebar
          // box's inset from the window frame, so the drawer
          // visually belongs to the same "floating card" tier as
          // the sidebar. Left margin (between drawer and the
          // underlying content) keeps the larger 12px gap. Full-
          // bleed on `xs` regardless.
          marginTop: windowMinSize.xs ? 0 : 8,
          marginRight: windowMinSize.xs ? 0 : 8,
          marginBottom: windowMinSize.xs ? 0 : 8,
          marginLeft: windowMinSize.xs ? 0 : 12,
          ...(resolvedPropsStyles?.wrapper || {}),
        },
        body: {
          backgroundColor: token.colorBgLayout,
          ...(resolvedPropsStyles?.body || {}),
        },
      }}
      drawerRender={node => {
        return (
          <div
            ref={drawerDivRef}
            className={'w-full h-full'}
            onTouchStart={e => e.stopPropagation()}
            onTouchMove={e => e.stopPropagation()}
            onTouchEnd={e => e.stopPropagation()}
            onScroll={e => e.stopPropagation()}
            onWheel={e => e.stopPropagation()}
          >
            <div className={'w-full h-full'}>{node}</div>
            <ResizeHandle placement={'left'} parentLevel={[1]} />
          </div>
        )
      }}
    >
      {noBodyScrollWrap ? (
        // Direct render — caller manages its own scroll. The flex
        // wrapper still adds `w-full h-full pr-3` so layout matches
        // the wrapped path (consumers can rely on a known parent
        // box). The `pr-3` matches the body padding the wrapped
        // path applies to compensate for the body's `!pr-0` class.
        <div className={'flex w-full h-full pr-3'}>
          {React.Children.map(children, child => {
            if (React.isValidElement<{ className?: string }>(child)) {
              return React.cloneElement(child, {
                ...child.props,
                className: `w-full ${child.props.className || ''}`.trim(),
              })
            }
            return child
          })}
        </div>
      ) : (
        <DivScrollY className={'flex w-full h-full'}>
          <div className={'flex w-full h-full pr-3'}>
            {React.Children.map(children, child => {
              if (React.isValidElement<{ className?: string }>(child)) {
                return React.cloneElement(child, {
                  ...child.props,
                  className:
                    `w-full ${child.props.className || ''}`.trim(),
                })
              }
              return child
            })}
          </div>
        </DivScrollY>
      )}
    </AntDrawer>
  )
}
