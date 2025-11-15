# AI Provider Test Recommendations

## Executive Summary

Based on comprehensive research of current (2025) AI model offerings, we identified **15 critical missing models** that should be added to our test suite. These models have **different API behaviors** or represent **different capability tiers** that warrant separate testing.

## Current Test Coverage vs. Market Reality

### What We Test Now ✅
- **OpenAI**: gpt-3.5-turbo, o3-mini, gpt-5, embeddings, Groq
- **Anthropic**: claude-sonnet-4.5, claude-haiku-4.5
- **Gemini**: gemini-2.5-flash, embeddings

### What We're Missing ❌
- **OpenAI**: Missing gpt-4o family (most popular), gpt-4-turbo, o1 series
- **Anthropic**: Missing claude-opus-4.1 (most powerful), claude-3.5 variants
- **Gemini**: Missing gemini-2.0 GA models, 2.5-pro, 2.0-pro-exp

## Priority 1: Add These 5 Models IMMEDIATELY

These models have **fundamentally different APIs** and are **widely used in production**:

### 1. `gpt-4o` (OpenAI)
```rust
const MODEL_GPT4O: &str = "gpt-4o";
```
- **Why Critical**: Most popular GPT-4 variant, 128k context, multimodal (vision + audio)
- **API Difference**: Audio input/output support, vision capabilities
- **Test Focus**: Streaming chat, vision (if supported), context length
- **Market Share**: Dominant model in production deployments

### 2. `gpt-4o-mini` (OpenAI)
```rust
const MODEL_GPT4O_MINI: &str = "gpt-4o-mini";
```
- **Why Critical**: Most affordable GPT-4 class model, cost-optimized
- **API Difference**: 16k context (vs 128k), different token limits
- **Test Focus**: Cost efficiency, streaming performance
- **Use Case**: High-volume applications

### 3. `gpt-4-turbo` (OpenAI)
```rust
const MODEL_GPT4_TURBO: &str = "gpt-4-turbo";
// OR specific version:
const MODEL_GPT4_TURBO: &str = "gpt-4-turbo-2024-04-09";
```
- **Why Critical**: Different from GPT-4o, optimized for traditional completions
- **API Difference**: Traditional completion optimization, different pricing
- **Test Focus**: Chat completions, multimodal
- **Legacy Support**: Many apps still use this model

### 4. `claude-opus-4-1-20250805` (Anthropic)
```rust
const MODEL_OPUS_41: &str = "claude-opus-4-1-20250805";
```
- **Why Critical**: Most powerful Claude model, highest capability tier
- **API Difference**: Superior performance, different pricing tier
- **Test Focus**: Complex reasoning, extended thinking
- **Capability**: "Best model to date" for code generation

### 5. `models/gemini-2.0-pro-exp` (Google)
```rust
const MODEL_GEMINI_2_PRO: &str = "models/gemini-2.0-pro-exp";
```
- **Why Critical**: Largest context window (2M tokens), best coding performance
- **API Difference**: 2M token context (vs 1M), experimental features
- **Test Focus**: Massive context handling, coding tasks
- **Unique Feature**: Largest context window available

## Priority 2: Add These 5 Models Soon

These represent **latest generation models** and **important capability tiers**:

### 6. `o1` or `o1-preview` (OpenAI)
```rust
const MODEL_O1: &str = "o1";  // or "o1-preview"
```
- **Why**: Full reasoning model, more capable than o1-mini/o3-mini
- **API Difference**: More reasoning tokens, different prompt engineering
- **Test Focus**: Complex multi-step reasoning, chain-of-thought

### 7. `gpt-4.1-mini` (OpenAI - 2025 Latest)
```rust
const MODEL_GPT41_MINI: &str = "gpt-4.1-mini";
```
- **Why**: Latest 2025 model, outperforms GPT-4o across the board
- **API Difference**: Superior coding, instruction following
- **Test Focus**: Coding benchmarks, instruction adherence

### 8. `claude-3-5-sonnet-20241022` (Anthropic)
```rust
const MODEL_SONNET_35: &str = "claude-3-5-sonnet-20241022";
```
- **Why**: Widely deployed, 49% SWE-bench performance
- **API Difference**: Updated version with improved performance
- **Migration**: Important for Claude 3.x to 4.x migration path

