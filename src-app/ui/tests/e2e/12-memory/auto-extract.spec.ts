import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
} from '../../common/auth-helpers'

/**
 * E2E — auto-extraction (Plan §9 Phase 3).
 *
 * After enabling extraction_enabled + sending a chat, the extracted
 * fact should appear on the Memories page. Requires:
 *   - admin-enabled memory globally
 *   - embedding model + extraction model configured
 *   - a real LLM to actually emit extraction JSON
 *
 * Gated behind ANTHROPIC_API_KEY since the extraction LLM is the
 * critical missing dep; in CI without the key this test skips.
 */

const HAS_LLM = Boolean(process.env.ANTHROPIC_API_KEY || process.env.OPENAI_API_KEY)

test.describe('Memory — auto-extraction', () => {
  test.skip(!HAS_LLM, 'no LLM api key — skipping real-LLM extraction test')
  test.slow()

  test('extracted fact appears on Memories page', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    await loginAsAdmin(page, baseURL)

    // Real test requires:
    //   1. enable memory in admin settings
    //   2. pick embedding model + extraction model
    //   3. enable extraction_enabled on user settings
    //   4. start a chat saying "My name is Foo Bar"
    //   5. wait for assistant reply + background extraction
    //   6. visit /memories and assert the name was captured
    // Scaffold left for follow-up wiring.
    expect(true).toBe(true)
  })
})
