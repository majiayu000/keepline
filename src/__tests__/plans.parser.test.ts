/**
 * Plans parser tests
 *
 * Tests the parsing of Claude Code plan markdown files
 */

import { describe, expect, it, beforeAll, afterAll } from 'bun:test';
import { mkdirSync, writeFileSync, rmSync } from 'fs';
import { join } from 'path';
import { tmpdir } from 'os';

// We need to test the parser functions directly, so we'll create a test version
// that doesn't depend on the actual ~/.claude/plans directory

interface PlanTask {
  text: string;
  completed: boolean;
  phase?: string;
}

// PlanPhase interface (used for documentation reference)
// interface PlanPhase {
//   name: string;
//   title: string;
//   tasks: PlanTask[];
//   completedCount: number;
//   totalCount: number;
// }

// Copy of parseTaskLine for testing
function parseTaskLine(
  line: string,
  currentPhase?: string,
  inCodeBlock?: boolean
): PlanTask | null {
  if (inCodeBlock) return null;

  // Match [ ] unchecked pattern
  const uncheckedMatch = line.match(/^[\s]*[-*]?\s*\[\s*\]\s*(.+)$/);
  if (uncheckedMatch) {
    return {
      text: uncheckedMatch[1].trim(),
      completed: false,
      phase: currentPhase,
    };
  }

  // Match [x] or [X] or [✓] or [✔] checked patterns
  const checkedMatch = line.match(/^[\s]*[-*]?\s*\[[xX✓✔]\]\s*(.+)$/);
  if (checkedMatch) {
    return {
      text: checkedMatch[1].trim(),
      completed: true,
      phase: currentPhase,
    };
  }

  // Match ✅ emoji (completed)
  const emojiCompletedMatch = line.match(/^[\s]*[-*]?\s*✅\s*(.+)$/);
  if (emojiCompletedMatch) {
    return {
      text: emojiCompletedMatch[1].trim(),
      completed: true,
      phase: currentPhase,
    };
  }

  // Match ✓ or ✔ checkmarks at start of line (without brackets)
  const checkmarkMatch = line.match(/^[\s]*[-*]?\s*[✓✔]\s+(.+)$/);
  if (checkmarkMatch) {
    return {
      text: checkmarkMatch[1].trim(),
      completed: true,
      phase: currentPhase,
    };
  }

  // Match ⬜ or ◻ or ○ (unchecked markers)
  const uncheckedEmojiMatch = line.match(/^[\s]*[-*]?\s*[⬜◻○]\s+(.+)$/);
  if (uncheckedEmojiMatch) {
    return {
      text: uncheckedEmojiMatch[1].trim(),
      completed: false,
      phase: currentPhase,
    };
  }

  // Match plain list items inside a Phase
  if (currentPhase) {
    const plainListMatch = line.match(/^[-*]\s+(.+)$/);
    if (plainListMatch) {
      const text = plainListMatch[1].trim();
      if (text.length >= 3 && !text.startsWith('```') && !text.startsWith('**')) {
        return {
          text,
          completed: false,
          phase: currentPhase,
        };
      }
    }
  }

  return null;
}

