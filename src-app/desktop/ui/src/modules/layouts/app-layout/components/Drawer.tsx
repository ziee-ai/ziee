/**
 * Desktop Override: Drawer
 *
 * Adds Tauri-specific features:
 * - TauriDragRegion in title for window movement
 * - Position monitoring to adjust title padding for macOS traffic lights
 * - Tauri-aware wrapper styles (maxWidth, border, borderRadius)
 */

import {
  Button,
  Drawer as AntDrawer,
  DrawerProps as AntDrawerProps,
  theme,
  Typography,
} from 'antd'
import React, { useEffect, useRef } from 'react'
import { ResizeHandle } from '@/modules/layouts/app-layout/components/ResizeHandle'
import tinycolor from 'tinycolor2'
import { useWindowMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'
import { IoIosArrowBack } from 'react-icons/io'
import { DivScrollY } from '@/components/common/DivScrollY'
import { isTauriView, isMacOS } from '@ziee/desktop/core/platform'
import { TauriDragRegion } from '@ziee/desktop/components/TauriDragRegion'

export interface DrawerProps extends AntDrawerProps {
  children?: React.ReactNode
}

export const Drawer: React.FC<DrawerProps> = props => {
  const { token } = theme.useToken()
  const windowMinSize = useWindowMinSize()

  const drawerDivRef = useRef<HTMLDivElement>(null)
  const titleRef = useRef<HTMLDivElement>(null)

  // Monitor the left position of the drawer div and adjust title padding for macOS traffic lights
  useEffect(() => {
    if (!isTauriView) return
    if (!props.open) return

    const monitorPosition = () => {
      if (drawerDivRef.current && titleRef.current) {
        const rect = drawerDivRef.current.getBoundingClientRect()
        const leftMin = isMacOS ? 72 : 0
        if (rect.left < leftMin) {
          titleRef.current.style.paddingLeft = leftMin - rect.left + 'px'
        } else {
          titleRef.current.style.paddingLeft = ''
        }
      }
    }

    // Run after drawer animation completes to get correct position
    const initialTimeout = setTimeout(monitorPosition, 300)

    const resizeObserver = new ResizeObserver(monitorPosition)

    if (drawerDivRef.current) {
      resizeObserver.observe(drawerDivRef.current)
    }

    return () => {
      clearTimeout(initialTimeout)
      resizeObserver.disconnect()
    }
  }, [props.open])

  // Calculate max resize width (leave room for traffic lights on macOS)
  const resizeMaxWidth =
    isTauriView && isMacOS ? window.innerWidth - 90 : window.innerWidth - 24

  const {
    placement = 'right',
    size = 520,
    width, // deprecated - for backwards compatibility
    maskClosable = true,
    children,
    styles: propsStyles,
    ...restProps
  } = props

  // Resolve styles if it's a function
  const resolvedPropsStyles =
    typeof propsStyles === 'function' ? propsStyles({ props }) : propsStyles

  // Use size, fallback to width for backwards compatibility
  const drawerSize = width !== undefined ? width : size

  // Determine if we should use size or width prop
  const useSizeProp =
    typeof drawerSize === 'number' ||
    drawerSize === 'default' ||
    drawerSize === 'large'

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
        ? { size: drawerSize as number | 'default' | 'large' }
        : { width: windowMinSize.xs ? '100%' : drawerSize })}
      maskClosable={maskClosable}
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
            ref={titleRef}
            className={
              'flex w-full items-center gap-1 py-2 pt-[10px] px-1 relative'
            }
            style={{
              // Initial padding for full-width drawers on small screens (macOS traffic lights)
              paddingLeft:
                windowMinSize.xs && isTauriView && isMacOS ? 74 : undefined,
            }}
          >
            <TauriDragRegion
              className={'h-full w-full absolute top-0 left-0'}
            />
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
          border:
            windowMinSize.xs && !isTauriView
              ? 'none'
              : `1px solid ${token.colorBorderSecondary}`,
          borderRadius: isTauriView ? 8 : windowMinSize.xs ? 0 : 8,
          maxWidth: `calc(100vw - ${isTauriView && windowMinSize.xs ? 0 : isTauriView ? 90 : windowMinSize.xs ? 0 : 24}px)`,
          boxShadow: 'none',
          margin: windowMinSize.xs ? 0 : 12,
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
            <ResizeHandle
              placement={'left'}
              parentLevel={[1]}
              maxWidth={resizeMaxWidth}
            />
          </div>
        )
      }}
    >
      <DivScrollY className={'flex w-full h-full'}>
        <div className={'flex w-full h-full pr-3'}>
          {React.Children.map(children, child => {
            if (React.isValidElement(child)) {
              return React.cloneElement(child, {
                ...child.props,
                className: `w-full ${child.props.className || ''}`.trim(),
              })
            }
            return child
          })}
        </div>
      </DivScrollY>
    </AntDrawer>
  )
}
