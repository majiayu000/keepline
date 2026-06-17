import { matchProcessesToSessions } from '../services/session.process-matcher.js';
import type { ClaudeProcessInfo } from '../adapters/process/types.js';
import type { SessionProcessCandidate } from '../services/session.process-matcher.js';
import { aggregateUsageStats } from '../services/usage.extractor.js';
import type { ClaudeEntry } from '../adapters/claude/types.js';
import { summarizeSessionEntries } from '../adapters/claude/parser/jsonl.js';
import type { ClaudeEntryWithAgent } from '../adapters/claude/parser/jsonl.js';
import { parseClaudePsOutput } from '../adapters/process/scanner.js';

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
    client: 'claude',
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

function buildUsageAggregationScenario(): {
  entries: ClaudeEntry[];
  innerLoops: number;
} {
  const entries: ClaudeEntry[] = [];
  const models = [
    'claude-3-5-sonnet-20241022',
    'claude-3-5-haiku-20241022',
    'claude-3-opus-20240229',
  ];

  for (let i = 0; i < 4_800; i++) {
    const model = models[i % models.length];
    entries.push({
      type: 'assistant',
      uuid: `usage-entry-${i}`,
      sessionId: `usage-session-${Math.floor(i / 30)}`,
      cwd: '/tmp/benchmark/usage',
      timestamp: new Date(BASE_TIME + i * 1000).toISOString(),
      message: {
        role: 'assistant',
        model,
        content: [{ type: 'text', text: 'benchmark output' }],
        usage: {
          input_tokens: 1_000 + (i % 40),
          output_tokens: 600 + (i % 25),
          cache_creation_input_tokens: i % 9,
          cache_read_input_tokens: i % 13,
        } as unknown as { input_tokens: number; output_tokens: number },
      },
    } as ClaudeEntry);
  }

  // Add user entries that should be ignored by aggregation path.
  for (let i = 0; i < 1_200; i++) {
    entries.push({
      type: 'user',
      uuid: `usage-user-${i}`,
      sessionId: `usage-session-${Math.floor(i / 30)}`,
      cwd: '/tmp/benchmark/usage',
      timestamp: new Date(BASE_TIME + i * 1000).toISOString(),
      userType: 'external',
      message: {
        role: 'user',
        content: 'hi',
      },
    });
  }

  return { entries, innerLoops: 30 };
}

function runUsageAggregationScenario(
  name: string,
  entries: ClaudeEntry[],
  innerLoops: number
): BenchmarkResult {
  for (let i = 0; i < WARMUP_ROUNDS; i++) {
    aggregateUsageStats(entries);
  }

  const samplesMs: number[] = [];
  for (let round = 0; round < BENCH_ROUNDS; round++) {
    const start = process.hrtime.bigint();
    for (let i = 0; i < innerLoops; i++) {
      aggregateUsageStats(entries);
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

function buildJsonlSummaryScenario(): {
  entries: ClaudeEntryWithAgent[];
  innerLoops: number;
} {
  const entries: ClaudeEntryWithAgent[] = [];
  const models = [
    'claude-3-5-sonnet-20241022',
    'claude-3-5-haiku-20241022',
  ];

  for (let i = 0; i < 2_400; i++) {
    entries.push({
      type: 'user',
      uuid: `jsonl-user-${i}`,
      sessionId: 'jsonl-session-main',
      cwd: '/tmp/benchmark/jsonl',
      timestamp: new Date(BASE_TIME + i * 1_000).toISOString(),
      userType: 'external',
      message: {
        role: 'user',
        content: `Implement benchmark case ${i}`,
      },
    });

    entries.push({
      type: 'assistant',
      uuid: `jsonl-assistant-${i}`,
      sessionId: 'jsonl-session-main',
      cwd: '/tmp/benchmark/jsonl',
      timestamp: new Date(BASE_TIME + i * 1_000 + 500).toISOString(),
      agentId: i % 17 === 0 ? `agent-${i}` : undefined,
      isSidechain: i % 19 === 0 ? true : undefined,
      message: {
        role: 'assistant',
        model: models[i % models.length],
        content: [
          { type: 'text', text: `analysis ${i}` },
          {
            type: 'tool_use',
            id: `tool-${i}`,
            name: i % 2 === 0 ? 'Read' : 'Write',
            input: {
              path: `/tmp/benchmark/jsonl/file-${i % 200}.ts`,
            },
          },
          { type: 'text', text: `result ${i}` },
        ],
        usage: {
          input_tokens: 400 + (i % 20),
          output_tokens: 220 + (i % 10),
          cache_creation_input_tokens: i % 5,
          cache_read_input_tokens: i % 11,
        } as unknown as { input_tokens: number; output_tokens: number },
      },
    });
  }

  return { entries, innerLoops: 12 };
}

function runJsonlSummaryScenario(
  name: string,
  entries: ClaudeEntryWithAgent[],
  innerLoops: number
): BenchmarkResult {
  for (let i = 0; i < WARMUP_ROUNDS; i++) {
    summarizeSessionEntries(entries);
  }

  const samplesMs: number[] = [];
  for (let round = 0; round < BENCH_ROUNDS; round++) {
    const start = process.hrtime.bigint();
    for (let i = 0; i < innerLoops; i++) {
      summarizeSessionEntries(entries);
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

function buildProcessParserScenario(): {
  output: string;
  innerLoops: number;
} {
  const lines: string[] = [];

  for (let i = 0; i < 2_400; i++) {
    const pid = 10_000 + i;
    const cpu = (i % 10).toFixed(1);
    const mem = (0.2 + (i % 6) * 0.1).toFixed(1);
    const day = ((i % 28) + 1).toString().padStart(2, ' ');
    const seconds = (i % 60).toString().padStart(2, '0');
    const lstart = `Mon Jan ${day} 10:30:${seconds} 2026`;
    const command =
      i % 3 === 0
        ? '/usr/local/bin/claude --model sonnet'
        : '/usr/bin/python worker.py';
    lines.push(`${pid} ${cpu} ${mem} ttys001 ${lstart} ${command}`);
  }

  return {
    output: lines.join('\n'),
    innerLoops: 40,
  };
}

function runProcessParserScenario(
  name: string,
  output: string,
  innerLoops: number
): BenchmarkResult {
  for (let i = 0; i < WARMUP_ROUNDS; i++) {
    parseClaudePsOutput(output);
  }

  const samplesMs: number[] = [];
  for (let round = 0; round < BENCH_ROUNDS; round++) {
    const start = process.hrtime.bigint();
    for (let i = 0; i < innerLoops; i++) {
      parseClaudePsOutput(output);
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
  const usage = buildUsageAggregationScenario();
  const jsonl = buildJsonlSummaryScenario();
  const processParser = buildProcessParserScenario();

  const results = [
    runScenario('dense_same_directory', dense.sessions, dense.processes, dense.innerLoops),
    runScenario('multi_directory', mixed.sessions, mixed.processes, mixed.innerLoops),
    runUsageAggregationScenario('usage_aggregation', usage.entries, usage.innerLoops),
    runJsonlSummaryScenario('jsonl_summary', jsonl.entries, jsonl.innerLoops),
    runProcessParserScenario('process_ps_parser', processParser.output, processParser.innerLoops),
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
