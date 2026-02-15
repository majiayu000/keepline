/**
 * Unit tests for Texas Hold'em Game State Management
 */

import { describe, it, expect, beforeEach } from 'vitest';
import {
  getActivePlayers,
  getActingPlayers,
  getEligiblePlayers,
  getNextPlayerSeat,
  getPlayerBySeat,
  moveDealerButton,
  assignBlinds,
  postBlinds,
  dealHoleCards,
  dealCommunityCards,
  startBettingRound,
  isBettingRoundComplete,
  isOnlyOnePlayerLeft,
  shouldGoToShowdown,
  initNewHand,
  startNewHand,
  advancePhase,
  moveToNextPlayer,
  hasPlayerTimedOut,
  calculateSidePots,
  getTotalPot,
  endHandWithSingleWinner,
  getAvailableActions,
  getBetLimits
} from './game_state';
import {
  GameState,
  GamePhase,
  Player,
  PlayerStatus,
  PlayerAction,
  Card,
  Suit,
  Pot
} from './types';
import { createShuffledDeck } from './deck';

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
    deck: createShuffledDeck(),
    communityCards: [],
    pots: [{ amount: 0, eligiblePlayers: [] }],
    dealerSeatIndex: -1,
    currentPlayerSeatIndex: 0,
    currentBet: 0,
    minRaise: 20,
    lastRaiseAmount: 0,
    turnStartTick: 0,
    turnTimeoutTicks: 300,
    handNumber: 0,
    actionsThisRound: 0,
    ...overrides
  };
}

// Helper to setup a standard game with 3 players
function setupThreePlayerGame(): { state: GameState; players: Player[] } {
  const player1 = createPlayer({
    odid: 'p1',
    odisplayName: 'Player 1',
    seatIndex: 0,
    chips: 1000
  });
  const player2 = createPlayer({
    odid: 'p2',
    odisplayName: 'Player 2',
    seatIndex: 1,
    chips: 1000
  });
  const player3 = createPlayer({
    odid: 'p3',
    odisplayName: 'Player 3',
    seatIndex: 2,
    chips: 1000
  });

  const playersMap = new Map<string, Player>();
  playersMap.set('p1', player1);
  playersMap.set('p2', player2);
  playersMap.set('p3', player3);

  const state = createGameState({ players: playersMap });

  return { state, players: [player1, player2, player3] };
}

// ==================== getActivePlayers Tests ====================

describe('getActivePlayers', () => {
  it('should return players with Active or AllIn status', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.Active;
    players[1].status = PlayerStatus.AllIn;
    players[2].status = PlayerStatus.Folded;

    const active = getActivePlayers(state);
    expect(active).toHaveLength(2);
    expect(active.map(p => p.odid)).toContain('p1');
    expect(active.map(p => p.odid)).toContain('p2');
  });

  it('should exclude SittingOut players', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.Active;
    players[1].status = PlayerStatus.SittingOut;
    players[2].status = PlayerStatus.Active;

    const active = getActivePlayers(state);
    expect(active).toHaveLength(2);
    expect(active.map(p => p.odid)).not.toContain('p2');
  });

  it('should return empty array when no active players', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.Folded;
    players[1].status = PlayerStatus.Folded;
    players[2].status = PlayerStatus.SittingOut;

    const active = getActivePlayers(state);
    expect(active).toHaveLength(0);
  });

  it('should sort players by seat index', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].seatIndex = 5;
    players[1].seatIndex = 2;
    players[2].seatIndex = 8;

    const active = getActivePlayers(state);
    expect(active[0].seatIndex).toBe(2);
    expect(active[1].seatIndex).toBe(5);
    expect(active[2].seatIndex).toBe(8);
  });
});

// ==================== getActingPlayers Tests ====================

describe('getActingPlayers', () => {
  it('should return only Active status players (not AllIn)', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.Active;
    players[1].status = PlayerStatus.AllIn;
    players[2].status = PlayerStatus.Active;

    const acting = getActingPlayers(state);
    expect(acting).toHaveLength(2);
    expect(acting.map(p => p.odid)).toContain('p1');
    expect(acting.map(p => p.odid)).toContain('p3');
    expect(acting.map(p => p.odid)).not.toContain('p2');
  });

  it('should exclude folded players', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.Active;
    players[1].status = PlayerStatus.Folded;
    players[2].status = PlayerStatus.Active;

    const acting = getActingPlayers(state);
    expect(acting).toHaveLength(2);
  });
});

// ==================== getEligiblePlayers Tests ====================

