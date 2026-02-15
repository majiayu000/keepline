/**
 * Poker Match Handler
 * Implements Nakama's authoritative multiplayer match handler
 */

import { GameState, GamePhase, PlayerStatus, Player, OpCode, Suit, Rank, Card, PlayerAction, ClientMessage, HandRank, Pot, Spectator } from './types';
import {
  getEligiblePlayers,
  getActivePlayers,
  getActingPlayers,
  startNewHand,
  advancePhase,
  moveToNextPlayer,
  isBettingRoundComplete,
  isOnlyOnePlayerLeft,
  shouldGoToShowdown,
  hasPlayerTimedOut,
  endHandWithSingleWinner,
  calculateSidePots,
  getTotalPot,
  getPlayerBySeat,
} from './game_state';
import { executePlayerAction, autoFold, getActionInfo } from './betting';
import { evaluateHand, compareEvaluatedHands, EvaluatedHand, getHandRankName } from './hand_evaluator';

// Match tick rate (10 ticks per second)
const TICK_RATE = 10;

// Turn timeout in seconds
const TURN_TIMEOUT_SECONDS = 30;

// Disconnection grace period in seconds (time to reconnect before auto-fold)
const DISCONNECT_GRACE_SECONDS = 60;

// Game settings defaults
const DEFAULT_SETTINGS = {
  minPlayers: 2,
  maxPlayers: 9,
  smallBlind: 10,
  bigBlind: 20,
  startingChips: 1000,
  maxSpectators: 50    // Maximum spectators per match
};

/**
 * Serialize game state to JSON for storage
 */
function serializeState(state: GameState): { [key: string]: any } {
  return {
    matchId: state.matchId,
    tickRate: state.tickRate,
    label: state.label,
    minPlayers: state.minPlayers,
    maxPlayers: state.maxPlayers,
    smallBlind: state.smallBlind,
    bigBlind: state.bigBlind,
    startingChips: state.startingChips,
    maxSpectators: state.maxSpectators,
    phase: state.phase,
    players: { ...state.players },
    spectators: { ...state.spectators },
    deck: state.deck,
    communityCards: state.communityCards,
    pots: state.pots,
    dealerSeatIndex: state.dealerSeatIndex,
    currentPlayerSeatIndex: state.currentPlayerSeatIndex,
    currentBet: state.currentBet,
    minRaise: state.minRaise,
    lastRaiseAmount: state.lastRaiseAmount,
    turnStartTick: state.turnStartTick,
    turnTimeoutTicks: state.turnTimeoutTicks,
    handNumber: state.handNumber,
    actionsThisRound: state.actionsThisRound
  };
}

/**
 * Deserialize game state from JSON
 */
function deserializeState(data: { [key: string]: any }): GameState {
  return {
    matchId: data.matchId,
    tickRate: data.tickRate,
    label: data.label,
    minPlayers: data.minPlayers,
    maxPlayers: data.maxPlayers,
    smallBlind: data.smallBlind,
    bigBlind: data.bigBlind,
    startingChips: data.startingChips,
    maxSpectators: data.maxSpectators || DEFAULT_SETTINGS.maxSpectators,
    phase: data.phase,
    players: data.players || {},
    spectators: data.spectators || {},
    deck: data.deck,
    communityCards: data.communityCards,
    pots: data.pots,
    dealerSeatIndex: data.dealerSeatIndex,
    currentPlayerSeatIndex: data.currentPlayerSeatIndex,
    currentBet: data.currentBet,
    minRaise: data.minRaise,
    lastRaiseAmount: data.lastRaiseAmount,
    turnStartTick: data.turnStartTick,
    turnTimeoutTicks: data.turnTimeoutTicks,
    handNumber: data.handNumber,
    actionsThisRound: data.actionsThisRound
  };
}

/**
 * Find next available seat at the table
 */
function findAvailableSeat(players: { [odid: string]: Player }, maxPlayers: number): number {
  const takenSeats = new Set<number>();
  Object.values(players).forEach((p: Player) => takenSeats.add(p.seatIndex));

  for (let i = 0; i < maxPlayers; i++) {
    if (!takenSeats.has(i)) {
      return i;
    }
  }
  return -1;
}

/**
 * Broadcast message to all players in match
 */
function broadcastMessage(
  dispatcher: nkruntime.MatchDispatcher,
  opCode: number,
  data: any,
  presences?: nkruntime.Presence[] | null,
  sender?: nkruntime.Presence | null
): void {
  const payload = JSON.stringify(data);
  dispatcher.broadcastMessage(opCode, payload, presences, sender, true);
}

/**
 * Send message to specific player
 */
