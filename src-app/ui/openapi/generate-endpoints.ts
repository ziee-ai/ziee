import { readFileSync, writeFileSync } from 'fs'
import { resolve, dirname } from 'path'
import { fileURLToPath } from 'url'

// Get the directory where this script is located
const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)

// Read openapi.json from the openapi directory and write types.ts to ../src/api-client/types.ts
const openapiJsonPath = resolve(__dirname, 'openapi.json')
const targetPath = resolve(__dirname, '../src/api-client/types.ts')

interface OpenApiSpec {
  openapi: string
  info: {
    title: string
    version: string
  }
  paths: Record<string, Record<string, PathOperation>>
  components: {
    schemas: Record<string, SchemaDefinition>
  }
}

interface PathOperation {
  tags?: string[]
  description?: string
  operationId: string
  parameters?: Parameter[]
  requestBody?: RequestBody
  responses?: Record<string, ResponseDefinition>
}

interface Parameter {
  in: 'query' | 'path' | 'header'
  name: string
  schema: SchemaReference | SchemaType
  required?: boolean
  style?: string
}

interface RequestBody {
  content: Record<string, { schema: SchemaReference }>
  required?: boolean
}

interface ResponseDefinition {
  description?: string
  content?: Record<string, { schema: SchemaReference | SchemaType }>
}

interface SchemaReference {
  $ref: string
}

interface SchemaType {
  type: string | string[]
  format?: string
  properties?: Record<string, SchemaDefinition>
  items?: SchemaDefinition
  minimum?: number
}

interface SchemaDefinition extends SchemaType {
  $ref?: string
  required?: string[]
  anyOf?: (SchemaReference | SchemaType)[]
  allOf?: (SchemaReference | SchemaType)[]
  oneOf?: (SchemaReference | SchemaType)[]
  enum?: any[]
}

function isSchemaReference(schema: any): schema is SchemaReference {
  return schema && typeof schema === 'object' && '$ref' in schema
}

function extractSchemaName(ref: string): string {
  const schemaName = ref.replace('#/components/schemas/', '')

  // Special cases: convert to primitive types
  if (schemaName === 'AnyType') {
    return 'any'
  }
  if (schemaName === 'BlobType') {
    return 'Blob'
  }

  return schemaName
}

interface PermissionInfo {
  name: string
  value: string
  description: string
}

function extractPermissionsFromSpec(spec: OpenApiSpec): PermissionInfo[] {
  const permissionsMap = new Map<string, PermissionInfo>()

  // Scan through all paths and operations
  for (const [, methods] of Object.entries(spec.paths)) {
    for (const [, operation] of Object.entries(methods)) {
      // Check if there's a 403 response with required_permissions
      const forbiddenResponse = operation.responses?.['403']
      if (!forbiddenResponse?.content) continue

      const jsonContent = forbiddenResponse.content['application/json'] as any
      if (!jsonContent) continue

      // The example is directly on jsonContent, not on schema
      if (jsonContent.example?.details?.required_permissions) {
        const requiredPerms = jsonContent.example.details.required_permissions

        if (Array.isArray(requiredPerms)) {
          for (const perm of requiredPerms) {
            if (perm.name && perm.value && perm.description) {
              // Use name as key to avoid duplicates
              permissionsMap.set(perm.name, {
                name: perm.name,
                value: perm.value,
                description: perm.description,
              })
            }
          }
        }
      }
    }
  }

  // Convert map to array and sort by name
  return Array.from(permissionsMap.values()).sort((a, b) =>
    a.name.localeCompare(b.name),
  )
}

