/**
 * Game State Manager
 * Handles game flow, phase transitions, and state management
 */

import {
  GameState,
  GamePhase,
  PlayerStatus,
  Player,
  Card,
  Pot,
  OpCode,
  PlayerAction
} from './types';
import { createShuffledDeck, dealCards, dealCard, cardsToString } from './deck';

/**
 * Get all active players in current hand (not folded, not sitting out)
 */
export function getActivePlayers(state: GameState): Player[] {
  return Array.from(Object.values(state.players))
    .filter(p => p.status === PlayerStatus.Active || p.status === PlayerStatus.AllIn)
    .sort((a, b) => a.seatIndex - b.seatIndex);
}

/**
 * Get all players who can still act (not folded, not all-in, not sitting out)
 */
export function getActingPlayers(state: GameState): Player[] {
  return Array.from(Object.values(state.players))
    .filter(p => p.status === PlayerStatus.Active)
    .sort((a, b) => a.seatIndex - b.seatIndex);
}

/**
 * Get players eligible for a new hand (have chips, not sitting out)
 */
export function getEligiblePlayers(state: GameState): Player[] {
  return Array.from(Object.values(state.players))
    .filter(p => p.chips > 0 && p.status !== PlayerStatus.SittingOut)
    .sort((a, b) => a.seatIndex - b.seatIndex);
}

/**
 * Find the next player seat index in clockwise order
 */
export function getNextPlayerSeat(
  state: GameState,
  currentSeat: number,
  skipFolded: boolean = true,
  skipAllIn: boolean = true
): number {
  const players = Array.from(Object.values(state.players))
    .sort((a, b) => a.seatIndex - b.seatIndex);

  if (players.length === 0) return -1;

  let nextSeat = currentSeat;
  let attempts = 0;
  const maxAttempts = state.maxPlayers;

  do {
    nextSeat = (nextSeat + 1) % state.maxPlayers;
    attempts++;

    const player = players.find(p => p.seatIndex === nextSeat);
    if (player) {
      if (skipFolded && player.status === PlayerStatus.Folded) continue;
      if (skipAllIn && player.status === PlayerStatus.AllIn) continue;
      if (player.status === PlayerStatus.SittingOut) continue;
      return nextSeat;
    }
  } while (attempts < maxAttempts);

  return -1;
}

/**
 * Get player by seat index
 */
export function getPlayerBySeat(state: GameState, seatIndex: number): Player | undefined {
  return Array.from(Object.values(state.players)).find(p => p.seatIndex === seatIndex);
}

/**
 * Move dealer button to next position
 */
export function moveDealerButton(state: GameState): void {
  const eligiblePlayers = getEligiblePlayers(state);
  if (eligiblePlayers.length === 0) return;

  if (state.dealerSeatIndex === -1) {
    // First hand - assign to first player
    state.dealerSeatIndex = eligiblePlayers[0].seatIndex;
  } else {
    // Move to next eligible player
    state.dealerSeatIndex = getNextPlayerSeat(state, state.dealerSeatIndex, false, false);
  }
}

/**
 * Assign blinds based on dealer position
 */
export function assignBlinds(state: GameState): { smallBlindSeat: number; bigBlindSeat: number } {
  const eligiblePlayers = getEligiblePlayers(state);

  // Clear previous blind assignments
  Object.values(state.players).forEach((p: Player) => {
    p.isDealer = false;
    p.isSmallBlind = false;
    p.isBigBlind = false;
  });

  const dealer = getPlayerBySeat(state, state.dealerSeatIndex);
  if (dealer) dealer.isDealer = true;

  let smallBlindSeat: number;
  let bigBlindSeat: number;

  if (eligiblePlayers.length === 2) {
    // Heads-up: dealer is small blind
    smallBlindSeat = state.dealerSeatIndex;
    bigBlindSeat = getNextPlayerSeat(state, state.dealerSeatIndex, false, false);
  } else {
    // 3+ players: standard positions
    smallBlindSeat = getNextPlayerSeat(state, state.dealerSeatIndex, false, false);
    bigBlindSeat = getNextPlayerSeat(state, smallBlindSeat, false, false);
  }

  const sbPlayer = getPlayerBySeat(state, smallBlindSeat);
  const bbPlayer = getPlayerBySeat(state, bigBlindSeat);

  if (sbPlayer) sbPlayer.isSmallBlind = true;
  if (bbPlayer) bbPlayer.isBigBlind = true;

  return { smallBlindSeat, bigBlindSeat };
}

