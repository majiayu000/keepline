import { create } from 'zustand'
import type {
  GameState,
  ConnectionState,
  Player,
  Card,
  TurnInfo,
  ServerGameStateData,
  ServerPlayerInfo,
  HandRankNumber,
  Pot,
  Spectator,
  UserChipsData,
  AuthMode,
} from '../types/poker'

interface ShowdownPlayer {
  id: string
  name: string
  cards: Card[]
  handRank: HandRankNumber
  handDescription: string
  bestHand: Card[]
}

interface Winner {
  id: string
  name: string
  amount: number
  handRank?: HandRankNumber
  handDescription?: string
  seatIndex?: number
}

// Animation event for chip animations
interface BetAnimation {
  playerId: string
  seatIndex: number
  amount: number
  action: string
  timestamp: number
}

interface GameStore {
  // Connection state
  connectionState: ConnectionState
  setConnectionState: (state: ConnectionState) => void

  // Auth mode
  authMode: AuthMode
  setAuthMode: (mode: AuthMode) => void

  // User info
  userId: string | null
  username: string | null
  setUser: (userId: string, username: string) => void

  // User chips/balance
  userChips: UserChipsData | null
  setUserChips: (chips: UserChipsData) => void
  updateChipsBalance: (balance: number) => void

  // Match info
  matchId: string | null
  setMatchId: (matchId: string | null) => void

  // Blinds info (from hand start)
  smallBlind: number
  bigBlind: number
  setBlinds: (small: number, big: number) => void

  // Game state
  gameState: GameState | null
  setGameState: (state: GameState) => void
  updateGameStateFromServer: (data: ServerGameStateData) => void

  // Current player's hole cards (private)
  myHoleCards: Card[]
  setMyHoleCards: (cards: Card[]) => void

  // Turn info
  turnInfo: TurnInfo | null
  setTurnInfo: (info: TurnInfo | null) => void

  // Showdown state
  showdownPlayers: ShowdownPlayer[]
  setShowdownPlayers: (players: ShowdownPlayer[]) => void

  // Hand result
  winners: Winner[]
  setWinners: (winners: Winner[]) => void
  clearHandResult: () => void

  // Chat messages
  chatMessages: { id: string; username: string; message: string; timestamp: number }[]
  addChatMessage: (id: string, username: string, message: string) => void

  // Error handling
  lastError: string | null
  setLastError: (error: string | null) => void

  // Current player (self)
  currentPlayer: Player | null
  getCurrentPlayer: () => Player | null

  // Helpers
  isMyTurn: () => boolean
  getPlayerById: (id: string) => Player | null

  // Connection management
  setPlayerDisconnected: (playerId: string) => void
  setPlayerReconnected: (playerId: string) => void

  // Reconnection state
  reconnectInfo: { retries: number; matchId: string | null } | null
  setReconnectInfo: (info: { retries: number; matchId: string | null } | null) => void

  // Spectator mode
  isSpectator: boolean
  setIsSpectator: (isSpectator: boolean) => void
  spectators: Spectator[]
  setSpectators: (spectators: Spectator[]) => void
  addSpectator: (spectator: Spectator) => void
  removeSpectator: (spectatorId: string) => void

  // Animation events
  lastBetAnimation: BetAnimation | null
  triggerBetAnimation: (playerId: string, seatIndex: number, amount: number, action: string) => void
  clearBetAnimation: () => void

  // Actions
  reset: () => void
}

const initialState = {
  connectionState: 'disconnected' as ConnectionState,
  authMode: 'guest' as AuthMode,
  userId: null as string | null,
  username: null as string | null,
  userChips: null as UserChipsData | null,
  matchId: null as string | null,
  smallBlind: 10,
  bigBlind: 20,
  gameState: null as GameState | null,
  myHoleCards: [] as Card[],
  turnInfo: null as TurnInfo | null,
  showdownPlayers: [] as ShowdownPlayer[],
  winners: [] as Winner[],
  chatMessages: [] as { id: string; username: string; message: string; timestamp: number }[],
  lastError: null as string | null,
  currentPlayer: null as Player | null,
  reconnectInfo: null as { retries: number; matchId: string | null } | null,
  isSpectator: false,
  spectators: [] as Spectator[],
  lastBetAnimation: null as BetAnimation | null,
}

