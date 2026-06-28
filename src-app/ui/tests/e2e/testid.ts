import type { Page, Locator } from '@playwright/test'
import type { TestIdLike } from '../../src/components/ui/testIds.generated'

/**
 * i18n-safe selector helper. Prefer this over getByText / getByRole({ name }) — visible
 * text and accessible names change under translation; data-testid does not.
 *
 *   await byTestId(page, 'user-form-email').fill('a@b.c')   // known id → autocompleted + typo-checked
 *   await byTestId(row, `user-row-${id}`).click()           // derived id → plain string accepted
 *
 * `id` is typed `TestIdLike` = a KnownTestId (autocompleted, compile-error on typo) OR any
 * string (so template-derived row/item ids still work).
 */
export const byTestId = (scope: Page | Locator, id: TestIdLike): Locator =>
  scope.getByTestId(id)