/**
 * Post blinds from players
 */
export function postBlinds(state: GameState, smallBlindSeat: number, bigBlindSeat: number): void {
  const sbPlayer = getPlayerBySeat(state, smallBlindSeat);
  const bbPlayer = getPlayerBySeat(state, bigBlindSeat);

  if (sbPlayer) {
    const sbAmount = Math.min(state.smallBlind, sbPlayer.chips);
    sbPlayer.chips -= sbAmount;
    sbPlayer.currentBet = sbAmount;
    sbPlayer.totalBetThisHand = sbAmount;
    state.pots[0].amount += sbAmount;
  }

  if (bbPlayer) {
    const bbAmount = Math.min(state.bigBlind, bbPlayer.chips);
    bbPlayer.chips -= bbAmount;
    bbPlayer.currentBet = bbAmount;
    bbPlayer.totalBetThisHand = bbAmount;
    state.pots[0].amount += bbAmount;
  }

  // Set current bet to big blind
  state.currentBet = state.bigBlind;
  state.minRaise = state.bigBlind;
  state.lastRaiseAmount = state.bigBlind;
}

/**
 * Deal hole cards to all active players
 */
export function dealHoleCards(state: GameState): { [odid: string]: Card[] } {
  const holeCards: { [odid: string]: Card[] } = {};
  const eligiblePlayers = getEligiblePlayers(state);

  // Deal 2 cards to each player
  for (const player of eligiblePlayers) {
    const cards = dealCards(state.deck, 2);
    player.holeCards = cards;
    player.status = PlayerStatus.Active;
    holeCards[player.odid] = cards;
  }

  return holeCards;
}

/**
 * Deal community cards (flop, turn, river)
 */
export function dealCommunityCards(state: GameState, count: number): Card[] {
  // Burn one card first (standard poker rule)
  dealCard(state.deck);

  // Deal community cards
  const cards = dealCards(state.deck, count);
  state.communityCards.push(...cards);
  return cards;
}

/**
 * Start a new betting round
 */
export function startBettingRound(state: GameState): void {
  // Reset betting round state
  Object.values(state.players).forEach((p: Player) => {
    p.currentBet = 0;
    p.hasActed = false;
    p.lastAction = undefined;
  });

  state.currentBet = 0;
  state.actionsThisRound = 0;

  // Determine first player to act based on phase
  if (state.phase === GamePhase.PreFlop) {
    // Pre-flop: first to act is after big blind
    const eligiblePlayers = getEligiblePlayers(state);
    const bbPlayer = eligiblePlayers.find(p => p.isBigBlind);
    if (bbPlayer) {
      state.currentPlayerSeatIndex = getNextPlayerSeat(state, bbPlayer.seatIndex);
    }
  } else {
    // Post-flop: first to act is after dealer (small blind position)
    state.currentPlayerSeatIndex = getNextPlayerSeat(state, state.dealerSeatIndex);
  }
}

/**
 * Check if betting round is complete
 */
export function isBettingRoundComplete(state: GameState): boolean {
  const actingPlayers = getActingPlayers(state);

  // If only one player remains, round is complete
  if (actingPlayers.length <= 1) return true;

  // Check if all active players have acted and bets are matched
  for (const player of actingPlayers) {
    if (!player.hasActed) return false;
    if (player.currentBet !== state.currentBet && player.status !== PlayerStatus.AllIn) {
      return false;
    }
  }

  return true;
}

/**
 * Check if only one player remains (others folded)
 */
