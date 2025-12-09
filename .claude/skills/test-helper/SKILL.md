---
name: test-helper
description: Generate and run tests for TypeScript/JavaScript code using Bun test runner. Use this skill when you need to write tests, check coverage, or debug test failures.
---

# Test Helper Skill

## Purpose
This skill helps with testing tasks in this Bun-based TypeScript project.

## Testing Framework
This project uses **Bun's built-in test runner** with Jest-compatible API.

## Test File Conventions
- Test files: `src/__tests__/*.test.ts`
- Import test utilities: `import { describe, test, expect, beforeEach, afterEach } from 'bun:test'`

## Commands

### Run all tests
```bash
bun test
```

### Run tests in watch mode
```bash
bun test --watch
```

### Run specific test file
```bash
bun test src/__tests__/validation.test.ts
```

## Test Structure Best Practices

1. **Group related tests with `describe`**
```typescript
describe('ModuleName', () => {
  describe('functionName', () => {
    test('should do something', () => {
      // ...
    });
  });
});
```

2. **Use `beforeEach`/`afterEach` for setup/teardown**
```typescript
beforeEach(() => {
  // Reset state before each test
});
```

3. **Use `test.each` for parameterized tests**
```typescript
test.each([1, 2, 3])('handles value %i', (value) => {
  expect(fn(value)).toBeDefined();
});
```

4. **Type-narrow validation results**
```typescript
const result = validate(input);
expect(result.valid).toBe(true);
if (result.valid) {
  expect(result.data.field).toBe(expected);
}
```

## Existing Test Files
- `detector.test.ts` - Process detector logic
- `validation.test.ts` - API input validation
- `config.test.ts` - Configuration management
- `scanner-cache.test.ts` - Process scanner caching

## Key Modules to Test
- `src/session/service.ts` - Session sync logic
- `src/claude/parser/jsonl.ts` - JSONL parsing
- `src/storage/session.repository.ts` - Database operations

## Tips
- Import actual functions, don't duplicate code in tests
- Test edge cases and error conditions
- Use descriptive test names that explain the scenario
- Keep tests independent - no shared mutable state