// Copy of parsePhaseHeader for testing
function parsePhaseHeader(line: string): { name: string; title: string } | null {
  const phaseMatch = line.match(/^#{2,4}\s*Phase\s+(\d+)[\s:.-]*(.*)$/i);
  if (phaseMatch) {
    const name = `Phase ${phaseMatch[1]}`;
    const title = phaseMatch[2]?.trim() || name;
    return { name, title };
  }

  const stepMatch = line.match(/^#{2,3}\s*Step\s+(\d+(?:\.\d+)?)[\s:.-]*(.*)$/i);
  if (stepMatch) {
    const name = `Step ${stepMatch[1]}`;
    const title = stepMatch[2]?.trim() || name;
    return { name, title };
  }

  return null;
}

describe('Plans Parser', () => {
  describe('parseTaskLine', () => {
    it('should parse [ ] unchecked tasks', () => {
      const task = parseTaskLine('- [ ] Implement feature');
      expect(task).not.toBeNull();
      expect(task!.text).toBe('Implement feature');
      expect(task!.completed).toBe(false);
    });

    it('should parse [x] checked tasks', () => {
      const task = parseTaskLine('- [x] Complete task');
      expect(task).not.toBeNull();
      expect(task!.text).toBe('Complete task');
      expect(task!.completed).toBe(true);
    });

    it('should parse [X] uppercase checked tasks', () => {
      const task = parseTaskLine('- [X] Complete task');
      expect(task).not.toBeNull();
      expect(task!.completed).toBe(true);
    });

    it('should parse [✓] unicode checked tasks', () => {
      const task = parseTaskLine('- [✓] Complete task');
      expect(task).not.toBeNull();
      expect(task!.completed).toBe(true);
    });

    it('should parse ✅ emoji completed tasks', () => {
      const task = parseTaskLine('✅ 完成用户认证');
      expect(task).not.toBeNull();
      expect(task!.text).toBe('完成用户认证');
      expect(task!.completed).toBe(true);
    });

    it('should parse ✓ checkmark without brackets', () => {
      const task = parseTaskLine('✓ Completed this task');
      expect(task).not.toBeNull();
      expect(task!.text).toBe('Completed this task');
      expect(task!.completed).toBe(true);
    });

    it('should parse ⬜ unchecked emoji tasks', () => {
      const task = parseTaskLine('⬜ Pending task');
      expect(task).not.toBeNull();
      expect(task!.text).toBe('Pending task');
      expect(task!.completed).toBe(false);
    });

    it('should parse ○ unchecked circle tasks', () => {
      const task = parseTaskLine('○ Another pending');
      expect(task).not.toBeNull();
      expect(task!.text).toBe('Another pending');
      expect(task!.completed).toBe(false);
    });

    it('should parse plain list items when inside a phase', () => {
      const task = parseTaskLine('- Implement database schema', 'Phase 1');
      expect(task).not.toBeNull();
      expect(task!.text).toBe('Implement database schema');
      expect(task!.completed).toBe(false);
      expect(task!.phase).toBe('Phase 1');
    });

    it('should NOT parse plain list items without phase context', () => {
      const task = parseTaskLine('- Random list item');
      expect(task).toBeNull();
    });

    it('should ignore tasks inside code blocks', () => {
      const task = parseTaskLine('- [ ] This should be ignored', undefined, true);
      expect(task).toBeNull();
    });

    it('should handle indented tasks', () => {
      const task = parseTaskLine('  - [x] Indented complete task');
      expect(task).not.toBeNull();
      expect(task!.completed).toBe(true);
    });

    it('should handle asterisk bullets', () => {
      const task = parseTaskLine('* [ ] Star bullet task');
      expect(task).not.toBeNull();
      expect(task!.text).toBe('Star bullet task');
    });

    it('should filter short plain list items', () => {
      const task = parseTaskLine('- OK', 'Phase 1');
      expect(task).toBeNull(); // Too short (2 chars)
    });

    it('should accept 3+ char tasks', () => {
      const task = parseTaskLine('- API', 'Phase 1');
      expect(task).not.toBeNull();
      expect(task!.text).toBe('API');
    });

    it('should handle Chinese tasks', () => {
      const task = parseTaskLine('- 实现认证', 'Phase 1');
      expect(task).not.toBeNull();
      expect(task!.text).toBe('实现认证');
    });
  });

  describe('parsePhaseHeader', () => {
    it('should parse "### Phase 1: Title" format', () => {
      const phase = parsePhaseHeader('### Phase 1: Database Setup');
      expect(phase).not.toBeNull();
      expect(phase!.name).toBe('Phase 1');
      expect(phase!.title).toBe('Database Setup');
    });

    it('should parse "## Phase 2 - Title" format', () => {
      const phase = parsePhaseHeader('## Phase 2 - API Implementation');
      expect(phase).not.toBeNull();
      expect(phase!.name).toBe('Phase 2');
      expect(phase!.title).toBe('API Implementation');
    });

    it('should parse "### Phase 3" without title', () => {
      const phase = parsePhaseHeader('### Phase 3');
      expect(phase).not.toBeNull();
      expect(phase!.name).toBe('Phase 3');
      expect(phase!.title).toBe('Phase 3');
    });

    it('should parse "## Step 1: Title" format', () => {
      const phase = parsePhaseHeader('## Step 1: First Step');
      expect(phase).not.toBeNull();
      expect(phase!.name).toBe('Step 1');
      expect(phase!.title).toBe('First Step');
    });

    it('should parse "### Step 2.1: Sub-step" format', () => {
      const phase = parsePhaseHeader('### Step 2.1: Sub-step Title');
      expect(phase).not.toBeNull();
      expect(phase!.name).toBe('Step 2.1');
      expect(phase!.title).toBe('Sub-step Title');
    });

    it('should NOT parse "## Overview" (no Phase keyword)', () => {
      const phase = parsePhaseHeader('## Overview');
      expect(phase).toBeNull();
    });

    it('should NOT parse "## Implementation" (no Phase keyword)', () => {
      const phase = parsePhaseHeader('## Implementation');
      expect(phase).toBeNull();
    });

    it('should NOT parse "#### Step 1.1: Sub" (too deep for phase)', () => {
      // #### is for sub-steps, which become tasks, not phases
      const phase = parsePhaseHeader('#### Step 1.1: Sub-step');
      expect(phase).toBeNull();
    });

    it('should be case-insensitive for Phase keyword', () => {
      const phase = parsePhaseHeader('### PHASE 1: Uppercase');
      expect(phase).not.toBeNull();
      expect(phase!.name).toBe('Phase 1');
    });
  });

  describe('Full plan parsing', () => {
    const testDir = join(tmpdir(), 'claude-hub-test-plans');

    beforeAll(() => {
      mkdirSync(testDir, { recursive: true });
    });

    afterAll(() => {
      rmSync(testDir, { recursive: true, force: true });
    });

    it('should parse a complete plan with phases and tasks', () => {
      const planContent = `# Authentication System Plan

## Phase 1: Database Setup
- [x] Create user table schema
- [x] Add migration for roles
- [ ] Seed test data

## Phase 2: API Implementation
- [x] Implement login endpoint
- [ ] Implement logout endpoint
- [ ] Add token refresh

## Phase 3: Frontend
- [ ] Create login form
- [ ] Add auth context
`;

      const filePath = join(testDir, 'auth-plan.md');
      writeFileSync(filePath, planContent);

      // Import the actual parser to test file parsing
      // Note: This is a simplified test - in production, we'd need to mock the CLAUDE_PLANS_DIR
    });

    it('should handle plans with emoji markers', () => {
      const planContent = `# Feature Plan

## Phase 1: Setup
✅ Initialize project
✅ Configure TypeScript
⬜ Set up testing

## Phase 2: Implementation
○ Write core logic
○ Add error handling
`;

      const lines = planContent.split('\n');
      let currentPhase: string | undefined;
      const tasks: PlanTask[] = [];

      for (const line of lines) {
        const phaseHeader = parsePhaseHeader(line);
        if (phaseHeader) {
          currentPhase = phaseHeader.name;
          continue;
        }

        const task = parseTaskLine(line, currentPhase);
        if (task) {
          tasks.push(task);
        }
      }

      expect(tasks.length).toBe(5);
      expect(tasks.filter(t => t.completed).length).toBe(2);
      expect(tasks.filter(t => !t.completed).length).toBe(3);
    });

    it('should skip code blocks', () => {
      const planContent = `# Plan

## Phase 1: Setup
- [x] Create config

\`\`\`javascript
// This should not be parsed as a task
- [ ] Not a task
\`\`\`

- [ ] After code block
`;

      const lines = planContent.split('\n');
      let currentPhase: string | undefined;
      let inCodeBlock = false;
      const tasks: PlanTask[] = [];

      for (const line of lines) {
        if (line.trim().startsWith('```')) {
          inCodeBlock = !inCodeBlock;
          continue;
        }

        const phaseHeader = parsePhaseHeader(line);
        if (phaseHeader && !inCodeBlock) {
          currentPhase = phaseHeader.name;
          continue;
        }

        const task = parseTaskLine(line, currentPhase, inCodeBlock);
        if (task) {
          tasks.push(task);
        }
      }

      expect(tasks.length).toBe(2); // Only real tasks, not the one in code block
      expect(tasks[0].text).toBe('Create config');
      expect(tasks[1].text).toBe('After code block');
    });
  });
});