export function isOnlyOnePlayerLeft(state: GameState): boolean {
  const activePlayers = getActivePlayers(state);
  return activePlayers.length <= 1;
}

/**
 * Check if hand should go to showdown (all players all-in)
 */
export function shouldGoToShowdown(state: GameState): boolean {
  const actingPlayers = getActingPlayers(state);
  // If no one can act anymore (all all-in or folded), go to showdown
  return actingPlayers.length === 0 && getActivePlayers(state).length > 1;
}

/**
 * Initialize state for a new hand
 */
export function initNewHand(state: GameState, tick: number): void {
  // Increment hand number
  state.handNumber++;

  // Create and shuffle new deck
  state.deck = createShuffledDeck();

  // Clear community cards
  state.communityCards = [];

  // Reset pots
  state.pots = [{ amount: 0, eligiblePlayers: [] }];

  // Reset player states
  Object.values(state.players).forEach((p: Player) => {
    p.holeCards = [];
    p.currentBet = 0;
    p.totalBetThisHand = 0;
    p.status = p.chips > 0 && p.status !== PlayerStatus.SittingOut
      ? PlayerStatus.Waiting
      : p.status;
    p.hasActed = false;
    p.lastAction = undefined;
    p.isDealer = false;
    p.isSmallBlind = false;
    p.isBigBlind = false;
  });

  // Reset betting state
  state.currentBet = 0;
  state.minRaise = state.bigBlind;
  state.lastRaiseAmount = 0;
  state.actionsThisRound = 0;
  state.turnStartTick = tick;
}

/**
 * Start a new hand - full sequence
 */
export function startNewHand(state: GameState, tick: number): { [odid: string]: Card[] } {
  // Initialize hand
  initNewHand(state, tick);

  // Move dealer button
  moveDealerButton(state);

  // Assign and post blinds
  const { smallBlindSeat, bigBlindSeat } = assignBlinds(state);
  postBlinds(state, smallBlindSeat, bigBlindSeat);

  // Update eligible players for pot
  const eligiblePlayers = getEligiblePlayers(state);
  state.pots[0].eligiblePlayers = eligiblePlayers.map(p => p.odid);

  // Deal hole cards
  const holeCards = dealHoleCards(state);

  // Set phase to pre-flop
  state.phase = GamePhase.PreFlop;

  // Start betting round
  startBettingRound(state);

  // For pre-flop, we need to restore the current bet since blinds were posted
  state.currentBet = state.bigBlind;

  return holeCards;
}

/**
 * Advance to next game phase
 */
export function advancePhase(state: GameState): GamePhase {
  switch (state.phase) {
    case GamePhase.PreFlop:
      state.phase = GamePhase.Flop;
      dealCommunityCards(state, 3); // Deal flop (3 cards)
      break;
    case GamePhase.Flop:
      state.phase = GamePhase.Turn;
      dealCommunityCards(state, 1); // Deal turn (1 card)
      break;
    case GamePhase.Turn:
      state.phase = GamePhase.River;
      dealCommunityCards(state, 1); // Deal river (1 card)
      break;
    case GamePhase.River:
      state.phase = GamePhase.Showdown;
      break;
    default:
      break;
  }

  // Start new betting round if not showdown
  if (state.phase !== GamePhase.Showdown) {
    startBettingRound(state);
  }

  return state.phase;
}

/**
 * Move to next player's turn
 */
export function moveToNextPlayer(state: GameState, tick: number): number {
  state.currentPlayerSeatIndex = getNextPlayerSeat(state, state.currentPlayerSeatIndex);
  state.turnStartTick = tick;
  return state.currentPlayerSeatIndex;
}

/**
 * Check if current player's turn has timed out
 */
export function hasPlayerTimedOut(state: GameState, currentTick: number): boolean {
  const elapsed = currentTick - state.turnStartTick;
  return elapsed >= state.turnTimeoutTicks;
}

/**
 * Calculate side pots when players are all-in with different amounts
 */
