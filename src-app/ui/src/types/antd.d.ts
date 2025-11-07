/**
 * TypeScript module augmentation for Ant Design
 *
 * This file overrides Ant Design's type definitions to enforce stricter type safety.
 */

import type { FormProps as AntFormProps } from 'antd'

declare module 'antd' {
  /**
   * Override FormProps to make `name` required
   *
   * Why this is important:
   * - Ant Design generates input IDs as: {formName}_{itemName}
   * - Without a form name, IDs become just {itemName}, causing duplicates across forms
   * - Duplicate IDs break:
   *   - Accessibility (screen readers)
   *   - Form labels (label[for] won't work correctly)
   *   - E2E tests (ambiguous selectors)
   *
   * @example
   * // ❌ TypeScript Error - name is required
   * <Form onFinish={handleSubmit}>
   *   <Form.Item name="username"><Input /></Form.Item>
   * </Form>
   *
   * // ✅ Correct - name is provided
   * <Form name="login-form" onFinish={handleSubmit}>
   *   <Form.Item name="username"><Input /></Form.Item>
   * </Form>
   *
   * // Generated ID will be: login-form_username
   */
  export interface FormProps<Values = any> extends Omit<AntFormProps<Values>, 'name'> {
    /**
     * Form name (REQUIRED)
     *
     * Used to generate unique field IDs: {name}_{fieldName}
     * This prevents ID collisions when multiple forms exist on the same page.
     */
    name: string
  }
}
