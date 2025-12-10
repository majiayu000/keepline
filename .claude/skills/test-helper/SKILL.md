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

## Testing Best Practices (NO MOCKS)

This project follows integration testing best practices. **NO MOCK TESTS.**

### Core Principles

1. **Test features, not implementation details**
   - Organize tests by feature/behavior, not by function name
   - Use section headers with `// ============` to separate features

2. **Use real objects, no mocks**
   - Use actual function calls with real inputs
   - Use real system values (e.g., `process.pid`, `process.cwd()`)
   - Create helper functions that build real data structures

3. **Test observable behavior**
   - Test what the function returns or its side effects
   - Don't test internal implementation

### Test Structure

```typescript
/**
 * Integration tests for [Module Name]
 *
 * These tests verify [behavior] using real data.
 * Following best practices:
 * - Test features, not implementation details
 * - Use real objects, no mocks
 * - Test observable behavior
 */

import { describe, test, expect } from 'bun:test';
import { actualFunction } from '../path/to/module.js';

// ============================================================================
// Feature: [Feature Name]
// ============================================================================

describe('[Feature Name]', () => {
  describe('when [condition]', () => {
    test('[expected behavior]', () => {
      // Arrange: set up real data
      // Act: call real function
      // Assert: verify observable result
    });
  });
});
```

### Real Data Helpers

Create helper functions that construct real data structures:

```typescript
/** Creates a valid ProcessInfo with realistic values */
function processInfo(overrides: Partial<ProcessInfo> = {}): ProcessInfo {
  return {
    pid: process.pid, // Use real current process PID
    cwd: process.cwd(), // Use real current directory
    cpu: 0.5,
    memory: 50.0,
    startTime: new Date(),
    args: ['--version'],
    ...overrides,
  };
}

/** Creates a Date representing seconds ago from now */
function secondsAgo(seconds: number): Date {
  return new Date(Date.now() - seconds * 1000);
}
```

### What NOT to Do

```typescript
// ❌ DON'T: Mock functions
jest.mock('../module', () => ({ fn: jest.fn() }));

// ❌ DON'T: Use fake data when real data works
const fakeProcess = { pid: 12345, ... };

// ❌ DON'T: Test implementation details
test('internal helper function works', () => { ... });

// ❌ DON'T: Organize by function name
describe('functionName', () => { ... });
```

### What TO Do

```typescript
// ✅ DO: Use real imports
import { actualFunction } from '../module.js';

// ✅ DO: Use real system values
const realPid = process.pid;

// ✅ DO: Test observable behavior
test('status is "running" when CPU > threshold', () => {
  const result = detectStatus(activeProcess);
  expect(result).toBe('running');
});

// ✅ DO: Organize by feature
describe('Session Status Detection', () => { ... });
```

## Existing Test Files
- `detector.test.ts` - Session status detection logic
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
- Use `test.each` for parameterized tests with multiple inputs