function sendMessage(
  dispatcher: nkruntime.MatchDispatcher,
  opCode: number,
  data: any,
  presence: nkruntime.Presence
): void {
  const payload = JSON.stringify(data);
  dispatcher.broadcastMessage(opCode, payload, [presence], null, true);
}

/**
 * Get public player info (hide private data like hole cards)
 */
function getPublicPlayerInfo(player: Player): any {
  return {
    odid: player.odid,
    displayName: player.odisplayName,
    seatIndex: player.seatIndex,
    chips: player.chips,
    status: player.status,
    currentBet: player.currentBet,
    isDealer: player.isDealer,
    lastAction: player.lastAction,
    isConnected: player.isConnected,
    avatarUrl: player.avatarUrl
  };
}

/**
 * Get full game state for broadcasting (without private info)
 */
function getPublicGameState(state: GameState): any {
  const players: any[] = [];
  Object.values(state.players).forEach(p => players.push(getPublicPlayerInfo(p)));

  return {
    phase: state.phase,
    communityCards: state.communityCards,
    pots: state.pots,
    currentBet: state.currentBet,
    currentPlayerSeat: state.currentPlayerSeatIndex,
    dealerSeat: state.dealerSeatIndex,
    handNumber: state.handNumber,
    players,
    spectatorCount: Object.keys(state.spectators).length,
    spectators: getSpectatorList(state)
  };
}

/**
 * Match initialization
 */
const matchInit: nkruntime.MatchInitFunction<GameState> = function(
  ctx: nkruntime.Context,
  logger: nkruntime.Logger,
  nk: nkruntime.Nakama,
  params: { [key: string]: string }
): { state: GameState; tickRate: number; label: string } {

  logger.info('Initializing poker match', { params });

  const label = params.label || 'Texas Hold\'em';

  const state: GameState = {
    matchId: ctx.matchId || '',
    tickRate: TICK_RATE,
    label: label,
    minPlayers: params.minPlayers ? parseInt(params.minPlayers) : DEFAULT_SETTINGS.minPlayers,
    maxPlayers: params.maxPlayers ? parseInt(params.maxPlayers) : DEFAULT_SETTINGS.maxPlayers,
    smallBlind: params.smallBlind ? parseInt(params.smallBlind) : DEFAULT_SETTINGS.smallBlind,
    bigBlind: params.bigBlind ? parseInt(params.bigBlind) : DEFAULT_SETTINGS.bigBlind,
    startingChips: params.startingChips ? parseInt(params.startingChips) : DEFAULT_SETTINGS.startingChips,
    maxSpectators: params.maxSpectators ? parseInt(params.maxSpectators) : DEFAULT_SETTINGS.maxSpectators,
    phase: GamePhase.Waiting,
    players: {},
    spectators: {},
    deck: [],
    communityCards: [],
    pots: [{ amount: 0, eligiblePlayers: [] }],
    dealerSeatIndex: -1,
    currentPlayerSeatIndex: -1,
    currentBet: 0,
    minRaise: 0,
    lastRaiseAmount: 0,
    turnStartTick: 0,
    turnTimeoutTicks: TURN_TIMEOUT_SECONDS * TICK_RATE,
    handNumber: 0,
    actionsThisRound: 0
  };

  return {
    state,
    tickRate: TICK_RATE,
    label: JSON.stringify({
      name: label,
      players: 0,
      maxPlayers: state.maxPlayers,
      blinds: `${state.smallBlind}/${state.bigBlind}`
    })
  };
};

/**
 * Validate join attempt
 */
const matchJoinAttempt: nkruntime.MatchJoinAttemptFunction<GameState> = function(
  ctx: nkruntime.Context,
  logger: nkruntime.Logger,
  nk: nkruntime.Nakama,
  dispatcher: nkruntime.MatchDispatcher,
  tick: number,
  state: GameState,
  presence: nkruntime.Presence,
  metadata: { [key: string]: any }
): { state: GameState; accept: boolean; rejectMessage?: string } {

  const asSpectator = metadata?.spectator === true;
  logger.info('Join attempt', { userId: presence.userId, username: presence.username, asSpectator });

  // Check if user is already in the match as player
  if (((presence.userId) in state.players)) {
    return { state, accept: true }; // Allow rejoin
  }

  // Check if user is already in the match as spectator
  if (((presence.userId) in state.spectators)) {
    return { state, accept: true }; // Allow rejoin
  }

  // If joining as spectator
  if (asSpectator) {
    if (Object.keys(state.spectators).length >= state.maxSpectators) {
      return { state, accept: false, rejectMessage: 'Spectator limit reached' };
    }
    return { state, accept: true };
  }

  // Joining as player
  if (Object.keys(state.players).length >= state.maxPlayers) {
    // If table is full, suggest joining as spectator
    return { state, accept: false, rejectMessage: 'Match is full. Try joining as spectator.' };
  }

  return { state, accept: true };
};

