// Card suits and ranks
export type Suit = 'hearts' | 'diamonds' | 'clubs' | 'spades'

// Server sends ranks as numbers (2-14), we convert to display strings
export type RankNumber = 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14
export type RankDisplay = '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' | '10' | 'J' | 'Q' | 'K' | 'A'

export interface Card {
  suit: Suit
  rank: RankNumber  // Server format
}

// Rank conversion for display
export const RANK_DISPLAY: Record<RankNumber, RankDisplay> = {
  2: '2', 3: '3', 4: '4', 5: '5', 6: '6', 7: '7', 8: '8', 9: '9', 10: '10',
  11: 'J', 12: 'Q', 13: 'K', 14: 'A'
}

export function getRankDisplay(rank: RankNumber): RankDisplay {
  return RANK_DISPLAY[rank]
}

// Player state
export type PlayerStatus = 'waiting' | 'active' | 'folded' | 'all_in' | 'sitting_out'

export interface Player {
  id: string          // odid from server
  name: string        // displayName from server
  chips: number
  bet: number         // currentBet from server
  status: PlayerStatus
  cards: Card[]       // holeCards (only visible for current player)
  seatIndex: number
  isDealer: boolean
  isSmallBlind: boolean
  isBigBlind: boolean
  isTurn: boolean     // Computed on client
  lastAction?: ActionType
  isConnected: boolean // Connection status
  avatarUrl?: string  // Optional custom avatar URL
}

// Game phases
export type GamePhase =
  | 'waiting'      // Waiting for players
  | 'pre_flop'     // Before community cards
  | 'flop'         // First 3 community cards
  | 'turn'         // 4th community card
  | 'river'        // 5th community card
  | 'showdown'     // Reveal hands

// Hand rankings (server sends as numbers 1-10)
export type HandRankNumber = 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10
export type HandRankName =
  | 'high_card'
  | 'one_pair'
  | 'two_pair'
  | 'three_of_a_kind'
  | 'straight'
  | 'flush'
  | 'full_house'
  | 'four_of_a_kind'
  | 'straight_flush'
  | 'royal_flush'

export const HAND_RANK_NAMES: Record<HandRankNumber, HandRankName> = {
  1: 'high_card',
  2: 'one_pair',
  3: 'two_pair',
  4: 'three_of_a_kind',
  5: 'straight',
  6: 'flush',
  7: 'full_house',
  8: 'four_of_a_kind',
  9: 'straight_flush',
  10: 'royal_flush'
}

export const HAND_RANK_DISPLAY: Record<HandRankNumber, string> = {
  1: 'High Card',
  2: 'One Pair',
  3: 'Two Pair',
  4: 'Three of a Kind',
  5: 'Straight',
  6: 'Flush',
  7: 'Full House',
  8: 'Four of a Kind',
  9: 'Straight Flush',
  10: 'Royal Flush'
}

// Game state
export interface GameState {
  matchId: string
  phase: GamePhase
  players: Player[]
  communityCards: Card[]
  pot: number           // Total from all pots
  pots: Pot[]           // Individual pots (for side pots)
  currentBet: number
  smallBlind: number
  bigBlind: number
  dealerSeatIndex: number
  currentPlayerSeatIndex: number
  handNumber: number
}

export interface Pot {
  amount: number
  eligiblePlayers: string[]
}

// Player actions
export type ActionType = 'fold' | 'check' | 'call' | 'bet' | 'raise' | 'all_in'

export interface PlayerAction {
  action: ActionType
  amount?: number
}

// Op codes for messages (must match server)
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
} as const

// Server messages (raw format from server)
export interface ServerGameStateData {
  phase: GamePhase
  communityCards: Card[]
  pots: Pot[]
  currentBet: number
  currentPlayerSeat: number
  dealerSeat: number
  handNumber: number
  players: ServerPlayerInfo[]
  spectatorCount?: number
  spectators?: { odid: string; displayName: string }[]
}