describe('getEligiblePlayers', () => {
  it('should return players with chips who are not sitting out', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].chips = 1000;
    players[1].chips = 0; // No chips
    players[2].chips = 500;
    players[2].status = PlayerStatus.SittingOut;

    const eligible = getEligiblePlayers(state);
    expect(eligible).toHaveLength(1);
    expect(eligible[0].odid).toBe('p1');
  });

  it('should include players who have folded but have chips', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.Folded;
    players[0].chips = 500;

    const eligible = getEligiblePlayers(state);
    expect(eligible.map(p => p.odid)).toContain('p1');
  });
});

// ==================== getNextPlayerSeat Tests ====================

describe('getNextPlayerSeat', () => {
  it('should return next seat in clockwise order', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].seatIndex = 0;
    players[1].seatIndex = 1;
    players[2].seatIndex = 2;

    const nextSeat = getNextPlayerSeat(state, 0);
    expect(nextSeat).toBe(1);
  });

  it('should wrap around to first player', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].seatIndex = 0;
    players[1].seatIndex = 1;
    players[2].seatIndex = 2;
    state.maxPlayers = 9;

    const nextSeat = getNextPlayerSeat(state, 2);
    expect(nextSeat).toBe(0);
  });

  it('should skip folded players when skipFolded is true', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].seatIndex = 0;
    players[1].seatIndex = 1;
    players[1].status = PlayerStatus.Folded;
    players[2].seatIndex = 2;

    const nextSeat = getNextPlayerSeat(state, 0, true, false);
    expect(nextSeat).toBe(2);
  });

  it('should skip all-in players when skipAllIn is true', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].seatIndex = 0;
    players[1].seatIndex = 1;
    players[1].status = PlayerStatus.AllIn;
    players[2].seatIndex = 2;

    const nextSeat = getNextPlayerSeat(state, 0, false, true);
    expect(nextSeat).toBe(2);
  });

  it('should return -1 when no valid next player', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.Folded;
    players[1].status = PlayerStatus.Folded;
    players[2].status = PlayerStatus.Folded;

    const nextSeat = getNextPlayerSeat(state, 0);
    expect(nextSeat).toBe(-1);
  });

  it('should handle non-consecutive seats', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].seatIndex = 0;
    players[1].seatIndex = 4;
    players[2].seatIndex = 7;
    state.maxPlayers = 9;

    const nextSeat = getNextPlayerSeat(state, 0);
    expect(nextSeat).toBe(4);
  });
});

// ==================== getPlayerBySeat Tests ====================

describe('getPlayerBySeat', () => {
  it('should return player at specified seat', () => {
    const { state, players } = setupThreePlayerGame();
    players[1].seatIndex = 5;

    const player = getPlayerBySeat(state, 5);
    expect(player).toBeDefined();
    expect(player?.odid).toBe('p2');
  });

  it('should return undefined for empty seat', () => {
    const { state } = setupThreePlayerGame();

    const player = getPlayerBySeat(state, 8);
    expect(player).toBeUndefined();
  });
});

// ==================== moveDealerButton Tests ====================

describe('moveDealerButton', () => {
  it('should assign dealer to first player on first hand', () => {
    const { state, players } = setupThreePlayerGame();
    state.dealerSeatIndex = -1;

    moveDealerButton(state);
    expect(state.dealerSeatIndex).toBe(0);
  });

  it('should move dealer to next player', () => {
    const { state, players } = setupThreePlayerGame();
    state.dealerSeatIndex = 0;

    moveDealerButton(state);
    expect(state.dealerSeatIndex).toBe(1);
  });

  it('should not skip players with no chips (chips check is at hand start)', () => {
    // Note: moveDealerButton uses getNextPlayerSeat which doesn't check chips
    // Chips are checked at getEligiblePlayers (for dealing cards, not dealer rotation)
    const { state, players } = setupThreePlayerGame();
    state.dealerSeatIndex = 0;
    players[1].chips = 0;

    moveDealerButton(state);
    // Dealer moves to next seat regardless of chips
    expect(state.dealerSeatIndex).toBe(1);
  });

  it('should wrap around to first player', () => {
    const { state, players } = setupThreePlayerGame();
    state.dealerSeatIndex = 2;

    moveDealerButton(state);
    expect(state.dealerSeatIndex).toBe(0);
  });
});

// ==================== assignBlinds Tests ====================

