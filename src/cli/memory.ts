/**
 * Memory command - manage session memory for the "relay race" pattern
 */

import chalk from 'chalk';
import { runMigrations } from '../db/migrations.js';
import { memoryRepository } from '../infrastructure/database/index.js';
import { buildContext, buildMinimalContext } from '../domain/memory/index.js';
import type { SessionMemory, MemoryUpsertData } from '../domain/memory/index.js';

/** Format a memory for display */
function formatMemory(memory: SessionMemory, verbose: boolean = false): string {
  const lines: string[] = [];

  // Header
  lines.push(chalk.bold.cyan(`Session: ${memory.sessionId.slice(0, 8)}...`));
  lines.push(chalk.gray(`Directory: ${memory.directory}`));
  lines.push(chalk.gray(`Iterations: ${memory.iterationCount}`));
  lines.push('');

  // Last progress
  if (memory.lastProgress) {
    lines.push(chalk.bold('Last Progress:'));
    lines.push(`  ${memory.lastProgress}`);
    lines.push('');
  }

  // Pending tasks
  if (memory.pendingTasks.length > 0) {
    lines.push(chalk.bold.yellow(`Pending Tasks (${memory.pendingTasks.length}):`));
    for (const task of memory.pendingTasks.slice(0, verbose ? undefined : 5)) {
      lines.push(`  - [ ] ${task}`);
    }
    if (!verbose && memory.pendingTasks.length > 5) {
      lines.push(chalk.gray(`  ... and ${memory.pendingTasks.length - 5} more`));
    }
    lines.push('');
  }

  // Completed tasks (limited unless verbose)
  if (memory.completedTasks.length > 0) {
    const showCount = verbose ? memory.completedTasks.length : 3;
    lines.push(chalk.bold.green(`Completed Tasks (${memory.completedTasks.length}):`));
    for (const task of memory.completedTasks.slice(-showCount)) {
      lines.push(`  - [x] ${task}`);
    }
    if (!verbose && memory.completedTasks.length > 3) {
      lines.push(chalk.gray(`  ... and ${memory.completedTasks.length - 3} more`));
    }
    lines.push('');
  }

  // Known issues
  if (memory.knownIssues.length > 0) {
    lines.push(chalk.bold.red(`Known Issues (${memory.knownIssues.length}):`));
    for (const issue of memory.knownIssues) {
      lines.push(`  - ${issue}`);
    }
    lines.push('');
  }

  // Decisions
  if (memory.decisions.length > 0) {
    lines.push(chalk.bold.blue(`Decisions (${memory.decisions.length}):`));
    for (const decision of memory.decisions) {
      lines.push(`  - ${decision}`);
    }
    lines.push('');
  }

  // Handoff notes
  if (memory.handoffNotes) {
    lines.push(chalk.bold.magenta('Handoff Notes:'));
    lines.push(`  ${memory.handoffNotes}`);
    lines.push('');
  }

  // Handoff priority
  if (memory.handoffPriority.length > 0) {
    lines.push(chalk.bold.magenta('Handoff Priority:'));
    memory.handoffPriority.forEach((item, index) => {
      lines.push(`  ${index + 1}. ${item}`);
    });
    lines.push('');
  }

  // Metadata
  lines.push(chalk.gray(`Updated: ${memory.updatedAt.toLocaleString()}`));
  lines.push(chalk.gray(`Tokens used: ${memory.totalTokensUsed.toLocaleString()}`));

  return lines.join('\n');
}

/** List all memories */
export async function memoryListCommand(options: { directory?: string; limit?: string }): Promise<void> {
  runMigrations();

  let memories: SessionMemory[];

  if (options.directory) {
    memories = memoryRepository.findByDirectory(options.directory);
  } else {
    const limit = options.limit ? parseInt(options.limit, 10) : 10;
    memories = memoryRepository.findRecent(limit);
  }

  if (memories.length === 0) {
    console.log(chalk.yellow('No session memories found.'));
    return;
  }

  console.log(chalk.bold('\nSession Memories:\n'));

  memories.forEach((memory, index) => {
    const minimal = buildMinimalContext(memory);
    console.log(chalk.cyan(`[${index + 1}]`), chalk.bold(memory.sessionId.slice(0, 8)));
    console.log(chalk.gray(`    ${memory.directory}`));
    console.log(chalk.gray(`    ${minimal}`));
    console.log(chalk.gray(`    Iterations: ${memory.iterationCount} | Tasks: ${memory.pendingTasks.length} pending, ${memory.completedTasks.length} done`));
    console.log('');
  });
}

/** Show memory details */
export async function memoryShowCommand(sessionId: string, options: { verbose?: boolean; context?: boolean }): Promise<void> {
  runMigrations();

  // Try to find by prefix match
  const memories = memoryRepository.findAll();
  const memory = memories.find(m =>
    m.sessionId === sessionId || m.sessionId.startsWith(sessionId)
  );

  if (!memory) {
    console.log(chalk.red(`Memory not found for session: ${sessionId}`));
    process.exit(1);
  }

  if (options.context) {
    // Show the context that would be injected on recovery
    console.log(chalk.bold('\nRecovery Context:\n'));
    console.log(buildContext(memory));
  } else {
    // Show formatted memory
    console.log('');
    console.log(formatMemory(memory, options.verbose));
  }
}