### 9. `models/gemini-2.5-pro` (Google)
```rust
const MODEL_GEMINI_25_PRO: &str = "models/gemini-2.5-pro";
```
- **Why**: Latest Gemini generation, evolution beyond 2.0
- **API Difference**: Latest generation improvements
- **Test Focus**: General purpose, quality improvements

### 10. `models/gemini-2.0-flash` (Google - GA)
```rust
const MODEL_GEMINI_20_FLASH: &str = "models/gemini-2.0-flash";
```
- **Why**: Generally available (not experimental), production-ready
- **API Difference**: 1M context, native tool use, simplified pricing
- **Production**: Recommended for production deployments

## Priority 3: Add These 5 Models for Complete Coverage

### 11. `o1-mini` (OpenAI)
```rust
const MODEL_O1_MINI: &str = "o1-mini";
```
- **Why**: Faster/cheaper reasoning model
- **Comparison**: Good benchmark against o3-mini

### 12. `text-embedding-3-large` (OpenAI)
```rust
const MODEL_EMBEDDING_LARGE: &str = "text-embedding-3-large";
```
- **Why**: Higher quality embeddings, configurable dimensions
- **Dimensions**: Up to 3072 (vs 1536 for small)

### 13. `claude-3-5-haiku-20241022` (Anthropic)
```rust
const MODEL_HAIKU_35: &str = "claude-3-5-haiku-20241022";
```
- **Why**: Matches Claude 3 Opus performance at Haiku speed
- **Capability**: Significant upgrade from previous Haiku

### 14. `models/gemini-2.0-flash-thinking-exp` (Google)
```rust
const MODEL_GEMINI_THINKING: &str = "models/gemini-2.0-flash-thinking-exp";
```
- **Why**: Reasoning before answering capability
- **Feature**: Similar to OpenAI's O-series reasoning

### 15. `models/gemini-2.0-flash-lite` (Google)
```rust
const MODEL_GEMINI_LITE: &str = "models/gemini-2.0-flash-lite";
```
- **Why**: Cost-optimized for large-scale text output
- **Use Case**: Bulk generation, cost optimization

## Implementation Plan

### Step 1: Update Test Files

**File: `ai-providers/tests/test_openai.rs`**
```rust
// Add constants
const MODEL_GPT4O: &str = "gpt-4o";
const MODEL_GPT4O_MINI: &str = "gpt-4o-mini";
const MODEL_GPT4_TURBO: &str = "gpt-4-turbo";
const MODEL_O1: &str = "o1";
const MODEL_GPT41_MINI: &str = "gpt-4.1-mini";
const MODEL_O1_MINI: &str = "o1-mini";
const MODEL_EMBEDDING_LARGE: &str = "text-embedding-3-large";

// Add test functions
#[tokio::test]
#[ignore]
async fn test_openai_gpt4o_streaming_chat() { ... }

#[tokio::test]
#[ignore]
async fn test_openai_gpt4o_mini_streaming_chat() { ... }

#[tokio::test]
#[ignore]
async fn test_openai_gpt4_turbo_streaming_chat() { ... }

#[tokio::test]
#[ignore]
async fn test_openai_o1_reasoning() { ... }

#[tokio::test]
#[ignore]
async fn test_openai_gpt41_mini_streaming() { ... }

#[tokio::test]
#[ignore]
async fn test_openai_embedding_large() { ... }
```

**File: `ai-providers/tests/test_anthropic.rs`**
```rust
// Add constants
const MODEL_OPUS_41: &str = "claude-opus-4-1-20250805";
const MODEL_SONNET_35: &str = "claude-3-5-sonnet-20241022";
const MODEL_HAIKU_35: &str = "claude-3-5-haiku-20241022";

// Add test functions
#[tokio::test]
#[ignore]
async fn test_anthropic_opus_41_streaming() { ... }

#[tokio::test]
#[ignore]
async fn test_anthropic_sonnet_35_streaming() { ... }

#[tokio::test]
#[ignore]
async fn test_anthropic_haiku_35_streaming() { ... }
```