describe('assignBlinds', () => {
  it('should assign dealer, SB, and BB correctly for 3+ players', () => {
    const { state, players } = setupThreePlayerGame();
    state.dealerSeatIndex = 0;

    const { smallBlindSeat, bigBlindSeat } = assignBlinds(state);

    expect(smallBlindSeat).toBe(1);
    expect(bigBlindSeat).toBe(2);
    expect(players[0].isDealer).toBe(true);
    expect(players[1].isSmallBlind).toBe(true);
    expect(players[2].isBigBlind).toBe(true);
  });

  it('should handle heads-up correctly (dealer is SB)', () => {
    const player1 = createPlayer({ odid: 'p1', seatIndex: 0, chips: 1000 });
    const player2 = createPlayer({ odid: 'p2', seatIndex: 1, chips: 1000 });

    const playersMap = new Map<string, Player>();
    playersMap.set('p1', player1);
    playersMap.set('p2', player2);

    const state = createGameState({ players: playersMap, dealerSeatIndex: 0 });

    const { smallBlindSeat, bigBlindSeat } = assignBlinds(state);

    expect(smallBlindSeat).toBe(0); // Dealer is SB in heads-up
    expect(bigBlindSeat).toBe(1);
    expect(player1.isDealer).toBe(true);
    expect(player1.isSmallBlind).toBe(true);
    expect(player2.isBigBlind).toBe(true);
  });

  it('should clear previous blind assignments', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].isDealer = true;
    players[0].isSmallBlind = true;
    players[1].isBigBlind = true;
    state.dealerSeatIndex = 1;

    assignBlinds(state);

    expect(players[0].isDealer).toBe(false);
    expect(players[0].isSmallBlind).toBe(false);
    expect(players[1].isDealer).toBe(true);
  });
});

// ==================== postBlinds Tests ====================

describe('postBlinds', () => {
  it('should deduct blinds from players', () => {
    const { state, players } = setupThreePlayerGame();
    state.smallBlind = 10;
    state.bigBlind = 20;
    state.pots = [{ amount: 0, eligiblePlayers: [] }];

    postBlinds(state, 1, 2);

    expect(players[1].chips).toBe(990);
    expect(players[1].currentBet).toBe(10);
    expect(players[2].chips).toBe(980);
    expect(players[2].currentBet).toBe(20);
  });

  it('should update pot amount', () => {
    const { state, players } = setupThreePlayerGame();
    state.smallBlind = 10;
    state.bigBlind = 20;
    state.pots = [{ amount: 0, eligiblePlayers: [] }];

    postBlinds(state, 0, 1);

    expect(state.pots[0].amount).toBe(30);
  });

  it('should handle player with insufficient chips for full blind', () => {
    const { state, players } = setupThreePlayerGame();
    state.smallBlind = 10;
    state.bigBlind = 20;
    players[1].chips = 5; // Less than small blind
    state.pots = [{ amount: 0, eligiblePlayers: [] }];

    postBlinds(state, 1, 2);

    expect(players[1].chips).toBe(0);
    expect(players[1].currentBet).toBe(5);
    expect(state.pots[0].amount).toBe(25); // 5 + 20
  });

  it('should update currentBet and minRaise', () => {
    const { state } = setupThreePlayerGame();
    state.smallBlind = 10;
    state.bigBlind = 20;
    state.pots = [{ amount: 0, eligiblePlayers: [] }];

    postBlinds(state, 0, 1);

    expect(state.currentBet).toBe(20);
    expect(state.minRaise).toBe(20);
  });
});

// ==================== dealHoleCards Tests ====================

describe('dealHoleCards', () => {
  it('should deal 2 cards to each eligible player', () => {
    const { state, players } = setupThreePlayerGame();

    const holeCards = dealHoleCards(state);

    expect(holeCards.size).toBe(3);
    holeCards.forEach((cards) => {
      expect(cards).toHaveLength(2);
    });
  });

  it('should set player status to Active', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.Waiting;

    dealHoleCards(state);

    expect(players[0].status).toBe(PlayerStatus.Active);
  });

  it('should not deal to sitting out players', () => {
    const { state, players } = setupThreePlayerGame();
    players[2].status = PlayerStatus.SittingOut;

    const holeCards = dealHoleCards(state);

    expect(holeCards.has('p3')).toBe(false);
  });

  it('should remove cards from deck', () => {
    const { state, players } = setupThreePlayerGame();
    const initialDeckSize = state.deck.length;

    dealHoleCards(state);

    expect(state.deck.length).toBe(initialDeckSize - 6); // 3 players * 2 cards
  });
});

// ==================== dealCommunityCards Tests ====================

