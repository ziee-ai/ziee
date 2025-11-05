import { useWindowSize } from 'react-use'

export type Breakpoint =
  | 'xxs'
  | 'xs'
  | 'sm'
  | 'md'
  | 'lg'
  | 'xl'
  | '2xl'
  | '3xl'

const breakpointValues: Record<Breakpoint, number> = {
  xxs: 0,
  xs: 480,
  sm: 640,
  md: 768,
  lg: 1024,
  xl: 1280,
  '2xl': 1536,
  '3xl': 1920,
}

export type MinSize = {
  xxs: boolean
  xs: boolean
  sm: boolean
  md: boolean
  lg: boolean
  xl: boolean
  '2xl': boolean
  '3xl': boolean
}

export const useWindowMinSize = (): MinSize => {
  const { width } = useWindowSize()

  return {
    xxs: width <= breakpointValues.xs,
    xs: width <= breakpointValues.sm,
    sm: width <= breakpointValues.md,
    md: width <= breakpointValues.lg,
    lg: width <= breakpointValues.xl,
    xl: width <= breakpointValues.xl,
    '2xl': width <= breakpointValues['xl'],
    '3xl': width <= breakpointValues['2xl'],
  }
}
