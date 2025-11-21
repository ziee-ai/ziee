# Gemini Provider Implementation Audit

## Audit Date: 2025-01-20

## Summary

**Status**: ❌ **CRITICAL BUGS FOUND**

The Gemini provider implementation has critical bugs that prevent proper function calling:
1. Missing `FunctionResponse` part type
2. Tool results sent as plain text instead of proper `functionResponse` structure

---

## Bug #1: Missing FunctionResponse Part Type

### Location
`ai-providers/src/providers/gemini.rs:51-67`

### Current Code
```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
enum GeminiPart {
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        thought: Option<bool>,
    },
    InlineData {
        inline_data: GeminiInlineData,
    },
    FileData {
        file_data: GeminiFileData,
    },
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: GeminiFunctionCall,
    },
    // ❌ MISSING: FunctionResponse variant
}
```

### Problem
The `GeminiPart` enum is missing the `FunctionResponse` variant, which is required to send function execution results back to the model.

### Expected Structure
According to Gemini API documentation:
```rust
FunctionResponse {
    #[serde(rename = "functionResponse")]
    function_response: GeminiFunctionResponse,
}
```

Where `GeminiFunctionResponse` should be:
```rust
#[derive(Serialize, Deserialize, Debug, Clone)]
struct GeminiFunctionResponse {
    name: String,
    response: serde_json::Value,
}
```

### Impact
- ⚠️ **HIGH**: Function calling doesn't work properly
- Tool results are sent as plain text instead of structured responses
- Model can't properly process function results
- Multi-turn function calling breaks

---

## Bug #2: ToolResult Converted to Plain Text

### Location
`ai-providers/src/providers/gemini.rs:293-307`

### Current Code
```rust
ContentBlock::ToolResult {
    tool_use_id: _,
    content,
    is_error: _,
} => {
    // Convert tool result content blocks to text
    for sub_block in content {
        if let ContentBlock::Text { text } = sub_block {
            parts.push(GeminiPart::Text {
                text: text.clone(),
                thought: None,
            });
        }
    }
}
```

### Problem
When converting `ToolResult` content blocks, the code:
1. Ignores `tool_use_id` (needed to match with function call)
2. Ignores `is_error` flag (important for error handling)
3. Converts result to plain text instead of proper `functionResponse` structure

### Expected Behavior
According to Gemini API docs, function responses should be structured as:
```json
{
  "role": "user",
  "parts": [
    {
      "functionResponse": {
        "name": "get_weather",
        "response": {
          "temperature": 20,
          "unit": "C"
        }
      }
    }
  ]
}
```

### Current Behavior
```json
{
  "role": "user",
  "parts": [
    {
      "text": "{\"temperature\": 20, \"unit\": \"C\"}"
    }
  ]
}
```

### Impact
- ⚠️ **HIGH**: Model receives function results as unstructured text
- Model can't properly parse and use function results
- Function name association is lost
- Error states are not communicated

---

## Bug #3: No Test Verification of Tool Calling

### Location
`ai-providers/tests/test_gemini.rs:140-206`

### Current Test
```rust
#[tokio::test]
#[ignore]
async fn test_gemini_streaming_with_tools() {
    // ... setup request with tools ...

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                for delta in &chunk.content {
                    match delta {
                        ContentBlockDelta::ToolUseDelta { .. } => {
                            // Skip tool use deltas  ❌ DOESN'T VERIFY!
                        }
                        // ...
                    }
                }
                chunk_count += 1;
            }
            Err(e) => panic!("Stream error: {:?}", e),
        }
    }

    // ❌ NO ASSERTION that tool was called!
    println!("Test passed - streaming with tools completed (tool calls may result in 0 content chunks)");
}
```

### Problem
The test:
1. ❌ Doesn't assert that a tool call was generated
2. ❌ Doesn't verify the tool call structure (id, name, args)
3. ❌ Doesn't test the complete round-trip (call → execute → response)
4. ✅ Only verifies the request doesn't crash

### Impact
- **MEDIUM**: False confidence in tool calling support
- No verification that Gemini actually calls tools
- No verification of round-trip workflow

---

## Additional Observations

### Correct Implementations

✅ **Tool Configuration Mapping** (`gemini.rs:439-452`)
- Correctly maps `ToolChoice::Auto` → "AUTO"
- Correctly maps `ToolChoice::Required` → "ANY"
- Correctly maps `ToolChoice::Specific` → "ANY"

✅ **Function Declaration Structure** (`gemini.rs:122-136`)
- Correct `functionDeclarations` structure
- Proper JSON schema format

✅ **UUID Generation for Function Calls** (`gemini.rs:481-492`)
- Correctly generates synthetic IDs: `gemini_{uuid}`
- Necessary because Gemini API doesn't provide IDs

✅ **ToolUse → FunctionCall Conversion** (`gemini.rs:363-369`)
- Correctly converts `ToolUse` in assistant messages to `FunctionCall`