// Track users who want to join as spectators (used between matchJoinAttempt and matchJoin)
const spectatorIntents: { [key: string]: boolean } = {};

/**
 * Handle player or spectator joining
 */
const matchJoin: nkruntime.MatchJoinFunction<GameState> = function(
  ctx: nkruntime.Context,
  logger: nkruntime.Logger,
  nk: nkruntime.Nakama,
  dispatcher: nkruntime.MatchDispatcher,
  tick: number,
  state: GameState,
  presences: nkruntime.Presence[]
): { state: GameState } | null {

  for (const presence of presences) {
    logger.info('User joined', { userId: presence.userId, username: presence.username });

    // Check if this is a player rejoin
    if (((presence.userId) in state.players)) {
      const player = state.players[presence.userId]!;
      const wasDisconnected = !player.isConnected;

      // Mark as connected
      player.isConnected = true;
      player.disconnectedAt = undefined;

      // If player was sitting out due to disconnect and game hasn't started, restore to waiting
      if (player.status === PlayerStatus.SittingOut && state.phase === GamePhase.Waiting && player.chips > 0) {
        player.status = PlayerStatus.Waiting;
      }

      logger.info('Player rejoined', {
        userId: presence.userId,
        seatIndex: player.seatIndex,
        wasDisconnected,
        status: player.status
      });

      // Broadcast reconnection to all players
      if (wasDisconnected) {
        broadcastMessage(dispatcher, OpCode.PLAYER_RECONNECTED, {
          odid: presence.userId,
          displayName: player.odisplayName,
          seatIndex: player.seatIndex
        });
      }

      // Send current game state to rejoining player
      sendMessage(dispatcher, OpCode.GAME_STATE, getPublicGameState(state), presence);

      // Send their hole cards if in active hand
      if (player.holeCards.length > 0 && player.status !== PlayerStatus.Folded) {
        sendMessage(dispatcher, OpCode.HOLE_CARDS, { cards: player.holeCards }, presence);
      }

      // If it's their turn, notify them
      if (state.currentPlayerSeatIndex === player.seatIndex) {
        notifyCurrentPlayer(state, dispatcher);
      }

      continue;
    }

    // Check if this is a spectator rejoin
    if (((presence.userId) in state.spectators)) {
      logger.info('Spectator rejoined', { userId: presence.userId });

      // Send current game state to rejoining spectator
      sendMessage(dispatcher, OpCode.GAME_STATE, getPublicGameState(state), presence);

      // Send spectator list
      sendMessage(dispatcher, OpCode.SPECTATOR_LIST, {
        spectators: getSpectatorList(state)
      }, presence);

      continue;
    }

    // Find available seat for player
    const seatIndex = findAvailableSeat(state.players, state.maxPlayers);

    // If no seat available, add as spectator
    if (seatIndex === -1) {
      // Add as spectator
      const spectator: Spectator = {
        odid: presence.userId,
        displayName: presence.username || `Spectator`,
        joinedAt: tick
      };

      state.spectators[presence.userId] = spectator;
      logger.info('Spectator added', { userId: presence.userId });

      // Broadcast spectator joined to all
      broadcastMessage(dispatcher, OpCode.SPECTATOR_JOINED, {
        odid: presence.userId,
        displayName: spectator.displayName
      });

      // Send current game state to new spectator
      sendMessage(dispatcher, OpCode.GAME_STATE, getPublicGameState(state), presence);

      // Send spectator list
      sendMessage(dispatcher, OpCode.SPECTATOR_LIST, {
        spectators: getSpectatorList(state)
      }, presence);

      continue;
    }

    // Create new player
    const player: Player = {
      odid: presence.userId,
      odisplayName: presence.username || `Player ${seatIndex + 1}`,
      seatIndex: seatIndex,
      chips: state.startingChips,
      status: PlayerStatus.Waiting,
      holeCards: [],
      currentBet: 0,
      totalBetThisHand: 0,
      isDealer: false,
      isSmallBlind: false,
      isBigBlind: false,
      hasActed: false,
      isConnected: true
    };

    state.players[presence.userId] = player;
    logger.info('Player added', { userId: presence.userId, seatIndex, chips: player.chips });

    // Broadcast player joined to all
    broadcastMessage(dispatcher, OpCode.PLAYER_JOINED, getPublicPlayerInfo(player));

    // Send current game state to new player
    sendMessage(dispatcher, OpCode.GAME_STATE, getPublicGameState(state), presence);

    // Send spectator list to new player
    if (Object.keys(state.spectators).length > 0) {
      sendMessage(dispatcher, OpCode.SPECTATOR_LIST, {
        spectators: getSpectatorList(state)
      }, presence);
    }
  }

  // Update match label with player and spectator count
  updateMatchLabel(state, dispatcher);

  return { state };
};

