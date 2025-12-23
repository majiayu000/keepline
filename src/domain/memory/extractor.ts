/**
 * Memory extractor
 *
 * Extracts progress information from Claude's output
 * and hook events to automatically update memory.
 */

import type { MemoryUpsertData } from './entity.js';

/** Hook event data (simplified) */
export interface HookEventData {
  sessionId: string;
  toolName: string;
  toolInput: Record<string, unknown>;
  timestamp: Date;
}

/** Extraction patterns */
const PATTERNS = {
  // Task completion markers
  completed: [
    /✓\s*(.+)/g,
    /\[x\]\s*(.+)/gi,
    /completed[:\s]+(.+)/gi,
    /done[:\s]+(.+)/gi,
    /finished[:\s]+(.+)/gi,
  ],

  // Pending task markers
  pending: [
    /\[ \]\s*(.+)/g,
    /TODO[:\s]+(.+)/gi,
    /next[:\s]+(.+)/gi,
    /remaining[:\s]+(.+)/gi,
  ],

  // Issue markers
  issues: [
    /issue[:\s]+(.+)/gi,
    /problem[:\s]+(.+)/gi,
    /bug[:\s]+(.+)/gi,
    /error[:\s]+(.+)/gi,
    /failed[:\s]+(.+)/gi,
  ],

  // Decision markers
  decisions: [
    /decided[:\s]+(.+)/gi,
    /chose[:\s]+(.+)/gi,
    /decision[:\s]+(.+)/gi,
    /will use[:\s]+(.+)/gi,
  ],
};

/**
 * Extract memory updates from Claude's output text
 */
export function extractFromOutput(output: string): Partial<MemoryUpsertData> {
  const result: Partial<MemoryUpsertData> = {};

  const completedTasks: string[] = [];
  const pendingTasks: string[] = [];
  const knownIssues: string[] = [];
  const decisions: string[] = [];

  // Extract completed tasks
  for (const pattern of PATTERNS.completed) {
    let match;
    while ((match = pattern.exec(output)) !== null) {
      const task = match[1].trim();
      if (task && task.length > 3 && task.length < 200) {
        completedTasks.push(task);
      }
    }
  }

  // Extract pending tasks
  for (const pattern of PATTERNS.pending) {
    let match;
    while ((match = pattern.exec(output)) !== null) {
      const task = match[1].trim();
      if (task && task.length > 3 && task.length < 200) {
        pendingTasks.push(task);
      }
    }
  }

  // Extract issues
  for (const pattern of PATTERNS.issues) {
    let match;
    while ((match = pattern.exec(output)) !== null) {
      const issue = match[1].trim();
      if (issue && issue.length > 3 && issue.length < 200) {
        knownIssues.push(issue);
      }
    }
  }

  // Extract decisions
  for (const pattern of PATTERNS.decisions) {
    let match;
    while ((match = pattern.exec(output)) !== null) {
      const decision = match[1].trim();
      if (decision && decision.length > 3 && decision.length < 200) {
        decisions.push(decision);
      }
    }
  }

  // Only include non-empty arrays
  if (completedTasks.length > 0) {
    result.completedTasks = [...new Set(completedTasks)]; // Dedupe
  }
  if (pendingTasks.length > 0) {
    result.pendingTasks = [...new Set(pendingTasks)];
  }
  if (knownIssues.length > 0) {
    result.knownIssues = [...new Set(knownIssues)];
  }
  if (decisions.length > 0) {
    result.decisions = [...new Set(decisions)];
  }

  return result;
}

/**
 * Extract progress from hook event
 */
export function extractFromHookEvent(event: HookEventData): Partial<MemoryUpsertData> {
  const result: Partial<MemoryUpsertData> = {
    sessionId: event.sessionId,
  };

  // Build last progress from tool name and input
  const toolName = event.toolName;
  const input = event.toolInput;

  // Format progress based on tool type
  switch (toolName) {
    case 'Edit':
    case 'Write':
      if (input.file_path) {
        result.lastProgress = `编辑文件: ${shortenPath(input.file_path as string)}`;
      }
      break;

    case 'Read':
      if (input.file_path) {
        result.lastProgress = `读取文件: ${shortenPath(input.file_path as string)}`;
      }
      break;

    case 'Bash':
      if (input.command) {
        const cmd = (input.command as string).slice(0, 50);
        result.lastProgress = `执行命令: ${cmd}`;
      }
      break;

    case 'Grep':
    case 'Glob':
      if (input.pattern) {
        result.lastProgress = `搜索: ${input.pattern}`;
      }
      break;

    case 'Task':
      if (input.description) {
        result.lastProgress = `子任务: ${input.description}`;
      }
      break;

    default:
      result.lastProgress = `使用工具: ${toolName}`;
  }

  return result;
}

/**
 * Shorten file path for display
 */
function shortenPath(path: string): string {
  const parts = path.split('/');
  if (parts.length <= 3) return path;

  // Keep first and last 2 parts
  return `${parts[0]}/.../${parts.slice(-2).join('/')}`;
}

/**
 * Merge extracted data with existing memory
 */
export function mergeExtracted(
  existing: Partial<MemoryUpsertData>,
  extracted: Partial<MemoryUpsertData>
): Partial<MemoryUpsertData> {
  const result = { ...existing };

  // Overwrite simple fields
  if (extracted.lastProgress) {
    result.lastProgress = extracted.lastProgress;
  }

  // Merge arrays (append new items)
  if (extracted.pendingTasks) {
    result.pendingTasks = [
      ...(result.pendingTasks || []),
      ...extracted.pendingTasks,
    ].slice(-20); // Keep last 20
  }

  if (extracted.completedTasks) {
    result.completedTasks = [
      ...(result.completedTasks || []),
      ...extracted.completedTasks,
    ].slice(-50); // Keep last 50
  }

  if (extracted.knownIssues) {
    result.knownIssues = [
      ...(result.knownIssues || []),
      ...extracted.knownIssues,
    ].slice(-10); // Keep last 10
  }

  if (extracted.decisions) {
    result.decisions = [
      ...(result.decisions || []),
      ...extracted.decisions,
    ].slice(-10);
  }

  return result;
}
