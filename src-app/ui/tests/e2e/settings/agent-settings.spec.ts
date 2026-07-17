import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid.ts'

/**
 * TEST-31 — the agent admin-settings page happy path: an admin opens
 * `/settings/agent`, the policy form renders, edits a bounded field
 * (`default_max_steps`), saves, and the new value PERSISTS across a reload
 * (GET/PUT roundtrip through the real backend).
 */
test.describe('Agent settings — admin happy path', () => {
  test('admin edits default_max_steps and it persists', async ({
    page,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    await page.goto(`${baseURL}/settings/agent`)

    // The policy form + the field render.
    await expect(byTestId(page, 'agent-settings-card')).toBeVisible({
      timeout: 30000,
    })
    // The testid is forwarded to the underlying number input.
    const field = byTestId(page, 'agent-settings-default-max-steps')
    await expect(field).toBeVisible({ timeout: 30000 })

    // Edit to a fresh in-range value + save.
    const value = '77'
    await field.fill('')
    await field.fill(value)
    await page.getByRole('button', { name: /save/i }).first().click()

    // Persistence: reload and the field shows the saved value (real GET after PUT).
    await page.waitForTimeout(1500)
    await page.reload()
    await expect(byTestId(page, 'agent-settings-card')).toBeVisible({
      timeout: 30000,
    })
    await expect(
      byTestId(page, 'agent-settings-default-max-steps'),
    ).toHaveValue(value, { timeout: 30000 })
  })
})