describe('dealCommunityCards', () => {
  it('should deal specified number of cards', () => {
    const { state } = setupThreePlayerGame();
    state.communityCards = [];

    const cards = dealCommunityCards(state, 3);

    expect(cards).toHaveLength(3);
    expect(state.communityCards).toHaveLength(3);
  });

  it('should burn one card before dealing', () => {
    const { state } = setupThreePlayerGame();
    const initialDeckSize = state.deck.length;

    dealCommunityCards(state, 3);

    expect(state.deck.length).toBe(initialDeckSize - 4); // 1 burn + 3 cards
  });

  it('should append to existing community cards', () => {
    const { state } = setupThreePlayerGame();
    state.communityCards = [
      { suit: Suit.Hearts, rank: 14 } // 14 = Ace
    ];

    dealCommunityCards(state, 1);

    expect(state.communityCards).toHaveLength(2);
  });
});

// ==================== startBettingRound Tests ====================

describe('startBettingRound', () => {
  it('should reset player betting state', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].currentBet = 50;
    players[0].hasActed = true;
    players[0].lastAction = PlayerAction.Bet;

    startBettingRound(state);

    expect(players[0].currentBet).toBe(0);
    expect(players[0].hasActed).toBe(false);
    expect(players[0].lastAction).toBeUndefined();
  });

  it('should reset state currentBet', () => {
    const { state } = setupThreePlayerGame();
    state.currentBet = 100;

    startBettingRound(state);

    expect(state.currentBet).toBe(0);
  });

  it('should set first to act after big blind in pre-flop', () => {
    const { state, players } = setupThreePlayerGame();
    state.phase = GamePhase.PreFlop;
    players[2].isBigBlind = true;

    startBettingRound(state);

    expect(state.currentPlayerSeatIndex).toBe(0); // Next after seat 2
  });

  it('should set first to act after dealer post-flop', () => {
    const { state, players } = setupThreePlayerGame();
    state.phase = GamePhase.Flop;
    state.dealerSeatIndex = 0;

    startBettingRound(state);

    expect(state.currentPlayerSeatIndex).toBe(1);
  });
});

// ==================== isBettingRoundComplete Tests ====================

describe('isBettingRoundComplete', () => {
  it('should return true when only one player remains', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.Folded;
    players[1].status = PlayerStatus.Folded;
    players[2].status = PlayerStatus.Active;

    expect(isBettingRoundComplete(state)).toBe(true);
  });

  it('should return false when players have not acted', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].hasActed = true;
    players[0].currentBet = 20;
    players[1].hasActed = false;
    players[1].currentBet = 0;
    state.currentBet = 20;

    expect(isBettingRoundComplete(state)).toBe(false);
  });

  it('should return false when bets are not matched', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].hasActed = true;
    players[0].currentBet = 20;
    players[1].hasActed = true;
    players[1].currentBet = 10; // Not matched
    state.currentBet = 20;

    expect(isBettingRoundComplete(state)).toBe(false);
  });

  it('should return true when all acted and bets matched', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].hasActed = true;
    players[0].currentBet = 20;
    players[1].hasActed = true;
    players[1].currentBet = 20;
    players[2].hasActed = true;
    players[2].currentBet = 20;
    state.currentBet = 20;

    expect(isBettingRoundComplete(state)).toBe(true);
  });

  it('should consider all-in players as matched', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].hasActed = true;
    players[0].currentBet = 100;
    players[1].hasActed = true;
    players[1].currentBet = 50;
    players[1].status = PlayerStatus.AllIn; // All-in with less
    players[2].hasActed = true;
    players[2].currentBet = 100;
    state.currentBet = 100;

    expect(isBettingRoundComplete(state)).toBe(true);
  });
});

// ==================== isOnlyOnePlayerLeft Tests ====================

describe('isOnlyOnePlayerLeft', () => {
  it('should return true when one player left', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.Folded;
    players[1].status = PlayerStatus.Folded;
    players[2].status = PlayerStatus.Active;

    expect(isOnlyOnePlayerLeft(state)).toBe(true);
  });

  it('should return false when multiple players left', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.Active;
    players[1].status = PlayerStatus.AllIn;
    players[2].status = PlayerStatus.Folded;

    expect(isOnlyOnePlayerLeft(state)).toBe(false);
  });

  it('should return true when zero players left', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.Folded;
    players[1].status = PlayerStatus.Folded;
    players[2].status = PlayerStatus.Folded;

    expect(isOnlyOnePlayerLeft(state)).toBe(true);
  });
});

// ==================== shouldGoToShowdown Tests ====================