/**
 * Get spectator list for broadcasting
 */
function getSpectatorList(state: GameState): { odid: string; displayName: string }[] {
  const spectators: { odid: string; displayName: string }[] = [];
  Object.values(state.spectators).forEach(s => {
    spectators.push({ odid: s.odid, displayName: s.displayName });
  });
  return spectators;
}

/**
 * Update match label with current player/spectator count
 */
function updateMatchLabel(state: GameState, dispatcher: nkruntime.MatchDispatcher): void {
  // Count connected players
  let connectedPlayers = 0;
  Object.values(state.players).forEach(p => { if (p.isConnected) connectedPlayers++; });

  const labelData = {
    name: state.label,
    players: connectedPlayers,
    maxPlayers: state.maxPlayers,
    spectators: Object.keys(state.spectators).length,
    blinds: `${state.smallBlind}/${state.bigBlind}`,
    phase: state.phase
  };
  dispatcher.matchLabelUpdate(JSON.stringify(labelData));
}

/**
 * Handle player or spectator leaving (disconnect)
 */
const matchLeave: nkruntime.MatchLeaveFunction<GameState> = function(
  ctx: nkruntime.Context,
  logger: nkruntime.Logger,
  nk: nkruntime.Nakama,
  dispatcher: nkruntime.MatchDispatcher,
  tick: number,
  state: GameState,
  presences: nkruntime.Presence[]
): { state: GameState } | null {

  for (const presence of presences) {
    logger.info('User disconnected', { userId: presence.userId });

    // Check if this is a spectator
    if (((presence.userId) in state.spectators)) {
      const spectator = state.spectators[presence.userId]!;
      delete state.spectators[presence.userId];
      logger.info('Spectator removed', { userId: presence.userId });

      // Broadcast spectator left
      broadcastMessage(dispatcher, OpCode.SPECTATOR_LEFT, {
        odid: presence.userId,
        displayName: spectator.displayName
      });

      continue;
    }

    // Check if this is a player
    const player = state.players[presence.userId];
    if (!player) continue;

    // Mark as disconnected
    player.isConnected = false;
    player.disconnectedAt = tick;

    // If game is in progress
    if (state.phase !== GamePhase.Waiting) {
      // Don't change status immediately - give player grace period to reconnect
      // Status will be changed to SittingOut if they don't reconnect in time
      logger.info('Player disconnected during game, starting grace period', {
        userId: presence.userId,
        phase: state.phase,
        isCurrentPlayer: state.currentPlayerSeatIndex === player.seatIndex
      });

      // Broadcast disconnection to all players
      broadcastMessage(dispatcher, OpCode.PLAYER_DISCONNECTED, {
        odid: presence.userId,
        displayName: player.odisplayName,
        seatIndex: player.seatIndex,
        graceSeconds: DISCONNECT_GRACE_SECONDS
      });
    } else {
      // Game hasn't started - remove player immediately
      delete state.players[presence.userId];
      logger.info('Player removed (game not started)', { userId: presence.userId });

      // Broadcast player left
      broadcastMessage(dispatcher, OpCode.PLAYER_LEFT, {
        odid: presence.userId,
        sittingOut: false
      });
    }
  }

  // Update match label
  updateMatchLabel(state, dispatcher);

  // End match if no players and no spectators left
  if (Object.keys(state.players).length === 0 && Object.keys(state.spectators).length === 0) {
    logger.info('No players or spectators left, ending match');
    return null;
  }

  return { state };
};

/**
 * Check if a disconnected player has exceeded their grace period
 */
function hasDisconnectedPlayerTimedOut(player: Player, tick: number): boolean {
  if (player.isConnected || !player.disconnectedAt) return false;
  const elapsedTicks = tick - player.disconnectedAt;
  const graceTicks = DISCONNECT_GRACE_SECONDS * TICK_RATE;
  return elapsedTicks >= graceTicks;
}

/**
 * Handle disconnected players who have exceeded grace period
 */
