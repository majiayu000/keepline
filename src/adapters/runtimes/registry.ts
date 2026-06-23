/**
 * Runtime adapter registry.
 */

import type {
  AgentRuntimeAdapter,
  RuntimeFeatureCapability,
  RuntimeId,
  RuntimeScanError,
  RuntimeScanOptions,
  RuntimeScanResult,
} from '../../domain/runtime/index.js';
import { ClaudeCodeRuntimeAdapter } from './claude-code.js';
import { CodexRuntimeAdapter } from './codex.js';

const FEATURE_CAPABILITIES: readonly RuntimeFeatureCapability[] = ['quota', 'plans', 'hooks'];

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function hasFeatureContract(
  adapter: AgentRuntimeAdapter,
  capability: RuntimeFeatureCapability
): boolean {
  const provider = adapter.descriptor.featureProviders?.[capability];
  const routes = adapter.descriptor.compatibilityRoutes?.[capability];
  return provider !== undefined || (routes !== undefined && routes.length > 0);
}

function validateAdapter(adapter: AgentRuntimeAdapter): void {
  if (adapter.descriptor.capabilities.includes('resume') && !adapter.buildRecoveryCommand) {
    throw new Error(
      `Runtime adapter ${adapter.descriptor.id} declares resume without buildRecoveryCommand`
    );
  }

  for (const capability of FEATURE_CAPABILITIES) {
    if (
      adapter.descriptor.capabilities.includes(capability) &&
      !hasFeatureContract(adapter, capability)
    ) {
      throw new Error(
        `Runtime adapter ${adapter.descriptor.id} declares ${capability} without a provider or compatibility route`
      );
    }
  }
}

export class RuntimeRegistry {
  private readonly adapters = new Map<RuntimeId, AgentRuntimeAdapter>();

  constructor(adapters: AgentRuntimeAdapter[] = []) {
    for (const adapter of adapters) {
      this.register(adapter);
    }
  }

  register(adapter: AgentRuntimeAdapter): void {
    validateAdapter(adapter);
    if (this.adapters.has(adapter.descriptor.id)) {
      throw new Error(`Runtime adapter already registered: ${adapter.descriptor.id}`);
    }
    this.adapters.set(adapter.descriptor.id, adapter);
  }

  list(): AgentRuntimeAdapter[] {
    return [...this.adapters.values()];
  }

  get(runtimeId: RuntimeId): AgentRuntimeAdapter | undefined {
    return this.adapters.get(runtimeId);
  }

  async scanAll(options: RuntimeScanOptions = {}): Promise<RuntimeScanResult[]> {
    return Promise.all(
      this.list().map(async (adapter) => {
        try {
          return await adapter.scanSessions(options);
        } catch (error) {
          const scanError: RuntimeScanError = {
            runtimeId: adapter.descriptor.id,
            code: 'unknown',
            message: errorMessage(error),
            recoverable: true,
          };
          return {
            runtime: adapter.descriptor,
            sessions: [],
            errors: [scanError],
          };
        }
      })
    );
  }
}

export function createDefaultRuntimeRegistry(): RuntimeRegistry {
  return new RuntimeRegistry([
    new ClaudeCodeRuntimeAdapter(),
    new CodexRuntimeAdapter(),
  ]);
}
