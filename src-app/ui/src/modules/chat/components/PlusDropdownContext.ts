import { createContext, useContext } from 'react'

interface PlusDropdownContextValue {
  close: () => void
}

export const PlusDropdownContext = createContext<PlusDropdownContextValue>({
  close: () => {},
})

export const usePlusDropdown = () => useContext(PlusDropdownContext)
