/**
 * Texas Hold'em Poker Type Definitions
 */

// Card suits
export enum Suit {
  Hearts = 'hearts',
  Diamonds = 'diamonds',
  Clubs = 'clubs',
  Spades = 'spades'
}

// Card ranks (2-14, where 11=J, 12=Q, 13=K, 14=A)
export type Rank = 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14;

// A single card
export interface Card {
  suit: Suit;
  rank: Rank;
}

// Player states in a game
export enum PlayerStatus {
  Waiting = 'waiting',     // Waiting for next hand
  Active = 'active',       // In current hand
  Folded = 'folded',       // Folded this hand
  AllIn = 'all_in',        // All-in this hand
  SittingOut = 'sitting_out' // Away from table
}

// Player actions
export enum PlayerAction {
  Fold = 'fold',
  Check = 'check',
  Call = 'call',
  Bet = 'bet',
  Raise = 'raise',
  AllIn = 'all_in'
}

// Game phases
export enum GamePhase {
  Waiting = 'waiting',     // Waiting for players
  PreFlop = 'pre_flop',    // Before community cards
  Flop = 'flop',           // 3 community cards
  Turn = 'turn',           // 4th community card
  River = 'river',         // 5th community card
  Showdown = 'showdown'    // Compare hands
}

// Hand rankings
export enum HandRank {
  HighCard = 1,
  OnePair = 2,
  TwoPair = 3,
  ThreeOfAKind = 4,
  Straight = 5,
  Flush = 6,
  FullHouse = 7,
  FourOfAKind = 8,
  StraightFlush = 9,
  RoyalFlush = 10
}

// Player in a match
export interface Player {
  odid: string;          // Nakama user ID
  odisplayName: string;
  seatIndex: number;      // 0-8 position at table
  chips: number;          // Current chip count
  status: PlayerStatus;
  holeCards: Card[];      // 2 private cards
  currentBet: number;     // Bet in current round
  totalBetThisHand: number; // Total bet this hand
  isDealer: boolean;
  isSmallBlind: boolean;
  isBigBlind: boolean;
  lastAction?: PlayerAction;
  hasActed: boolean;      // Has acted this betting round
  // Disconnection tracking
  isConnected: boolean;
  disconnectedAt?: number; // Tick when disconnected
  // Profile
  avatarUrl?: string;     // Optional custom avatar URL
}

// Pot (for side pots)
export interface Pot {
  amount: number;
  eligiblePlayers: string[]; // User IDs eligible for this pot
}

// Spectator in a match
export interface Spectator {
  odid: string;          // Nakama user ID
  displayName: string;
  joinedAt: number;      // Tick when joined
}

// Game state stored in match
export interface GameState {
  // Match info
  matchId: string;
  tickRate: number;
  label: string;

  // Game settings
  minPlayers: number;
  maxPlayers: number;
  smallBlind: number;
  bigBlind: number;
  startingChips: number;
  maxSpectators: number;      // Maximum number of spectators allowed

  // Current game state
  phase: GamePhase;
  players: { [odid: string]: Player }; // keyed by odid (using plain object for Nakama goja compatibility)
  spectators: { [odid: string]: Spectator }; // keyed by odid (using plain object for Nakama goja compatibility)
  deck: Card[];
  communityCards: Card[];
  pots: Pot[];

  // Position tracking
  dealerSeatIndex: number;
  currentPlayerSeatIndex: number;

  // Betting state
  currentBet: number;        // Current bet to match
  minRaise: number;          // Minimum raise amount
  lastRaiseAmount: number;   // Last raise size

  // Timing
  turnStartTick: number;     // When current turn started
  turnTimeoutTicks: number;  // Ticks before auto-fold

  // Round tracking
  handNumber: number;
  actionsThisRound: number;
}

// Messages from client to server
export interface ClientMessage {
  action: PlayerAction;
  amount?: number;  // For bet/raise
}

// Op codes for messages
export const OpCode = {
  // Client -> Server
  PLAYER_ACTION: 1,
  CHAT_MESSAGE: 2,
  SIT_OUT: 3,
  SIT_IN: 4,
  REQUEST_SEAT: 5,        // Spectator requests to become a player

  // Server -> Client
  GAME_STATE: 10,
  PLAYER_JOINED: 11,
  PLAYER_LEFT: 12,
  HAND_START: 13,
  HOLE_CARDS: 14,
  COMMUNITY_CARDS: 15,
  PLAYER_TURN: 16,
  PLAYER_ACTED: 17,
  POT_UPDATE: 18,
  SHOWDOWN: 19,
  HAND_RESULT: 20,
  PLAYER_DISCONNECTED: 21,
  PLAYER_RECONNECTED: 22,
  SPECTATOR_JOINED: 23,   // Spectator joined the match
  SPECTATOR_LEFT: 24,     // Spectator left the match
  SPECTATOR_TO_PLAYER: 25, // Spectator became a player
  SPECTATOR_LIST: 26,     // List of spectators
  ERROR: 99
} as const;

// Server messages
export interface GameStateMessage {
  phase: GamePhase;
  communityCards: Card[];
  pots: Pot[];
  currentBet: number;
  currentPlayerSeat: number;
  players: PublicPlayerInfo[];
}

export interface PublicPlayerInfo {
  odid: string;
  displayName: string;
  seatIndex: number;
  chips: number;
  status: PlayerStatus;
  currentBet: number;
  isDealer: boolean;
  lastAction?: PlayerAction;
  isConnected?: boolean;
  avatarUrl?: string;
}

export interface HoleCardsMessage {
  cards: Card[];
}

export interface PlayerActedMessage {
  odid: string;
  action: PlayerAction;
  amount: number;
  newChips: number;
  potTotal: number;
}

export interface HandResultMessage {
  winners: {
    odid: string;
    amount: number;
    hand?: Card[];
    handRank?: HandRank;
  }[];
  showdown: {
    odid: string;
    cards: Card[];
    handRank: HandRank;
  }[];
}
