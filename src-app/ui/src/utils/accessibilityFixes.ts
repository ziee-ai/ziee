/**
 * Global accessibility fixes for Ant Design components
 *
 * These fixes address known accessibility issues in Ant Design that cannot be
 * easily fixed through props or configuration.
 */

/**
 * Inject CSS to fix color contrast issues in Ant Design components
 *
 * Some Ant Design components don't properly respect theme tokens, so we need
 * to inject CSS overrides to meet WCAG AA contrast requirements.
 */
function injectAccessibilityCSS() {
  // Check if we already injected the styles
  if (document.getElementById('accessibility-fixes-css')) {
    return
  }

  const style = document.createElement('style')
  style.id = 'accessibility-fixes-css'
  style.textContent = `
    /* Fix dropdown menu item text color for proper contrast (WCAG AA 4.5:1) */
    /* Use high specificity to override Ant Design default styles */
    .ant-dropdown .ant-dropdown-menu .ant-dropdown-menu-item .ant-dropdown-menu-title-content,
    .ant-dropdown-menu .ant-dropdown-menu-item .ant-dropdown-menu-title-content {
      color: rgba(0, 0, 0, 0.88) !important;
    }

    /* Dark mode fix for dropdown menu items */
    .dark .ant-dropdown .ant-dropdown-menu .ant-dropdown-menu-item .ant-dropdown-menu-title-content,
    .dark .ant-dropdown-menu .ant-dropdown-menu-item .ant-dropdown-menu-title-content {
      color: rgba(255, 255, 255, 0.85) !important;
    }

    /* Also target the menu item wrapper to ensure coverage */
    .ant-dropdown-menu-item:not(.ant-dropdown-menu-item-disabled) {
      color: rgba(0, 0, 0, 0.88) !important;
    }

    .dark .ant-dropdown-menu-item:not(.ant-dropdown-menu-item-disabled) {
      color: rgba(255, 255, 255, 0.85) !important;
    }
  `
  document.head.appendChild(style)
}

/**
 * Remove aria-required from Select components
 *
 * Ant Design's Form.Item adds aria-required="true" to Select components when
 * the field is required, but according to ARIA spec, the combobox role (used by
 * Select) does not support the aria-required attribute. This causes accessibility
 * violations.
 *
 * This function sets up a MutationObserver to automatically remove aria-required
 * from all Select components.
 */
export function setupAccessibilityFixes() {
  // Only run in browser environment
  if (typeof window === 'undefined' || typeof document === 'undefined') {
    return
  }

  // Inject CSS fixes for color contrast
  injectAccessibilityCSS()

  // Function to remove aria-required from Select components
  const removeAriaRequiredFromSelects = () => {
    const selects = document.querySelectorAll(
      '.ant-select[aria-required="true"]',
    )
    selects.forEach(select => {
      select.removeAttribute('aria-required')
    })
  }

  // Run once on initial load
  removeAriaRequiredFromSelects()

  // Set up MutationObserver to handle dynamically added Selects
  const observer = new MutationObserver(mutations => {
    for (const mutation of mutations) {
      if (
        mutation.type === 'attributes' &&
        mutation.attributeName === 'aria-required'
      ) {
        const target = mutation.target as HTMLElement
        if (target.classList.contains('ant-select')) {
          target.removeAttribute('aria-required')
        }
      } else if (mutation.type === 'childList') {
        removeAriaRequiredFromSelects()
      }
    }
  })

  // Observe the entire document for changes
  observer.observe(document.body, {
    attributes: true,
    attributeFilter: ['aria-required'],
    childList: true,
    subtree: true,
  })

  // Return cleanup function
  return () => {
    observer.disconnect()
  }
}
