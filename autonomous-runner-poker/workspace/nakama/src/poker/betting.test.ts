/**
 * Unit tests for Texas Hold'em Betting Logic
 */

import { describe, it, expect, beforeEach } from 'vitest';
import {
  executePlayerAction,
  autoFold,
  getCallAmount,
  getActionInfo,
  ActionResult
} from './betting';
import {
  GameState,
  GamePhase,
  Player,
  PlayerStatus,
  PlayerAction,
  Pot
} from './types';

// Helper function to create a minimal player
function createPlayer(overrides: Partial<Player> = {}): Player {
  return {
    odid: 'player1',
    odisplayName: 'Player 1',
    seatIndex: 0,
    chips: 1000,
    status: PlayerStatus.Active,
    holeCards: [],
    currentBet: 0,
    totalBetThisHand: 0,
    isDealer: false,
    isSmallBlind: false,
    isBigBlind: false,
    hasActed: false,
    isConnected: true,
    ...overrides
  };
}

// Helper function to create a minimal game state
function createGameState(overrides: Partial<GameState> = {}): GameState {
  return {
    matchId: 'test-match',
    tickRate: 10,
    label: 'test',
    minPlayers: 2,
    maxPlayers: 9,
    smallBlind: 10,
    bigBlind: 20,
    startingChips: 1000,
    phase: GamePhase.PreFlop,
    players: new Map(),
    deck: [],
    communityCards: [],
    pots: [{ amount: 30, eligiblePlayers: [] }],
    dealerSeatIndex: 0,
    currentPlayerSeatIndex: 0,
    currentBet: 20,
    minRaise: 20,
    lastRaiseAmount: 20,
    turnStartTick: 0,
    turnTimeoutTicks: 300,
    handNumber: 1,
    actionsThisRound: 0,
    ...overrides
  };
}

// Helper to setup a standard game with 2 players
function setupTwoPlayerGame(): { state: GameState; player1: Player; player2: Player } {
  const player1 = createPlayer({
    odid: 'p1',
    odisplayName: 'Player 1',
    seatIndex: 0,
    chips: 1000,
    isDealer: true,
    isSmallBlind: true,
    currentBet: 10,
    totalBetThisHand: 10
  });

  const player2 = createPlayer({
    odid: 'p2',
    odisplayName: 'Player 2',
    seatIndex: 1,
    chips: 980,
    isBigBlind: true,
    currentBet: 20,
    totalBetThisHand: 20
  });

  const players = new Map<string, Player>();
  players.set('p1', player1);
  players.set('p2', player2);

  const state = createGameState({
    players,
    currentPlayerSeatIndex: 0, // Player 1's turn
    currentBet: 20,
    pots: [{ amount: 30, eligiblePlayers: ['p1', 'p2'] }]
  });

  return { state, player1, player2 };
}

// Helper to setup 3 player game
function setupThreePlayerGame(): { state: GameState; player1: Player; player2: Player; player3: Player } {
  const player1 = createPlayer({
    odid: 'p1',
    odisplayName: 'Player 1',
    seatIndex: 0,
    chips: 1000,
    isDealer: true
  });

  const player2 = createPlayer({
    odid: 'p2',
    odisplayName: 'Player 2',
    seatIndex: 1,
    chips: 990,
    isSmallBlind: true,
    currentBet: 10,
    totalBetThisHand: 10
  });

  const player3 = createPlayer({
    odid: 'p3',
    odisplayName: 'Player 3',
    seatIndex: 2,
    chips: 980,
    isBigBlind: true,
    currentBet: 20,
    totalBetThisHand: 20
  });

  const players = new Map<string, Player>();
  players.set('p1', player1);
  players.set('p2', player2);
  players.set('p3', player3);

  const state = createGameState({
    players,
    currentPlayerSeatIndex: 0,
    currentBet: 20,
    pots: [{ amount: 30, eligiblePlayers: ['p1', 'p2', 'p3'] }]
  });

  return { state, player1, player2, player3 };
}

