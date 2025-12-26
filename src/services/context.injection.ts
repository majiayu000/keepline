/**
 * Context Injection Service
 *
 * Retrieves relevant memories from the vector store and injects them
 * into Claude Code sessions via CLAUDE.md or system prompts.
 */

import { logger } from '../lib/logger.js';
import { getVectorStore } from '../infrastructure/vector/lancedb.adapter.js';
import { getEmbeddingService } from '../infrastructure/vector/embedding.service.js';
import type { Observation, SearchResult, ObservationCategory } from '../infrastructure/vector/types.js';

/** Context injection configuration */
export interface ContextInjectionConfig {
  maxObservations?: number;
  maxTokens?: number;
  minScore?: number;
  includeCategories?: ObservationCategory[];
  excludeCategories?: ObservationCategory[];
  sessionRecency?: number; // Days to consider for recent sessions
}

/** Injected context result */
export interface InjectedContext {
  observations: Observation[];
  totalTokens: number;
  contextBlock: string;
  searchQuery: string;
}

/** Context relevance weights by category */
const CATEGORY_WEIGHTS: Record<ObservationCategory, number> = {
  decision: 1.5, // Architecture decisions are most important
  bugfix: 1.3, // Bug fixes often have recurring patterns
  feature: 1.2, // Feature context helps with continuity
  refactor: 1.1, // Refactoring context
  discovery: 1.0, // Code exploration
  change: 0.9, // File changes are less contextually important
};

/** Default configuration */
const DEFAULT_CONFIG: Required<ContextInjectionConfig> = {
  maxObservations: 10,
  maxTokens: 2000,
  minScore: 0.3,
  includeCategories: ['decision', 'bugfix', 'feature', 'refactor', 'discovery', 'change'],
  excludeCategories: [],
  sessionRecency: 30, // Last 30 days
};

/**
 * Context Injection Service
 */
export class ContextInjectionService {
  private config: Required<ContextInjectionConfig>;

  constructor(config: ContextInjectionConfig = {}) {
    this.config = { ...DEFAULT_CONFIG, ...config };
  }

  /**
   * Generate context block for a session
   */
  async generateContext(
    projectPath: string,
    userPrompt?: string
  ): Promise<InjectedContext> {
    const vectorStore = getVectorStore();
    const embeddingService = getEmbeddingService();

    await vectorStore.initialize();

    // Build search query from project path and optional user prompt
    const searchQuery = this.buildSearchQuery(projectPath, userPrompt);

    // Generate query embedding
    const queryVector = await embeddingService.embed(searchQuery);

    // Search for relevant observations
    const searchResults = await vectorStore.search(queryVector, {
      limit: this.config.maxObservations * 2, // Get extra for filtering
      minScore: this.config.minScore,
    });

    // Filter and rank results
    const rankedResults = this.filterAndRank(searchResults);

    // Build context within token limit
    const { observations, totalTokens } = this.buildContextWithinLimit(rankedResults);

    // Format as context block
    const contextBlock = this.formatContextBlock(observations, projectPath);

    logger.info(
      `Generated context: ${observations.length} observations, ${totalTokens} tokens`
    );

    return {
      observations,
      totalTokens,
      contextBlock,
      searchQuery,
    };
  }

  /**
   * Build search query from project context
   */
  private buildSearchQuery(projectPath: string, userPrompt?: string): string {
    const parts: string[] = [];

    // Extract project name from path
    const projectName = projectPath.split('/').pop() || 'project';
    parts.push(`Project: ${projectName}`);

    // Add project path for context
    parts.push(`Path: ${projectPath}`);

    // Add user prompt if provided
    if (userPrompt) {
      parts.push(`Task: ${userPrompt}`);
    }

    return parts.join('\n');
  }

  /**
   * Filter and rank search results
   */
  private filterAndRank(results: SearchResult[]): SearchResult[] {
    const { includeCategories, excludeCategories } = this.config;

    return results
      .filter((result) => {
        const category = result.observation.category;

        // Apply include filter
        if (!includeCategories.includes(category)) {
          return false;
        }

        // Apply exclude filter
        if (excludeCategories.includes(category)) {
          return false;
        }

        return true;
      })
      .map((result) => ({
        ...result,
        // Apply category weight to score
        score: result.score * CATEGORY_WEIGHTS[result.observation.category],
      }))
      .sort((a, b) => b.score - a.score);
  }