function handleDisconnectedPlayers(
  state: GameState,
  dispatcher: nkruntime.MatchDispatcher,
  logger: nkruntime.Logger,
  tick: number
): boolean {
  let stateChanged = false;

  Object.entries(state.players).forEach(([odid, player]) => {
    if (hasDisconnectedPlayerTimedOut(player, tick)) {
      logger.info('Disconnected player grace period expired', { userId: odid });

      // If player is in an active hand and is current player, auto-fold
      if ((player.status === PlayerStatus.Active || player.status === PlayerStatus.AllIn) &&
          state.currentPlayerSeatIndex === player.seatIndex) {
        const result = autoFold(state, odid);
        if (result.success) {
          broadcastMessage(dispatcher, OpCode.PLAYER_ACTED, {
            odid: odid,
            action: PlayerAction.Fold,
            amount: 0,
            newChips: player.chips,
            potTotal: getTotalPot(state),
            timeout: true,
            disconnected: true
          });
          stateChanged = true;
        }
      }

      // Mark player as sitting out
      player.status = PlayerStatus.SittingOut;
      player.disconnectedAt = undefined; // Clear so we don't process again

      // Broadcast player status update
      broadcastMessage(dispatcher, OpCode.PLAYER_LEFT, {
        odid: odid,
        sittingOut: true,
        reason: 'disconnect_timeout'
      });
    }
  });

  return stateChanged;
}

/**
 * Handle spectator requesting to become a player
 */
function handleRequestSeat(
  state: GameState,
  dispatcher: nkruntime.MatchDispatcher,
  logger: nkruntime.Logger,
  tick: number,
  presence: nkruntime.Presence
): void {
  const odid = presence.userId;

  // Check if user is a spectator
  const spectator = state.spectators[odid];
  if (!spectator) {
    sendMessage(dispatcher, OpCode.ERROR, { message: 'You are not a spectator' }, presence);
    return;
  }

  // Check if there's an available seat
  const seatIndex = findAvailableSeat(state.players, state.maxPlayers);
  if (seatIndex === -1) {
    sendMessage(dispatcher, OpCode.ERROR, { message: 'No available seats' }, presence);
    return;
  }

  // Remove from spectators
  delete state.spectators[odid];

  // Create new player
  const player: Player = {
    odid: odid,
    odisplayName: spectator.displayName,
    seatIndex: seatIndex,
    chips: state.startingChips,
    status: PlayerStatus.Waiting,
    holeCards: [],
    currentBet: 0,
    totalBetThisHand: 0,
    isDealer: false,
    isSmallBlind: false,
    isBigBlind: false,
    hasActed: false,
    isConnected: true
  };

  state.players[odid] = player;
  logger.info('Spectator became player', { userId: odid, seatIndex, chips: player.chips });

  // Broadcast spectator left
  broadcastMessage(dispatcher, OpCode.SPECTATOR_LEFT, {
    odid: odid,
    displayName: spectator.displayName
  });

  // Broadcast player joined
  broadcastMessage(dispatcher, OpCode.PLAYER_JOINED, getPublicPlayerInfo(player));

  // Notify the player they switched successfully
  broadcastMessage(dispatcher, OpCode.SPECTATOR_TO_PLAYER, {
    odid: odid,
    seatIndex: seatIndex,
    chips: player.chips
  });

  // Send current game state to new player
  sendMessage(dispatcher, OpCode.GAME_STATE, getPublicGameState(state), presence);

  // Update match label
  updateMatchLabel(state, dispatcher);
}

/**
 * Main game loop - called every tick
 */