export function calculateSidePots(state: GameState): void {
  const activePlayers = getActivePlayers(state);
  if (activePlayers.length === 0) return;

  // Get all unique bet amounts from active players
  const allInAmounts = activePlayers
    .filter(p => p.status === PlayerStatus.AllIn)
    .map(p => p.totalBetThisHand)
    .sort((a, b) => a - b);

  // If no all-ins, keep single main pot
  if (allInAmounts.length === 0) return;

  // Calculate pots based on all-in amounts
  const pots: Pot[] = [];
  let previousLevel = 0;

  for (const level of [...new Set(allInAmounts)]) {
    const contribution = level - previousLevel;
    const eligiblePlayers = activePlayers
      .filter(p => p.totalBetThisHand >= level)
      .map(p => p.odid);

    // Calculate pot amount at this level
    let potAmount = 0;
    for (const player of activePlayers) {
      const playerContribution = Math.min(player.totalBetThisHand - previousLevel, contribution);
      if (playerContribution > 0) {
        potAmount += playerContribution;
      }
    }

    if (potAmount > 0) {
      pots.push({ amount: potAmount, eligiblePlayers });
    }

    previousLevel = level;
  }

  // Add final pot for remaining amounts
  const maxAllIn = Math.max(...allInAmounts);
  const remainingPlayers = activePlayers
    .filter(p => p.totalBetThisHand > maxAllIn)
    .map(p => p.odid);

  if (remainingPlayers.length > 0) {
    let remainingAmount = 0;
    for (const player of activePlayers) {
      if (player.totalBetThisHand > maxAllIn) {
        remainingAmount += player.totalBetThisHand - maxAllIn;
      }
    }
    if (remainingAmount > 0) {
      pots.push({ amount: remainingAmount, eligiblePlayers: remainingPlayers });
    }
  }

  if (pots.length > 0) {
    state.pots = pots;
  }
}

/**
 * Get the total pot amount
 */
export function getTotalPot(state: GameState): number {
  return state.pots.reduce((sum, pot) => sum + pot.amount, 0);
}

/**
 * End the current hand and determine winner (when only one player left)
 */
export function endHandWithSingleWinner(state: GameState): { winnerId: string; amount: number } | null {
  const activePlayers = getActivePlayers(state);

  if (activePlayers.length !== 1) return null;

  const winner = activePlayers[0];
  const totalPot = getTotalPot(state);

  winner.chips += totalPot;

  // Reset to waiting phase
  state.phase = GamePhase.Waiting;

  return { winnerId: winner.odid, amount: totalPot };
}

/**
 * Get available actions for a player
 */
export function getAvailableActions(state: GameState, playerId: string): PlayerAction[] {
  const player = state.players[playerId];
  if (!player || player.status !== PlayerStatus.Active) {
    return [];
  }

  const actions: PlayerAction[] = [PlayerAction.Fold];

  const toCall = state.currentBet - player.currentBet;

  if (toCall === 0) {
    // No bet to match
    actions.push(PlayerAction.Check);
  } else if (toCall > 0 && player.chips > 0) {
    // There's a bet to match
    if (player.chips >= toCall) {
      actions.push(PlayerAction.Call);
    }
  }

  // Can raise/bet if has more chips than call amount
  if (player.chips > toCall) {
    if (state.currentBet === 0) {
      actions.push(PlayerAction.Bet);
    } else {
      actions.push(PlayerAction.Raise);
    }
  }

  // All-in is always available if player has chips
  if (player.chips > 0) {
    actions.push(PlayerAction.AllIn);
  }

  return actions;
}

/**
 * Get minimum and maximum bet amounts for current player
 */
export function getBetLimits(state: GameState, playerId: string): { min: number; max: number } {
  const player = state.players[playerId];
  if (!player) return { min: 0, max: 0 };

  const toCall = state.currentBet - player.currentBet;
  const minRaise = state.currentBet + state.minRaise;

  // Minimum bet/raise
  const min = Math.max(minRaise - player.currentBet, state.bigBlind);

  // Maximum is all chips
  const max = player.chips;

  return { min: Math.min(min, max), max };
}