describe('Betting Logic', () => {
  describe('executePlayerAction - Validation', () => {
    it('should fail if player not found', () => {
      const { state } = setupTwoPlayerGame();
      const result = executePlayerAction(state, 'nonexistent', PlayerAction.Fold);
      expect(result.success).toBe(false);
      expect(result.error).toBe('Player not found');
    });

    it('should fail if not player\'s turn', () => {
      const { state } = setupTwoPlayerGame();
      // Player 2 tries to act when it's player 1's turn
      const result = executePlayerAction(state, 'p2', PlayerAction.Fold);
      expect(result.success).toBe(false);
      expect(result.error).toBe('Not your turn');
    });

    it('should fail if player cannot act (folded)', () => {
      const { state, player1 } = setupTwoPlayerGame();
      player1.status = PlayerStatus.Folded;
      const result = executePlayerAction(state, 'p1', PlayerAction.Check);
      expect(result.success).toBe(false);
      expect(result.error).toBe('Player cannot act');
    });

    it('should fail if player cannot act (all-in)', () => {
      const { state, player1 } = setupTwoPlayerGame();
      player1.status = PlayerStatus.AllIn;
      const result = executePlayerAction(state, 'p1', PlayerAction.Check);
      expect(result.success).toBe(false);
      expect(result.error).toBe('Player cannot act');
    });

    it('should fail if action is not available', () => {
      const { state } = setupTwoPlayerGame();
      // Player 1 tries to check when there's a bet (currentBet = 20)
      const result = executePlayerAction(state, 'p1', PlayerAction.Check);
      expect(result.success).toBe(false);
      expect(result.error).toContain('Action check not available');
    });
  });

  describe('executePlayerAction - Fold', () => {
    it('should successfully fold', () => {
      const { state, player1 } = setupTwoPlayerGame();
      const result = executePlayerAction(state, 'p1', PlayerAction.Fold);

      expect(result.success).toBe(true);
      expect(result.action).toBe(PlayerAction.Fold);
      expect(result.amount).toBe(0);
      expect(player1.status).toBe(PlayerStatus.Folded);
      expect(player1.hasActed).toBe(true);
      expect(player1.lastAction).toBe(PlayerAction.Fold);
      expect(state.actionsThisRound).toBe(1);
    });

    it('should preserve chips when folding', () => {
      const { state, player1 } = setupTwoPlayerGame();
      const chipsBefore = player1.chips;
      executePlayerAction(state, 'p1', PlayerAction.Fold);
      expect(player1.chips).toBe(chipsBefore);
    });
  });

  describe('executePlayerAction - Check', () => {
    it('should successfully check when no bet', () => {
      const { state, player1 } = setupTwoPlayerGame();
      // Set current bet to 0 and player's current bet to 0
      state.currentBet = 0;
      player1.currentBet = 0;

      const result = executePlayerAction(state, 'p1', PlayerAction.Check);

      expect(result.success).toBe(true);
      expect(result.action).toBe(PlayerAction.Check);
      expect(result.amount).toBe(0);
      expect(player1.hasActed).toBe(true);
      expect(player1.lastAction).toBe(PlayerAction.Check);
    });

    it('should successfully check when bet is matched', () => {
      const { state, player1 } = setupTwoPlayerGame();
      // Player has already matched the current bet
      player1.currentBet = 20;
      state.currentBet = 20;

      const result = executePlayerAction(state, 'p1', PlayerAction.Check);

      expect(result.success).toBe(true);
      expect(result.action).toBe(PlayerAction.Check);
    });

    it('should not modify chips when checking', () => {
      const { state, player1 } = setupTwoPlayerGame();
      state.currentBet = 0;
      player1.currentBet = 0;
      const chipsBefore = player1.chips;

      executePlayerAction(state, 'p1', PlayerAction.Check);

      expect(player1.chips).toBe(chipsBefore);
    });
  });

  describe('executePlayerAction - Call', () => {
    it('should successfully call a bet', () => {
      const { state, player1 } = setupTwoPlayerGame();
      // Player 1 needs to call 10 more (currentBet is 20, player1's bet is 10)

      const result = executePlayerAction(state, 'p1', PlayerAction.Call);

      expect(result.success).toBe(true);
      expect(result.action).toBe(PlayerAction.Call);
      expect(result.amount).toBe(10); // Called 10 more
      expect(player1.chips).toBe(990);
      expect(player1.currentBet).toBe(20);
      expect(player1.totalBetThisHand).toBe(20);
      expect(player1.hasActed).toBe(true);
      expect(player1.lastAction).toBe(PlayerAction.Call);
    });

    it('should call all-in when chips are less than call amount', () => {
      const { state, player1 } = setupTwoPlayerGame();
      player1.chips = 5; // Only 5 chips, need to call 10
      state.currentBet = 20;
      player1.currentBet = 10;

      // When chips < call amount, call is not available, player should use all-in instead
      const result = executePlayerAction(state, 'p1', PlayerAction.AllIn);

      expect(result.success).toBe(true);
      expect(result.action).toBe(PlayerAction.AllIn);
      expect(result.amount).toBe(5);
      expect(result.playerAllIn).toBe(true);
      expect(player1.chips).toBe(0);
      expect(player1.status).toBe(PlayerStatus.AllIn);
    });

    it('should update pot when calling', () => {
      const { state, player1 } = setupTwoPlayerGame();
      const potBefore = state.pots[0].amount;

      executePlayerAction(state, 'p1', PlayerAction.Call);

      expect(state.pots[0].amount).toBe(potBefore + 10);
    });
  });

  describe('executePlayerAction - Bet', () => {
    it('should successfully place a bet', () => {
      const { state, player1 } = setupTwoPlayerGame();
      state.currentBet = 0;
      player1.currentBet = 0;

      const result = executePlayerAction(state, 'p1', PlayerAction.Bet, 50);

      expect(result.success).toBe(true);
      expect(result.action).toBe(PlayerAction.Bet);
      expect(result.amount).toBe(50);
      expect(player1.chips).toBe(950);
      expect(player1.currentBet).toBe(50);
      expect(state.currentBet).toBe(50);
      expect(state.minRaise).toBe(50);
    });

    it('should fail bet when there is already a bet', () => {
      const { state, player1 } = setupTwoPlayerGame();
      state.currentBet = 20; // Already a bet

      const result = executePlayerAction(state, 'p1', PlayerAction.Bet, 50);

      expect(result.success).toBe(false);
      expect(result.error).toContain('not available'); // Bet is not in available actions when there's already a bet
    });

    it('should fail bet without amount', () => {
      const { state, player1 } = setupTwoPlayerGame();
      state.currentBet = 0;
      player1.currentBet = 0;

      const result = executePlayerAction(state, 'p1', PlayerAction.Bet);

      expect(result.success).toBe(false);
      expect(result.error).toBe('Bet amount required');
    });

    it('should fail bet below minimum', () => {
      const { state, player1 } = setupTwoPlayerGame();
      state.currentBet = 0;
      player1.currentBet = 0;
      // Min bet is big blind (20)

      const result = executePlayerAction(state, 'p1', PlayerAction.Bet, 15);

      expect(result.success).toBe(false);
      expect(result.error).toContain('Minimum bet');
    });

    it('should allow bet equal to chips even if below minimum', () => {
      const { state, player1 } = setupTwoPlayerGame();
      state.currentBet = 0;
      player1.currentBet = 0;
      player1.chips = 15; // Less than min bet (20)

      const result = executePlayerAction(state, 'p1', PlayerAction.Bet, 15);

      expect(result.success).toBe(true);
      expect(result.action).toBe(PlayerAction.AllIn);
      expect(player1.chips).toBe(0);
      expect(player1.status).toBe(PlayerStatus.AllIn);
    });

    it('should reset hasActed for other players after bet', () => {
      const { state, player1, player2 } = setupTwoPlayerGame();
      state.currentBet = 0;
      player1.currentBet = 0;
      player2.currentBet = 0;
      player2.hasActed = true;

      executePlayerAction(state, 'p1', PlayerAction.Bet, 50);

      expect(player2.hasActed).toBe(false);
    });
  });

  describe('executePlayerAction - Raise', () => {
    it('should successfully raise', () => {
      const { state, player1 } = setupTwoPlayerGame();
      // currentBet = 20, minRaise = 20, so min raise to = 40
      // Player 1 has currentBet = 10, so raising to 60 means putting in 50 more

      const result = executePlayerAction(state, 'p1', PlayerAction.Raise, 60);

      expect(result.success).toBe(true);
      expect(result.action).toBe(PlayerAction.Raise);
      expect(result.amount).toBe(50); // Put in 50 more chips
      expect(player1.chips).toBe(950);
      expect(player1.currentBet).toBe(60);
      expect(state.currentBet).toBe(60);
    });

    it('should fail raise when no bet', () => {
      const { state, player1 } = setupTwoPlayerGame();
      state.currentBet = 0;
      player1.currentBet = 0;

      const result = executePlayerAction(state, 'p1', PlayerAction.Raise, 40);

      expect(result.success).toBe(false);
      expect(result.error).toContain('not available'); // Raise is not available when there's no bet
    });

    it('should fail raise without amount', () => {
      const { state } = setupTwoPlayerGame();

      const result = executePlayerAction(state, 'p1', PlayerAction.Raise);

      expect(result.success).toBe(false);
      expect(result.error).toBe('Raise amount required');
    });

    it('should fail raise below minimum', () => {
      const { state, player1 } = setupTwoPlayerGame();
      // currentBet = 20, minRaise = 20, so min raise to = 40
      // Raising to 30 is not enough

      const result = executePlayerAction(state, 'p1', PlayerAction.Raise, 30);

      expect(result.success).toBe(false);
      expect(result.error).toContain('Minimum raise');
    });

    it('should allow all-in raise below minimum', () => {
      const { state, player1 } = setupTwoPlayerGame();
      player1.chips = 25; // Can only afford to raise to 35
      // currentBet = 20, player1 has currentBet = 10

      const result = executePlayerAction(state, 'p1', PlayerAction.Raise, 35);

      expect(result.success).toBe(true);
      expect(result.action).toBe(PlayerAction.AllIn);
      expect(player1.chips).toBe(0);
    });

    it('should update minRaise after raise', () => {
      const { state, player1 } = setupTwoPlayerGame();
      // currentBet = 20, raise to 60 (raise of 40)

      executePlayerAction(state, 'p1', PlayerAction.Raise, 60);

      expect(state.lastRaiseAmount).toBe(40);
      expect(state.minRaise).toBe(40);
    });

    it('should reset hasActed for other players after raise', () => {
      const { state, player1, player2 } = setupTwoPlayerGame();
      player2.hasActed = true;

      executePlayerAction(state, 'p1', PlayerAction.Raise, 60);

      expect(player2.hasActed).toBe(false);
    });
  });

  describe('executePlayerAction - All-In', () => {
    it('should successfully go all-in', () => {
      const { state, player1 } = setupTwoPlayerGame();

      const result = executePlayerAction(state, 'p1', PlayerAction.AllIn);

      expect(result.success).toBe(true);
      expect(result.action).toBe(PlayerAction.AllIn);
      expect(result.amount).toBe(1000);
      expect(result.playerAllIn).toBe(true);
      expect(player1.chips).toBe(0);
      expect(player1.status).toBe(PlayerStatus.AllIn);
      expect(player1.currentBet).toBe(1010); // Previous 10 + all-in 1000
    });

    it('should fail all-in with no chips', () => {
      const { state, player1 } = setupTwoPlayerGame();
      player1.chips = 0;

      const result = executePlayerAction(state, 'p1', PlayerAction.AllIn);

      expect(result.success).toBe(false);
      expect(result.error).toContain('not available'); // All-in is not available when player has no chips
    });

    it('should update currentBet if all-in is higher', () => {
      const { state, player1 } = setupTwoPlayerGame();
      // currentBet = 20, player goes all-in for 1000 + 10 = 1010

      executePlayerAction(state, 'p1', PlayerAction.AllIn);

      expect(state.currentBet).toBe(1010);
    });

    it('should not increase currentBet if all-in is lower', () => {
      const { state, player1 } = setupTwoPlayerGame();
      player1.chips = 5; // All-in would only bring total bet to 15
      state.currentBet = 20;

      executePlayerAction(state, 'p1', PlayerAction.AllIn);

      expect(state.currentBet).toBe(20); // Unchanged
    });

    it('should reset hasActed for others if all-in is a raise', () => {
      const { state, player1, player2 } = setupTwoPlayerGame();
      player2.hasActed = true;

      executePlayerAction(state, 'p1', PlayerAction.AllIn);

      expect(player2.hasActed).toBe(false);
    });

    it('should not reset hasActed if all-in is not a raise', () => {
      const { state, player1, player2 } = setupTwoPlayerGame();
      player1.chips = 5; // Can only contribute 15 total, less than current bet of 20
      player2.hasActed = true;

      executePlayerAction(state, 'p1', PlayerAction.AllIn);

      expect(player2.hasActed).toBe(true); // Unchanged
    });
  });

  describe('autoFold', () => {
    it('should automatically fold a player', () => {
      const { state, player1 } = setupTwoPlayerGame();

      const result = autoFold(state, 'p1');

      expect(result.success).toBe(true);
      expect(result.action).toBe(PlayerAction.Fold);
      expect(player1.status).toBe(PlayerStatus.Folded);
    });

    it('should fail for non-existent player', () => {
      const { state } = setupTwoPlayerGame();

      const result = autoFold(state, 'nonexistent');

      expect(result.success).toBe(false);
      expect(result.error).toBe('Player not found');
    });
  });

  describe('getCallAmount', () => {
    it('should return correct call amount', () => {
      const { state, player1 } = setupTwoPlayerGame();
      // currentBet = 20, player1's currentBet = 10

      const callAmount = getCallAmount(state, 'p1');

      expect(callAmount).toBe(10);
    });

    it('should return 0 when bet is matched', () => {
      const { state, player1 } = setupTwoPlayerGame();
      player1.currentBet = 20;
      state.currentBet = 20;

      const callAmount = getCallAmount(state, 'p1');

      expect(callAmount).toBe(0);
    });

    it('should cap call amount at available chips', () => {
      const { state, player1 } = setupTwoPlayerGame();
      player1.chips = 5;
      state.currentBet = 100;
      player1.currentBet = 0;

      const callAmount = getCallAmount(state, 'p1');

      expect(callAmount).toBe(5); // All their chips
    });

    it('should return 0 for non-existent player', () => {
      const { state } = setupTwoPlayerGame();

      const callAmount = getCallAmount(state, 'nonexistent');

      expect(callAmount).toBe(0);
    });
  });

  describe('getActionInfo', () => {
    it('should return correct action info when bet exists', () => {
      const { state, player1 } = setupTwoPlayerGame();
      // currentBet = 20, player1 currentBet = 10, chips = 1000

      const info = getActionInfo(state, 'p1');

      expect(info.canCheck).toBe(false);
      expect(info.canCall).toBe(true);
      expect(info.canBet).toBe(false);
      expect(info.canRaise).toBe(true);
      expect(info.canAllIn).toBe(true);
      expect(info.callAmount).toBe(10);
      expect(info.potTotal).toBeGreaterThan(0);
    });

    it('should return correct action info when no bet', () => {
      const { state, player1 } = setupTwoPlayerGame();
      state.currentBet = 0;
      player1.currentBet = 0;

      const info = getActionInfo(state, 'p1');

      expect(info.canCheck).toBe(true);
      expect(info.canCall).toBe(false);
      expect(info.canBet).toBe(true);
      expect(info.canRaise).toBe(false);
      expect(info.callAmount).toBe(0);
    });

    it('should handle player with limited chips', () => {
      const { state, player1 } = setupTwoPlayerGame();
      player1.chips = 5;
      state.currentBet = 100;

      const info = getActionInfo(state, 'p1');

      expect(info.callAmount).toBe(5); // All chips
      expect(info.canAllIn).toBe(true);
    });
  });

  describe('Pot Updates', () => {
    it('should correctly update pot on call', () => {
      const { state, player1 } = setupTwoPlayerGame();
      const initialPot = state.pots[0].amount;

      executePlayerAction(state, 'p1', PlayerAction.Call);

      expect(state.pots[0].amount).toBe(initialPot + 10);
    });

    it('should correctly update pot on bet', () => {
      const { state, player1 } = setupTwoPlayerGame();
      state.currentBet = 0;
      player1.currentBet = 0;
      const initialPot = state.pots[0].amount;

      executePlayerAction(state, 'p1', PlayerAction.Bet, 100);

      expect(state.pots[0].amount).toBe(initialPot + 100);
    });

    it('should correctly update pot on raise', () => {
      const { state, player1 } = setupTwoPlayerGame();
      const initialPot = state.pots[0].amount;

      executePlayerAction(state, 'p1', PlayerAction.Raise, 60);

      expect(state.pots[0].amount).toBe(initialPot + 50); // Raised 50 more
    });

    it('should correctly update pot on all-in', () => {
      const { state, player1 } = setupTwoPlayerGame();
      const initialPot = state.pots[0].amount;

      executePlayerAction(state, 'p1', PlayerAction.AllIn);

      expect(state.pots[0].amount).toBe(initialPot + 1000);
    });
  });

  describe('Actions This Round Tracking', () => {
    it('should increment actionsThisRound on each action', () => {
      const { state, player1, player2 } = setupTwoPlayerGame();
      expect(state.actionsThisRound).toBe(0);

      executePlayerAction(state, 'p1', PlayerAction.Call);
      expect(state.actionsThisRound).toBe(1);

      state.currentPlayerSeatIndex = 1;
      executePlayerAction(state, 'p2', PlayerAction.Check);
      expect(state.actionsThisRound).toBe(2);
    });
  });

  describe('Complex Scenarios', () => {
    it('should handle pre-flop raise and re-raise', () => {
      const { state, player1, player2 } = setupTwoPlayerGame();

      // Player 1 raises to 60
      let result = executePlayerAction(state, 'p1', PlayerAction.Raise, 60);
      expect(result.success).toBe(true);
      expect(state.currentBet).toBe(60);
      expect(state.minRaise).toBe(40); // Raise was 40

      // Player 2's turn
      state.currentPlayerSeatIndex = 1;

      // Player 2 re-raises to 140 (minRaise is 40, so min re-raise is 60 + 40 = 100, raising to 140 means raise of 80)
      result = executePlayerAction(state, 'p2', PlayerAction.Raise, 140);
      expect(result.success).toBe(true);
      expect(state.currentBet).toBe(140);
    });

    it('should handle three-way all-in correctly', () => {
      const { state, player1, player2, player3 } = setupThreePlayerGame();

      // Player 1 all-in
      let result = executePlayerAction(state, 'p1', PlayerAction.AllIn);
      expect(result.success).toBe(true);
      expect(player1.status).toBe(PlayerStatus.AllIn);

      // Player 2's turn
      state.currentPlayerSeatIndex = 1;
      result = executePlayerAction(state, 'p2', PlayerAction.AllIn);
      expect(result.success).toBe(true);
      expect(player2.status).toBe(PlayerStatus.AllIn);

      // Player 3's turn
      state.currentPlayerSeatIndex = 2;
      result = executePlayerAction(state, 'p3', PlayerAction.AllIn);
      expect(result.success).toBe(true);
      expect(player3.status).toBe(PlayerStatus.AllIn);
    });

    it('should handle post-flop betting correctly', () => {
      const { state, player1, player2 } = setupTwoPlayerGame();

      // Simulate post-flop state
      state.phase = GamePhase.Flop;
      state.currentBet = 0;
      player1.currentBet = 0;
      player2.currentBet = 0;
      player1.hasActed = false;
      player2.hasActed = false;

      // Player 1 checks
      let result = executePlayerAction(state, 'p1', PlayerAction.Check);
      expect(result.success).toBe(true);

      // Player 2 bets
      state.currentPlayerSeatIndex = 1;
      result = executePlayerAction(state, 'p2', PlayerAction.Bet, 50);
      expect(result.success).toBe(true);
      expect(player1.hasActed).toBe(false); // Reset after bet

      // Player 1 calls
      state.currentPlayerSeatIndex = 0;
      result = executePlayerAction(state, 'p1', PlayerAction.Call);
      expect(result.success).toBe(true);
      expect(player1.currentBet).toBe(50);
    });
  });

  describe('Edge Cases', () => {
    it('should handle exact all-in call', () => {
      const { state, player1 } = setupTwoPlayerGame();
      player1.chips = 10; // Exactly enough to call
      player1.currentBet = 10;
      state.currentBet = 20;

      const result = executePlayerAction(state, 'p1', PlayerAction.Call);

      expect(result.success).toBe(true);
      expect(result.playerAllIn).toBe(true);
      expect(player1.chips).toBe(0);
      expect(player1.status).toBe(PlayerStatus.AllIn);
    });

    it('should handle min-raise correctly', () => {
      const { state, player1 } = setupTwoPlayerGame();
      // currentBet = 20, minRaise = 20, so minimum total raise is 40
      // player1 has currentBet = 10, needs 30 more to raise to 40

      const result = executePlayerAction(state, 'p1', PlayerAction.Raise, 40);

      expect(result.success).toBe(true);
      expect(result.amount).toBe(30);
      expect(player1.currentBet).toBe(40);
    });

    it('should handle bet/raise limits correctly', () => {
      const { state, player1 } = setupTwoPlayerGame();
      state.currentBet = 0;
      player1.currentBet = 0;

      // Try to bet more than available chips
      const result = executePlayerAction(state, 'p1', PlayerAction.Bet, 2000);

      expect(result.success).toBe(false);
      expect(result.error).toContain('Maximum bet');
    });
  });
});