function generateEndpoints(): void {
  try {
    // Read and parse the OpenAPI JSON
    const openapiContent = readFileSync(openapiJsonPath, 'utf8')
    const spec: OpenApiSpec = JSON.parse(openapiContent)

    // Extract all endpoints
    const endpoints: Record<string, string> = {}
    const parameters: Record<string, string> = {}
    const responses: Record<string, string> = {}

    // Process each path
    for (const [path, methods] of Object.entries(spec.paths)) {
      for (const [method, operation] of Object.entries(methods)) {
        const operationId = operation.operationId
        if (!operationId) continue

        // Generate endpoint mapping
        const httpMethod = method.toUpperCase()
        const apiPath = path.replace(/{([^}]+)}/g, '{$1}') // Keep parameter format
        endpoints[operationId] = `${httpMethod} ${apiPath}`

        // Generate parameter types
        parameters[operationId] = generateParameterType(operation, path)

        // Generate response types
        responses[operationId] = generateResponseType(operation, httpMethod)
      }
    }

    // Extract all permissions from required_permissions
    const permissions = extractPermissionsFromSpec(spec)

    // Generate the TypeScript file content
    const content = generateTypeScriptContent(
      endpoints,
      parameters,
      responses,
      spec.components?.schemas || {},
      permissions,
    )

    // Write the generated file
    writeFileSync(targetPath, content, 'utf8')
    console.log(`✅ Generated API endpoints to ${targetPath}`)
  } catch (error) {
    console.error('❌ Error generating endpoints:', error)
    // eslint-disable-next-line no-process-exit
    process.exit(1)
  }
}

function detectQuerySchemaType(queryParams: Parameter[]): string | null {
  if (!queryParams.length) return null

  const paramNames = queryParams.map(p => p.name).sort()

  // Detect standard pagination pattern
  if (
    paramNames.length === 2 &&
    paramNames.includes('page') &&
    paramNames.includes('per_page')
  ) {
    return 'PaginationQuery'
  }

  return null
}

function generateParameterType(operation: PathOperation, path: string): string {
  const paramTypes: string[] = []

  // Add path parameters
  const pathParams = path.match(/{([^}]+)}/g)
  if (pathParams) {
    for (const param of pathParams) {
      const paramName = param.slice(1, -1) // Remove { }
      paramTypes.push(`${paramName}: string`)
    }
  }

  // Collect query parameters and detect schema patterns
  const queryParams: string[] = []
  let querySchemaType: string | null = null

  if (operation.parameters) {
    for (const param of operation.parameters) {
      if (param.in === 'query') {
        const isOptional = !param.required
        const paramType = getTypeFromSchema(param.schema, isOptional)
        queryParams.push(`${param.name}${isOptional ? '?' : ''}: ${paramType}`)
      }
    }

    // Detect common query parameter patterns and map to schema types
    querySchemaType = detectQuerySchemaType(
      operation.parameters.filter(p => p.in === 'query'),
    )
  }

  // Use schema type if detected, otherwise use individual parameters
  if (querySchemaType) {
    // Don't add individual query params, we'll use the schema type
  } else {
    paramTypes.push(...queryParams)
  }

  // Add request body type
  let requestBodyType: string | null = null
  if (operation.requestBody) {
    // Try application/json first
    let content = operation.requestBody.content['application/json']

    // If no application/json, try multipart/form-data
    if (!content) {
      content = operation.requestBody.content['multipart/form-data']
    }

    if (content && isSchemaReference(content.schema)) {
      requestBodyType = extractSchemaName(content.schema.$ref)
    } else if (content) {
      // For multipart/form-data or complex inline schemas, use FormData or any
      if (operation.requestBody.content['multipart/form-data']) {
        requestBodyType = 'FormData'
      } else {
        requestBodyType = 'any' // Generic fallback for complex inline schemas
      }
    }
  }

  // Return parameter object type or void
  if (paramTypes.length === 0 && !querySchemaType && !requestBodyType) {
    return 'void'
  } else if (paramTypes.length === 0 && !querySchemaType && requestBodyType) {
    // Only request body, no parameters
    return requestBodyType
  } else if (querySchemaType && !requestBodyType && paramTypes.length === 0) {
    // Only query schema, no path params or request body
    return querySchemaType
  } else if (querySchemaType && requestBodyType && paramTypes.length === 0) {
    // Query schema and request body, no path params
    return `${querySchemaType} & ${requestBodyType}`
  } else if (querySchemaType && paramTypes.length > 0 && !requestBodyType) {
    // Query schema and path params, no request body
    return `{ ${paramTypes.join('; ')} } & ${querySchemaType}`
  } else if (querySchemaType && paramTypes.length > 0 && requestBodyType) {
    // Query schema, path params, and request body
    return `{ ${paramTypes.join('; ')} } & ${querySchemaType} & ${requestBodyType}`
  } else if (paramTypes.length > 0 && requestBodyType) {
    // Both parameters and request body - combine them (fallback to old logic)
    return `{ ${paramTypes.join('; ')} } & ${requestBodyType}`
  } else if (
    paramTypes.length === 1 &&
    !operation.parameters?.some(p => p.in === 'query')
  ) {
    // Single path parameter, return as object
    return `{ ${paramTypes[0]} }`
  } else {
    return `{ ${paramTypes.join('; ')} }`
  }
}