---

## Root Cause Analysis

### Why Wasn't This Caught?

1. **Test Doesn't Verify Tool Calling**: The test accepts 0 chunks as success
2. **No Integration Test**: No test that verifies complete function calling workflow
3. **No Round-Trip Test**: No test that calls a function and sends results back

### Why It Fails in MCP Tests

The MCP approval workflow tests fail because:
1. Gemini models receive the prompt with tools
2. Models choose not to call tools (with `ToolChoice::Auto`)
3. No pending approvals are created
4. Test fails immediately

**Why models don't call tools:**
- May require `ToolChoice::Required` (ANY mode) to force calling
- May require more explicit prompting
- OR: Even if model calls tool, function response is broken (sent as text)

---

## Required Fixes

### Fix #1: Add FunctionResponse Part Type

```rust
// Add to GeminiPart enum
FunctionResponse {
    #[serde(rename = "functionResponse")]
    function_response: GeminiFunctionResponse,
}

// Add new struct
#[derive(Serialize, Deserialize, Debug, Clone)]
struct GeminiFunctionResponse {
    name: String,
    response: serde_json::Value,
}
```

### Fix #2: Proper ToolResult Conversion

```rust
ContentBlock::ToolResult {
    tool_use_id,
    content,
    is_error,
} => {
    // Extract the function name from tool_use_id or content
    // For now, we need to track the original function name
    // This is a design issue - ToolResult should include function name

    // Convert content to response value
    let response_value = if let Some(ContentBlock::Text { text }) = content.first() {
        // Try to parse as JSON, fallback to string
        serde_json::from_str(text).unwrap_or_else(|_| {
            serde_json::json!({ "result": text })
        })
    } else {
        serde_json::json!({ "error": "Empty result" })
    };

    // Add error info if needed
    let final_response = if *is_error {
        serde_json::json!({
            "error": response_value,
            "is_error": true
        })
    } else {
        response_value
    };

    parts.push(GeminiPart::FunctionResponse {
        function_response: GeminiFunctionResponse {
            name: /* NEED FUNCTION NAME */,
            response: final_response,
        },
    });
}
```

**Problem**: The `ToolResult` content block doesn't include the function name, only `tool_use_id`. We need to either:
1. Track function names by tool_use_id (requires state)
2. Add function name to `ToolResult` structure (requires API change)
3. Extract function name from tool_use_id if it contains the name

### Fix #3: Create Proper Test

```rust
#[tokio::test]
#[ignore]
async fn test_gemini_function_calling_complete_workflow() {
    // Setup with ToolChoice::Required to force function calling
    let tool = Tool::function(
        "get_weather",
        "Get the current weather",
        json!({
            "type": "object",
            "properties": {
                "location": {"type": "string"}
            },
            "required": ["location"]
        }),
    );

    let request = ChatRequest {
        model: MODEL_GEMINI_25_FLASH,
        messages: vec![ChatMessage::user("What's the weather in Tokyo?")],
        tools: vec![tool],
        tool_choice: Some(ToolChoice::Required),  // Force tool calling
        max_tokens: Some(500),
        ..Default::default()
    };

    let mut stream = provider.chat_stream(request).await.expect("Stream failed");

    let mut tool_calls = Vec::new();
    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                for delta in &chunk.content {
                    if let ContentBlockDelta::ToolUseDelta { id, name, input_delta, .. } = delta {
                        // Track tool calls
                        if let (Some(id), Some(name), Some(input)) = (id, name, input_delta) {
                            tool_calls.push((id.clone(), name.clone(), input.clone()));
                        }
                    }
                }
            }
            Err(e) => panic!("Stream error: {:?}", e),
        }
    }

    // VERIFY: At least one tool was called
    assert!(!tool_calls.is_empty(), "Expected at least one tool call");
    assert_eq!(tool_calls[0].1, "get_weather", "Expected get_weather function");

    // TODO: Test round-trip by sending function response back
}
```

---

## Priority

1. **CRITICAL**: Fix #1 (Add FunctionResponse) - Required for any tool calling
2. **CRITICAL**: Fix #2 (Proper ToolResult conversion) - Required for round-trip
3. **HIGH**: Fix #3 (Proper test) - Required to verify fixes work
4. **MEDIUM**: Address ToolResult design issue (missing function name)

---

## Next Steps

1. Implement Fix #1: Add FunctionResponse part type
2. Implement Fix #2: Convert ToolResult to functionResponse
3. Implement Fix #3: Create proper verification test
4. Run test to verify tool calling works end-to-end
5. Update MCP tests to use ToolChoice::Required for Gemini
6. Document Gemini-specific requirements

---

## Expected Outcome

After fixes:
- ✅ Gemini can receive and parse tool definitions
- ✅ Gemini can generate function calls (with Required mode)
- ✅ Function results sent back properly as functionResponse
- ✅ Multi-turn function calling works
- ✅ MCP approval workflow tests pass for Gemini models
