# Gemini Tool Calling Investigation - MCP Approval Workflow

## Problem Statement

**All 4 Gemini models fail MCP approval workflow tests with "no pending approvals created"**

- Gemini 2.5 Flash ✗
- Gemini 2.5 Pro ✗
- Gemini 2.0 Flash ✗
- Gemini 2.0 Flash Lite ✗

Meanwhile:
- Anthropic models: 4/4 PASSED ✓
- OpenAI models: 4/4 PASSED ✓

## Root Cause Analysis

### 1. Gemini Doesn't Generate Tool Use IDs

**Location**: `ai-providers/src/providers/gemini.rs:481-487`

```rust
GeminiPart::FunctionCall { function_call } => {
    content_deltas.push(crate::models::ContentBlockDelta::ToolUseDelta {
        index: idx,
        id: None,  // ❌ GEMINI PROVIDES NO ID!
        name: Some(function_call.name.clone()),
        input_delta: Some(function_call.args.to_string()),
    });
}
```

**Comparison with Other Providers**:

**Anthropic** (works ✓):
- Generates IDs like: `toolu_01BfYv57L3aeQahoTV7dtAJh`
- ID included in every tool use event

**OpenAI** (works ✓):
- Generates IDs like: `call_F6UweqbxXgtF7rAp1b56rLOh`
- ID included in every tool use event

**Gemini** (fails ✗):
- NO ID generation
- Function calls identified only by name and position

### 2. MCP Extension Requires Tool Use IDs

**Location**: `src/modules/chat/extensions/mcp/content.rs:15-19`

```rust
pub enum McpContentData {
    ToolUse {
        id: String,  // ⚠️ REQUIRED, NOT Optional<String>
        name: String,
        input: serde_json::Value,
    },
    // ...
}
```

**Location**: `src/modules/chat/extensions/mcp/mcp.rs:472-473`

```rust
if let McpContentData::ToolUse { id, name, input } = mcp_content {
    tool_uses.push((id, name, input));  // ID is required to create approval
}
```

### 3. Approval Records Need Tool Use IDs

**Location**: `src/modules/chat/extensions/mcp/approval/models.rs`

```rust
pub struct ToolUseApproval {
    pub tool_use_id: String,  // Used for matching approvals
    // ...
}
```

**Location**: `src/modules/chat/extensions/mcp/approval/repository.rs:271-280`

```rust
pub async fn approve_tool_use(
    pool: &PgPool,
    tool_use_id: String,  // Required for WHERE clause
    branch_id: Uuid,
    // ...
) -> Result<ToolUseApproval, AppError>
```

### 4. The Failure Chain

```
1. User sends message with tool request
   ↓
2. Gemini generates function_call response
   - No ID provided (Gemini API design)
   ↓
3. GeminiProvider converts to ToolUseDelta
   - Sets id: None
   ↓
4. Streaming service persists content to database
   - Tool use stored with missing/empty ID
   ↓
5. MCP extension's after_llm_call extracts tool uses
   - from_content_block fails to parse (no ID)
   - OR creates ToolUse with empty string ID
   ↓
6. No tool uses extracted = No approvals created
   ↓
7. Test fails: "no pending approvals created" ✗
```

## Technical Details

### Gemini API Response Format

Gemini uses `functionCall` instead of `tool_use`:

```json
{
  "candidates": [{
    "content": {
      "role": "model",
      "parts": [{
        "functionCall": {
          "name": "fetch_server__fetch",
          "args": {
            "url": "https://httpbin.org/get"
          }
        }
      }]
    }
  }]
}
```

**Key Difference**: No `id` field in `functionCall` object.

### Anthropic API Response (for comparison)

```json
{
  "content": [{
    "type": "tool_use",
    "id": "toolu_01BfYv57L3aeQahoTV7dtAJh",  // ✓ ID PROVIDED
    "name": "fetch_server__fetch",
    "input": {
      "url": "https://httpbin.org/get"
    }
  }]
}
```

### OpenAI API Response (for comparison)

```json
{
  "choices": [{
    "message": {
      "tool_calls": [{
        "id": "call_F6UweqbxXgtF7rAp1b56rLOh",  // ✓ ID PROVIDED
        "type": "function",
        "function": {
          "name": "fetch_server__fetch",
          "arguments": "{\"url\":\"https://httpbin.org/get\"}"
        }
      }]
    }
  }]
}
```

## Solution Options

### Option 1: Generate IDs in Gemini Provider (RECOMMENDED ✓)

**Location**: `ai-providers/src/providers/gemini.rs:481-487`

```rust
GeminiPart::FunctionCall { function_call } => {
    // Generate a unique ID for Gemini function calls
    let tool_use_id = format!("gemini_{}", uuid::Uuid::new_v4().to_string());

    content_deltas.push(crate::models::ContentBlockDelta::ToolUseDelta {
        index: idx,
        id: Some(tool_use_id),  // ✓ GENERATE ID
        name: Some(function_call.name.clone()),
        input_delta: Some(function_call.args.to_string()),
    });
}
```