function generateResponseType(
  operation: PathOperation,
  httpMethod?: string,
): string {
  if (!operation.responses || operation.responses['204']) {
    return 'void'
  }

  // Look for successful responses (200, 201, 204, etc.)
  const successResponse =
    operation.responses['200'] ||
    operation.responses['201'] ||
    operation.responses['202']

  if (!successResponse) {
    return 'any'
  }

  // If there's no content, return void for non-POST or any for POST
  if (!successResponse.content) {
    return httpMethod === 'POST' ? 'any' : 'void'
  }

  // Look for application/json content
  const jsonContent = successResponse.content['application/json']
  if (!jsonContent || !jsonContent.schema) {
    return httpMethod === 'POST' ? 'any' : 'any'
  }

  // Extract schema reference or type
  if (isSchemaReference(jsonContent.schema)) {
    return extractSchemaName(jsonContent.schema.$ref)
  } else {
    return getTypeFromSchema(jsonContent.schema)
  }
}

function getTypeFromSchema(
  schema: any,
  isOptionalParamOrNullable = false,
): string {
  // Handle boolean literal values (like profile: true in User schema)
  if (typeof schema === 'boolean') {
    return 'any' // or could be the literal boolean value
  }

  if (isSchemaReference(schema)) {
    const typeName = extractSchemaName(schema.$ref)
    
    // Handle JsonOption_for_ types - extract the actual type
    if (typeName.startsWith('JsonOption_for_')) {
      const actualType = typeName.replace('JsonOption_for_', '')
      
      // Convert specific patterns to proper TypeScript types
      if (actualType === 'Array_of_File') {
        return 'File[]'
      } else if (actualType === 'Array_of_MessageMetadata') {
        return 'MessageMetadata[]'
      } else if (actualType === 'Array_of_string') {
        return 'string[]'
      } else if (actualType.startsWith('Array_of_')) {
        // Generic array pattern: Array_of_TypeName -> TypeName[]
        const itemType = actualType.replace('Array_of_', '')
        return `${itemType}[]`
      } else {
        // Single type: JsonOption_for_TypeName -> TypeName
        return actualType
      }
    }

    // Handle EnumOption_for_ types - extract the actual enum type
    if (typeName.startsWith('EnumOption_for_')) {
      const actualType = typeName.replace('EnumOption_for_', '')
      return actualType
    }
    
    return typeName
  }

  // Handle anyOf patterns (union types with schema references)
  if (schema.anyOf && Array.isArray(schema.anyOf)) {
    const types = schema.anyOf
      .map((subSchema: any) => {
        if (isSchemaReference(subSchema)) {
          return extractSchemaName(subSchema.$ref)
        } else if (subSchema.type === 'null') {
          // If there's a null in anyOf and we're dealing with optional/nullable, skip it
          return isOptionalParamOrNullable ? null : 'null'
        } else {
          return getTypeFromSchema(subSchema, isOptionalParamOrNullable)
        }
      })
      .filter((type: string | null) => type !== null) // Remove null entries when filtered out

    return types.length === 1 ? types[0] : types.join(' | ')
  }

  // Handle oneOf patterns (union types with schema objects)
  if (schema.oneOf && Array.isArray(schema.oneOf)) {
    const types = schema.oneOf
      .map((subSchema: any) => {
        if (isSchemaReference(subSchema)) {
          return extractSchemaName(subSchema.$ref)
        } else if (subSchema.type === 'object' && subSchema.properties) {
          // Handle inline object schemas with discriminator type
          const props: string[] = []
          for (const [propName, propSchema] of Object.entries(subSchema.properties)) {
            let propType = getTypeFromSchema(propSchema, false)

            // Handle const values (literal types)
            if (propSchema.const !== undefined) {
              propType = typeof propSchema.const === 'string' ? `'${propSchema.const}'` : String(propSchema.const)
            }

            const isRequired = subSchema.required?.includes(propName)
            const optionalMarker = isRequired ? '' : '?'
            props.push(`${propName}${optionalMarker}: ${propType}`)
          }
          return `{ ${props.join('; ')} }`
        } else if (subSchema.const !== undefined) {
          // Handle const values (literal types) in oneOf
          return typeof subSchema.const === 'string'
            ? `'${subSchema.const}'`
            : String(subSchema.const)
        } else {
          return getTypeFromSchema(subSchema, isOptionalParamOrNullable)
        }
      })
      .filter((type: string | null) => type !== null)

    return types.length === 1 ? types[0] : types.join(' | ')
  }

  // Handle allOf patterns (intersection types with schema references)
  if (schema.allOf && Array.isArray(schema.allOf)) {
    const types = schema.allOf
      .map((subSchema: any) => {
        if (isSchemaReference(subSchema)) {
          return extractSchemaName(subSchema.$ref)
        } else {
          return getTypeFromSchema(subSchema, isOptionalParamOrNullable)
        }
      })
      .filter((type: string | null) => type !== null)

    // For allOf with a single reference (common pattern for enums), return the single type
    if (types.length === 1) {
      return types[0]
    }
    // For multiple types, use intersection (though this is less common)
    return types.join(' & ')
  }

  // Handle enum types
  if (schema.enum && Array.isArray(schema.enum)) {
    // Convert enum values to union type with string literals
    return schema.enum.map((value: any) => `'${value}'`).join(' | ')
  }

  if (typeof schema.type === 'string') {
    switch (schema.type) {
      case 'string':
        if (schema.format === 'date-time') {
          return 'string' // Could be Date if preferred
        }
        return 'string'
      case 'integer':
      case 'number':
        return 'number'
      case 'boolean':
        return 'boolean'
      case 'array':
        if (schema.items) {
          const itemType = getTypeFromSchema(schema.items)
          return `${itemType}[]`
        }
        return 'any[]'
      case 'object':
        if (schema.properties) {
          // Generate inline object type
          const props: string[] = []
          for (const [propName, propSchema] of Object.entries(
            schema.properties,
          )) {
            const propType = getTypeFromSchema(propSchema)
            props.push(`${propName}: ${propType}`)
          }
          return `{ ${props.join('; ')} }`
        }
        return 'any'
      default:
        return 'any'
    }
  } else if (Array.isArray(schema.type)) {
    // Handle union types like ["string", "null"] or ["array", "null"]
    const types = schema.type
      .filter((t: string) => {
        // If this is an optional parameter or nullable property, exclude null from the union
        // since optional parameters/properties don't need explicit null
        if (isOptionalParamOrNullable && t === 'null') {
          return false
        }
        return true
      })
      .map((t: string) => {
        switch (t) {
          case 'string':
            return 'string'
          case 'integer':
          case 'number':
            return 'number'
          case 'boolean':
            return 'boolean'
          case 'array':
            // Handle array type in union - use the items schema
            if (schema.items) {
              const itemType = getTypeFromSchema(schema.items)
              return `${itemType}[]`
            }
            return 'any[]'
          case 'null':
            return 'null'
          default:
            return 'any'
        }
      })
    return types.length === 1 ? types[0] : types.join(' | ')
  }

  return 'any'
}

