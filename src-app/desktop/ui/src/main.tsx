import React from 'react'
import ReactDOM from 'react-dom/client'
import { App, loadModules as loadCoreModules } from '@ziee/ui-core'
import { loadDesktopModules } from './modules/loader'
import '@ziee/ui-core/index.css'

/**
 * Desktop Application Entry Point
 *
 * Loads both core UI modules and desktop-specific modules,
 * then renders the App component with all modules registered.
 */

// Load core UI modules (auth, settings, llm-provider, etc.)
console.log('Loading core UI modules...')
loadCoreModules()

// Load desktop-specific modules (window, tray, file-dialog, etc.)
console.log('Loading desktop modules...')
loadDesktopModules()

// Initialize all modules (core + desktop)
// This is handled by the core UI's initializeModules() call

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