const matchLoop: nkruntime.MatchLoopFunction<GameState> = function(
  ctx: nkruntime.Context,
  logger: nkruntime.Logger,
  nk: nkruntime.Nakama,
  dispatcher: nkruntime.MatchDispatcher,
  tick: number,
  state: GameState,
  messages: nkruntime.MatchMessage[]
): { state: GameState } | null {

  // Handle disconnected players who have exceeded grace period
  const disconnectHandled = handleDisconnectedPlayers(state, dispatcher, logger, tick);
  if (disconnectHandled) {
    // Check if we need to handle post-action logic after auto-fold
    handlePostAction(state, dispatcher, logger, tick);
  }

  // Process incoming messages
  for (const message of messages) {
    if (message.opCode === OpCode.PLAYER_ACTION) {
      try {
        const data: ClientMessage = JSON.parse(nk.binaryToString(message.data));
        const result = executePlayerAction(state, message.sender.userId, data.action, data.amount);

        if (result.success) {
          logger.info('Player action executed', {
            userId: message.sender.userId,
            action: result.action,
            amount: result.amount
          });

          // Broadcast player acted
          broadcastMessage(dispatcher, OpCode.PLAYER_ACTED, {
            odid: message.sender.userId,
            action: result.action,
            amount: result.amount,
            newChips: result.newChips,
            potTotal: result.newPotTotal
          });

          // Handle post-action game flow
          handlePostAction(state, dispatcher, logger, tick);
        } else {
          // Send error to player
          sendMessage(dispatcher, OpCode.ERROR, { message: result.error }, message.sender);
          logger.warn('Invalid player action', {
            userId: message.sender.userId,
            action: data.action,
            error: result.error
          });
        }
      } catch (e) {
        logger.error('Failed to parse player action', { error: e });
        sendMessage(dispatcher, OpCode.ERROR, { message: 'Invalid action format' }, message.sender);
      }
    } else if (message.opCode === OpCode.CHAT_MESSAGE) {
      // Broadcast chat message to all players and spectators
      try {
        const chatData = JSON.parse(nk.binaryToString(message.data));
        broadcastMessage(dispatcher, OpCode.CHAT_MESSAGE, {
          odid: message.sender.userId,
          username: message.sender.username,
          message: chatData.message
        });
      } catch (e) {
        logger.error('Failed to parse chat message', { error: e });
      }
    } else if (message.opCode === OpCode.REQUEST_SEAT) {
      // Spectator requesting to become a player
      handleRequestSeat(state, dispatcher, logger, tick, message.sender);
    }
  }

  // Game phase logic
  switch (state.phase) {
    case GamePhase.Waiting:
      // Check if we have enough players to start
      const eligiblePlayers = getEligiblePlayers(state);

      if (eligiblePlayers.length >= state.minPlayers) {
        logger.info('Starting new hand', { playerCount: eligiblePlayers.length });

        // Start a new hand
        const holeCards = startNewHand(state, tick);

        // Broadcast hand start
        broadcastMessage(dispatcher, OpCode.HAND_START, {
          handNumber: state.handNumber,
          dealerSeat: state.dealerSeatIndex,
          smallBlind: state.smallBlind,
          bigBlind: state.bigBlind
        });

        // Send hole cards to each player privately
        Object.entries(holeCards).forEach(([odid, cards]) => {
          const presence = getPresenceByUserId(ctx, odid);
          if (presence) {
            sendMessage(dispatcher, OpCode.HOLE_CARDS, { cards }, presence);
          }
        });

        // Broadcast game state
        broadcastMessage(dispatcher, OpCode.GAME_STATE, getPublicGameState(state));

        // Notify current player it's their turn
        notifyCurrentPlayer(state, dispatcher);
      }
      break;

    case GamePhase.PreFlop:
    case GamePhase.Flop:
    case GamePhase.Turn:
    case GamePhase.River:
      // Check for turn timeout
      if (hasPlayerTimedOut(state, tick)) {
        const currentPlayer = getPlayerBySeat(state, state.currentPlayerSeatIndex);
        if (currentPlayer) {
          logger.info('Player timed out, auto-folding', { userId: currentPlayer.odid });

          // Auto-fold the player
          const result = autoFold(state, currentPlayer.odid);

          if (result.success) {
            // Broadcast player acted (auto-fold)
            broadcastMessage(dispatcher, OpCode.PLAYER_ACTED, {
              odid: currentPlayer.odid,
              action: PlayerAction.Fold,
              amount: 0,
              newChips: currentPlayer.chips,
              potTotal: getTotalPot(state),
              timeout: true
            });

            // Handle post-action game flow
            handlePostAction(state, dispatcher, logger, tick);
          }
        }
      }
      break;

    case GamePhase.Showdown:
      // Execute showdown and determine winners
      const showdownResult = executeShowdown(state, logger);

      // Broadcast showdown results
      broadcastMessage(dispatcher, OpCode.SHOWDOWN, {
        players: showdownResult.showdownPlayers
      });

      // Broadcast hand result
      broadcastMessage(dispatcher, OpCode.HAND_RESULT, {
        winners: showdownResult.winners,
        showdown: showdownResult.showdownPlayers
      });

      // Reset for next hand
      state.phase = GamePhase.Waiting;

      // Broadcast updated game state
      broadcastMessage(dispatcher, OpCode.GAME_STATE, getPublicGameState(state));
      break;
  }

  return { state };
};

/**
 * Showdown result interface
 */
interface ShowdownResult {
  winners: {
    odid: string;
    amount: number;
    hand: Card[];
    handRank: HandRank;
    handDescription: string;
  }[];
  showdownPlayers: {
    odid: string;
    cards: Card[];
    handRank: HandRank;
    handDescription: string;
    bestHand: Card[];
  }[];
}

/**
 * Execute showdown - evaluate hands and distribute pots
 */
