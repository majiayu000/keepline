import { matchProcessesToSessions } from '../services/session.process-matcher.js';
import type { ClaudeProcessInfo } from '../adapters/process/types.js';
import type { SessionProcessCandidate } from '../services/session.process-matcher.js';

type BenchmarkResult = {
  name: string;
  rounds: number;
  innerLoops: number;
  meanMs: number;
  medianMs: number;
  p95Ms: number;
  minMs: number;
  maxMs: number;
};

const BENCH_ROUNDS = 200;
const WARMUP_ROUNDS = 40;
const BASE_TIME = Date.parse('2026-01-01T00:00:00.000Z');

function createSession(
  directory: string,
  index: number,
  pid?: number
): SessionProcessCandidate {
  const startedAt = new Date(BASE_TIME - index * 15_000);
  const lastActiveAt = new Date(BASE_TIME - index * 7_000);
  return {
    sessionId: `${directory.replace(/\W+/g, '-')}-session-${index}`,
    directory,
    startedAt,
    lastActiveAt,
    pid,
  };
}

function createProcess(
  directory: string,
  index: number
): ClaudeProcessInfo {
  return {
    pid: 10_000 + index,
    cwd: directory,
    cpu: (index % 11) * 0.1,
    memory: 20 + (index % 7),
    startTime: new Date(BASE_TIME - index * 16_000),
    args: ['claude'],
  };
}

function buildDenseSameDirectoryScenario(): {
  sessions: SessionProcessCandidate[];
  processes: ClaudeProcessInfo[];
  innerLoops: number;
} {
  const directory = '/tmp/benchmark/dense';
  const sessions: SessionProcessCandidate[] = [];
  const processes: ClaudeProcessInfo[] = [];

  for (let i = 0; i < 48; i++) {
    // Keep some PID continuity to exercise fast-path matching.
    const pinnedPid = i < 6 ? 10_000 + i : undefined;
    sessions.push(createSession(directory, i, pinnedPid));
  }

  for (let i = 0; i < 6; i++) {
    processes.push(createProcess(directory, i));
  }

  return { sessions, processes, innerLoops: 120 };
}

function buildMultiDirectoryScenario(): {
  sessions: SessionProcessCandidate[];
  processes: ClaudeProcessInfo[];
  innerLoops: number;
} {
  const sessions: SessionProcessCandidate[] = [];
  const processes: ClaudeProcessInfo[] = [];
  let processIndex = 200;

  for (let dirIndex = 0; dirIndex < 10; dirIndex++) {
    const directory = `/tmp/benchmark/mixed/project-${dirIndex}`;

    for (let i = 0; i < 10; i++) {
      sessions.push(createSession(directory, dirIndex * 100 + i));
    }

    for (let i = 0; i < 3; i++) {
      processes.push(createProcess(directory, processIndex++));
    }
  }

  return { sessions, processes, innerLoops: 160 };
}

function percentile(sortedValues: number[], p: number): number {
  if (sortedValues.length === 0) return 0;
  const index = Math.min(
    sortedValues.length - 1,
    Math.max(0, Math.floor(sortedValues.length * p))
  );
  return sortedValues[index];
}

function runScenario(
  name: string,
  sessions: SessionProcessCandidate[],
  processes: ClaudeProcessInfo[],
  innerLoops: number
): BenchmarkResult {
  for (let i = 0; i < WARMUP_ROUNDS; i++) {
    matchProcessesToSessions(sessions, processes);
  }

  const samplesMs: number[] = [];
  for (let round = 0; round < BENCH_ROUNDS; round++) {
    const start = process.hrtime.bigint();
    for (let i = 0; i < innerLoops; i++) {
      matchProcessesToSessions(sessions, processes);
    }
    const durationNs = Number(process.hrtime.bigint() - start);
    samplesMs.push((durationNs / innerLoops) / 1_000_000);
  }

  const sorted = [...samplesMs].sort((a, b) => a - b);
  const total = samplesMs.reduce((sum, value) => sum + value, 0);

  return {
    name,
    rounds: BENCH_ROUNDS,
    innerLoops,
    meanMs: total / samplesMs.length,
    medianMs: percentile(sorted, 0.5),
    p95Ms: percentile(sorted, 0.95),
    minMs: sorted[0],
    maxMs: sorted[sorted.length - 1],
  };
}

function main(): void {
  const dense = buildDenseSameDirectoryScenario();
  const mixed = buildMultiDirectoryScenario();

  const results = [
    runScenario('dense_same_directory', dense.sessions, dense.processes, dense.innerLoops),
    runScenario('multi_directory', mixed.sessions, mixed.processes, mixed.innerLoops),
  ];

  const compositeMeanMs = results.reduce((sum, item) => sum + item.meanMs, 0) / results.length;
  const compositeP95Ms = results.reduce((sum, item) => sum + item.p95Ms, 0) / results.length;

  const report = {
    benchmark: 'session-process-matcher',
    rounds: BENCH_ROUNDS,
    generatedAt: new Date().toISOString(),
    compositeMeanMs,
    compositeP95Ms,
    results,
  };

  console.log(JSON.stringify(report, null, 2));
}

main();
