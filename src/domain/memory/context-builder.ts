/**
 * Context builder for recovery
 *
 * Builds a context prompt from session memory to inject
 * when recovering a lost session.
 */

import type { SessionMemory } from './entity.js';

/** Context builder options */
export interface ContextBuilderOptions {
  /** Maximum length of context (chars) */
  maxLength?: number;
  /** Include completed tasks */
  includeCompletedTasks?: boolean;
  /** Maximum number of completed tasks to include */
  maxCompletedTasks?: number;
}

const DEFAULT_OPTIONS: Required<ContextBuilderOptions> = {
  maxLength: 4000,
  includeCompletedTasks: true,
  maxCompletedTasks: 5,
};

/**
 * Build context string from session memory
 */
export function buildContext(
  memory: SessionMemory,
  options: ContextBuilderOptions = {}
): string {
  const opts = { ...DEFAULT_OPTIONS, ...options };

  const sections: string[] = [];

  // Header
  sections.push('## 上次会话上下文\n');

  // Last progress
  if (memory.lastProgress) {
    sections.push(`**最后进展**: ${memory.lastProgress}\n`);
  }

  // Pending tasks
  if (memory.pendingTasks.length > 0) {
    sections.push('**待完成任务**:');
    for (const task of memory.pendingTasks) {
      sections.push(`- [ ] ${task}`);
    }
    sections.push('');
  }

  // Completed tasks (limited)
  if (opts.includeCompletedTasks && memory.completedTasks.length > 0) {
    const recentCompleted = memory.completedTasks.slice(-opts.maxCompletedTasks);
    sections.push('**最近完成**:');
    for (const task of recentCompleted) {
      sections.push(`- [x] ${task}`);
    }
    sections.push('');
  }

  // Known issues
  if (memory.knownIssues.length > 0) {
    sections.push('**已知问题**:');
    for (const issue of memory.knownIssues) {
      sections.push(`- ${issue}`);
    }
    sections.push('');
  }

  // Decisions
  if (memory.decisions.length > 0) {
    sections.push('**重要决策**:');
    for (const decision of memory.decisions) {
      sections.push(`- ${decision}`);
    }
    sections.push('');
  }

  // Handoff notes
  if (memory.handoffNotes) {
    sections.push('**交接笔记**:');
    sections.push(memory.handoffNotes);
    sections.push('');
  }

  // Priority items
  if (memory.handoffPriority.length > 0) {
    sections.push('**优先处理**:');
    for (const item of memory.handoffPriority) {
      sections.push(`1. ${item}`);
    }
    sections.push('');
  }

  // Footer
  sections.push('---');
  sections.push(`这是第 ${memory.iterationCount + 1} 次迭代。请继续从上次的进度开始工作。`);
  sections.push('完成后，请更新进度并留下笔记给下一次迭代。');

  let context = sections.join('\n');

  // Truncate if too long
  if (context.length > opts.maxLength) {
    context = context.slice(0, opts.maxLength - 3) + '...';
  }

  return context;
}

/**
 * Build a minimal context for quick recovery
 */
export function buildMinimalContext(memory: SessionMemory): string {
  const parts: string[] = [];

  if (memory.lastProgress) {
    parts.push(`上次进展: ${memory.lastProgress}`);
  }

  if (memory.pendingTasks.length > 0) {
    parts.push(`待办: ${memory.pendingTasks.slice(0, 3).join(', ')}`);
  }

  if (memory.handoffPriority.length > 0) {
    parts.push(`优先: ${memory.handoffPriority[0]}`);
  }

  return parts.join(' | ') || '无上下文';
}

/**
 * Check if memory has meaningful content
 */
export function hasContent(memory: SessionMemory): boolean {
  return !!(
    memory.lastProgress ||
    memory.pendingTasks.length > 0 ||
    memory.completedTasks.length > 0 ||
    memory.knownIssues.length > 0 ||
    memory.decisions.length > 0 ||
    memory.handoffNotes ||
    memory.notes
  );
}