**File: `ai-providers/tests/test_gemini.rs`**
```rust
// Add constants
const MODEL_GEMINI_20_FLASH: &str = "models/gemini-2.0-flash";
const MODEL_GEMINI_20_PRO: &str = "models/gemini-2.0-pro-exp";
const MODEL_GEMINI_25_PRO: &str = "models/gemini-2.5-pro";
const MODEL_GEMINI_THINKING: &str = "models/gemini-2.0-flash-thinking-exp";
const MODEL_GEMINI_LITE: &str = "models/gemini-2.0-flash-lite";

// Add test functions
#[tokio::test]
#[ignore]
async fn test_gemini_20_flash_streaming() { ... }

#[tokio::test]
#[ignore]
async fn test_gemini_20_pro_large_context() { ... }

#[tokio::test]
#[ignore]
async fn test_gemini_25_pro_streaming() { ... }

#[tokio::test]
#[ignore]
async fn test_gemini_thinking_reasoning() { ... }

#[tokio::test]
#[ignore]
async fn test_gemini_lite_cost_optimization() { ... }
```

### Step 2: Test Pattern Template

For consistency, use this template for new tests:

```rust
#[tokio::test]
#[ignore]
async fn test_provider_model_feature() {
    use futures_util::StreamExt;

    let api_key = get_api_key();
    let provider = Provider::new("provider", &api_key, BASE_URL)
        .expect("Failed to create provider");

    let request = ChatRequest {
        model: MODEL_NAME.to_string(),
        messages: vec![ChatMessage::user("Count from 1 to 5, one number per line.")],
        temperature: Some(0.1),
        max_tokens: Some(100),
        ..Default::default()
    };

    let mut stream = provider
        .chat_stream(request)
        .await
        .expect("Stream chat request failed");

    let mut full_content = String::new();
    let mut chunk_count = 0;

    while let Some(result) = stream.next().await {
        match result {
            Ok(chunk) => {
                for delta in &chunk.content {
                    match delta {
                        ContentBlockDelta::TextDelta { delta, .. } => {
                            full_content.push_str(delta);
                            print!("{}", delta);
                        }
                        _ => {}
                    }
                }
                chunk_count += 1;
            }
            Err(e) => panic!("Stream error: {:?}", e),
        }
    }

    println!("\n\nReceived {} chunks", chunk_count);
    println!("Full content: {}", full_content);

    assert!(chunk_count > 0);
    assert!(!full_content.is_empty());
}
```

### Step 3: Environment Variables

Update `.env.test.example`:
```bash
# All tests use existing keys
OPENAI_API_KEY=your_openai_key
ANTHROPIC_API_KEY=your_anthropic_key
GEMINI_API_KEY=your_gemini_key
GROQ_API_KEY=your_groq_key  # Already present
```

## Special Testing Considerations

### Models Requiring Different Configurations:

1. **O-series (o1, o1-mini, o3-mini)**:
   - ❌ Don't use few-shot prompting (degrades performance)
   - ✅ Use focused questions without extraneous text
   - ✅ Test `reasoning_tokens` in usage metadata

2. **GPT-5**:
   - ✅ Already has non-streaming workaround test
   - ✅ Test for `usage` metadata presence

3. **Claude with Extended Thinking**:
   - ✅ Already tested with Sonnet 4.5
   - ✅ Test `thinking` configuration and budget

4. **Gemini 2.0 Pro (2M context)**:
   - ⚠️ Test large context handling
   - ⚠️ May need special rate limiting

## Testing Checklist

For each new model, test:
- [ ] ✅ Streaming chat completions
- [ ] ✅ Non-streaming (if different behavior)
- [ ] ✅ Temperature and parameters
- [ ] ✅ Token usage metadata
- [ ] ✅ Error handling
- [ ] ✅ (If applicable) Special features (thinking, audio, vision)
- [ ] ✅ (If embeddings) Embedding dimensions and quality

## Estimated Effort

- **Priority 1** (5 models): ~2-3 hours
- **Priority 2** (5 models): ~2 hours
- **Priority 3** (5 models): ~2 hours
- **Total**: ~6-7 hours for complete coverage

## Cost Considerations

Running full test suite with real API keys:
- **Development**: Use `.env.test` with test keys
- **CI/CD**: Consider mocking or sampling tests
- **Recommended**: Test Priority 1 in CI, Priority 2-3 manually

## Next Steps

1. ✅ Review this document
2. ⏭️ Add Priority 1 models (5 tests)
3. ⏭️ Add Priority 2 models (5 tests)
4. ⏭️ Add Priority 3 models (5 tests)
5. ⏭️ Update documentation
6. ⏭️ Run full test suite with real API keys
7. ⏭️ Document any model-specific quirks discovered

---

**Document Created**: 2025-01-14
**Research Date**: 2025-01-14
**Status**: Ready for implementation