function generateSchemaInterface(
  name: string,
  schema: SchemaDefinition,
): string {
  if (schema.$ref) {
    // This shouldn't happen for top-level schemas, but handle it just in case
    return `export type ${name} = ${extractSchemaName(schema.$ref)}`
  }

  // Skip JsonOption_for_ and EnumOption_for_ types since we convert them inline
  if (name.startsWith('JsonOption_for_') || name.startsWith('EnumOption_for_')) {
    return ''
  }

  // Special handling for SSE event types with oneOf pattern
  if (name.startsWith('SSE') && schema.oneOf && Array.isArray(schema.oneOf)) {
    return generateSSEEventType(name, schema.oneOf)
  }

  // Special handling for Permission type - convert to enum
  if (name === 'Permission' && schema.enum && Array.isArray(schema.enum)) {
    return generatePermissionEnum(schema.enum)
  }

  // Handle oneOf patterns for discriminated unions
  if (schema.oneOf && Array.isArray(schema.oneOf)) {
    // Special handling for MessageContentData to create separate named types
    if (name === 'MessageContentData') {
      return generateMessageContentDataTypes(schema.oneOf)
    }

    const types = schema.oneOf
      .map((subSchema: any) => {
        if (isSchemaReference(subSchema)) {
          return extractSchemaName(subSchema.$ref)
        } else if (subSchema.type === 'object' && subSchema.properties) {
          // Handle inline object schemas with discriminator type
          const props: string[] = []
          for (const [propName, propSchema] of Object.entries(subSchema.properties)) {
            let propType = getTypeFromSchema(propSchema, false)

            // Handle const values (literal types)
            if (propSchema.const !== undefined) {
              propType = typeof propSchema.const === 'string' ? `'${propSchema.const}'` : String(propSchema.const)
            }

            const isRequired = subSchema.required?.includes(propName)
            const optionalMarker = isRequired ? '' : '?'
            props.push(`  ${propName}${optionalMarker}: ${propType}`)
          }
          return `{\n${props.join('\n')}\n}`
        } else if (subSchema.const !== undefined) {
          // Handle const values (literal types) in oneOf
          return typeof subSchema.const === 'string'
            ? `'${subSchema.const}'`
            : String(subSchema.const)
        } else {
          return getTypeFromSchema(subSchema, false)
        }
      })
      .filter((type: string | null) => type !== null)

    return `export type ${name} = ${types.join(' | ')}`
  }

  if (schema.type === 'object' && schema.properties) {
    const properties: string[] = []

    for (const [propName, propSchema] of Object.entries(schema.properties)) {
      let isOptional = !schema.required?.includes(propName)

      // Check if this property references a JsonOption_for_ or EnumOption_for_ type
      let propType: string
      if (isSchemaReference(propSchema)) {
        const refTypeName = extractSchemaName(propSchema.$ref)
        if (refTypeName.startsWith('JsonOption_for_') || refTypeName.startsWith('EnumOption_for_')) {
          // This property uses a JsonOption or EnumOption type, so make it optional and get the actual type
          isOptional = true
          propType = getTypeFromSchema(propSchema, true) // This will handle the conversion
        } else {
          propType = getTypeFromSchema(propSchema)
        }
      } else {
        // Check if property is nullable (has null in union type or anyOf with null)
        const isNullableUnion =
          Array.isArray(propSchema.type) && propSchema.type.includes('null')
        const isNullableAnyOf =
          propSchema.anyOf &&
          Array.isArray(propSchema.anyOf) &&
          propSchema.anyOf.some((subSchema: any) => subSchema.type === 'null')
        const isNullableAllOf =
          propSchema.allOf &&
          Array.isArray(propSchema.allOf) &&
          propSchema.allOf.some((subSchema: any) => subSchema.type === 'null')
        const isNullable = isNullableUnion || isNullableAnyOf || isNullableAllOf

        // If property is nullable, make it optional and exclude null from type
        if (isNullable) {
          isOptional = true
        }

        propType = getTypeFromSchema(propSchema, isNullable)
      }

      const optionalMarker = isOptional ? '?' : ''
      properties.push(`  ${propName}${optionalMarker}: ${propType}`)
    }

    return `export interface ${name} {
${properties.join('\n')}
}`
  } else if (schema.type === 'array' && schema.items) {
    const itemType = getTypeFromSchema(schema.items)
    return `export type ${name} = ${itemType}[]`
  } else {
    // For primitive types or other cases
    const baseType = getTypeFromSchema(schema)
    return `export type ${name} = ${baseType}`
  }
}

