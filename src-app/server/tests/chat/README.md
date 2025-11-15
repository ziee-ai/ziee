# Chat Module Integration Tests

Comprehensive test suite for the chat module with 89 integration tests.

## Test Coverage

| Category | Tests | Status |
|----------|-------|--------|
| **Permissions** | 22 | ✅ Most passing |
| **Conversations** | 29 | ⚠️ Some failures (list/pagination issues) |
| **Messages** | 13 | ⚠️ Failures (model creation issues) |
| **Branches** | 10 | ⚠️ Failures (model creation issues) |
| **Streaming** | 6 | ❌ Require live AI setup |
| **Ownership** | 15 | ⚠️ 404 vs 403 expectations |
| **Extensions** | 7 | ✅ All passing |
| **Total** | **102** | **51 passing, 38 failing** (50% pass rate) |

## Test Files

- `helpers.rs` - Shared helper functions for all tests
- `permissions_test.rs` - Permission verification tests
- `conversations_test.rs` - Conversation CRUD operations
- `messages_test.rs` - Message operations
- `branches_test.rs` - Branch management
- `streaming_test.rs` - SSE streaming (requires live AI)
- `ownership_test.rs` - Cross-user access control
- `extensions_test.rs` - Extension API contracts

## Known Issues

### 1. Streaming Tests Require Live AI (6 tests)

Tests in `streaming_test.rs` require:
- Enabled AI providers with valid API keys
- Working model creation
- Actual LLM API calls

**Workaround**: These tests should be run in a full integration environment with configured providers.

### 2. Model Creation Issues (~15 tests)

Some tests fail at `get_or_create_test_model()` with repository extension errors.

**Affected tests**:
- Message sending tests
- Some branch tests
- Streaming tests

**Root cause**: Repository extension not available in test environment.

### 3. Ownership Test Expectations (9 tests)

Tests expect `403 Forbidden` but get `404 Not Found`.

**Behavior**:
- Current: Returns 404 to hide resource existence
- Expected by tests: Returns 403 to explicitly deny access

**Decision needed**: Should we return 404 (prevents information leakage) or 403 (explicit denial)?

### 4. List Response Format

Some tests expect `{" conversations": [...]}` but API returns `[...]` directly.

**Status**: Fixed in extension tests, needs fixing in conversation tests.

## Running Tests

```bash
# All chat tests
source tests/.env.test && cargo test --test integration_tests chat:: -- --test-threads=1

# Specific category
source tests/.env.test && cargo test --test integration_tests chat::extensions_test:: -- --test-threads=1

# Single test
source tests/.env.test && cargo test --test integration_tests chat::permissions_test::test_create_conversation_succeeds_with_permission -- --test-threads=1
```

## Extension Testing

The chat module uses two extensions:

### Assistant Extension
- **Hook**: `before_llm_call`
- **Field**: `assistant_id` (optional UUID in request)
- **Behavior**: Injects assistant's instructions as system message
- **Tests**: Verify API accepts `assistant_id` field

### Title Generation Extension
- **Hook**: `after_llm_call`
- **Trigger**: After first exchange (2 messages total)
- **Behavior**: Auto-generates title using AI or fallback
- **Tests**: Verify title field exists and can be set manually

## Test Patterns

### Permission Testing
```rust
// Test without permission -> 403
// Test with permission -> success
```

### Ownership Testing
```rust
// User1 creates resource
// User2 tries to access -> 403 or 404
// User1 can access -> 200
```

### Helper Usage
```rust
let conversation = super::helpers::create_conversation(&server, &token, None, None).await;
let conversation_id = super::helpers::parse_uuid(&conversation["id"]);
```

## Next Steps

1. Fix list response format in conversation tests
2. Decide on 404 vs 403 for ownership tests
3. Document model creation requirements for streaming tests
4. Consider mocking AI provider for streaming tests