  /**
   * Build context within token limit
   */
  private buildContextWithinLimit(
    results: SearchResult[]
  ): { observations: Observation[]; totalTokens: number } {
    const observations: Observation[] = [];
    let totalTokens = 0;

    for (const result of results) {
      const observation = result.observation;

      // Check if adding this would exceed token limit
      if (totalTokens + observation.tokenCount > this.config.maxTokens) {
        // If we have at least some observations, stop here
        if (observations.length >= 3) {
          break;
        }
        // Otherwise, try to include at least a few
        continue;
      }

      observations.push(observation);
      totalTokens += observation.tokenCount;

      // Stop if we have enough observations
      if (observations.length >= this.config.maxObservations) {
        break;
      }
    }

    return { observations, totalTokens };
  }

  /**
   * Format observations as a context block
   */
  private formatContextBlock(observations: Observation[], projectPath: string): string {
    if (observations.length === 0) {
      return '';
    }

    const lines: string[] = [
      '## Previous Session Context',
      '',
      `*Retrieved ${observations.length} relevant memories for ${projectPath}*`,
      '',
    ];

    // Group by category
    const byCategory = new Map<ObservationCategory, Observation[]>();
    for (const obs of observations) {
      const existing = byCategory.get(obs.category) || [];
      existing.push(obs);
      byCategory.set(obs.category, existing);
    }

    // Add each category
    const categoryLabels: Record<ObservationCategory, string> = {
      decision: '### Decisions',
      bugfix: '### Bug Fixes',
      feature: '### Features',
      refactor: '### Refactoring',
      discovery: '### Discoveries',
      change: '### Changes',
    };

    // Order categories by importance
    const categoryOrder: ObservationCategory[] = [
      'decision',
      'bugfix',
      'feature',
      'refactor',
      'discovery',
      'change',
    ];

    for (const category of categoryOrder) {
      const categoryObs = byCategory.get(category);
      if (!categoryObs || categoryObs.length === 0) continue;

      lines.push(categoryLabels[category]);
      lines.push('');

      for (const obs of categoryObs) {
        lines.push(`- ${obs.content}`);

        // Add file references if any
        if (obs.files.length > 0) {
          const fileList = obs.files.slice(0, 3).join(', ');
          lines.push(`  - Files: ${fileList}`);
        }

        // Add date
        const date = obs.timestamp.toISOString().split('T')[0];
        lines.push(`  - Date: ${date}`);
        lines.push('');
      }
    }

    return lines.join('\n');
  }

  /**
   * Search for memories related to specific files
   */
  async searchByFiles(files: string[], limit: number = 5): Promise<SearchResult[]> {
    const vectorStore = getVectorStore();
    const embeddingService = getEmbeddingService();

    await vectorStore.initialize();

    // Build query from file paths
    const query = `Files: ${files.join(', ')}`;
    const queryVector = await embeddingService.embed(query);

    const results = await vectorStore.search(queryVector, {
      limit,
      minScore: this.config.minScore,
    });

    // Additionally filter by file overlap
    return results.filter((result) => {
      const observationFiles = result.observation.files;
      return files.some((file) =>
        observationFiles.some((obsFile) =>
          obsFile.includes(file) || file.includes(obsFile)
        )
      );
    });
  }

  /**
   * Search for memories related to specific concepts
   */
  async searchByConcepts(concepts: string[], limit: number = 5): Promise<SearchResult[]> {
    const vectorStore = getVectorStore();
    const embeddingService = getEmbeddingService();

    await vectorStore.initialize();

    // Build query from concepts
    const query = concepts.join(' ');
    const queryVector = await embeddingService.embed(query);

    return vectorStore.search(queryVector, {
      limit,
      minScore: this.config.minScore,
    });
  }

  /**
   * Get recent memories for a session
   */
  async getRecentMemories(sessionId: string, limit: number = 10): Promise<Observation[]> {
    const vectorStore = getVectorStore();
    await vectorStore.initialize();

    const observations = await vectorStore.getBySessionId(sessionId);

    // Sort by timestamp descending and limit
    return observations
      .sort((a, b) => b.timestamp.getTime() - a.timestamp.getTime())
      .slice(0, limit);
  }
}

/** Singleton instance */
let injectionServiceInstance: ContextInjectionService | null = null;

/**
 * Get the context injection service instance
 */
export function getContextInjectionService(
  config?: ContextInjectionConfig
): ContextInjectionService {
  if (!injectionServiceInstance) {
    injectionServiceInstance = new ContextInjectionService(config);
  }
  return injectionServiceInstance;
}

/**
 * Generate context for a session
 */
export async function generateSessionContext(
  projectPath: string,
  userPrompt?: string
): Promise<InjectedContext> {
  const service = getContextInjectionService();
  return service.generateContext(projectPath, userPrompt);
}

/**
 * Reset the injection service (for testing)
 */
export function resetContextInjectionService(): void {
  injectionServiceInstance = null;
}
