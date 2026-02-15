/**
 * Betting Logic
 * Handles player actions: fold, check, call, bet, raise, all-in
 */

import {
  GameState,
  Player,
  PlayerAction,
  PlayerStatus,
} from './types';
import {
  getAvailableActions,
  getBetLimits,
  getTotalPot,
} from './game_state';

/**
 * Result of a player action
 */
export interface ActionResult {
  success: boolean;
  error?: string;
  action?: PlayerAction;
  amount?: number;
  newChips?: number;
  newPotTotal?: number;
  playerAllIn?: boolean;
}

/**
 * Validate and execute a player action
 */
export function executePlayerAction(
  state: GameState,
  playerId: string,
  action: PlayerAction,
  amount?: number
): ActionResult {
  const player = state.players[playerId];

  if (!player) {
    return { success: false, error: 'Player not found' };
  }

  // Check if it's this player's turn
  if (player.seatIndex !== state.currentPlayerSeatIndex) {
    return { success: false, error: 'Not your turn' };
  }

  // Check if player can act
  if (player.status !== PlayerStatus.Active) {
    return { success: false, error: 'Player cannot act' };
  }

  // Validate action is available
  const availableActions = getAvailableActions(state, playerId);
  if (!availableActions.includes(action)) {
    return { success: false, error: `Action ${action} not available. Available: ${availableActions.join(', ')}` };
  }

  // Execute the action
  switch (action) {
    case PlayerAction.Fold:
      return executeFold(state, player);
    case PlayerAction.Check:
      return executeCheck(state, player);
    case PlayerAction.Call:
      return executeCall(state, player);
    case PlayerAction.Bet:
      return executeBet(state, player, amount);
    case PlayerAction.Raise:
      return executeRaise(state, player, amount);
    case PlayerAction.AllIn:
      return executeAllIn(state, player);
    default:
      return { success: false, error: 'Unknown action' };
  }
}

/**
 * Execute fold action
 */
function executeFold(state: GameState, player: Player): ActionResult {
  player.status = PlayerStatus.Folded;
  player.hasActed = true;
  player.lastAction = PlayerAction.Fold;
  state.actionsThisRound++;

  return {
    success: true,
    action: PlayerAction.Fold,
    amount: 0,
    newChips: player.chips,
    newPotTotal: getTotalPot(state),
    playerAllIn: false
  };
}

/**
 * Execute check action
 */
function executeCheck(state: GameState, player: Player): ActionResult {
  // Can only check if no bet to match
  const toCall = state.currentBet - player.currentBet;
  if (toCall > 0) {
    return { success: false, error: 'Cannot check, there is a bet to match' };
  }

  player.hasActed = true;
  player.lastAction = PlayerAction.Check;
  state.actionsThisRound++;

  return {
    success: true,
    action: PlayerAction.Check,
    amount: 0,
    newChips: player.chips,
    newPotTotal: getTotalPot(state),
    playerAllIn: false
  };
}

/**
 * Execute call action
 */
function executeCall(state: GameState, player: Player): ActionResult {
  const toCall = state.currentBet - player.currentBet;

  if (toCall <= 0) {
    return { success: false, error: 'Nothing to call' };
  }

  // Calculate actual call amount (might be less if player doesn't have enough)
  const actualCall = Math.min(toCall, player.chips);
  const isAllIn = actualCall === player.chips && player.chips < toCall;

  // Execute the call
  player.chips -= actualCall;
  player.currentBet += actualCall;
  player.totalBetThisHand += actualCall;
  state.pots[0].amount += actualCall;

  player.hasActed = true;
  player.lastAction = PlayerAction.Call;
  state.actionsThisRound++;

  if (isAllIn || player.chips === 0) {
    player.status = PlayerStatus.AllIn;
  }

  return {
    success: true,
    action: isAllIn ? PlayerAction.AllIn : PlayerAction.Call,
    amount: actualCall,
    newChips: player.chips,
    newPotTotal: getTotalPot(state),
    playerAllIn: player.status === PlayerStatus.AllIn
  };
}

/**
 * Execute bet action (when no current bet)
 */
function executeBet(state: GameState, player: Player, amount?: number): ActionResult {
  if (state.currentBet > 0) {
    return { success: false, error: 'Cannot bet, there is already a bet. Use raise instead.' };
  }

  if (amount === undefined) {
    return { success: false, error: 'Bet amount required' };
  }

  const { min, max } = getBetLimits(state, player.odid);

  // Validate bet amount
  if (amount < min && amount !== player.chips) {
    return { success: false, error: `Minimum bet is ${min}` };
  }

  if (amount > max) {
    return { success: false, error: `Maximum bet is ${max}` };
  }

  // Execute the bet
  const actualBet = Math.min(amount, player.chips);
  player.chips -= actualBet;
  player.currentBet = actualBet;
  player.totalBetThisHand += actualBet;
  state.pots[0].amount += actualBet;

  // Update betting state
  state.currentBet = actualBet;
  state.lastRaiseAmount = actualBet;
  state.minRaise = actualBet;

  player.hasActed = true;
  player.lastAction = PlayerAction.Bet;
  state.actionsThisRound++;

  // Reset hasActed for other players (they need to respond to the bet)
  Object.values(state.players).forEach((p: Player) => {
    if (p.odid !== player.odid && p.status === PlayerStatus.Active) {
      p.hasActed = false;
    }
  });

  if (player.chips === 0) {
    player.status = PlayerStatus.AllIn;
  }

  return {
    success: true,
    action: player.chips === 0 ? PlayerAction.AllIn : PlayerAction.Bet,
    amount: actualBet,
    newChips: player.chips,
    newPotTotal: getTotalPot(state),
    playerAllIn: player.status === PlayerStatus.AllIn
  };
}

