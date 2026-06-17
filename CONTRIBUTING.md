# Contributing to Keepline

Thank you for your interest in contributing to Keepline! This document provides guidelines and information for contributors.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Making Changes](#making-changes)
- [Pull Request Process](#pull-request-process)
- [Coding Standards](#coding-standards)
- [Testing](#testing)
- [Documentation](#documentation)

## Code of Conduct

Be respectful and inclusive. We welcome contributors of all backgrounds and experience levels.

## Getting Started

### Prerequisites

- [Bun](https://bun.sh) v1.0+
- Node.js 18+ (for some tooling)
- Git

### Fork and Clone

```bash
# Fork the repository on GitHub, then:
git clone https://github.com/YOUR_USERNAME/keepline.git
cd keepline
```

## Development Setup

```bash
# Install dependencies
bun install

# Build the project
bun run build

# Start in development mode
bun run dev

# Start web server for testing
bun run start web
```

### Project Structure

```
src/
├── adapters/          # External integrations (Claude, process scanner)
├── application/       # Application services
├── cli/              # CLI commands (Commander.js)
├── db/               # Database migrations
├── domain/           # Domain entities and logic (DDD)
│   ├── session/      # Session management
│   ├── recovery/     # Recovery system
│   └── memory/       # Cross-session memory
├── infrastructure/   # Database, events, repositories
├── lib/              # Utilities (logger, config, paths)
├── services/         # Business services
├── ui/               # Terminal UI (Ink/React)
└── web/
    ├── api/          # REST API (Hono)
    └── client/       # Web frontend (React + Vite)
```

## Making Changes

### Branch Naming

- `feat/` - New features
- `fix/` - Bug fixes
- `docs/` - Documentation
- `refactor/` - Code refactoring
- `test/` - Adding tests

```bash
git checkout -b feat/cost-predictions
```

### Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add cost prediction feature
fix: resolve session recovery race condition
docs: update API documentation
refactor: simplify session aggregator logic
test: add memory repository tests
```

## Pull Request Process

1. **Create a branch** from `main`
2. **Make your changes** with clear commits
3. **Run tests** and type checking:
   ```bash
   bun run typecheck
   bun test
   ```
4. **Update documentation** if needed
5. **Submit PR** with a clear description

### PR Description Template

```markdown
## What does this PR do?

Brief description of changes.

## Why is this change needed?

Motivation and context.

## How to test?

Steps to verify the changes.

## Screenshots (if applicable)

Add screenshots for UI changes.
```

## Coding Standards

### TypeScript

- Use strict TypeScript (`"strict": true`)
- Prefer explicit types over `any`
- Use interfaces for object shapes
- Export types separately from implementations

### Code Style

- 2-space indentation
- Single quotes for strings
- Semicolons required
- Max line length: 100 characters

### File Organization

- One component/class per file
- Index files for module exports
- Keep files under 300 lines when possible

### Naming Conventions

| Type | Convention | Example |
|------|------------|---------|
| Files | kebab-case | `session-service.ts` |
| Classes | PascalCase | `SessionService` |
| Functions | camelCase | `getSessionById` |
| Constants | UPPER_SNAKE | `MAX_RETRY_COUNT` |
| Interfaces | PascalCase | `SessionData` |
| Types | PascalCase | `RecoveryMethod` |

## Testing

### Running Tests

```bash
# Run all tests
bun test

# Run specific test file
bun test src/__tests__/detector.test.ts

# Watch mode
bun test --watch
```

### Writing Tests

- Place tests in `src/__tests__/`
- Name test files `*.test.ts`
- Use descriptive test names
- Test edge cases and error conditions

```typescript
import { describe, it, expect } from 'bun:test';

describe('SessionDetector', () => {
  it('should detect running sessions', () => {
    // Arrange
    const processes = [{ pid: 123, cpu: 5.0 }];

    // Act
    const result = detectStatus(processes);

    // Assert
    expect(result).toBe('running');
  });
});
```

## Documentation

### Code Comments

- Use JSDoc for public APIs
- Explain "why" not "what"
- Keep comments up to date

```typescript
/**
 * Recovers a lost Claude Code session.
 *
 * @param sessionId - The session ID to recover
 * @param method - Recovery method (resume, continue, or new)
 * @returns Recovery result with success status
 *
 * @example
 * const result = await recoverSession('abc-123', 'resume');
 */
export async function recoverSession(
  sessionId: string,
  method: RecoveryMethod
): Promise<RecoveryResult> {
  // ...
}
```

### README Updates

Update relevant documentation when:
- Adding new features
- Changing CLI commands
- Modifying API endpoints
- Updating configuration options

## Need Help?

- Open an [issue](https://github.com/majiayu000/keepline/issues) for bugs
- Start a [discussion](https://github.com/majiayu000/keepline/discussions) for questions
- Check existing issues before creating new ones

---

Thank you for contributing!
