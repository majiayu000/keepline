/**
 * Security helpers for the browser-backed terminal endpoint.
 */

export function getAllowedTerminalOrigins(
  hostname: string,
  port: number,
  tlsEnabled: boolean,
  configuredOrigins: string[] = getConfiguredAllowedOrigins(),
): Set<string> {
  const allowed = new Set<string>();
  const protocol = tlsEnabled ? 'https' : 'http';
  addOrigin(allowed, protocol, hostname, port);

  if (isLoopbackHost(hostname) || hostname === '0.0.0.0' || hostname === '::') {
    addOrigin(allowed, protocol, '127.0.0.1', port);
    addOrigin(allowed, protocol, 'localhost', port);
    addOrigin(allowed, protocol, '[::1]', port);
  }

  for (const origin of configuredOrigins) {
    addConfiguredOrigin(allowed, origin);
  }

  return allowed;
}

export function isAllowedTerminalOrigin(
  req: Request,
  hostname: string,
  port: number,
  tlsEnabled: boolean,
  configuredOrigins?: string[],
): boolean {
  const origin = req.headers.get('origin');
  if (!origin) return false;

  let normalizedOrigin: string;
  try {
    normalizedOrigin = new URL(origin).origin;
  } catch {
    return false;
  }

  return getAllowedTerminalOrigins(hostname, port, tlsEnabled, configuredOrigins).has(normalizedOrigin);
}

function addOrigin(allowed: Set<string>, protocol: string, hostname: string, port: number) {
  const host = hostname.includes(':') && !hostname.startsWith('[') ? `[${hostname}]` : hostname;
  allowed.add(new URL(`${protocol}://${host}:${port}`).origin);
}

function addConfiguredOrigin(allowed: Set<string>, origin: string) {
  try {
    allowed.add(new URL(origin).origin);
  } catch {
    // Ignore malformed opt-in origins rather than widening the allowlist.
  }
}

export function getConfiguredAllowedOrigins(): string[] {
  const values = [
    process.env.KEEPLINE_PUBLIC_ORIGIN,
    process.env.KEEPLINE_ALLOWED_ORIGINS,
  ];
  return values
    .flatMap((value) => (value ?? '').split(','))
    .map((value) => value.trim())
    .filter(Boolean);
}

export function isLoopbackHost(hostname: string): boolean {
  return hostname === '127.0.0.1' || hostname === 'localhost' || hostname === '::1' || hostname === '[::1]';
}
