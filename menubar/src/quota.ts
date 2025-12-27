/**
 * Claude Quota Data Fetcher
 *
 * Reads OAuth token from macOS Keychain and fetches usage data from Anthropic API
 */

import { exec } from 'child_process';
import { promisify } from 'util';

const execAsync = promisify(exec);

export interface QuotaLimit {
  used: number;
  limit: number;
  remaining: number;
  percentage: number;
  resetsAt: Date;
  resetsIn: string; // Human readable, e.g., "2h 53m"
}

export interface QuotaData {
  connected: boolean;
  session?: QuotaLimit;      // 5-hour session limit
  weeklyTotal?: QuotaLimit;  // 7-day total limit
  weeklyOpus?: QuotaLimit;   // 7-day Opus limit
  weeklySonnet?: QuotaLimit; // 7-day Sonnet limit
  lastUpdated: Date;
  error?: string;
}

interface KeychainCredentials {
  claudeAiOauth?: {
    accessToken?: string;
  };
}

interface UsageEntry {
  utilization: number;  // percentage (0-100)
  resets_at: string;    // ISO8601 timestamp
}

interface AnthropicUsageResponse {
  // 5-hour session limit
  five_hour?: UsageEntry | null;

  // 7-day total limit
  seven_day?: UsageEntry | null;

  // 7-day OAuth apps limit
  seven_day_oauth_apps?: UsageEntry | null;

  // 7-day Opus limit
  seven_day_opus?: UsageEntry | null;

  // 7-day Sonnet limit
  seven_day_sonnet?: UsageEntry | null;

  // Extra usage info
  extra_usage?: {
    is_enabled: boolean;
    monthly_limit: number | null;
    used_credits: number | null;
    utilization: number | null;
  } | null;

  // Error response
  error?: {
    type: string;
    message: string;
  };
}

const CREDENTIAL_NAMES = [
  'Claude Code-credentials',
  'claude-credentials',
  'Claude-credentials',
  'claudecode-credentials',
];

const ANTHROPIC_USAGE_API = 'https://api.anthropic.com/api/oauth/usage';

/**
 * Read OAuth token from macOS Keychain
 */
async function getOAuthToken(): Promise<string | null> {
  for (const credName of CREDENTIAL_NAMES) {
    try {
      const { stdout } = await execAsync(
        `security find-generic-password -s "${credName}" -w 2>/dev/null`
      );

      const trimmed = stdout.trim();
      if (!trimmed) continue;

      const credentials: KeychainCredentials = JSON.parse(trimmed);
      const token = credentials.claudeAiOauth?.accessToken;

      if (token) {
        return token;
      }
    } catch {
      // Try next credential name
      continue;
    }
  }

  return null;
}

/**
 * Calculate human-readable time remaining
 */
function formatTimeRemaining(resetDate: Date): string {
  const now = new Date();
  const diffMs = resetDate.getTime() - now.getTime();

  if (diffMs <= 0) return 'Now';

  const diffMinutes = Math.floor(diffMs / (1000 * 60));
  const diffHours = Math.floor(diffMinutes / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffDays > 0) {
    const remainingHours = diffHours % 24;
    return `${diffDays}d ${remainingHours}h`;
  }

  if (diffHours > 0) {
    const remainingMinutes = diffMinutes % 60;
    return `${diffHours}h ${remainingMinutes}m`;
  }

  return `${diffMinutes}m`;
}

/**
 * Create a QuotaLimit object from utilization percentage
 */
function createQuotaLimitFromUtilization(
  utilization: number,
  resetsAtStr: string
): QuotaLimit {
  const resetsAt = new Date(resetsAtStr);
  const percentage = Math.round(utilization);

  return {
    used: percentage,
    limit: 100,
    remaining: Math.max(0, 100 - percentage),
    percentage,
    resetsAt,
    resetsIn: formatTimeRemaining(resetsAt),
  };
}

/**
 * Fetch quota data from Anthropic API
 */
export async function getQuotaData(): Promise<QuotaData> {
  const token = await getOAuthToken();

  if (!token) {
    return {
      connected: false,
      lastUpdated: new Date(),
      error: 'OAuth token not found. Please log into Claude Code first.',
    };
  }

  try {
    // Use curl for better compatibility with Anthropic's security
    const curlCommand = `curl -s \
      -H "Accept: application/json" \
      -H "Authorization: Bearer ${token}" \
      -H "anthropic-beta: oauth-2025-04-20" \
      "${ANTHROPIC_USAGE_API}"`;

    const { stdout } = await execAsync(curlCommand);
    const response: AnthropicUsageResponse = JSON.parse(stdout);

    // Check for API error
    if (response.error) {
      return {
        connected: false,
        lastUpdated: new Date(),
        error: `${response.error.message} (${response.error.type})`,
      };
    }

    const quotaData: QuotaData = {
      connected: true,
      lastUpdated: new Date(),
    };

    // Parse 5-hour session limit
    if (response.five_hour) {
      quotaData.session = createQuotaLimitFromUtilization(
        response.five_hour.utilization,
        response.five_hour.resets_at
      );
    }

    // Parse 7-day total limit
    if (response.seven_day) {
      quotaData.weeklyTotal = createQuotaLimitFromUtilization(
        response.seven_day.utilization,
        response.seven_day.resets_at
      );
    }

    // Parse 7-day Opus limit
    if (response.seven_day_opus) {
      quotaData.weeklyOpus = createQuotaLimitFromUtilization(
        response.seven_day_opus.utilization,
        response.seven_day_opus.resets_at
      );
    }

    // Parse 7-day Sonnet limit
    if (response.seven_day_sonnet) {
      quotaData.weeklySonnet = createQuotaLimitFromUtilization(
        response.seven_day_sonnet.utilization,
        response.seven_day_sonnet.resets_at
      );
    }

    return quotaData;
  } catch (error) {
    return {
      connected: false,
      lastUpdated: new Date(),
      error: error instanceof Error ? error.message : 'Failed to fetch quota data',
    };
  }
}
