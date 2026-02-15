/**
 * Nakama Server Entry Point
 * Registers match handlers and RPC functions for Texas Hold'em Poker
 */

import { pokerMatchHandler } from './poker/match_handler';
import { findMatchRpc, createPrivateMatchRpc, listRoomsRpc } from './rpc/find_match';
import { getChipsRpc, updateChipsRpc, claimDailyRewardRpc, getLeaderboardRpc } from './rpc/user_chips';

// Module name for the poker match handler
const POKER_MATCH_MODULE = 'poker';

/**
 * Initialize Nakama module
 */
function InitModule(ctx: nkruntime.Context, logger: nkruntime.Logger, nk: nkruntime.Nakama, initializer: nkruntime.Initializer): void {
  logger.info('Poker server initializing...');

  // Register the poker match handler
  initializer.registerMatch(POKER_MATCH_MODULE, pokerMatchHandler);
  logger.info('Registered poker match handler');

  // Register match RPC functions
  initializer.registerRpc('find_match', findMatchRpc);
  initializer.registerRpc('create_private_match', createPrivateMatchRpc);
  initializer.registerRpc('list_rooms', listRoomsRpc);
  logger.info('Registered match RPC functions');

  // Register user chips RPC functions
  initializer.registerRpc('get_chips', getChipsRpc);
  initializer.registerRpc('update_chips', updateChipsRpc);
  initializer.registerRpc('claim_daily_reward', claimDailyRewardRpc);
  initializer.registerRpc('get_leaderboard', getLeaderboardRpc);
  logger.info('Registered user chips RPC functions');

  logger.info('Poker server initialized successfully!');
}

// Export for Nakama runtime - must use globalThis for esbuild bundling
// @ts-ignore - Nakama runtime expects this global
(globalThis as any).InitModule = InitModule;
