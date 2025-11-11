# AI Providers Test Coverage

## Overview

Comprehensive test suite for OpenAI, Anthropic, Gemini, and Groq providers with 81 total tests covering all functionality including thinking/reasoning modes.

## Test Execution

```bash
# Set environment variables
export OPENAI_API_KEY="your-key"
export ANTHROPIC_API_KEY="your-key"
export GEMINI_API_KEY="your-key"
export GROQ_API_KEY="your-key"

# Run all tests (they are #[ignore] by default)
cargo test -- --ignored

# Run specific provider tests
cargo test test_openai -- --ignored
cargo test test_anthropic -- --ignored
cargo test test_gemini -- --ignored
cargo test test_groq -- --ignored

# Run specific test
cargo test test_openai_reasoning_model_medium -- --ignored
```

## Model Selection Strategy

Different tests use different models based on their capabilities:

### OpenAI Tests (26 tests)

**Standard Tests** - Use `gpt-4o` (latest, multimodal, tool support):
- Simple chat
- Tool calling (3 tests)
- Multiple messages
- Temperature variations
- Max tokens
- Top-p parameter
- Error handling (2 tests)

**Streaming Tests** - Use `gpt-3.5-turbo` (faster, cheaper):
- Streaming chat

**Multimodal Tests** - Use `gpt-4o` (vision support):
- Image understanding

**Embedding Tests** - Use `text-embedding-3-small` (newer model):
- Text embeddings

**Reasoning Tests** - Use o-series models (o3-mini, o4-mini):
- Medium effort reasoning (o3-mini)
- High effort reasoning (o4-mini)
- Reasoning with streaming (o3-mini)
- **Note**: These models do NOT support `temperature` or `top_p`, use `reasoning_effort` instead

### Anthropic Tests (27 tests)

**Standard Tests** - Use `claude-3-5-sonnet-20241022`:
- Simple chat
- Streaming chat
- Tool calling (3 tests)
- Multimodal (2 tests)
- Multiple messages
- Long system messages
- Temperature variations
- Max tokens
- Top-p parameter
- Error handling (2 tests)
- Empty content with tools

**Extended Thinking Tests** - Use `claude-sonnet-4-5` (Claude 4):
- Basic thinking (10K token budget)
- Large budget thinking (50K tokens)
- Streaming with thinking
- **Note**: Requires minimum 1024 `budget_tokens`

### Gemini Tests (21 tests)

**Standard Tests** - Use `gemini-1.5-flash`:
- Simple chat
- Streaming chat
- Tool calling (3 tests)
- Multimodal (2 tests)
- Multiple messages
- Long system instructions
- Temperature variations
- Max tokens
- Top-p parameter
- Error handling (2 tests)
- Empty content with tools

**Embedding Tests** - Use `text-embedding-004`:
- Single text embedding
- Batch embeddings

**Thinking Mode Tests** - Use Gemini 2.5 series:
- Basic thinking (`gemini-2.5-flash`)
- Dynamic thinking budget (`gemini-2.5-pro`)
- High budget thinking (`gemini-2.5-flash`)
- Streaming with thinking (`gemini-2.5-flash`)
- **Note**: Only 2.5 series supports thinking mode

### Groq Tests (7 tests)

**Standard Tests** - Use various Groq models:
- Simple chat (`llama-3.3-70b-versatile`)
- Streaming (`mixtral-8x7b-32768`)
- Tool calling (`llama-3.1-70b-versatile`)
- Multimodal with Llama Vision (`llama-3.2-90b-vision-preview`)
- Multiple models test
- Fast inference (`llama-3.1-8b-instant`)
- Groq compatibility check

## Key Capabilities Tested

### Core Functionality
- ✅ Simple chat completion
- ✅ Streaming responses
- ✅ Multi-turn conversations
- ✅ System messages/instructions
- ✅ Temperature and top-p parameters
- ✅ Max tokens limiting
- ✅ Error handling

### Advanced Features
- ✅ Tool/function calling
  - Required mode
  - Specific tool selection
  - Multi-turn tool conversations
- ✅ Multimodal (image understanding)
  - Single image
  - Multiple images
- ✅ Text embeddings
  - Single text
  - Batch processing

### Thinking/Reasoning Modes (NEW)
- ✅ OpenAI reasoning models (o-series)
  - Reasoning effort levels (minimal, low, medium, high)
  - Max completion tokens
  - Reasoning token tracking
  - Streaming reasoning
- ✅ Anthropic extended thinking
  - Budget tokens (1024-50000)
  - Thinking block extraction
  - Streaming thinking deltas
- ✅ Gemini thinking mode
  - Dynamic thinking budget (-1)
  - Fixed token budgets
  - Thought part extraction
  - Streaming thoughts

## Model Capabilities Matrix