function executeShowdown(state: GameState, logger: nkruntime.Logger): ShowdownResult {
  logger.info('Executing showdown');

  // Get all players who can participate in showdown (active or all-in, not folded)
  const showdownPlayers: Player[] = [];
  Object.values(state.players).forEach(player => {
    if (player.status === PlayerStatus.Active || player.status === PlayerStatus.AllIn) {
      showdownPlayers.push(player);
    }
  });

  logger.info('Showdown players', { count: showdownPlayers.length });

  // Evaluate each player's hand
  const evaluatedPlayers: {
    player: Player;
    evaluation: EvaluatedHand;
  }[] = [];

  for (const player of showdownPlayers) {
    // Combine hole cards with community cards
    const allCards = [...player.holeCards, ...state.communityCards];

    try {
      const evaluation = evaluateHand(allCards);
      evaluatedPlayers.push({ player, evaluation });

      logger.info('Evaluated player hand', {
        odid: player.odid,
        holeCards: player.holeCards,
        rank: evaluation.rank,
        description: evaluation.description
      });
    } catch (e) {
      logger.error('Failed to evaluate hand', { odid: player.odid, error: e });
    }
  }

  // Calculate side pots if not already calculated
  if (state.pots.length <= 1 || state.pots[0].eligiblePlayers.length === 0) {
    calculateSidePots(state);
  }

  // Distribute each pot to its winners
  const winners: ShowdownResult['winners'] = [];
  const potWinners: { [key: string]: number } = {}; // Track total winnings per player

  for (const pot of state.pots) {
    if (pot.amount === 0) continue;

    // Find eligible players for this pot
    const eligibleEvaluated = evaluatedPlayers.filter(ep =>
      pot.eligiblePlayers.includes(ep.player.odid)
    );

    if (eligibleEvaluated.length === 0) {
      logger.warn('No eligible players for pot', { pot });
      continue;
    }

    // Find the best hand(s) among eligible players
    let best = eligibleEvaluated[0];
    const potWinnersList: typeof evaluatedPlayers = [best];

    for (let i = 1; i < eligibleEvaluated.length; i++) {
      const comparison = compareEvaluatedHands(eligibleEvaluated[i].evaluation, best.evaluation);
      if (comparison > 0) {
        // New best hand
        best = eligibleEvaluated[i];
        potWinnersList.length = 0;
        potWinnersList.push(best);
      } else if (comparison === 0) {
        // Tie - add to winners
        potWinnersList.push(eligibleEvaluated[i]);
      }
    }

    // Split pot among winners
    const shareAmount = Math.floor(pot.amount / potWinnersList.length);
    const remainder = pot.amount % potWinnersList.length;

    for (let i = 0; i < potWinnersList.length; i++) {
      const winner = potWinnersList[i];
      // First winner gets the remainder (if any)
      const winAmount = shareAmount + (i === 0 ? remainder : 0);

      // Add to player's chips
      winner.player.chips += winAmount;

      // Track total winnings
      const currentWinnings = potWinners[winner.player.odid] || 0;
      potWinners[winner.player.odid] = currentWinnings + winAmount;

      logger.info('Pot distributed', {
        odid: winner.player.odid,
        amount: winAmount,
        handRank: winner.evaluation.rank,
        description: winner.evaluation.description
      });
    }
  }

  // Build winners array
  Object.entries(potWinners).forEach(([odid, amount]) => {
    const ep = evaluatedPlayers.find(e => e.player.odid === odid);
    if (ep) {
      winners.push({
        odid: odid,
        amount: amount,
        hand: ep.evaluation.cards,
        handRank: ep.evaluation.rank,
        handDescription: ep.evaluation.description
      });
    }
  });

  // Build showdown players array (all hands revealed)
  const showdownPlayersInfo: ShowdownResult['showdownPlayers'] = evaluatedPlayers.map(ep => ({
    odid: ep.player.odid,
    cards: ep.player.holeCards,
    handRank: ep.evaluation.rank,
    handDescription: ep.evaluation.description,
    bestHand: ep.evaluation.cards
  }));

  // Reset pot
  state.pots = [{ amount: 0, eligiblePlayers: [] }];

  // Reset player states for next hand
  Object.values(state.players).forEach(player => {
    player.holeCards = [];
    player.currentBet = 0;
    player.totalBetThisHand = 0;
    player.hasActed = false;
    player.lastAction = undefined;
    player.isDealer = false;
    player.isSmallBlind = false;
    player.isBigBlind = false;

    // Set status for next hand
    if (player.chips > 0 && player.status !== PlayerStatus.SittingOut) {
      player.status = PlayerStatus.Waiting;
    } else if (player.chips === 0) {
      player.status = PlayerStatus.SittingOut; // Busted
    }
  });

  // Reset community cards and deck
  state.communityCards = [];
  state.deck = [];
  state.currentBet = 0;
  state.minRaise = 0;
  state.lastRaiseAmount = 0;

  logger.info('Showdown complete', { winners: winners.map(w => ({ odid: w.odid, amount: w.amount })) });

  return {
    winners,
    showdownPlayers: showdownPlayersInfo
  };
}

/**
 * Handle game flow after a player action
 */