export interface ServerPlayerInfo {
  odid: string
  displayName: string
  seatIndex: number
  chips: number
  status: PlayerStatus
  currentBet: number
  isDealer: boolean
  lastAction?: ActionType
  isConnected?: boolean  // May be undefined for backwards compatibility
  avatarUrl?: string     // Optional custom avatar URL
}

export interface ServerHoleCardsData {
  cards: Card[]
}

export interface ServerPlayerJoinedData {
  odid: string
  displayName: string
  seatIndex: number
  chips: number
  status: PlayerStatus
  currentBet: number
  isDealer: boolean
}

export interface ServerPlayerLeftData {
  odid: string
  sittingOut: boolean
  reason?: 'disconnect_timeout'
}

export interface ServerPlayerDisconnectedData {
  odid: string
  displayName: string
  seatIndex: number
  graceSeconds: number
}

export interface ServerPlayerReconnectedData {
  odid: string
  displayName: string
  seatIndex: number
}

export interface ServerHandStartData {
  handNumber: number
  dealerSeat: number
  smallBlind: number
  bigBlind: number
}

export interface ServerCommunityCardsData {
  cards: Card[]
  phase: GamePhase
}

export interface ServerPlayerTurnData {
  odid: string
  seatIndex: number
  timeoutSeconds: number
  canCheck: boolean
  canCall: boolean
  canBet: boolean
  canRaise: boolean
  callAmount: number
  minBet: number
  minRaise: number
  maxBet: number
}

export interface ServerPlayerActedData {
  odid: string
  action: ActionType
  amount: number
  newChips: number
  potTotal: number
  timeout?: boolean
  disconnected?: boolean
}

export interface ServerPotUpdateData {
  pots: Pot[]
  total: number
}

export interface ServerShowdownData {
  players: {
    odid: string
    cards: Card[]
    handRank: HandRankNumber
    handDescription: string
    bestHand: Card[]
  }[]
}

export interface ServerHandResultData {
  winners: {
    odid: string
    amount: number
    hand?: Card[]
    handRank?: HandRankNumber
    handDescription?: string
  }[]
  showdown: {
    odid: string
    cards: Card[]
    handRank: HandRankNumber
    handDescription: string
  }[]
}

export interface ServerErrorData {
  message: string
}

export interface ServerChatData {
  odid: string
  username: string
  message: string
}

// Connection state - includes reconnecting state
export type ConnectionState = 'disconnected' | 'connecting' | 'connected' | 'in_match' | 'reconnecting'

// Turn info (for betting controls)
export interface TurnInfo {
  canCheck: boolean
  canCall: boolean
  canBet: boolean
  canRaise: boolean
  callAmount: number
  minBet: number
  minRaise: number
  maxBet: number
  timeoutSeconds: number
}

// Room info (from list_rooms RPC)
export interface RoomInfo {
  matchId: string
  label: string
  players: number
  maxPlayers: number
  spectators: number
  blinds: string
  phase: string
  createdAt?: number
}

export interface ListRoomsResponse {
  rooms: RoomInfo[]
  total: number
}

// Spectator types
export interface Spectator {
  id: string
  name: string
}

// Server spectator messages
export interface ServerSpectatorJoinedData {
  odid: string
  displayName: string
}

export interface ServerSpectatorLeftData {
  odid: string
  displayName: string
}

export interface ServerSpectatorToPlayerData {
  odid: string
  seatIndex: number
  chips: number
}

export interface ServerSpectatorListData {
  spectators: { odid: string; displayName: string }[]
}

// User chips and stats
export interface UserChipsData {
  balance: number
  totalWon: number
  totalLost: number
  handsPlayed: number
  handsWon: number
}

export interface DailyRewardResponse {
  rewarded: boolean
  amount: number
  balance: number
  nextRewardTime: number
  message: string
}

export interface LeaderboardEntry {
  rank: number
  odid: string
  username: string
  totalWon: number
}

export interface LeaderboardResponse {
  leaderboard: LeaderboardEntry[]
}

// Authentication mode
export type AuthMode = 'guest' | 'email'
