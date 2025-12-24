/**
 * Plan parser - parses Claude Code plan markdown files
 */

import { readFileSync, readdirSync, statSync, existsSync } from 'fs';
import { join, basename } from 'path';
import { homedir } from 'os';
import type { Plan, PlanPhase, PlanTask, PlanSummary } from './types.js';
import { logger } from '../../../lib/logger.js';

/** Claude plans directory */
const CLAUDE_PLANS_DIR = join(homedir(), '.claude', 'plans');

/** Parse task markers from text */
function parseTaskLine(
  line: string,
  currentPhase?: string,
  inCodeBlock?: boolean
): PlanTask | null {
  // Skip if inside code block
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

  // Match ✅ emoji (completed) - common in many plans
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

  // Match plain list items inside a Phase (only when we're in a phase context)
  // Format: "- Task description" at the start of line (with optional leading whitespace)
  // Only match if inside a phase to avoid matching random list items
  if (currentPhase) {
    const plainListMatch = line.match(/^[-*]\s+(.+)$/);
    if (plainListMatch) {
      const text = plainListMatch[1].trim();
      // Skip code blocks and bold markers
      // Minimum 3 chars to filter out single words/bullets
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

/** Parse phase header */
function parsePhaseHeader(line: string): { name: string; title: string } | null {
  // Match explicit "Phase X" patterns only:
  // - "### Phase 1: Title"
  // - "## Phase 1 - Title"
  // - "### Phase 1"
  // Must have "Phase" keyword followed by number
  const phaseMatch = line.match(/^#{2,4}\s*Phase\s+(\d+)[\s:.-]*(.*)$/i);
  if (phaseMatch) {
    const name = `Phase ${phaseMatch[1]}`;
    const title = phaseMatch[2]?.trim() || name;
    return { name, title };
  }

  // Match "Step X" or "Step X.Y" patterns (e.g., "Step 1", "Step 2.1")
  // But only at ## or ### level (not ####, which are sub-steps)
  const stepMatch = line.match(/^#{2,3}\s*Step\s+(\d+(?:\.\d+)?)[\s:.-]*(.*)$/i);
  if (stepMatch) {
    const name = `Step ${stepMatch[1]}`;
    const title = stepMatch[2]?.trim() || name;
    return { name, title };
  }

  return null;
}

/** Parse sub-step header (#### level) - these become tasks */
function parseSubStepHeader(line: string): string | null {
  // Match "#### Step X.Y: Title" patterns
  const subStepMatch = line.match(/^#{4}\s*Step\s+[\d.]+[\s:.-]*(.+)$/i);
  if (subStepMatch) {
    return subStepMatch[1].trim();
  }
  return null;
}

/** Extract title from markdown (first H1) */
function extractTitle(content: string): string {
  const match = content.match(/^#\s+(.+)$/m);
  return match ? match[1].trim() : 'Untitled Plan';
}

/** Parse a plan file */
export function parsePlanFile(filePath: string): Plan | null {
  try {
    const content = readFileSync(filePath, 'utf-8');
    const stat = statSync(filePath);
    const id = basename(filePath, '.md');
    const title = extractTitle(content);

    const phases: PlanPhase[] = [];
    const allTasks: PlanTask[] = [];
    let currentPhase: PlanPhase | null = null;
    let inCodeBlock = false;

    const lines = content.split('\n');

    for (const line of lines) {
      // Track code blocks (``` markers)
      if (line.trim().startsWith('```')) {
        inCodeBlock = !inCodeBlock;
        continue;
      }

      // Skip content inside code blocks
      if (inCodeBlock) continue;

      // Check for phase header
      const phaseHeader = parsePhaseHeader(line);
      if (phaseHeader) {
        // Save previous phase
        if (currentPhase) {
          currentPhase.completedCount = currentPhase.tasks.filter(t => t.completed).length;
          currentPhase.totalCount = currentPhase.tasks.length;
          phases.push(currentPhase);
        }
        // Start new phase
        currentPhase = {
          name: phaseHeader.name,
          title: phaseHeader.title,
          tasks: [],
          completedCount: 0,
          totalCount: 0,
        };
        continue;
      }

      // Check for task
      const task = parseTaskLine(line, currentPhase?.name, inCodeBlock);
      if (task) {
        allTasks.push(task);
        if (currentPhase) {
          currentPhase.tasks.push(task);
        }
        continue;
      }

      // Check for sub-step headers (#### Step X.Y: Title) - treat as tasks
      if (currentPhase) {
        const subStep = parseSubStepHeader(line);
        if (subStep) {
          const subTask: PlanTask = {
            text: subStep,
            completed: false,
            phase: currentPhase.name,
          };
          allTasks.push(subTask);
          currentPhase.tasks.push(subTask);
        }
      }
    }

    // Don't forget the last phase
    if (currentPhase) {
      currentPhase.completedCount = currentPhase.tasks.filter(t => t.completed).length;
      currentPhase.totalCount = currentPhase.tasks.length;
      phases.push(currentPhase);
    }

    const completedTasks = allTasks.filter(t => t.completed).length;
    const totalTasks = allTasks.length;

    return {
      id,
      filePath,
      title,
      modifiedAt: stat.mtime,
      createdAt: stat.birthtime,
      content,
      phases,
      tasks: allTasks,
      stats: {
        totalTasks,
        completedTasks,
        pendingTasks: totalTasks - completedTasks,
        completionPercent: totalTasks > 0 ? Math.round((completedTasks / totalTasks) * 100) : 0,
        phaseCount: phases.length,
      },
    };
  } catch (error) {
    logger.error(`Failed to parse plan file: ${filePath}`, error);
    return null;
  }
}

/** Scan plans directory */
export function scanPlansDirectory(): Plan[] {
  if (!existsSync(CLAUDE_PLANS_DIR)) {
    logger.debug('Claude plans directory not found');
    return [];
  }

  const plans: Plan[] = [];
  const files = readdirSync(CLAUDE_PLANS_DIR);

  for (const file of files) {
    if (!file.endsWith('.md')) continue;

    const filePath = join(CLAUDE_PLANS_DIR, file);
    const stat = statSync(filePath);

    if (!stat.isFile()) continue;

    const plan = parsePlanFile(filePath);
    if (plan) {
      plans.push(plan);
    }
  }

  // Sort by modification time (newest first)
  return plans.sort((a, b) => b.modifiedAt.getTime() - a.modifiedAt.getTime());
}

/** Get plan summaries */
export function getPlanSummaries(): PlanSummary[] {
  const plans = scanPlansDirectory();
  return plans.map(plan => ({
    id: plan.id,
    title: plan.title,
    modifiedAt: plan.modifiedAt,
    completedTasks: plan.stats.completedTasks,
    totalTasks: plan.stats.totalTasks,
    completionPercent: plan.stats.completionPercent,
    phaseCount: plan.stats.phaseCount,
  }));
}

/** Get a single plan by ID */
export function getPlanById(id: string): Plan | null {
  const filePath = join(CLAUDE_PLANS_DIR, `${id}.md`);
  if (!existsSync(filePath)) {
    return null;
  }
  return parsePlanFile(filePath);
}

/** Get aggregate stats */
export function getPlanStats(): {
  totalPlans: number;
  totalTasks: number;
  completedTasks: number;
  overallCompletion: number;
} {
  const plans = scanPlansDirectory();
  const totalTasks = plans.reduce((sum, p) => sum + p.stats.totalTasks, 0);
  const completedTasks = plans.reduce((sum, p) => sum + p.stats.completedTasks, 0);

  return {
    totalPlans: plans.length,
    totalTasks,
    completedTasks,
    overallCompletion: totalTasks > 0 ? Math.round((completedTasks / totalTasks) * 100) : 0,
  };
}
