import { afterEach, beforeEach, expect, test } from 'bun:test';
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { chmod } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import {
  acquireCodexAuthRefreshLock,
  CodexAuthPersistError,
  isRetryableFlockErrno,
  refreshPersistedCodexAuth,
  setCodexAuthFlockAttemptForTests,
  setCodexAuthPlatformForTests,
} from '../web/api/routes/usage.js';

const originalFetch = globalThis.fetch;
const TOKEN_URL = 'https://auth.openai.com/oauth/token';
const CLIENT_ID = 'app_EMoamEEZ73f0CkXaXp7hrann';

beforeEach(() => {
  globalThis.fetch = (async () => {
    throw new Error('unexpected global fetch');
  }) as unknown as typeof fetch;
});

afterEach(() => {
  globalThis.fetch = originalFetch;
  setCodexAuthFlockAttemptForTests(undefined);
  setCodexAuthPlatformForTests(undefined);
});

function jwtWithExp(exp: number): string {
  const header = Buffer.from(JSON.stringify({ alg: 'none', typ: 'JWT' })).toString('base64url');
  const payload = Buffer.from(JSON.stringify({ exp })).toString('base64url');
  return `${header}.${payload}.sig`;
}

function expiredAccessToken(): string {
  return jwtWithExp(Math.floor(Date.now() / 1000) - 120);
}

function freshAccessToken(): string {
  return jwtWithExp(Math.floor(Date.now() / 1000) + 3600);
}

function writeAuth(path: string, accessToken: string, refreshToken: string, idToken: string): void {
  const body = {
    auth_mode: 'chatgpt',
    OPENAI_API_KEY: 'sk-test',
    tokens: {
      id_token: idToken,
      access_token: accessToken,
      refresh_token: refreshToken,
      account_id: 'acc_kept',
    },
    last_refresh: '2020-01-01T00:00:00Z',
    extra: { keep: true },
  };
  writeFileSync(path, `${JSON.stringify(body, null, 2)}\n`, { mode: 0o644 });
}

function readAuth(path: string): {
  auth_mode: string;
  OPENAI_API_KEY: string;
  last_refresh: string;
  extra: { keep: boolean };
  tokens: {
    id_token: string;
    access_token: string;
    refresh_token: string;
    account_id: string;
  };
} {
  return JSON.parse(readFileSync(path, 'utf8')) as ReturnType<typeof readAuth>;
}

function tempAuthPath(): { dir: string; authPath: string } {
  const dir = mkdtempSync(join(tmpdir(), 'codex-quota-refresh-'));
  return { dir, authPath: join(dir, 'auth.json') };
}