function generateSSEEventType(name: string, oneOfVariants: (SchemaReference | SchemaType)[]): string {
  const eventTypes: string[] = []

  for (const variant of oneOfVariants) {
    // Handle discriminated union pattern (e.g., SSEChatStreamEvent)
    // Schema structure: { type: 'object', properties: { type: { const: "eventName" } }, allOf: [...] }
    if (variant.type === 'object' && variant.properties && variant.properties.type) {
      const typeProperty = variant.properties.type
      if (typeProperty.const) {
        const eventName = String(typeProperty.const)

        // Extract data type from allOf (the actual event data schema)
        let dataType = 'any'
        if (variant.allOf && Array.isArray(variant.allOf) && variant.allOf.length > 0) {
          const dataSchema = variant.allOf[0]
          if (isSchemaReference(dataSchema)) {
            dataType = extractSchemaName(dataSchema.$ref)
          } else {
            dataType = getTypeFromSchema(dataSchema)
          }
        }

        eventTypes.push(`  ${eventName}: ${dataType}`)
        continue
      }
    }

    // Handle simple object-based pattern (e.g., SSEHardwareUsageEvent)
    // Schema structure: { type: 'object', properties: { eventName: EventDataSchema } }
    if (variant.type === 'object' && variant.properties) {
      const eventNames = Object.keys(variant.properties)
      if (eventNames.length === 1) {
        const eventName = eventNames[0]
        const eventDataSchema = variant.properties[eventName]

        let dataType = 'any'
        if (isSchemaReference(eventDataSchema)) {
          dataType = extractSchemaName(eventDataSchema.$ref)
        } else {
          dataType = getTypeFromSchema(eventDataSchema)
        }

        eventTypes.push(`  ${eventName}: ${dataType}`)
      }
    }
  }

  return `export type ${name} = {
${eventTypes.join('\n')}
}`
}