function handlePostAction(
  state: GameState,
  dispatcher: nkruntime.MatchDispatcher,
  logger: nkruntime.Logger,
  tick: number
): void {
  // Check if only one player remains
  if (isOnlyOnePlayerLeft(state)) {
    logger.info('Only one player left, ending hand');
    const result = endHandWithSingleWinner(state);

    if (result) {
      // Broadcast hand result
      broadcastMessage(dispatcher, OpCode.HAND_RESULT, {
        winners: [{
          odid: result.winnerId,
          amount: result.amount
        }],
        showdown: []
      });

      // Broadcast updated game state
      broadcastMessage(dispatcher, OpCode.GAME_STATE, getPublicGameState(state));
    }
    return;
  }

  // Check if betting round is complete
  if (isBettingRoundComplete(state)) {
    logger.info('Betting round complete', { phase: state.phase });

    // Check if we should go straight to showdown (all players all-in)
    if (shouldGoToShowdown(state)) {
      logger.info('All players all-in, going to showdown');
      // Calculate side pots
      calculateSidePots(state);

      // Deal remaining community cards
      while (state.phase !== GamePhase.Showdown) {
        advancePhase(state);

        // Broadcast community cards
        broadcastMessage(dispatcher, OpCode.COMMUNITY_CARDS, {
          cards: state.communityCards,
          phase: state.phase
        });
      }

      // Broadcast pot update
      broadcastMessage(dispatcher, OpCode.POT_UPDATE, {
        pots: state.pots,
        total: getTotalPot(state)
      });
      return;
    }

    // Advance to next phase
    const newPhase = advancePhase(state);
    logger.info('Advanced to phase', { phase: newPhase });

    // Broadcast community cards if we're not in showdown
    if (newPhase !== GamePhase.Showdown) {
      broadcastMessage(dispatcher, OpCode.COMMUNITY_CARDS, {
        cards: state.communityCards,
        phase: newPhase
      });

      // Broadcast game state
      broadcastMessage(dispatcher, OpCode.GAME_STATE, getPublicGameState(state));

      // Notify current player it's their turn
      notifyCurrentPlayer(state, dispatcher);
    }
    return;
  }

  // Move to next player
  moveToNextPlayer(state, tick);

  // Broadcast game state
  broadcastMessage(dispatcher, OpCode.GAME_STATE, getPublicGameState(state));

  // Notify current player
  notifyCurrentPlayer(state, dispatcher);
}

/**
 * Notify the current player that it's their turn
 */
function notifyCurrentPlayer(state: GameState, dispatcher: nkruntime.MatchDispatcher): void {
  const currentPlayer = getPlayerBySeat(state, state.currentPlayerSeatIndex);
  if (!currentPlayer) return;

  const actionInfo = getActionInfo(state, currentPlayer.odid);

  broadcastMessage(dispatcher, OpCode.PLAYER_TURN, {
    odid: currentPlayer.odid,
    seatIndex: currentPlayer.seatIndex,
    timeoutSeconds: state.turnTimeoutTicks / TICK_RATE,
    ...actionInfo
  });
}

/**
 * Helper to get presence by user ID (simplified - in production would need proper presence tracking)
 */
function getPresenceByUserId(ctx: nkruntime.Context, odid: string): nkruntime.Presence | null {
  // Note: In a real implementation, you would track presences in the state
  // For now, we return a placeholder - the actual presence management
  // should be implemented with proper presence tracking
  return {
    userId: odid,
    sessionId: '',
    username: '',
    node: ''
  };
}

/**
 * Handle match termination
 */
const matchTerminate: nkruntime.MatchTerminateFunction<GameState> = function(
  ctx: nkruntime.Context,
  logger: nkruntime.Logger,
  nk: nkruntime.Nakama,
  dispatcher: nkruntime.MatchDispatcher,
  tick: number,
  state: GameState,
  graceSeconds: number
): { state: GameState } | null {

  logger.info('Match terminating', {
    matchId: state.matchId,
    graceSeconds,
    playerCount: Object.keys(state.players).length
  });

  return { state };
};

/**
 * Handle external signals
 */
const matchSignal: nkruntime.MatchSignalFunction<GameState> = function(
  ctx: nkruntime.Context,
  logger: nkruntime.Logger,
  nk: nkruntime.Nakama,
  dispatcher: nkruntime.MatchDispatcher,
  tick: number,
  state: GameState,
  data: string
): { state: GameState; data?: string } | null {

  logger.info('Match signal received', { data });

  return { state, data: 'signal_received' };
};

/**
 * Export match handler
 */
export const pokerMatchHandler: nkruntime.MatchHandler<GameState> = {
  matchInit: matchInit,
  matchJoinAttempt: matchJoinAttempt,
  matchJoin: matchJoin,
  matchLeave: matchLeave,
  matchLoop: matchLoop,
  matchTerminate: matchTerminate,
  matchSignal: matchSignal
};
