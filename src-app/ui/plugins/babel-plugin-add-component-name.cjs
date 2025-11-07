/**
 * Babel plugin to automatically add data-component-name attribute to React components
 * This helps with E2E testing by providing semantic, stable selectors
 *
 * Example transformation:
 * export function MyComponent() {
 *   return <div>Hello</div>
 * }
 *
 * Becomes:
 * export function MyComponent() {
 *   return <div data-component-name="MyComponent">Hello</div>
 * }
 */

module.exports = function ({ types: t }) {
  return {
    visitor: {
      // Track the current function component name
      FunctionDeclaration(path, state) {
        if (isReactComponent(path.node)) {
          state.componentName = path.node.id?.name
        }
      },

      VariableDeclarator(path, state) {
        // Handle: const MyComponent = () => <div />
        // Handle: const MyComponent = function() { return <div /> }
        if (
          t.isIdentifier(path.node.id) &&
          (t.isArrowFunctionExpression(path.node.init) ||
            t.isFunctionExpression(path.node.init))
        ) {
          const functionNode = path.node.init
          if (isReactComponent({ body: functionNode.body, params: functionNode.params })) {
            state.componentName = path.node.id.name
          }
        }
      },

      // Add data-component-name to the root JSX element
      JSXElement(path, state) {
        const componentName = state.componentName

        // Only add to root elements (elements that are direct return values)
        if (!componentName || !isRootJSXElement(path)) {
          return
        }

        // Check if data-component-name already exists
        const openingElement = path.node.openingElement
        const hasDataAttribute = openingElement.attributes.some(
          attr =>
            t.isJSXAttribute(attr) &&
            t.isJSXIdentifier(attr.name) &&
            attr.name.name === 'data-component-name'
        )

        if (hasDataAttribute) {
          return
        }

        // Add data-component-name attribute
        const dataAttribute = t.jsxAttribute(
          t.jsxIdentifier('data-component-name'),
          t.stringLiteral(componentName)
        )

        openingElement.attributes.push(dataAttribute)
      },
    },
  }
}

/**
 * Check if a function is likely a React component
 * - Name starts with uppercase letter
 * - Returns JSX or has JSX in body
 */
function isReactComponent(node) {
  if (!node) return false

  // Check if it has params that look like props (optional check)
  const hasValidParams =
    !node.params ||
    node.params.length === 0 ||
    node.params.length === 1

  if (!hasValidParams) return false

  // Check if body contains JSX
  let hasJSX = false

  if (node.body) {
    // For arrow functions with implicit return: () => <div />
    if (node.body.type === 'JSXElement' || node.body.type === 'JSXFragment') {
      hasJSX = true
    }

    // For functions with block body
    if (node.body.type === 'BlockStatement') {
      hasJSX = containsJSX(node.body)
    }
  }

  return hasJSX
}

/**
 * Check if a node or its children contain JSX
 */
function containsJSX(node) {
  if (!node) return false

  if (node.type === 'JSXElement' || node.type === 'JSXFragment') {
    return true
  }

  if (node.type === 'ReturnStatement' && node.argument) {
    return (
      node.argument.type === 'JSXElement' ||
      node.argument.type === 'JSXFragment' ||
      containsJSX(node.argument)
    )
  }

  // Check children
  const keys = Object.keys(node)
  for (const key of keys) {
    const child = node[key]

    if (Array.isArray(child)) {
      for (const item of child) {
        if (item && typeof item === 'object' && containsJSX(item)) {
          return true
        }
      }
    } else if (child && typeof child === 'object') {
      if (containsJSX(child)) {
        return true
      }
    }
  }

  return false
}

/**
 * Check if JSX element is a root element (direct return or variable assignment)
 */
function isRootJSXElement(path) {
  const parent = path.parent

  // Direct return: return <div />
  if (parent.type === 'ReturnStatement') {
    return true
  }

  // Arrow function implicit return: () => <div />
  if (parent.type === 'ArrowFunctionExpression') {
    return true
  }

  // Variable assignment: const element = <div />
  if (parent.type === 'VariableDeclarator') {
    return true
  }

  // Parenthesized expression: return (<div />)
  if (parent.type === 'ParenthesizedExpression') {
    return isRootJSXElement(path.parentPath)
  }

  return false
}