describe('shouldGoToShowdown', () => {
  it('should return true when all players are all-in', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.AllIn;
    players[1].status = PlayerStatus.AllIn;
    players[2].status = PlayerStatus.Folded;

    expect(shouldGoToShowdown(state)).toBe(true);
  });

  it('should return false when someone can still act', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.AllIn;
    players[1].status = PlayerStatus.Active; // Can still act
    players[2].status = PlayerStatus.Folded;

    expect(shouldGoToShowdown(state)).toBe(false);
  });

  it('should return false when only one player left', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.AllIn;
    players[1].status = PlayerStatus.Folded;
    players[2].status = PlayerStatus.Folded;

    expect(shouldGoToShowdown(state)).toBe(false);
  });
});

// ==================== initNewHand Tests ====================

describe('initNewHand', () => {
  it('should increment hand number', () => {
    const { state } = setupThreePlayerGame();
    state.handNumber = 5;

    initNewHand(state, 100);

    expect(state.handNumber).toBe(6);
  });

  it('should create new shuffled deck', () => {
    const { state } = setupThreePlayerGame();
    state.deck = [];

    initNewHand(state, 100);

    expect(state.deck).toHaveLength(52);
  });

  it('should clear community cards', () => {
    const { state } = setupThreePlayerGame();
    state.communityCards = [
      { suit: Suit.Hearts, rank: 14 }, // 14 = Ace
      { suit: Suit.Spades, rank: 13 }  // 13 = King
    ];

    initNewHand(state, 100);

    expect(state.communityCards).toHaveLength(0);
  });

  it('should reset pots', () => {
    const { state } = setupThreePlayerGame();
    state.pots = [
      { amount: 500, eligiblePlayers: ['p1'] },
      { amount: 200, eligiblePlayers: ['p1', 'p2'] }
    ];

    initNewHand(state, 100);

    expect(state.pots).toHaveLength(1);
    expect(state.pots[0].amount).toBe(0);
  });

  it('should reset player states', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].holeCards = [{ suit: Suit.Hearts, rank: 14 }]; // 14 = Ace
    players[0].currentBet = 100;
    players[0].totalBetThisHand = 200;
    players[0].isDealer = true;
    players[0].hasActed = true;
    players[0].lastAction = PlayerAction.Bet;
    players[0].status = PlayerStatus.Active;

    initNewHand(state, 100);

    expect(players[0].holeCards).toHaveLength(0);
    expect(players[0].currentBet).toBe(0);
    expect(players[0].totalBetThisHand).toBe(0);
    expect(players[0].isDealer).toBe(false);
    expect(players[0].hasActed).toBe(false);
    expect(players[0].lastAction).toBeUndefined();
    expect(players[0].status).toBe(PlayerStatus.Waiting);
  });

  it('should keep sitting out players as sitting out', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.SittingOut;

    initNewHand(state, 100);

    expect(players[0].status).toBe(PlayerStatus.SittingOut);
  });

  it('should keep players with no chips in their status', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].chips = 0;
    players[0].status = PlayerStatus.Active;

    initNewHand(state, 100);

    expect(players[0].status).toBe(PlayerStatus.Active); // Kept original
  });
});

// ==================== startNewHand Tests ====================

describe('startNewHand', () => {
  it('should complete full hand setup sequence', () => {
    const { state, players } = setupThreePlayerGame();
    state.dealerSeatIndex = -1;

    const holeCards = startNewHand(state, 100);

    // Check dealer moved
    expect(state.dealerSeatIndex).toBe(0);

    // Check blinds posted
    expect(state.pots[0].amount).toBe(30); // 10 + 20

    // Check hole cards dealt
    expect(holeCards.size).toBe(3);

    // Check phase is pre-flop
    expect(state.phase).toBe(GamePhase.PreFlop);

    // Check current bet is big blind
    expect(state.currentBet).toBe(20);
  });

  it('should set pot eligible players', () => {
    const { state, players } = setupThreePlayerGame();

    startNewHand(state, 100);

    expect(state.pots[0].eligiblePlayers).toHaveLength(3);
  });
});

// ==================== advancePhase Tests ====================