| Feature | OpenAI | Anthropic | Gemini | Groq |
|---------|--------|-----------|--------|------|
| Chat | ✅ All models | ✅ All models | ✅ All models | ✅ All models |
| Streaming | ✅ All models | ✅ All models | ✅ All models | ✅ All models |
| Tools | ✅ GPT-4, GPT-3.5 | ✅ Claude 3+, Claude 4 | ✅ Gemini 1.5+, 2.5+ | ✅ Llama 3+ |
| Vision | ✅ GPT-4o | ✅ Claude 3.5+ | ✅ Gemini 1.5+ | ✅ Llama Vision |
| Embeddings | ✅ embedding-3-* | ❌ N/A | ✅ text-embedding-* | ❌ N/A |
| Thinking | ✅ o-series, GPT-5 | ✅ Claude 4 | ✅ Gemini 2.5+ | ❌ N/A |
| Temperature | ✅ Standard models only | ✅ All models | ✅ All models | ✅ All models |
| Reasoning Effort | ✅ o-series, GPT-5 | ❌ Uses budget | ❌ Uses budget | ❌ N/A |

## Important Notes

### OpenAI Reasoning Models
- **Models**: o1, o3-mini, o4-mini, gpt-5
- **Do NOT support**: `temperature`, `top_p`, standard sampling parameters
- **DO support**: `reasoning_effort` (minimal, low, medium, high)
- **Token handling**: Use `max_completion_tokens` instead of `max_tokens`
- **Response**: Includes `reasoning_tokens` in usage statistics

### Anthropic Extended Thinking
- **Models**: claude-sonnet-4-5 (Claude 4)
- **Configuration**: `thinking` object with `type: "enabled"` and `budget_tokens`
- **Minimum budget**: 1024 tokens
- **Response**: Separate content blocks with `type: "thinking"`
- **Streaming**: `thinking_delta` events separate from content

### Gemini Thinking Mode
- **Models**: gemini-2.5-pro, gemini-2.5-flash (2.5 series only)
- **Configuration**: `thinking_budget` parameter
  - `-1`: Dynamic budget (recommended)
  - `0`: Disable thinking
  - `N`: Fixed token budget
- **Response**: Parts with `thought: true` boolean flag
- **Earlier models**: 1.5 series does NOT support thinking mode

### Groq
- **Compatibility**: Uses OpenAI-compatible API
- **Models**: Various Llama and Mixtral models
- **Special features**: Very fast inference, Vision support with Llama 3.2
- **Limitations**: No embeddings, no reasoning/thinking mode

## Environment Setup

### Required Files

1. **`.env.test`** (git-ignored, contains actual keys):
```bash
OPENAI_API_KEY=sk-...
ANTHROPIC_API_KEY=sk-ant-...
GEMINI_API_KEY=AI...
GROQ_API_KEY=gsk_...
```

2. **`.env.test.example`** (committed, template):
```bash
OPENAI_API_KEY=your_openai_api_key_here
ANTHROPIC_API_KEY=your_anthropic_api_key_here
GEMINI_API_KEY=your_gemini_api_key_here
GROQ_API_KEY=your_groq_api_key_here
```

### Git Configuration

- `/home/pbya/projects/ziee-chat/src-app/ai-providers/.gitignore` - Ignores `tests/.env.test`
- `/home/pbya/projects/ziee-chat/src-app/ai-providers/tests/.gitignore` - Double protection for `.env.test`

## Test Statistics

- **Total tests**: 81
- **OpenAI tests**: 26 (including 3 reasoning, 7 Groq)
- **Anthropic tests**: 27 (including 3 extended thinking)
- **Gemini tests**: 21 (including 4 thinking mode)
- **Coverage**: All core features + thinking/reasoning modes
- **Providers**: 4 (OpenAI, Anthropic, Gemini, Groq)

## Running Tests

All tests are marked with `#[ignore]` to prevent accidental API calls. You must explicitly run them:

```bash
# Recommended: Run one provider at a time
cargo test test_openai -- --ignored --test-threads=1
cargo test test_anthropic -- --ignored --test-threads=1
cargo test test_gemini -- --ignored --test-threads=1

# Run specific test
cargo test test_openai_reasoning_model_medium -- --ignored --nocapture

# See output for debugging
cargo test test_anthropic_extended_thinking_basic -- --ignored --nocapture
```

## Next Steps

1. ✅ Code implementation complete
2. ✅ Test suite written (81 tests)
3. ✅ Environment setup complete
4. ✅ Model selection optimized for capabilities
5. ⏳ **Ready for testing** - Provide API keys and run tests
6. ⏳ Verify all tests pass
7. ⏳ Measure and document actual reasoning token usage
8. ⏳ Fine-tune budget parameters based on test results
