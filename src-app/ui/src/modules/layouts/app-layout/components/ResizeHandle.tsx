import { CSSProperties, useEffect, useRef } from 'react'

export const ResizeHandle = ({
  placement,
  parentLevel,
  style,
  onStart,
  onEnd,
  scale,
  className,
  minWidth = 300,
  minHeight = 200,
  maxWidth = Infinity,
  maxHeight = Infinity,
}: {
  placement: 'top' | 'right' | 'bottom' | 'left'
  parentLevel?: number | number[] //to determine the parent element
  style?: CSSProperties
  onStart?: () => void
  onEnd?: () => void
  scale?: number
  className?: string
  minWidth?: number
  minHeight?: number
  maxWidth?: number
  maxHeight?: number
}) => {
  const ref = useRef<HTMLDivElement>(null)
  const parentRefs = useRef<HTMLElement[]>([])

  const parentLevels = Array.isArray(parentLevel)
    ? parentLevel
    : [parentLevel || 0]

  scale = scale ?? 1

  useEffect(() => {
    setTimeout(() => {
      const refs = []
      for (const parentLevel of parentLevels) {
        let parent = ref.current?.parentElement
        let level = 0
        while (parent && level < parentLevel) {
          parent = parent.parentElement
          level++
        }
        if (!parent) continue
        refs.push(parent)
      }
      // @ts-ignore
      parentRefs.current = refs
    }, 1000) // hack to wait for the parent to be rendered when in a portal
  }, [])

  const classNames = [
    'absolute',
    'transition-opacity',
    'duration-200',
    'pointer-events-auto',
    className,
  ]
  if (['top', 'bottom'].includes(placement)) {
    classNames.push('w-full', 'h-1', 'cursor-row-resize')
  } else {
    classNames.push('h-full', 'w-1', 'cursor-col-resize')
  }

  if (placement === 'top') classNames.push('top-0', 'left-0')
  if (placement === 'right') classNames.push('top-0', 'right-0')
  if (placement === 'bottom') classNames.push('bottom-0', 'left-0')
  if (placement === 'left') classNames.push('top-0', 'left-0')

  return (
    <div className={classNames.join(' ')} ref={ref} style={style}>
      <div
        className={'w-full h-full'}
        onMouseDown={event => {
          event.preventDefault()
          event.stopPropagation()

          if (onStart) onStart()

          const targets = parentRefs.current
          if (!targets.length) return

          for (const target of targets) {
            //disable css transition to prevent flickering
            const currentTransition = target.style.transition
            target.style.transition = 'none'

            const targetWindow = target.ownerDocument.defaultView!

            const currentPos = {
              top: event.clientY,
              left: event.clientX,
            }

            const currentDim = {
              width: target.offsetWidth,
              height: target.offsetHeight,
            }

            const previousDim = {
              width: target.offsetWidth,
              height: target.offsetHeight,
            }

            const currentScreenPos = {
              top: ref.current!.getBoundingClientRect().top,
              left: ref.current!.getBoundingClientRect().left,
            }

            const moveHandler = (e: MouseEvent) => {
              e.preventDefault()
              e.stopPropagation()

              const newPos = {
                top: e.clientY,
                left: e.clientX,
              }

              const newDim = {
                width:
                  placement === 'left'
                    ? currentDim.width +
                      (currentPos.left - newPos.left) * scale!
                    : currentDim.width +
                      (newPos.left - currentPos.left) * scale!,
                height:
                  placement === 'top'
                    ? currentDim.height + (currentPos.top - newPos.top) * scale!
                    : currentDim.height +
                      (newPos.top - currentPos.top) * scale!,
              }

              newDim.width = Math.max(newDim.width, minWidth)
              newDim.height = Math.max(newDim.height, minHeight)
              newDim.width = Math.min(newDim.width, maxWidth)
              newDim.height = Math.min(newDim.height, maxHeight)

              if (['top', 'bottom'].includes(placement)) {
                target.style.height = `${newDim.height}px`
              } else {
                target.style.width = `${newDim.width}px`
              }

              const newScreenPos = {
                top: ref.current!.getBoundingClientRect().top,
                left: ref.current!.getBoundingClientRect().left,
              }

              //restore old width/height if the element is at peak width/height
              if (
                currentScreenPos.top === newScreenPos.top &&
                currentScreenPos.left === newScreenPos.left
              ) {
                if (['top', 'bottom'].includes(placement)) {
                  target.style.height = `${previousDim.height}px`
                } else {
                  target.style.width = `${previousDim.width}px`
                }

                return
              }

              previousDim.width = newDim.width
              previousDim.height = newDim.height

              currentScreenPos.top = newScreenPos.top
              currentScreenPos.left = newScreenPos.left
            }

            const upHandler = (e: MouseEvent) => {
              e.preventDefault()
              e.stopPropagation()
              targetWindow.removeEventListener('mousemove', moveHandler)
              targetWindow.removeEventListener('mouseup', upHandler)
              target.style.transition = currentTransition

              if (onEnd) onEnd()
            }

            targetWindow.addEventListener('mousemove', moveHandler)
            targetWindow.addEventListener('mouseup', upHandler)
          }
        }}
      ></div>
    </div>
  )
}