function generateMessageContentDataTypes(oneOfVariants: (SchemaReference | SchemaType)[]): string {
  const typeDefinitions: string[] = []
  const unionTypes: string[] = []

  for (const variant of oneOfVariants) {
    if (variant.type === 'object' && variant.properties) {
      // Extract the type discriminator value
      const typeProperty = variant.properties.type
      if (typeProperty && typeProperty.const) {
        const typeValue = typeProperty.const

        // Convert type value to TypeScript interface name
        const typeName = `MessageContentData${typeValue.charAt(0).toUpperCase()}${typeValue.slice(1).replace(/_([a-z])/g, (_, letter) => letter.toUpperCase())}`

        // Generate properties for this variant (excluding the discriminator 'type' field)
        const props: string[] = []
        for (const [propName, propSchema] of Object.entries(variant.properties)) {
          // Skip the 'type' field as it's only a discriminator for TypeScript unions
          if (propName === 'type') {
            continue
          }

          let propType = getTypeFromSchema(propSchema, false)

          // Handle const values (literal types)
          if (propSchema.const !== undefined) {
            propType = typeof propSchema.const === 'string' ? `'${propSchema.const}'` : String(propSchema.const)
          }

          const isRequired = variant.required?.includes(propName)
          const optionalMarker = isRequired ? '' : '?'
          props.push(`  ${propName}${optionalMarker}: ${propType}`)
        }

        // Generate the interface
        typeDefinitions.push(`export interface ${typeName} {
${props.join('\n')}
}`)

        unionTypes.push(typeName)
      }
    }
  }

  // Generate the main union type
  const mainUnionType = `export type MessageContentData = ${unionTypes.join(' | ')}`

  // Return all type definitions
  return [...typeDefinitions, '', mainUnionType].join('\n')
}

function generatePermissionEnum(enumValues: any[]): string {
  // Convert permission string values to PascalCase enum keys
  const enumEntries: string[] = []

  for (const value of enumValues) {
    if (typeof value === 'string') {
      const enumKey = convertPermissionToPascalCase(value)
      enumEntries.push(`  ${enumKey} = '${value}'`)
    }
  }

  return `export enum Permission {
${enumEntries.join(',\n')}
}`
}

function convertPermissionToPascalCase(permission: string): string {
  // Handle special case for wildcard
  if (permission === '*') {
    return 'All'
  }

  // Split by :: and - then convert to PascalCase
  return permission
    .split('::')
    .map(part =>
      part
        .split('-')
        .map(word => word.charAt(0).toUpperCase() + word.slice(1))
        .join(''),
    )
    .join('')
}

function generateAllSchemas(schemas: Record<string, SchemaDefinition>): string {
  const interfaces: string[] = []

  // Sort schema names for consistent output
  const sortedNames = Object.keys(schemas).sort()

  for (const schemaName of sortedNames) {
    // Skip primitive type schemas since they should be treated as built-in types
    if (schemaName === 'AnyType' || schemaName === 'BlobType') {
      continue
    }

    const schema = schemas[schemaName]
    const interfaceDefinition = generateSchemaInterface(schemaName, schema)

    // Only add non-empty interface definitions
    if (interfaceDefinition.trim()) {
      interfaces.push(interfaceDefinition)
    }
  }

  return interfaces.join('\n\n')
}

function generatePermissionsEnum(permissions: PermissionInfo[]): string {
  if (permissions.length === 0) {
    return `export enum Permissions {}`
  }

  const enumEntries = permissions.map(perm => `  ${perm.name} = '${perm.value}'`)

  return `export enum Permissions {
${enumEntries.join(',\n')}
}`
}