test('persisted refresh updates the bundle and leaves mode 0600', async () => {
  const { dir, authPath } = tempAuthPath();
  try {
    writeAuth(authPath, expiredAccessToken(), 'old-refresh', 'old-id');
    const fetchImpl = async (input: string, init?: RequestInit) => {
      expect(String(input)).toBe(TOKEN_URL);
      expect(JSON.parse(String(init?.body))).toEqual({
        grant_type: 'refresh_token',
        refresh_token: 'old-refresh',
        client_id: CLIENT_ID,
      });
      return Response.json({
        access_token: 'new-access',
        refresh_token: 'new-refresh',
        id_token: 'new-id',
      });
    };

    const refreshed = await refreshPersistedCodexAuth(authPath, fetchImpl);
    expect(refreshed).toEqual({ accessToken: 'new-access', idToken: 'new-id' });

    const saved = readAuth(authPath);
    expect(saved.auth_mode).toBe('chatgpt');
    expect(saved.OPENAI_API_KEY).toBe('sk-test');
    expect(saved.extra.keep).toBe(true);
    expect(saved.tokens.account_id).toBe('acc_kept');
    expect(saved.tokens.access_token).toBe('new-access');
    expect(saved.tokens.refresh_token).toBe('new-refresh');
    expect(saved.tokens.id_token).toBe('new-id');
    expect(saved.last_refresh).toMatch(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/);
    expect(Math.abs(Date.now() - Date.parse(saved.last_refresh))).toBeLessThan(60_000);
    expect(statSync(authPath).mode & 0o777).toBe(0o600);
    expect(readdirSync(dir).filter((name) => name.includes('.tmp'))).toEqual([]);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('omitted or empty refresh and id tokens stay stored', async () => {
  const { dir, authPath } = tempAuthPath();
  const emptyPath = join(dir, 'empty.json');
  try {
    writeAuth(authPath, expiredAccessToken(), 'old-refresh', 'old-id');
    await refreshPersistedCodexAuth(authPath, async () => Response.json({
      access_token: 'new-access',
    }));
    const omitted = readAuth(authPath);
    expect(omitted.tokens.access_token).toBe('new-access');
    expect(omitted.tokens.refresh_token).toBe('old-refresh');
    expect(omitted.tokens.id_token).toBe('old-id');

    writeAuth(emptyPath, expiredAccessToken(), 'old-refresh', 'old-id');
    await refreshPersistedCodexAuth(emptyPath, async () => Response.json({
      access_token: 'newer-access',
      refresh_token: '',
      id_token: '',
    }));
    const empty = readAuth(emptyPath);
    expect(empty.tokens.access_token).toBe('newer-access');
    expect(empty.tokens.refresh_token).toBe('old-refresh');
    expect(empty.tokens.id_token).toBe('old-id');
    expect(empty.tokens.account_id).toBe('acc_kept');
    expect(empty.OPENAI_API_KEY).toBe('sk-test');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('locked reread of a fresh token skips the token endpoint', async () => {
  const { dir, authPath } = tempAuthPath();
  try {
    writeAuth(authPath, expiredAccessToken(), 'refresh-keep', 'id-keep');
    const lock = await acquireCodexAuthRefreshLock(authPath);
    let settled = false;
    const pending = refreshPersistedCodexAuth(authPath, async () => {
      throw new Error('token endpoint must not be called');
    }).then(
      (value) => {
        settled = true;
        return value;
      },
      (error) => {
        settled = true;
        throw error;
      },
    );

    await new Promise((resolve) => setTimeout(resolve, 200));
    expect(settled).toBe(false);

    const fresh = freshAccessToken();
    writeAuth(authPath, fresh, 'refresh-keep', 'id-keep');
    await lock.release();

    const refreshed = await pending;
    expect(refreshed.accessToken).toBe(fresh);
    const saved = readAuth(authPath);
    expect(saved.tokens.refresh_token).toBe('refresh-keep');
    expect(saved.last_refresh).toBe('2020-01-01T00:00:00Z');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('failed atomic write does not return the new token or retry', async () => {
  const { dir, authPath } = tempAuthPath();
  const oldAccess = expiredAccessToken();
  let calls = 0;
  try {
    writeAuth(authPath, oldAccess, 'old-refresh', 'old-id');
    const fetchImpl = async () => {
      calls += 1;
      expect(calls).toBe(1);
      await chmod(dir, 0o555);
      return Response.json({
        access_token: 'new-access',
        refresh_token: 'new-refresh',
        id_token: 'new-id',
      });
    };

    const error = await refreshPersistedCodexAuth(authPath, fetchImpl).then(
      () => {
        throw new Error('expected persist failure');
      },
      (caught: unknown) => caught,
    );
    expect(error).toBeInstanceOf(CodexAuthPersistError);
    expect(error instanceof Error ? error.message : '').not.toContain('new-access');
    expect(calls).toBe(1);
    await chmod(dir, 0o755);
    const saved = readAuth(authPath);
    expect(saved.tokens.access_token).toBe(oldAccess);
    expect(saved.tokens.refresh_token).toBe('old-refresh');
    expect(saved.tokens.id_token).toBe('old-id');
    expect(saved.last_refresh).toBe('2020-01-01T00:00:00Z');
    expect(readdirSync(dir).filter((name) => name.includes('.tmp'))).toEqual([]);
  } finally {
    await chmod(dir, 0o755);
    rmSync(dir, { recursive: true, force: true });
  }
});

test('token endpoint failure leaves auth.json unchanged', async () => {
  const { dir, authPath } = tempAuthPath();
  const oldAccess = expiredAccessToken();
  let calls = 0;
  try {
    writeAuth(authPath, oldAccess, 'old-refresh', 'old-id');
    const refreshed = await refreshPersistedCodexAuth(authPath, async () => {
      calls += 1;
      return new Response('nope', { status: 401 });
    });
    expect(calls).toBe(1);
    expect(refreshed.accessToken).toBe(oldAccess);
    expect(refreshed.idToken).toBe('old-id');
    const saved = readAuth(authPath);
    expect(saved.tokens.refresh_token).toBe('old-refresh');
    expect(saved.last_refresh).toBe('2020-01-01T00:00:00Z');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('flock retries only contention and interruption', async () => {
  expect(isRetryableFlockErrno(4)).toBe(true);
  expect(isRetryableFlockErrno(9)).toBe(false);
  if (process.platform === 'linux') {
    expect(isRetryableFlockErrno(11)).toBe(true);
    expect(isRetryableFlockErrno(35)).toBe(false);
  } else {
    expect(isRetryableFlockErrno(35)).toBe(true);
    expect(isRetryableFlockErrno(11)).toBe(false);
  }

  const { dir, authPath } = tempAuthPath();
  const oldAccess = expiredAccessToken();
  try {
    writeAuth(authPath, oldAccess, 'old-refresh', 'old-id');
    const retryable = process.platform === 'linux' ? [11, 4] : [35, 4];
    let attempts = 0;
    setCodexAuthFlockAttemptForTests(() => {
      const errno = retryable[attempts];
      attempts += 1;
      if (errno === undefined) return { rc: 0, errno: 0 };
      return { rc: -1, errno };
    });
    let calls = 0;
    const refreshed = await refreshPersistedCodexAuth(authPath, async () => {
      calls += 1;
      return Response.json({
        access_token: 'new-access',
        refresh_token: 'new-refresh',
        id_token: 'new-id',
      });
    });
    expect(calls).toBe(1);
    expect(refreshed.accessToken).toBe('new-access');
    expect(attempts).toBe(4);

    attempts = 0;
    setCodexAuthFlockAttemptForTests(() => {
      attempts += 1;
      return { rc: -1, errno: 9 };
    });
    const started = Date.now();
    let posts = 0;
    const error = await Promise.race([
      refreshPersistedCodexAuth(authPath, async () => {
        posts += 1;
        return Response.json({ access_token: 'should-not-post' });
      }).then(
        () => new Error('expected lock failure'),
        (caught: unknown) => caught,
      ),
      new Promise((resolve) => {
        setTimeout(() => resolve(new Error('lock retry did not stop')), 500);
      }),
    ]);
    expect(Date.now() - started).toBeLessThan(500);
    expect(error).toBeInstanceOf(Error);
    expect(error instanceof Error ? error.message : '').toContain('errno 9');
    expect(posts).toBe(0);
    expect(attempts).toBe(1);
    const saved = readAuth(authPath);
    expect(saved.tokens.access_token).toBe('new-access');
    expect(saved.tokens.refresh_token).toBe('new-refresh');
  } finally {
    setCodexAuthFlockAttemptForTests(undefined);
    rmSync(dir, { recursive: true, force: true });
  }
});

function expectNoRefreshLock(dir: string): void {
  expect(readdirSync(dir).filter((name) => name.includes('refresh.lock'))).toEqual([]);
}

test('non-posix platform serves a fresh token without locking or posting', async () => {
  const { dir, authPath } = tempAuthPath();
  try {
    setCodexAuthPlatformForTests('win32');
    const fresh = freshAccessToken();
    writeAuth(authPath, fresh, 'refresh-keep', 'id-keep');
    let calls = 0;
    const refreshed = await refreshPersistedCodexAuth(authPath, async () => {
      calls += 1;
      throw new Error('token endpoint must not be called');
    });
    expect(calls).toBe(0);
    expect(refreshed).toEqual({ accessToken: fresh, idToken: 'id-keep' });
    const saved = readAuth(authPath);
    expect(saved.tokens.refresh_token).toBe('refresh-keep');
    expect(saved.last_refresh).toBe('2020-01-01T00:00:00Z');
    expectNoRefreshLock(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('non-posix platform serves an access token when no refresh token is stored', async () => {
  const { dir, authPath } = tempAuthPath();
  try {
    setCodexAuthPlatformForTests('win32');
    const expired = expiredAccessToken();
    writeFileSync(authPath, `${JSON.stringify({
      auth_mode: 'chatgpt',
      tokens: {
        access_token: expired,
        id_token: 'id-keep',
      },
      last_refresh: '2020-01-01T00:00:00Z',
    }, null, 2)}\n`);
    let calls = 0;
    const refreshed = await refreshPersistedCodexAuth(authPath, async () => {
      calls += 1;
      throw new Error('token endpoint must not be called');
    });
    expect(calls).toBe(0);
    expect(refreshed).toEqual({ accessToken: expired, idToken: 'id-keep' });
    expectNoRefreshLock(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('non-posix platform fails before the token post when a refresh is required', async () => {
  const { dir, authPath } = tempAuthPath();
  const oldAccess = expiredAccessToken();
  try {
    setCodexAuthPlatformForTests('win32');
    writeAuth(authPath, oldAccess, 'old-refresh', 'old-id');
    let calls = 0;
    const error = await refreshPersistedCodexAuth(authPath, async () => {
      calls += 1;
      return Response.json({
        access_token: 'new-access',
        refresh_token: 'new-refresh',
        id_token: 'new-id',
      });
    }).then(
      () => {
        throw new Error('expected refresh to fail before the token post');
      },
      (caught: unknown) => caught,
    );
    expect(calls).toBe(0);
    expect(error).toBeInstanceOf(Error);
    expect(error instanceof CodexAuthPersistError).toBe(false);
    expect(error instanceof Error ? error.message : '').toBe(
      'Codex auth refresh cannot be persisted on win32',
    );
    const saved = readAuth(authPath);
    expect(saved.tokens.access_token).toBe(oldAccess);
    expect(saved.tokens.refresh_token).toBe('old-refresh');
    expect(saved.tokens.id_token).toBe('old-id');
    expect(saved.last_refresh).toBe('2020-01-01T00:00:00Z');
    expectNoRefreshLock(dir);

    const soon = jwtWithExp(Math.floor(Date.now() / 1000) + 30);
    writeAuth(authPath, soon, 'old-refresh', 'old-id');
    calls = 0;
    const soonError = await refreshPersistedCodexAuth(authPath, async () => {
      calls += 1;
      return Response.json({ access_token: 'new-access' });
    }).then(
      () => {
        throw new Error('expected the 60-second window to fail closed');
      },
      (caught: unknown) => caught,
    );
    expect(calls).toBe(0);
    expect(soonError instanceof Error ? soonError.message : '').toBe(
      'Codex auth refresh cannot be persisted on win32',
    );
    expect(readAuth(authPath).tokens.access_token).toBe(soon);
    expectNoRefreshLock(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('stalled token refresh aborts and releases the lock without a second post', async () => {
  const { dir, authPath } = tempAuthPath();
  const oldAccess = expiredAccessToken();
  try {
    writeAuth(authPath, oldAccess, 'old-refresh', 'old-id');
    let calls = 0;
    const refreshed = await refreshPersistedCodexAuth(authPath, (_input, init) => {
      calls += 1;
      const signal = init?.signal;
      if (!signal) return Promise.reject(new Error('missing abort signal'));
      return new Promise<Response>((_resolve, reject) => {
        const abort = () => reject(new Error('aborted'));
        if (signal.aborted) {
          abort();
          return;
        }
        signal.addEventListener('abort', abort, { once: true });
      });
    }, 40);
    expect(calls).toBe(1);
    expect(refreshed.accessToken).toBe(oldAccess);

    let secondCalls = 0;
    const second = await Promise.race([
      refreshPersistedCodexAuth(authPath, async () => {
        secondCalls += 1;
        return new Response('nope', { status: 401 });
      }),
      new Promise<never>((_resolve, reject) => {
        setTimeout(() => reject(new Error('lock still held')), 1000);
      }),
    ]);
    expect(secondCalls).toBe(1);
    expect(second.accessToken).toBe(oldAccess);
    const saved = readAuth(authPath);
    expect(saved.tokens.refresh_token).toBe('old-refresh');
    expect(saved.last_refresh).toBe('2020-01-01T00:00:00Z');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

for (const scenario of ['override', 'fallback', 'empty', 'explicit', 'missing', 'no-home', 'switch'] as const) {
  test(`quota route resolves the active auth file: ${scenario}`, async () => {
    const dir = mkdtempSync(join(tmpdir(), 'codex-quota-home-'));
    const defaultPath = join(dir, '.codex', 'auth.json');
    const customPath = join(dir, 'custom-codex', 'auth.json');
    const explicitPath = join(dir, 'explicit', 'auth.json');
    try {
      for (const authPath of [defaultPath, customPath, explicitPath]) {
        mkdirSync(join(authPath, '..'), { recursive: true });
        writeAuth(authPath, expiredAccessToken(), 'dummy-shared-refresh', 'dummy-id');
      }
      const env: Record<string, string | undefined> = { ...process.env, HOME: dir, KEEPLINE_HOME: join(dir, 'keepline') };
      delete env.CODEX_AUTH_PATH;
      delete env.CODEX_HOME;
      if (['override', 'explicit', 'switch'].includes(scenario)) env.CODEX_HOME = join(dir, 'custom-codex');
      if (scenario === 'empty') env.CODEX_HOME = '';
      if (scenario === 'explicit') env.CODEX_AUTH_PATH = explicitPath;
      if (scenario === 'missing') env.CODEX_HOME = join(dir, 'absent');
      if (scenario === 'no-home') delete env.HOME;
      const child = Bun.spawn([process.execPath, '-e', `
        import { expect } from 'bun:test';
        import usage from './src/web/api/routes/usage.ts';
        import { setupUser } from './src/services/auth.service.ts';
        import { runMigrations } from './src/db/migrations.ts';
        runMigrations();
        const scenario = ${JSON.stringify(scenario)};
        const paths = ${JSON.stringify([defaultPath, customPath, explicitPath])};
        let refreshCalls = 0;
        let usageCalls = 0;
        globalThis.fetch = async (input, init) => {
          if (String(input) === ${JSON.stringify(TOKEN_URL)}) {
            refreshCalls++;
            expect(JSON.parse(String(init?.body)).refresh_token).toBe('dummy-shared-refresh');
            return Response.json({ access_token: 'dummy-new-access', refresh_token: 'dummy-rotated-refresh' });
          }
          expect(String(input)).toBe('https://chatgpt.com/backend-api/wham/usage');
          usageCalls++;
          expect(init.headers.Authorization).toBe('Bearer dummy-new-access');
          return Response.json({ plan_type: 'dummy-' + usageCalls });
        };
        const { token } = await setupUser('dummy-user', 'dummy-password');
        const request = () => usage.request('/codex/quota', { headers: { Authorization: 'Bearer ' + token } });
        const before = await Promise.all(paths.map(path => Bun.file(path).text()));
        const response = await request();
        expect(response.status).toBe(scenario === 'missing' ? 404 : scenario === 'no-home' ? 500 : 200);
        const selected = scenario === 'explicit' ? 2 : ['override', 'switch'].includes(scenario) ? 1 : 0;
        for (let i = 0; i < paths.length; i++) {
          const contents = await Bun.file(paths[i]).text();
          if (['missing', 'no-home'].includes(scenario) || i !== selected) {
            expect(contents).toBe(before[i]);
            expect(await Bun.file(paths[i] + '.refresh.lock').exists()).toBe(false);
          } else {
            expect(JSON.parse(contents).tokens.refresh_token).toBe('dummy-rotated-refresh');
          }
        }
        if (scenario === 'switch') {
          const first = await response.json();
          delete process.env.CODEX_HOME;
          const second = await request();
          expect(second.status).toBe(200);
          expect((await second.json()).data.plan_type).not.toBe(first.data.plan_type);
          expect(JSON.parse(await Bun.file(paths[0]).text()).tokens.refresh_token).toBe('dummy-rotated-refresh');
        }
        expect(refreshCalls).toBe(['missing', 'no-home'].includes(scenario) ? 0 : scenario === 'switch' ? 2 : 1);
        expect(usageCalls).toBe(refreshCalls);
      `], { cwd: process.cwd(), env, stdout: 'pipe', stderr: 'pipe' });
      const [exitCode, stdout, stderr] = await Promise.all([
        child.exited, new Response(child.stdout).text(), new Response(child.stderr).text(),
      ]);
      expect({ exitCode, output: exitCode === 0 ? '' : stdout + stderr }).toEqual({ exitCode: 0, output: '' });
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
}
