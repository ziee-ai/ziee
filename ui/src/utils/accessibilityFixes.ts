/**
 * Global accessibility fixes for Ant Design components
 *
 * These fixes address known accessibility issues in Ant Design that cannot be
 * easily fixed through props or configuration.
 */

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

  // Function to remove aria-required from Select components
  const removeAriaRequiredFromSelects = () => {
    const selects = document.querySelectorAll('.ant-select[aria-required="true"]')
    selects.forEach(select => {
      select.removeAttribute('aria-required')
    })
  }

  // Run once on initial load
  removeAriaRequiredFromSelects()

  // Set up MutationObserver to handle dynamically added Selects
  const observer = new MutationObserver(mutations => {
    for (const mutation of mutations) {
      if (mutation.type === 'attributes' && mutation.attributeName === 'aria-required') {
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