describe('advancePhase', () => {
  it('should advance from PreFlop to Flop and deal 3 cards', () => {
    const { state } = setupThreePlayerGame();
    state.phase = GamePhase.PreFlop;
    state.communityCards = [];

    const newPhase = advancePhase(state);

    expect(newPhase).toBe(GamePhase.Flop);
    expect(state.communityCards).toHaveLength(3);
  });

  it('should advance from Flop to Turn and deal 1 card', () => {
    const { state } = setupThreePlayerGame();
    state.phase = GamePhase.Flop;
    state.communityCards = [
      { suit: Suit.Hearts, rank: 14 },   // 14 = Ace
      { suit: Suit.Spades, rank: 13 },   // 13 = King
      { suit: Suit.Diamonds, rank: 12 }  // 12 = Queen
    ];

    const newPhase = advancePhase(state);

    expect(newPhase).toBe(GamePhase.Turn);
    expect(state.communityCards).toHaveLength(4);
  });

  it('should advance from Turn to River', () => {
    const { state } = setupThreePlayerGame();
    state.phase = GamePhase.Turn;
    state.communityCards = [
      { suit: Suit.Hearts, rank: 14 },   // 14 = Ace
      { suit: Suit.Spades, rank: 13 },   // 13 = King
      { suit: Suit.Diamonds, rank: 12 }, // 12 = Queen
      { suit: Suit.Clubs, rank: 11 }     // 11 = Jack
    ];

    const newPhase = advancePhase(state);

    expect(newPhase).toBe(GamePhase.River);
    expect(state.communityCards).toHaveLength(5);
  });

  it('should advance from River to Showdown', () => {
    const { state } = setupThreePlayerGame();
    state.phase = GamePhase.River;

    const newPhase = advancePhase(state);

    expect(newPhase).toBe(GamePhase.Showdown);
  });

  it('should start new betting round when not showdown', () => {
    const { state, players } = setupThreePlayerGame();
    state.phase = GamePhase.PreFlop;
    players[0].hasActed = true;
    players[0].currentBet = 50;

    advancePhase(state);

    expect(players[0].hasActed).toBe(false);
    expect(players[0].currentBet).toBe(0);
  });
});

// ==================== moveToNextPlayer Tests ====================

describe('moveToNextPlayer', () => {
  it('should update currentPlayerSeatIndex', () => {
    const { state, players } = setupThreePlayerGame();
    state.currentPlayerSeatIndex = 0;

    const nextSeat = moveToNextPlayer(state, 100);

    expect(nextSeat).toBe(1);
    expect(state.currentPlayerSeatIndex).toBe(1);
  });

  it('should update turnStartTick', () => {
    const { state } = setupThreePlayerGame();
    state.turnStartTick = 50;

    moveToNextPlayer(state, 150);

    expect(state.turnStartTick).toBe(150);
  });
});

// ==================== hasPlayerTimedOut Tests ====================

describe('hasPlayerTimedOut', () => {
  it('should return true when timeout exceeded', () => {
    const { state } = setupThreePlayerGame();
    state.turnStartTick = 0;
    state.turnTimeoutTicks = 300;

    expect(hasPlayerTimedOut(state, 300)).toBe(true);
    expect(hasPlayerTimedOut(state, 400)).toBe(true);
  });

  it('should return false when within timeout', () => {
    const { state } = setupThreePlayerGame();
    state.turnStartTick = 0;
    state.turnTimeoutTicks = 300;

    expect(hasPlayerTimedOut(state, 100)).toBe(false);
    expect(hasPlayerTimedOut(state, 299)).toBe(false);
  });
});

// ==================== calculateSidePots Tests ====================

describe('calculateSidePots', () => {
  it('should not change pots when no all-ins', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].totalBetThisHand = 100;
    players[1].totalBetThisHand = 100;
    players[2].totalBetThisHand = 100;

    calculateSidePots(state);

    // No side pots created when no all-ins
  });

  it('should create side pots for different all-in amounts', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].totalBetThisHand = 50;
    players[0].status = PlayerStatus.AllIn;
    players[1].totalBetThisHand = 100;
    players[1].status = PlayerStatus.AllIn;
    players[2].totalBetThisHand = 100;
    players[2].status = PlayerStatus.Active;

    calculateSidePots(state);

    expect(state.pots.length).toBeGreaterThanOrEqual(2);
  });

  it('should calculate correct pot amounts', () => {
    const player1 = createPlayer({
      odid: 'p1',
      seatIndex: 0,
      chips: 0,
      totalBetThisHand: 30,
      status: PlayerStatus.AllIn
    });
    const player2 = createPlayer({
      odid: 'p2',
      seatIndex: 1,
      chips: 0,
      totalBetThisHand: 60,
      status: PlayerStatus.AllIn
    });
    const player3 = createPlayer({
      odid: 'p3',
      seatIndex: 2,
      chips: 40,
      totalBetThisHand: 60,
      status: PlayerStatus.Active
    });

    const playersMap = new Map<string, Player>();
    playersMap.set('p1', player1);
    playersMap.set('p2', player2);
    playersMap.set('p3', player3);

    const state = createGameState({ players: playersMap });

    calculateSidePots(state);

    // Main pot: 30 * 3 = 90 (all three eligible)
    // Side pot: 30 * 2 = 60 (p2 and p3 eligible)
    expect(state.pots[0].amount).toBe(90);
    expect(state.pots[0].eligiblePlayers).toHaveLength(3);
    expect(state.pots[1].amount).toBe(60);
    expect(state.pots[1].eligiblePlayers).toHaveLength(2);
  });
});

