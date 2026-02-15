/**
 * User Chips RPC Functions
 * Manages user chip balances using Nakama Storage
 */

// Storage collection and key for user chips
const CHIPS_COLLECTION = 'user_data';
const CHIPS_KEY = 'chips';

// Default starting chips for new users
const DEFAULT_STARTING_CHIPS = 10000;

// Minimum buy-in for tables
const MIN_BUY_IN = 100;

interface UserChipsData {
  balance: number;
  totalWon: number;
  totalLost: number;
  handsPlayed: number;
  handsWon: number;
  lastUpdated: number;
  createdAt: number;
}

interface GetChipsResponse {
  balance: number;
  totalWon: number;
  totalLost: number;
  handsPlayed: number;
  handsWon: number;
}

interface UpdateChipsRequest {
  amount: number;
  reason: 'buy_in' | 'cash_out' | 'win' | 'lose' | 'bonus' | 'daily_reward';
}

interface UpdateChipsResponse {
  balance: number;
  previousBalance: number;
  change: number;
}

interface DailyRewardResponse {
  rewarded: boolean;
  amount: number;
  balance: number;
  nextRewardTime: number;
  message: string;
}

// Helper to get or initialize user chips
function getUserChips(
  nk: nkruntime.Nakama,
  userId: string,
  logger: nkruntime.Logger
): UserChipsData {
  const objects = nk.storageRead([
    {
      collection: CHIPS_COLLECTION,
      key: CHIPS_KEY,
      userId: userId,
    },
  ]);

  if (objects.length > 0 && objects[0].value) {
    return objects[0].value as UserChipsData;
  }

  // Initialize new user with starting chips
  const now = Date.now();
  const initialData: UserChipsData = {
    balance: DEFAULT_STARTING_CHIPS,
    totalWon: 0,
    totalLost: 0,
    handsPlayed: 0,
    handsWon: 0,
    lastUpdated: now,
    createdAt: now,
  };

  nk.storageWrite([
    {
      collection: CHIPS_COLLECTION,
      key: CHIPS_KEY,
      userId: userId,
      value: initialData,
      permissionRead: 1, // Owner read
      permissionWrite: 0, // No client write
    },
  ]);

  logger.info(`Initialized new user ${userId} with ${DEFAULT_STARTING_CHIPS} chips`);
  return initialData;
}

// Helper to save user chips
function saveUserChips(
  nk: nkruntime.Nakama,
  userId: string,
  data: UserChipsData
): void {
  data.lastUpdated = Date.now();

  nk.storageWrite([
    {
      collection: CHIPS_COLLECTION,
      key: CHIPS_KEY,
      userId: userId,
      value: data,
      permissionRead: 1,
      permissionWrite: 0,
    },
  ]);
}

/**
 * Get user's chip balance and stats
 */
export const getChipsRpc: nkruntime.RpcFunction = (
  ctx: nkruntime.Context,
  logger: nkruntime.Logger,
  nk: nkruntime.Nakama,
  _payload: string
): string => {
  const userId = ctx.userId;
  if (!userId) {
    throw new Error('User not authenticated');
  }

  const chipsData = getUserChips(nk, userId, logger);

  const response: GetChipsResponse = {
    balance: chipsData.balance,
    totalWon: chipsData.totalWon,
    totalLost: chipsData.totalLost,
    handsPlayed: chipsData.handsPlayed,
    handsWon: chipsData.handsWon,
  };

  return JSON.stringify(response);
};

/**
 * Update user's chip balance (server-initiated)
 * This is called internally by match handlers, not directly by clients
 */
export const updateChipsRpc: nkruntime.RpcFunction = (
  ctx: nkruntime.Context,
  logger: nkruntime.Logger,
  nk: nkruntime.Nakama,
  payload: string
): string => {
  const userId = ctx.userId;
  if (!userId) {
    throw new Error('User not authenticated');
  }

  let request: UpdateChipsRequest;
  try {
    request = JSON.parse(payload);
  } catch {
    throw new Error('Invalid request payload');
  }

  if (typeof request.amount !== 'number') {
    throw new Error('Amount must be a number');
  }

  const chipsData = getUserChips(nk, userId, logger);
  const previousBalance = chipsData.balance;

  // Update based on reason
  switch (request.reason) {
    case 'buy_in':
      if (request.amount < MIN_BUY_IN) {
        throw new Error(`Minimum buy-in is ${MIN_BUY_IN} chips`);
      }
      if (chipsData.balance < request.amount) {
        throw new Error('Insufficient chips for buy-in');
      }
      chipsData.balance -= request.amount;
      break;

    case 'cash_out':
      if (request.amount < 0) {
        throw new Error('Cash out amount must be positive');
      }
      chipsData.balance += request.amount;
      break;

    case 'win':
      chipsData.balance += request.amount;
      chipsData.totalWon += request.amount;
      chipsData.handsWon += 1;
      break;

    case 'lose':
      chipsData.totalLost += Math.abs(request.amount);
      break;

    case 'bonus':
    case 'daily_reward':
      chipsData.balance += request.amount;
      break;

    default:
      throw new Error('Invalid reason');
  }

  // Increment hands played for game-related actions
  if (request.reason === 'win' || request.reason === 'lose') {
    chipsData.handsPlayed += 1;
  }

  saveUserChips(nk, userId, chipsData);
  logger.info(`User ${userId} chips updated: ${previousBalance} -> ${chipsData.balance} (${request.reason}: ${request.amount})`);

  const response: UpdateChipsResponse = {
    balance: chipsData.balance,
    previousBalance,
    change: chipsData.balance - previousBalance,
  };

  return JSON.stringify(response);
};

