import { createContext } from 'react'
import { ThemeConfig } from 'antd'

export const ThemeContext = createContext<ThemeConfig | undefined>(undefined)