// ==================== getTotalPot Tests ====================

describe('getTotalPot', () => {
  it('should sum all pot amounts', () => {
    const { state } = setupThreePlayerGame();
    state.pots = [
      { amount: 100, eligiblePlayers: ['p1', 'p2', 'p3'] },
      { amount: 50, eligiblePlayers: ['p1', 'p2'] }
    ];

    expect(getTotalPot(state)).toBe(150);
  });

  it('should return 0 for empty pots', () => {
    const { state } = setupThreePlayerGame();
    state.pots = [];

    expect(getTotalPot(state)).toBe(0);
  });
});

// ==================== endHandWithSingleWinner Tests ====================

describe('endHandWithSingleWinner', () => {
  it('should award pot to single remaining player', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.Folded;
    players[1].status = PlayerStatus.Folded;
    players[2].status = PlayerStatus.Active;
    players[2].chips = 500;
    state.pots = [{ amount: 200, eligiblePlayers: [] }];

    const result = endHandWithSingleWinner(state);

    expect(result).not.toBeNull();
    expect(result?.winnerId).toBe('p3');
    expect(result?.amount).toBe(200);
    expect(players[2].chips).toBe(700);
  });

  it('should set phase to Waiting', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.Folded;
    players[1].status = PlayerStatus.Folded;
    players[2].status = PlayerStatus.Active;
    state.pots = [{ amount: 100, eligiblePlayers: [] }];

    endHandWithSingleWinner(state);

    expect(state.phase).toBe(GamePhase.Waiting);
  });

  it('should return null when multiple players remain', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.Active;
    players[1].status = PlayerStatus.Active;
    players[2].status = PlayerStatus.Folded;

    const result = endHandWithSingleWinner(state);

    expect(result).toBeNull();
  });

  it('should handle multiple pots', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.Folded;
    players[1].status = PlayerStatus.Folded;
    players[2].status = PlayerStatus.Active;
    players[2].chips = 500;
    state.pots = [
      { amount: 100, eligiblePlayers: [] },
      { amount: 50, eligiblePlayers: [] }
    ];

    const result = endHandWithSingleWinner(state);

    expect(result?.amount).toBe(150);
    expect(players[2].chips).toBe(650);
  });
});

// ==================== getAvailableActions Tests ====================

describe('getAvailableActions', () => {
  it('should return fold as always available', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.Active;

    const actions = getAvailableActions(state, 'p1');

    expect(actions).toContain(PlayerAction.Fold);
  });

  it('should allow check when no bet to call', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.Active;
    players[0].currentBet = 0;
    state.currentBet = 0;

    const actions = getAvailableActions(state, 'p1');

    expect(actions).toContain(PlayerAction.Check);
    expect(actions).not.toContain(PlayerAction.Call);
  });

  it('should allow call when there is a bet', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.Active;
    players[0].currentBet = 0;
    players[0].chips = 100;
    state.currentBet = 20;

    const actions = getAvailableActions(state, 'p1');

    expect(actions).toContain(PlayerAction.Call);
    expect(actions).not.toContain(PlayerAction.Check);
  });

  it('should allow bet when no current bet', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.Active;
    players[0].chips = 100;
    state.currentBet = 0;

    const actions = getAvailableActions(state, 'p1');

    expect(actions).toContain(PlayerAction.Bet);
    expect(actions).not.toContain(PlayerAction.Raise);
  });

  it('should allow raise when there is a bet', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.Active;
    players[0].currentBet = 0;
    players[0].chips = 100;
    state.currentBet = 20;

    const actions = getAvailableActions(state, 'p1');

    expect(actions).toContain(PlayerAction.Raise);
    expect(actions).not.toContain(PlayerAction.Bet);
  });

  it('should allow all-in when player has chips', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.Active;
    players[0].chips = 100;

    const actions = getAvailableActions(state, 'p1');

    expect(actions).toContain(PlayerAction.AllIn);
  });

  it('should return empty for non-active player', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.Folded;

    const actions = getAvailableActions(state, 'p1');

    expect(actions).toHaveLength(0);
  });

  it('should return empty for non-existent player', () => {
    const { state } = setupThreePlayerGame();

    const actions = getAvailableActions(state, 'nonexistent');

    expect(actions).toHaveLength(0);
  });

  it('should not allow raise when chips only cover call', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].status = PlayerStatus.Active;
    players[0].currentBet = 0;
    players[0].chips = 20; // Exact call amount
    state.currentBet = 20;

    const actions = getAvailableActions(state, 'p1');

    expect(actions).toContain(PlayerAction.Call);
    expect(actions).not.toContain(PlayerAction.Raise);
    expect(actions).toContain(PlayerAction.AllIn);
  });
});