/**
 * Claim daily reward
 * Players can claim once every 24 hours
 */
export const claimDailyRewardRpc: nkruntime.RpcFunction = (
  ctx: nkruntime.Context,
  logger: nkruntime.Logger,
  nk: nkruntime.Nakama,
  _payload: string
): string => {
  const userId = ctx.userId;
  if (!userId) {
    throw new Error('User not authenticated');
  }

  // Daily reward storage
  const DAILY_REWARD_KEY = 'daily_reward';
  const DAILY_REWARD_AMOUNT = 500;
  const REWARD_COOLDOWN_MS = 24 * 60 * 60 * 1000; // 24 hours

  // Check last claim time
  const rewardObjects = nk.storageRead([
    {
      collection: CHIPS_COLLECTION,
      key: DAILY_REWARD_KEY,
      userId: userId,
    },
  ]);

  const now = Date.now();
  let lastClaimTime = 0;

  if (rewardObjects.length > 0 && rewardObjects[0].value) {
    lastClaimTime = (rewardObjects[0].value as { lastClaim: number }).lastClaim || 0;
  }

  const timeSinceLastClaim = now - lastClaimTime;
  const nextRewardTime = lastClaimTime + REWARD_COOLDOWN_MS;

  if (timeSinceLastClaim < REWARD_COOLDOWN_MS) {
    const hoursRemaining = Math.ceil((REWARD_COOLDOWN_MS - timeSinceLastClaim) / (60 * 60 * 1000));

    const chipsData = getUserChips(nk, userId, logger);
    const response: DailyRewardResponse = {
      rewarded: false,
      amount: 0,
      balance: chipsData.balance,
      nextRewardTime,
      message: `Come back in ${hoursRemaining} hour(s) for your daily reward!`,
    };
    return JSON.stringify(response);
  }

  // Give the reward
  const chipsData = getUserChips(nk, userId, logger);
  chipsData.balance += DAILY_REWARD_AMOUNT;
  saveUserChips(nk, userId, chipsData);

  // Update last claim time
  nk.storageWrite([
    {
      collection: CHIPS_COLLECTION,
      key: DAILY_REWARD_KEY,
      userId: userId,
      value: { lastClaim: now },
      permissionRead: 1,
      permissionWrite: 0,
    },
  ]);

  logger.info(`User ${userId} claimed daily reward: ${DAILY_REWARD_AMOUNT} chips`);

  const response: DailyRewardResponse = {
    rewarded: true,
    amount: DAILY_REWARD_AMOUNT,
    balance: chipsData.balance,
    nextRewardTime: now + REWARD_COOLDOWN_MS,
    message: `You received ${DAILY_REWARD_AMOUNT} chips!`,
  };

  return JSON.stringify(response);
};

/**
 * Get leaderboard data
 * Returns top players by total chips won
 */
export const getLeaderboardRpc: nkruntime.RpcFunction = (
  ctx: nkruntime.Context,
  logger: nkruntime.Logger,
  nk: nkruntime.Nakama,
  payload: string
): string => {
  let limit = 10;

  try {
    const request = JSON.parse(payload || '{}');
    if (request.limit && typeof request.limit === 'number') {
      limit = Math.min(Math.max(request.limit, 1), 100);
    }
  } catch {
    // Use default limit
  }

  // Use Nakama leaderboard for rankings
  const LEADERBOARD_ID = 'poker_total_won';

  // Ensure leaderboard exists
  try {
    nk.leaderboardCreate(
      LEADERBOARD_ID,
      true, // authoritative
      'best', // sort - highest score wins
      'set', // operator - set score directly
      'alltime', // reset schedule - never reset
      undefined // metadata
    );
  } catch {
    // Leaderboard already exists
  }

  // Get leaderboard records
  const result = nk.leaderboardRecordsList(
    LEADERBOARD_ID,
    [], // owner IDs (empty = all)
    limit,
    undefined, // cursor
    0 // expiry (0 = no expiry filter)
  );

  const leaderboard = (result.records || []).map((record, index) => ({
    rank: index + 1,
    odid: record.ownerId,
    username: record.username || 'Unknown',
    totalWon: record.score,
  }));

  return JSON.stringify({ leaderboard });
};

/**
 * Update leaderboard score (called after winning)
 */
export function updateLeaderboardScore(
  nk: nkruntime.Nakama,
  userId: string,
  totalWon: number,
  logger: nkruntime.Logger
): void {
  const LEADERBOARD_ID = 'poker_total_won';

  try {
    nk.leaderboardRecordWrite(
      LEADERBOARD_ID,
      userId,
      undefined, // username (will use account username)
      totalWon,
      0, // subscore
      undefined // metadata
    );
    logger.debug(`Updated leaderboard for user ${userId}: ${totalWon}`);
  } catch (e) {
    logger.error(`Failed to update leaderboard: ${e}`);
  }
}