/**
 * Execute raise action
 */
function executeRaise(state: GameState, player: Player, amount?: number): ActionResult {
  if (state.currentBet === 0) {
    return { success: false, error: 'Cannot raise, no bet to raise. Use bet instead.' };
  }

  if (amount === undefined) {
    return { success: false, error: 'Raise amount required' };
  }

  const toCall = state.currentBet - player.currentBet;
  const { min, max } = getBetLimits(state, player.odid);

  // Amount is the total raise amount (including the call)
  const raiseAmount = amount - state.currentBet;

  // Validate raise amount
  if (raiseAmount < state.minRaise && amount !== player.chips + player.currentBet) {
    return { success: false, error: `Minimum raise is ${state.minRaise}. Total bet must be at least ${state.currentBet + state.minRaise}` };
  }

  const actualAmount = Math.min(amount - player.currentBet, player.chips);

  if (actualAmount <= 0) {
    return { success: false, error: 'Invalid raise amount' };
  }

  // Execute the raise
  player.chips -= actualAmount;
  player.currentBet += actualAmount;
  player.totalBetThisHand += actualAmount;
  state.pots[0].amount += actualAmount;

  // Update betting state
  const newCurrentBet = player.currentBet;
  state.lastRaiseAmount = newCurrentBet - state.currentBet;
  state.minRaise = state.lastRaiseAmount;
  state.currentBet = newCurrentBet;

  player.hasActed = true;
  player.lastAction = PlayerAction.Raise;
  state.actionsThisRound++;

  // Reset hasActed for other players (they need to respond to the raise)
  Object.values(state.players).forEach((p: Player) => {
    if (p.odid !== player.odid && p.status === PlayerStatus.Active) {
      p.hasActed = false;
    }
  });

  if (player.chips === 0) {
    player.status = PlayerStatus.AllIn;
  }

  return {
    success: true,
    action: player.chips === 0 ? PlayerAction.AllIn : PlayerAction.Raise,
    amount: actualAmount,
    newChips: player.chips,
    newPotTotal: getTotalPot(state),
    playerAllIn: player.status === PlayerStatus.AllIn
  };
}

/**
 * Execute all-in action
 */
function executeAllIn(state: GameState, player: Player): ActionResult {
  if (player.chips <= 0) {
    return { success: false, error: 'No chips to go all-in' };
  }

  const allInAmount = player.chips;
  const newBet = player.currentBet + allInAmount;

  // Execute all-in
  player.chips = 0;
  player.currentBet = newBet;
  player.totalBetThisHand += allInAmount;
  state.pots[0].amount += allInAmount;

  player.hasActed = true;
  player.lastAction = PlayerAction.AllIn;
  player.status = PlayerStatus.AllIn;
  state.actionsThisRound++;

  // Check if this is a raise
  if (newBet > state.currentBet) {
    const raiseAmount = newBet - state.currentBet;
    state.lastRaiseAmount = raiseAmount;
    state.minRaise = Math.max(state.minRaise, raiseAmount);
    state.currentBet = newBet;

    // Reset hasActed for other players if this was a raise
    Object.values(state.players).forEach((p: Player) => {
      if (p.odid !== player.odid && p.status === PlayerStatus.Active) {
        p.hasActed = false;
      }
    });
  }

  return {
    success: true,
    action: PlayerAction.AllIn,
    amount: allInAmount,
    newChips: 0,
    newPotTotal: getTotalPot(state),
    playerAllIn: true
  };
}

/**
 * Auto-fold a player (timeout)
 */
export function autoFold(state: GameState, playerId: string): ActionResult {
  const player = state.players[playerId];
  if (!player) {
    return { success: false, error: 'Player not found' };
  }

  return executeFold(state, player);
}

/**
 * Calculate the amount needed to call
 */
export function getCallAmount(state: GameState, playerId: string): number {
  const player = state.players[playerId];
  if (!player) return 0;

  return Math.min(state.currentBet - player.currentBet, player.chips);
}

/**
 * Get detailed action info for UI
 */
export function getActionInfo(state: GameState, playerId: string): {
  canCheck: boolean;
  canCall: boolean;
  canBet: boolean;
  canRaise: boolean;
  canAllIn: boolean;
  callAmount: number;
  minBet: number;
  maxBet: number;
  potTotal: number;
} {
  const player = state.players[playerId];
  const availableActions = getAvailableActions(state, playerId);
  const { min, max } = getBetLimits(state, playerId);

  return {
    canCheck: availableActions.includes(PlayerAction.Check),
    canCall: availableActions.includes(PlayerAction.Call),
    canBet: availableActions.includes(PlayerAction.Bet),
    canRaise: availableActions.includes(PlayerAction.Raise),
    canAllIn: availableActions.includes(PlayerAction.AllIn),
    callAmount: player ? getCallAmount(state, playerId) : 0,
    minBet: min,
    maxBet: max,
    potTotal: getTotalPot(state)
  };
}