// Helper to convert server player info to client player format
function convertServerPlayer(
  serverPlayer: ServerPlayerInfo,
  currentPlayerSeat: number,
  dealerSeat: number,
  myHoleCards: Card[],
  userId: string | null
): Player {
  const isMe = serverPlayer.odid === userId

  return {
    id: serverPlayer.odid,
    name: serverPlayer.displayName,
    chips: serverPlayer.chips,
    bet: serverPlayer.currentBet,
    status: serverPlayer.status,
    cards: isMe ? myHoleCards : [], // Only show own cards
    seatIndex: serverPlayer.seatIndex,
    isDealer: serverPlayer.isDealer || serverPlayer.seatIndex === dealerSeat,
    isSmallBlind: false, // Will be calculated based on dealer position
    isBigBlind: false,   // Will be calculated based on dealer position
    isTurn: serverPlayer.seatIndex === currentPlayerSeat,
    lastAction: serverPlayer.lastAction,
    isConnected: serverPlayer.isConnected !== false, // Default to true if undefined
    avatarUrl: serverPlayer.avatarUrl, // Custom avatar URL if set
  }
}

// Calculate total pot from pots array
function calculateTotalPot(pots: Pot[] | undefined): number {
  if (!pots || pots.length === 0) return 0
  return pots.reduce((sum, pot) => sum + pot.amount, 0)
}