function generatePermissionDescriptions(permissions: PermissionInfo[]): string {
  if (permissions.length === 0) {
    return `export const PermissionDescriptions: Record<string, string> = {}`
  }

  const descriptionEntries = permissions.map(
    perm => `  ${perm.name}: '${perm.description.replace(/'/g, "\\'")}'`,
  )

  return `export const PermissionDescriptions: Record<string, string> = {
${descriptionEntries.join(',\n')}
}`
}

function generateTypeScriptContent(
  endpoints: Record<string, string>,
  parameters: Record<string, string>,
  responses: Record<string, string>,
  schemas: Record<string, SchemaDefinition>,
  permissions: PermissionInfo[],
): string {
  const sortedEndpoints = Object.keys(endpoints).sort()

  // Generate header and schema definitions
  const header = `/**
 * Generated API endpoint definitions
 * Auto-generated from OpenAPI specification
 * 
 * ⚠️  DO NOT EDIT THIS FILE MANUALLY ⚠️
 * This file is automatically generated from the OpenAPI specification generated from the server code.
 */

// =============================================================================
// TYPE DEFINITIONS
// =============================================================================

`

  // Generate all schema interfaces
  const schemaDefinitions = generateAllSchemas(schemas) + '\n\n'

  // Generate permissions enum and descriptions
  const permissionsSection = `// =============================================================================
// PERMISSIONS
// =============================================================================

${generatePermissionsEnum(permissions)}

${generatePermissionDescriptions(permissions)}

`

  // Generate endpoints object
  const endpointsSection = `// =============================================================================
// API ENDPOINTS
// =============================================================================

// API endpoint definitions
export const ApiEndpoints = {
${sortedEndpoints.map(key => `  '${key}': '${endpoints[key]}'`).join(',\n')}
} as const

`

  // Generate parameter types
  const parametersSection = `// API endpoint parameters
export type ApiEndpointParameters = {
${sortedEndpoints.map(key => `  '${key}': ${parameters[key]}`).join('\n')}
}

`

  // Generate response types
  const responsesSection = `// API endpoint responses
export type ApiEndpointResponses = {
${sortedEndpoints.map(key => `  '${key}': ${responses[key]}`).join('\n')}
}

`

  // Generate helper types
  const helpersSection = `// Type helpers
export type ApiEndpoint = keyof typeof ApiEndpoints
export type ApiEndpointUrl = (typeof ApiEndpoints)[ApiEndpoint]

// Extract endpoint key from URL pattern
export function getEndpointKey(url: string): ApiEndpoint | undefined {
  const entries = Object.entries(ApiEndpoints) as [ApiEndpoint, string][]
  const found = entries.find(([_key, value]) => value === url)
  return found ? found[0] : undefined
}

// Get parameter type for endpoint
export type GetParameterType<K extends ApiEndpoint> = ApiEndpointParameters[K]

// Get response type for endpoint  
export type GetResponseType<K extends ApiEndpoint> = ApiEndpointResponses[K]

// Create reverse mapping from URL to endpoint key
export type UrlToEndpoint<U extends ApiEndpointUrl> = {
  [K in keyof typeof ApiEndpoints]: (typeof ApiEndpoints)[K] extends U
    ? K
    : never
}[keyof typeof ApiEndpoints]

// Helper types to get parameter and response types by URL
export type ParameterByUrl<U extends ApiEndpointUrl> =
  ApiEndpointParameters[UrlToEndpoint<U>]
export type ResponseByUrl<U extends ApiEndpointUrl> =
  ApiEndpointResponses[UrlToEndpoint<U>]

// Type-safe validation - this will cause a TypeScript error if any endpoint is missing
type ValidateParametersComplete = {
  [K in keyof typeof ApiEndpoints]: K extends keyof ApiEndpointParameters
    ? true
    : false
}

type ValidateResponsesComplete = {
  [K in keyof typeof ApiEndpoints]: K extends keyof ApiEndpointResponses
    ? true
    : false
}

// Type-safe validation - these will cause a TypeScript error if any endpoint is missing
// from Parameters or Responses. They are used for compile-time validation only.
export type { ValidateParametersComplete, ValidateResponsesComplete }
`

  return (
    header +
    schemaDefinitions +
    permissionsSection +
    endpointsSection +
    parametersSection +
    responsesSection +
    helpersSection
  )
}

// Run the generator
generateEndpoints()