**Pros**:
- Minimal code change
- Fixes issue at the source
- Maintains consistency with other providers
- No changes needed to MCP extension

**Cons**:
- Requires adding uuid dependency to ai-providers crate
- Generated IDs won't match any Gemini-side reference

### Option 2: Make Tool Use ID Optional

**Location**: `src/modules/chat/extensions/mcp/content.rs:15-19`

```rust
pub enum McpContentData {
    ToolUse {
        id: Option<String>,  // Make optional
        name: String,
        input: serde_json::Value,
    },
    // ...
}
```

**Pros**:
- More flexible design
- Handles providers that don't generate IDs

**Cons**:
- Requires changes throughout MCP extension
- Approval matching becomes complex (would need to match on name+input?)
- Breaks existing approval workflow assumptions
- More extensive refactoring needed

### Option 3: Generate IDs at Content Persistence Layer

**Location**: Streaming service when persisting extension content

**Pros**:
- Centralized ID generation
- Works for all providers

**Cons**:
- More complex - need to detect missing IDs
- May affect other extensions
- Harder to trace where IDs come from

## Recommended Fix

**Option 1: Generate IDs in Gemini Provider**

### Implementation Steps

1. **Add UUID dependency to ai-providers**:
   ```toml
   # ai-providers/Cargo.toml
   uuid = { version = "1.0", features = ["v4"] }
   ```

2. **Update Gemini provider streaming**:
   ```rust
   // ai-providers/src/providers/gemini.rs
   GeminiPart::FunctionCall { function_call } => {
       use uuid::Uuid;
       let tool_use_id = format!("gemini_{}", Uuid::new_v4());

       content_deltas.push(crate::models::ContentBlockDelta::ToolUseDelta {
           index: idx,
           id: Some(tool_use_id),
           name: Some(function_call.name.clone()),
           input_delta: Some(function_call.args.to_string()),
       });
   }
   ```

3. **Test with Gemini models**:
   ```bash
   source tests/.env.test && cargo test --test integration_tests \
     test_approval_workflow_multi_model -- --test-threads=1 --nocapture
   ```

### Expected Result

After fix:
- Gemini 2.5 Flash: ✓ PASSED
- Gemini 2.5 Pro: ✓ PASSED
- Gemini 2.0 Flash: ✓ PASSED
- Gemini 2.0 Flash Lite: ✓ PASSED

**Total**: 12/12 models passing (100% success rate)

## Alternative: Why Not Make ID Optional?

Making `id: Option<String>` would require:

1. **Update McpContentData** - Change id field type
2. **Update approval creation** - Handle missing IDs (match on name+input hash?)
3. **Update approval matching** - More complex WHERE clauses
4. **Update all approval queries** - Add NULL handling
5. **Update SSE events** - Handle missing tool_use_id
6. **Update frontend** - Display and track approvals without IDs

**Estimated effort**: 10-15 files, 50+ changes
**vs. Option 1**: 1 file, 5 lines

## Testing Strategy

### Unit Test for ID Generation

```rust
#[test]
fn test_gemini_generates_tool_use_ids() {
    let function_call = GeminiFunctionCall {
        name: "test_tool".to_string(),
        args: json!({"key": "value"}),
    };

    let part = GeminiPart::FunctionCall { function_call };

    // Convert to ToolUseDelta
    let delta = convert_part_to_delta(part);

    // Assert ID is present and starts with "gemini_"
    match delta {
        ContentBlockDelta::ToolUseDelta { id, .. } => {
            assert!(id.is_some());
            assert!(id.unwrap().starts_with("gemini_"));
        }
        _ => panic!("Expected ToolUseDelta"),
    }
}
```

### Integration Test

Use existing `test_approval_workflow_multi_model` which will automatically test all Gemini models once fix is applied.

## Files to Modify

1. **ai-providers/Cargo.toml** - Add uuid dependency
2. **ai-providers/src/providers/gemini.rs** - Generate IDs for function calls
3. **(Optional) Add tests** - Verify ID generation

## Impact Analysis

**Impact**: Minimal
- Change isolated to Gemini provider
- No changes to API contracts
- No database schema changes
- Backward compatible (existing messages unaffected)

**Risk**: Low
- UUID generation is well-tested
- Format matches other providers ("provider_uuid")
- Only affects new Gemini tool calls

## Conclusion

Gemini's lack of tool use IDs is a fundamental API difference from Anthropic and OpenAI. The cleanest solution is to generate synthetic IDs in the Gemini provider to maintain consistency with our approval workflow's expectations.

**Recommendation**: Implement Option 1 (Generate IDs in Gemini Provider)