export const useGameStore = create<GameStore>((set, get) => ({
  ...initialState,

  setConnectionState: (connectionState) => set({ connectionState }),

  setAuthMode: (authMode) => set({ authMode }),

  setUser: (userId, username) => set({ userId, username }),

  setUserChips: (userChips) => set({ userChips }),

  updateChipsBalance: (balance) => {
    const { userChips } = get()
    if (userChips) {
      set({ userChips: { ...userChips, balance } })
    }
  },

  setMatchId: (matchId) => set({ matchId }),

  setBlinds: (smallBlind, bigBlind) => set({ smallBlind, bigBlind }),

  setGameState: (gameState) => {
    const { userId } = get()
    const currentPlayer = gameState.players.find(p => p.id === userId) ?? null
    set({ gameState, currentPlayer })
  },

  updateGameStateFromServer: (data: ServerGameStateData) => {
    const { userId, myHoleCards, matchId, smallBlind, bigBlind } = get()

    // Convert server players to client format
    const players: Player[] = data.players
      .map(sp => convertServerPlayer(sp, data.currentPlayerSeat, data.dealerSeat, myHoleCards, userId))
      .sort((a, b) => a.seatIndex - b.seatIndex)

    // Calculate small blind and big blind positions
    const dealerIdx = players.findIndex(p => p.isDealer)
    if (dealerIdx >= 0 && players.length >= 2) {
      // In heads-up, dealer is small blind
      if (players.length === 2) {
        players[dealerIdx].isSmallBlind = true
        players[(dealerIdx + 1) % 2].isBigBlind = true
      } else {
        // Small blind is next after dealer, big blind is after that
        const activePlayers = players.filter(p => p.status !== 'sitting_out')
        const dealerInActive = activePlayers.findIndex(p => p.isDealer)
        if (dealerInActive >= 0 && activePlayers.length >= 2) {
          const sbIdx = (dealerInActive + 1) % activePlayers.length
          const bbIdx = (dealerInActive + 2) % activePlayers.length
          activePlayers[sbIdx].isSmallBlind = true
          activePlayers[bbIdx].isBigBlind = true
        }
      }
    }

    const safePots = data.pots || [{ amount: 0, eligiblePlayers: [] }]

    const gameState: GameState = {
      matchId: matchId || '',
      phase: data.phase,
      players,
      communityCards: data.communityCards || [],
      pot: calculateTotalPot(safePots),
      pots: safePots,
      currentBet: data.currentBet,
      smallBlind,
      bigBlind,
      dealerSeatIndex: data.dealerSeat,
      currentPlayerSeatIndex: data.currentPlayerSeat,
      handNumber: data.handNumber,
    }

    const currentPlayer = players.find(p => p.id === userId) ?? null

    // Update spectators if provided
    const spectators: Spectator[] = (data.spectators || []).map(s => ({
      id: s.odid,
      name: s.displayName,
    }))

    set({ gameState, currentPlayer, spectators })
  },

  setMyHoleCards: (cards) => {
    set({ myHoleCards: cards })

    // Update current player's cards in game state
    const { gameState, userId } = get()
    if (gameState && userId) {
      const updatedPlayers = gameState.players.map(p =>
        p.id === userId ? { ...p, cards } : p
      )
      set({
        gameState: { ...gameState, players: updatedPlayers },
        currentPlayer: updatedPlayers.find(p => p.id === userId) ?? null,
      })
    }
  },

  setTurnInfo: (turnInfo) => set({ turnInfo }),

  setShowdownPlayers: (showdownPlayers) => set({ showdownPlayers }),

  setWinners: (winners) => set({ winners }),

  clearHandResult: () => set({ showdownPlayers: [], winners: [] }),

  addChatMessage: (id, username, message) => {
    const { chatMessages } = get()
    const newMessage = { id, username, message, timestamp: Date.now() }
    // Keep last 100 messages
    const updatedMessages = [...chatMessages, newMessage].slice(-100)
    set({ chatMessages: updatedMessages })
  },

  setLastError: (lastError) => set({ lastError }),

  getCurrentPlayer: () => {
    const { gameState, userId } = get()
    if (!gameState || !userId) return null
    return gameState.players.find(p => p.id === userId) ?? null
  },

  isMyTurn: () => {
    const { gameState, userId } = get()
    if (!gameState || !userId) return false
    const myPlayer = gameState.players.find(p => p.id === userId)
    return myPlayer?.isTurn ?? false
  },

  getPlayerById: (id) => {
    const { gameState } = get()
    if (!gameState) return null
    return gameState.players.find(p => p.id === id) ?? null
  },

  setPlayerDisconnected: (playerId) => {
    const { gameState } = get()
    if (!gameState) return

    const updatedPlayers = gameState.players.map(p =>
      p.id === playerId ? { ...p, isConnected: false } : p
    )
    set({ gameState: { ...gameState, players: updatedPlayers } })
  },

  setPlayerReconnected: (playerId) => {
    const { gameState } = get()
    if (!gameState) return

    const updatedPlayers = gameState.players.map(p =>
      p.id === playerId ? { ...p, isConnected: true } : p
    )
    set({ gameState: { ...gameState, players: updatedPlayers } })
  },

  setReconnectInfo: (reconnectInfo) => set({ reconnectInfo }),

  // Spectator methods
  setIsSpectator: (isSpectator) => set({ isSpectator }),

  setSpectators: (spectators) => set({ spectators }),

  addSpectator: (spectator) => {
    const { spectators } = get()
    // Avoid duplicates
    if (spectators.some(s => s.id === spectator.id)) return
    set({ spectators: [...spectators, spectator] })
  },

  removeSpectator: (spectatorId) => {
    const { spectators } = get()
    set({ spectators: spectators.filter(s => s.id !== spectatorId) })
  },

  // Animation methods
  triggerBetAnimation: (playerId, seatIndex, amount, action) => {
    set({
      lastBetAnimation: {
        playerId,
        seatIndex,
        amount,
        action,
        timestamp: Date.now(),
      }
    })
  },

  clearBetAnimation: () => set({ lastBetAnimation: null }),

  reset: () => set(initialState),
}))