/** Edit memory */
export async function memoryEditCommand(
  sessionId: string,
  options: {
    progress?: string;
    addTask?: string;
    completeTask?: string;
    addIssue?: string;
    resolveIssue?: string;
    decision?: string;
    handoff?: string;
    priority?: string;
    notes?: string;
    clear?: boolean;
  }
): Promise<void> {
  runMigrations();

  // Find existing memory
  const memories = memoryRepository.findAll();
  const existingMemory = memories.find(m =>
    m.sessionId === sessionId || m.sessionId.startsWith(sessionId)
  );

  if (!existingMemory && !options.progress) {
    console.log(chalk.red(`Memory not found for session: ${sessionId}`));
    console.log(chalk.gray('Use --progress to create a new memory entry.'));
    process.exit(1);
  }

  const data: MemoryUpsertData = {
    sessionId: existingMemory?.sessionId || sessionId,
  };

  // Clear all fields if requested
  if (options.clear) {
    data.pendingTasks = [];
    data.completedTasks = [];
    data.knownIssues = [];
    data.decisions = [];
    data.handoffNotes = '';
    data.handoffPriority = [];
    data.notes = '';
    console.log(chalk.yellow('Clearing all memory fields...'));
  }

  // Update progress
  if (options.progress) {
    data.lastProgress = options.progress;
  }

  // Add task
  if (options.addTask) {
    const existing = existingMemory?.pendingTasks || [];
    data.pendingTasks = [...existing, options.addTask];
    console.log(chalk.green(`Added task: ${options.addTask}`));
  }

  // Complete task (move from pending to completed)
  if (options.completeTask) {
    const pending = existingMemory?.pendingTasks || [];
    const completed = existingMemory?.completedTasks || [];

    // Find task by index or substring match
    const taskIndex = parseInt(options.completeTask, 10) - 1;
    let taskToComplete: string | undefined;

    if (!isNaN(taskIndex) && taskIndex >= 0 && taskIndex < pending.length) {
      taskToComplete = pending[taskIndex];
      data.pendingTasks = pending.filter((_, i) => i !== taskIndex);
    } else {
      // Find by substring match
      taskToComplete = pending.find(t =>
        t.toLowerCase().includes(options.completeTask!.toLowerCase())
      );
      if (taskToComplete) {
        data.pendingTasks = pending.filter(t => t !== taskToComplete);
      }
    }

    if (taskToComplete) {
      data.completedTasks = [...completed, taskToComplete];
      console.log(chalk.green(`Completed task: ${taskToComplete}`));
    } else {
      console.log(chalk.red(`Task not found: ${options.completeTask}`));
    }
  }

  // Add issue
  if (options.addIssue) {
    const existing = existingMemory?.knownIssues || [];
    data.knownIssues = [...existing, options.addIssue];
    console.log(chalk.yellow(`Added issue: ${options.addIssue}`));
  }

  // Resolve issue
  if (options.resolveIssue) {
    const issues = existingMemory?.knownIssues || [];
    const issueIndex = parseInt(options.resolveIssue, 10) - 1;

    if (!isNaN(issueIndex) && issueIndex >= 0 && issueIndex < issues.length) {
      const resolved = issues[issueIndex];
      data.knownIssues = issues.filter((_, i) => i !== issueIndex);
      console.log(chalk.green(`Resolved issue: ${resolved}`));
    } else {
      data.knownIssues = issues.filter(i =>
        !i.toLowerCase().includes(options.resolveIssue!.toLowerCase())
      );
    }
  }

  // Add decision
  if (options.decision) {
    const existing = existingMemory?.decisions || [];
    data.decisions = [...existing, options.decision];
    console.log(chalk.blue(`Added decision: ${options.decision}`));
  }

  // Set handoff notes
  if (options.handoff) {
    data.handoffNotes = options.handoff;
    console.log(chalk.magenta(`Set handoff notes`));
  }

  // Add to handoff priority
  if (options.priority) {
    const existing = existingMemory?.handoffPriority || [];
    data.handoffPriority = [...existing, options.priority];
    console.log(chalk.magenta(`Added priority: ${options.priority}`));
  }

  // Set notes
  if (options.notes) {
    data.notes = options.notes;
  }

  // Update memory
  const updated = memoryRepository.upsert(data);
  console.log(chalk.green(`\nMemory updated for session: ${updated.sessionId.slice(0, 8)}...`));
}

/** Delete memory */
export async function memoryDeleteCommand(sessionId: string, options: { force?: boolean }): Promise<void> {
  runMigrations();

  // Find by prefix
  const memories = memoryRepository.findAll();
  const memory = memories.find(m =>
    m.sessionId === sessionId || m.sessionId.startsWith(sessionId)
  );

  if (!memory) {
    console.log(chalk.red(`Memory not found for session: ${sessionId}`));
    process.exit(1);
  }

  if (!options.force) {
    console.log(chalk.yellow(`This will delete memory for session: ${memory.sessionId}`));
    console.log(chalk.gray('Use --force to confirm deletion.'));
    return;
  }

  const deleted = memoryRepository.delete(memory.sessionId);

  if (deleted) {
    console.log(chalk.green(`Memory deleted for session: ${memory.sessionId.slice(0, 8)}...`));
  } else {
    console.log(chalk.red('Failed to delete memory.'));
  }
}

/** Export memory context */
export async function memoryExportCommand(sessionId: string, options: { output?: string }): Promise<void> {
  runMigrations();

  // Find by prefix
  const memories = memoryRepository.findAll();
  const memory = memories.find(m =>
    m.sessionId === sessionId || m.sessionId.startsWith(sessionId)
  );

  if (!memory) {
    console.log(chalk.red(`Memory not found for session: ${sessionId}`));
    process.exit(1);
  }

  const context = buildContext(memory);

  if (options.output) {
    const fs = await import('fs');
    fs.writeFileSync(options.output, context, 'utf-8');
    console.log(chalk.green(`Context exported to: ${options.output}`));
  } else {
    console.log(context);
  }
}