// ==================== getBetLimits Tests ====================

describe('getBetLimits', () => {
  it('should return correct min and max for bet', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].chips = 500;
    state.currentBet = 0;
    state.bigBlind = 20;
    state.minRaise = 20;

    const limits = getBetLimits(state, 'p1');

    expect(limits.min).toBe(20); // Minimum bet is big blind
    expect(limits.max).toBe(500); // Maximum is all chips
  });

  it('should return correct min for raise', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].chips = 500;
    players[0].currentBet = 0;
    state.currentBet = 50;
    state.minRaise = 30;
    state.bigBlind = 20;

    const limits = getBetLimits(state, 'p1');

    // minRaise = currentBet + minRaise = 50 + 30 = 80
    // Actual min = 80 - currentBet(0) = 80
    expect(limits.min).toBe(80);
    expect(limits.max).toBe(500);
  });

  it('should cap min at max when chips are low', () => {
    const { state, players } = setupThreePlayerGame();
    players[0].chips = 30;
    state.currentBet = 0;
    state.bigBlind = 50;
    state.minRaise = 50;

    const limits = getBetLimits(state, 'p1');

    expect(limits.min).toBe(30); // Capped at max chips
    expect(limits.max).toBe(30);
  });

  it('should return zeros for non-existent player', () => {
    const { state } = setupThreePlayerGame();

    const limits = getBetLimits(state, 'nonexistent');

    expect(limits.min).toBe(0);
    expect(limits.max).toBe(0);
  });
});

// ==================== Complex Scenario Tests ====================

describe('Complex Game Scenarios', () => {
  it('should handle complete pre-flop betting round', () => {
    const { state, players } = setupThreePlayerGame();

    // Start new hand
    startNewHand(state, 0);

    // Verify initial state
    expect(state.phase).toBe(GamePhase.PreFlop);
    expect(state.pots[0].amount).toBe(30); // Blinds

    // Simulate betting round completion
    players[0].hasActed = true;
    players[0].currentBet = 20;
    players[1].hasActed = true;
    players[1].currentBet = 20;
    players[2].hasActed = true;
    players[2].currentBet = 20;
    state.currentBet = 20;

    expect(isBettingRoundComplete(state)).toBe(true);

    // Advance to flop
    advancePhase(state);
    expect(state.phase).toBe(GamePhase.Flop);
    expect(state.communityCards).toHaveLength(3);
  });

  it('should handle all-in showdown scenario', () => {
    const { state, players } = setupThreePlayerGame();

    players[0].status = PlayerStatus.AllIn;
    players[0].totalBetThisHand = 100;
    players[1].status = PlayerStatus.AllIn;
    players[1].totalBetThisHand = 200;
    players[2].status = PlayerStatus.Folded;

    // Should go to showdown
    expect(shouldGoToShowdown(state)).toBe(true);

    // Calculate side pots
    calculateSidePots(state);

    // Should have 2 pots
    expect(state.pots.length).toBe(2);
  });

  it('should handle heads-up game correctly', () => {
    const player1 = createPlayer({ odid: 'p1', seatIndex: 0, chips: 1000 });
    const player2 = createPlayer({ odid: 'p2', seatIndex: 1, chips: 1000 });

    const playersMap = new Map<string, Player>();
    playersMap.set('p1', player1);
    playersMap.set('p2', player2);

    const state = createGameState({ players: playersMap });

    startNewHand(state, 0);

    // In heads-up, dealer should be small blind
    expect(player1.isDealer).toBe(true);
    expect(player1.isSmallBlind).toBe(true);
    expect(player2.isBigBlind).toBe(true);
  });

  it('should track multiple hands correctly', () => {
    const { state, players } = setupThreePlayerGame();

    // Play first hand
    startNewHand(state, 0);
    expect(state.handNumber).toBe(1);
    const firstDealer = state.dealerSeatIndex;

    // Reset for next hand
    players[0].status = PlayerStatus.Folded;
    players[1].status = PlayerStatus.Folded;
    endHandWithSingleWinner(state);

    // Play second hand
    startNewHand(state, 100);
    expect(state.handNumber).toBe(2);
    expect(state.dealerSeatIndex).not.toBe(firstDealer);
  });
});
