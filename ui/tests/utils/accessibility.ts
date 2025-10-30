import { Page } from '@playwright/test'
import AxeBuilder from '@axe-core/playwright'

export interface AccessibilityCheckOptions {
  /**
   * WCAG tags to test against
   * Default: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa']
   */
  tags?: string[]

  /**
   * Rules to disable for this check
   */
  disabledRules?: string[]

  /**
   * Specific elements to include/exclude
   */
  include?: string[]
  exclude?: string[]
}

/**
 * Check accessibility violations on a page using axe-core
 */
export async function checkAccessibility(
  page: Page,
  options: AccessibilityCheckOptions = {}
): Promise<any[]> {
  const {
    tags = ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'],
    disabledRules = [],
    include = [],
    exclude = [],
  } = options

  let builder = new AxeBuilder({ page }).withTags(tags)

  // Disable specific rules if requested
  for (const rule of disabledRules) {
    builder = builder.disableRules([rule])
  }

  // Include/exclude specific elements
  if (include.length > 0) {
    for (const selector of include) {
      builder = builder.include(selector)
    }
  }

  if (exclude.length > 0) {
    for (const selector of exclude) {
      builder = builder.exclude(selector)
    }
  }

  const results = await builder.analyze()
  return results.violations
}

/**
 * Format violations for better readability
 */
export function formatViolations(violations: any[]): string {
  if (violations.length === 0) {
    return 'No accessibility violations found.'
  }

  let output = `Found ${violations.length} accessibility violation(s):\n\n`

  for (const violation of violations) {
    output += `❌ ${violation.id}: ${violation.description}\n`
    output += `   Impact: ${violation.impact}\n`
    output += `   Help: ${violation.help}\n`
    output += `   Learn more: ${violation.helpUrl}\n`
    output += `   Affected elements:\n`

    for (const node of violation.nodes) {
      output += `     - ${node.html.substring(0, 100)}${node.html.length > 100 ? '...' : ''}\n`
      if (node.failureSummary) {
        output += `       ${node.failureSummary}\n`
      }
    }

    output += '\n'
  }

  return output
}

/**
 * Assert that there are no accessibility violations on the page
 * This should be called in every test after the page is loaded
 */
export async function assertNoAccessibilityViolations(
  page: Page,
  options: AccessibilityCheckOptions = {}
) {
  const violations = await checkAccessibility(page, options)

  if (violations.length > 0) {
    const formatted = formatViolations(violations)
    throw new Error(`Accessibility violations detected:\n\n${formatted}`)
  }
}
