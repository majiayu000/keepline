import { useCallback, useEffect, useRef } from 'react'
import { Client, Session, Socket } from '@heroiclabs/nakama-js'
import { useGameStore } from '../store/gameStore'
import {
  OpCode,
  type PlayerAction,
  type ServerGameStateData,
  type ServerHoleCardsData,
  type ServerPlayerJoinedData,
  type ServerPlayerLeftData,
  type ServerHandStartData,
  type ServerCommunityCardsData,
  type ServerPlayerTurnData,
  type ServerPlayerActedData,
  type ServerPotUpdateData,
  type ServerShowdownData,
  type ServerHandResultData,
  type ServerErrorData,
  type ServerChatData,
  type ServerPlayerDisconnectedData,
  type ServerPlayerReconnectedData,
  type ServerSpectatorJoinedData,
  type ServerSpectatorLeftData,
  type ServerSpectatorToPlayerData,
  type ServerSpectatorListData,
  type TurnInfo,
  type RoomInfo,
  type ListRoomsResponse,
  type UserChipsData,
  type DailyRewardResponse,
  type LeaderboardEntry,
  type LeaderboardResponse,
} from '../types/poker'

const NAKAMA_HOST = 'localhost'
const NAKAMA_PORT = '7350'
const NAKAMA_USE_SSL = false
const NAKAMA_SERVER_KEY = 'defaultkey'

// Reconnection settings
const RECONNECT_MAX_RETRIES = 5
const RECONNECT_INITIAL_DELAY_MS = 1000
const RECONNECT_MAX_DELAY_MS = 30000

export function useNakama() {
  const clientRef = useRef<Client | null>(null)
  const sessionRef = useRef<Session | null>(null)
  const socketRef = useRef<Socket | null>(null)
  const reconnectTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const isReconnectingRef = useRef(false)

  const {
    connectionState,
    setConnectionState,
    setUser,
    matchId,
    setMatchId,
    setBlinds,
    updateGameStateFromServer,
    setMyHoleCards,
    setTurnInfo,
    setShowdownPlayers,
    setWinners,
    clearHandResult,
    addChatMessage,
    setLastError,
    setPlayerDisconnected,
    setPlayerReconnected,
    reconnectInfo,
    setReconnectInfo,
    setIsSpectator,
    setSpectators,
    addSpectator,
    removeSpectator,
    triggerBetAnimation,
    reset,
  } = useGameStore()

  // Initialize client
  useEffect(() => {
    clientRef.current = new Client(
      NAKAMA_SERVER_KEY,
      NAKAMA_HOST,
      NAKAMA_PORT,
      NAKAMA_USE_SSL
    )
  }, [])

  // Handle server messages based on opCode
  const handleMatchData = useCallback((opCode: number, data: unknown) => {
    console.log('Received opCode:', opCode, 'data:', data)

    switch (opCode) {
      case OpCode.GAME_STATE: {
        const gameStateData = data as ServerGameStateData
        updateGameStateFromServer(gameStateData)
        break
      }

      case OpCode.PLAYER_JOINED: {
        const playerData = data as ServerPlayerJoinedData
        console.log('Player joined:', playerData.displayName)
        // Game state update will be handled by next GAME_STATE message
        break
      }

      case OpCode.PLAYER_LEFT: {
        const leftData = data as ServerPlayerLeftData
        console.log('Player left:', leftData.odid, 'sitting out:', leftData.sittingOut)
        // Game state update will be handled by next GAME_STATE message
        break
      }

      case OpCode.HAND_START: {
        const handStartData = data as ServerHandStartData
        console.log('Hand started:', handStartData.handNumber)
        setBlinds(handStartData.smallBlind, handStartData.bigBlind)
        clearHandResult()
        break
      }

      case OpCode.HOLE_CARDS: {
        const holeCardsData = data as ServerHoleCardsData
        console.log('Received hole cards:', holeCardsData.cards)
        setMyHoleCards(holeCardsData.cards)
        break
      }

      case OpCode.COMMUNITY_CARDS: {
        const communityData = data as ServerCommunityCardsData
        console.log('Community cards:', communityData.cards, 'phase:', communityData.phase)
        // Game state update will include these cards
        break
      }

      case OpCode.PLAYER_TURN: {
        const turnData = data as ServerPlayerTurnData
        console.log('Player turn:', turnData.odid)

        // Check if it's our turn
        const { userId } = useGameStore.getState()
        if (turnData.odid === userId) {
          const turnInfo: TurnInfo = {
            canCheck: turnData.canCheck,
            canCall: turnData.canCall,
            canBet: turnData.canBet,
            canRaise: turnData.canRaise,
            callAmount: turnData.callAmount,
            minBet: turnData.minBet,
            minRaise: turnData.minRaise,
            maxBet: turnData.maxBet,
            timeoutSeconds: turnData.timeoutSeconds,
          }
          setTurnInfo(turnInfo)
        } else {
          setTurnInfo(null)
        }
        break
      }

      case OpCode.PLAYER_ACTED: {
        const actedData = data as ServerPlayerActedData
        console.log('Player acted:', actedData.odid, actedData.action, actedData.amount)

        // Trigger chip animation for bet/call/raise/all-in
        if (actedData.amount > 0 && actedData.action !== 'fold') {
          const { getPlayerById } = useGameStore.getState()
          const player = getPlayerById(actedData.odid)
          if (player) {
            triggerBetAnimation(actedData.odid, player.seatIndex, actedData.amount, actedData.action)
          }
        }

        // Game state update will reflect the action
        if (actedData.timeout) {
          console.log('Player timed out (auto-fold)')
        }
        break
      }

      case OpCode.POT_UPDATE: {
        const potData = data as ServerPotUpdateData
        console.log('Pot updated:', potData.total, 'pots:', potData.pots)
        // Game state update will reflect the pot
        break
      }

      case OpCode.SHOWDOWN: {
        const showdownData = data as ServerShowdownData
        console.log('Showdown:', showdownData.players)

        // Convert to client format and enrich with player names
        const { getPlayerById } = useGameStore.getState()
        const showdownPlayers = showdownData.players.map(p => {
          const player = getPlayerById(p.odid)
          return {
            id: p.odid,
            name: player?.name || 'Unknown',
            cards: p.cards,
            handRank: p.handRank,
            handDescription: p.handDescription,
            bestHand: p.bestHand,
          }
        })
        setShowdownPlayers(showdownPlayers)
        break
      }

      case OpCode.HAND_RESULT: {
        const resultData = data as ServerHandResultData
        console.log('Hand result - winners:', resultData.winners)

        // Convert winners to client format
        const { getPlayerById } = useGameStore.getState()
        const winners = resultData.winners.map(w => {
          const player = getPlayerById(w.odid)
          return {
            id: w.odid,
            name: player?.name || 'Unknown',
            amount: w.amount,
            handRank: w.handRank,
            handDescription: w.handDescription,
          }
        })
        setWinners(winners)
        setTurnInfo(null)
        break
      }

      case OpCode.CHAT_MESSAGE: {
        const chatData = data as ServerChatData
        addChatMessage(chatData.odid, chatData.username, chatData.message)
        break
      }

      case OpCode.PLAYER_DISCONNECTED: {
        const disconnectData = data as ServerPlayerDisconnectedData
        console.log('Player disconnected:', disconnectData.displayName, 'grace period:', disconnectData.graceSeconds)
        setPlayerDisconnected(disconnectData.odid)
        addChatMessage('system', 'System', `${disconnectData.displayName} disconnected. Waiting ${disconnectData.graceSeconds}s for reconnect...`)
        break
      }

      case OpCode.PLAYER_RECONNECTED: {
        const reconnectData = data as ServerPlayerReconnectedData
        console.log('Player reconnected:', reconnectData.displayName)
        setPlayerReconnected(reconnectData.odid)
        addChatMessage('system', 'System', `${reconnectData.displayName} reconnected!`)
        break
      }

      case OpCode.SPECTATOR_JOINED: {
        const spectatorData = data as ServerSpectatorJoinedData
        console.log('Spectator joined:', spectatorData.displayName)
        addSpectator({ id: spectatorData.odid, name: spectatorData.displayName })
        addChatMessage('system', 'System', `${spectatorData.displayName} is now watching`)
        break
      }

      case OpCode.SPECTATOR_LEFT: {
        const spectatorData = data as ServerSpectatorLeftData
        console.log('Spectator left:', spectatorData.displayName)
        removeSpectator(spectatorData.odid)
        addChatMessage('system', 'System', `${spectatorData.displayName} stopped watching`)
        break
      }

      case OpCode.SPECTATOR_TO_PLAYER: {
        const spectatorData = data as ServerSpectatorToPlayerData
        const { userId } = useGameStore.getState()
        console.log('Spectator became player:', spectatorData.odid, 'seat:', spectatorData.seatIndex)

        // If this is us, we're no longer a spectator
        if (spectatorData.odid === userId) {
          setIsSpectator(false)
          addChatMessage('system', 'System', 'You are now a player!')
        }
        // Remove from spectator list (will be added to players via GAME_STATE)
        removeSpectator(spectatorData.odid)
        break
      }

      case OpCode.SPECTATOR_LIST: {
        const listData = data as ServerSpectatorListData
        console.log('Spectator list:', listData.spectators)
        setSpectators(listData.spectators.map(s => ({ id: s.odid, name: s.displayName })))
        break
      }

      case OpCode.ERROR: {
        const errorData = data as ServerErrorData
        console.error('Server error:', errorData.message)
        setLastError(errorData.message)
        break
      }

      default:
        console.warn('Unknown opCode:', opCode)
    }
  }, [
    updateGameStateFromServer,
    setBlinds,
    setMyHoleCards,
    setTurnInfo,
    setShowdownPlayers,
    setWinners,
    clearHandResult,
    addChatMessage,
    setLastError,
    setPlayerDisconnected,
    setPlayerReconnected,
    setIsSpectator,
    setSpectators,
    addSpectator,
    removeSpectator,
    triggerBetAnimation,
  ])

  // Authenticate with device ID (guest/anonymous)
  const authenticateGuest = useCallback(async (deviceId?: string) => {
    if (!clientRef.current) return

    setConnectionState('connecting')

    try {
      const id = deviceId || crypto.randomUUID()
      const session = await clientRef.current.authenticateDevice(id, true)
      sessionRef.current = session

      // Get account info for username
      const account = await clientRef.current.getAccount(session)
      const username = account.user?.display_name || account.user?.username || `Player_${id.slice(0, 6)}`

      setUser(session.user_id!, username)
      setConnectionState('connected')

      console.log('Authenticated as guest:', session.user_id, username)
      return session
    } catch (error) {
      console.error('Guest authentication failed:', error)
      setConnectionState('disconnected')
      throw error
    }
  }, [setConnectionState, setUser])

  // Authenticate with email (for returning users)
  const authenticateEmail = useCallback(async (email: string, password: string) => {
    if (!clientRef.current) return

    setConnectionState('connecting')

    try {
      const session = await clientRef.current.authenticateEmail(email, password, false)
      sessionRef.current = session

      // Get account info for username
      const account = await clientRef.current.getAccount(session)
      const username = account.user?.display_name || account.user?.username || email.split('@')[0]

      setUser(session.user_id!, username)
      setConnectionState('connected')

      console.log('Authenticated with email:', session.user_id, username)
      return session
    } catch (error) {
      console.error('Email authentication failed:', error)
      setConnectionState('disconnected')
      throw error
    }
  }, [setConnectionState, setUser])

  // Register with email (for new users)
  const registerEmail = useCallback(async (email: string, password: string, displayName: string) => {
    if (!clientRef.current) return

    setConnectionState('connecting')

    try {
      // Create account with email
      const session = await clientRef.current.authenticateEmail(email, password, true, displayName)
      sessionRef.current = session

      setUser(session.user_id!, displayName)
      setConnectionState('connected')

      console.log('Registered with email:', session.user_id, displayName)
      return session
    } catch (error) {
      console.error('Email registration failed:', error)
      setConnectionState('disconnected')
      throw error
    }
  }, [setConnectionState, setUser])

  // Legacy authenticate function (defaults to guest)
  const authenticate = useCallback(async (deviceId?: string) => {
    return authenticateGuest(deviceId)
  }, [authenticateGuest])

  // Connect to realtime socket
  const connectSocket = useCallback(async () => {
    if (!clientRef.current || !sessionRef.current) {
      throw new Error('Not authenticated')
    }

    const socket = clientRef.current.createSocket(NAKAMA_USE_SSL, false)
    socketRef.current = socket

    // Set up message handlers
    socket.onmatchdata = (matchData) => {
      try {
        const decoder = new TextDecoder()
        const jsonStr = decoder.decode(matchData.data)
        const data = JSON.parse(jsonStr)

        handleMatchData(matchData.op_code, data)
      } catch (error) {
        console.error('Failed to parse match data:', error)
      }
    }

    socket.onmatchpresence = (presences) => {
      console.log('Match presence update:', {
        joins: presences.joins?.length || 0,
        leaves: presences.leaves?.length || 0
      })
    }

    socket.ondisconnect = () => {
      console.log('Socket disconnected')

      // Get current match ID before clearing state
      const currentMatchId = useGameStore.getState().matchId

      // If we were in a match, attempt reconnection
      if (currentMatchId && !isReconnectingRef.current) {
        console.log('Disconnected from match, will attempt reconnection')
        setConnectionState('reconnecting')
        setReconnectInfo({ retries: 0, matchId: currentMatchId })
      } else {
        setConnectionState('connected')
        setMatchId(null)
        setTurnInfo(null)
      }
    }

    socket.onerror = (error) => {
      console.error('Socket error:', error)
    }

    await socket.connect(sessionRef.current, true)
    console.log('Socket connected')
  }, [setConnectionState, setMatchId, handleMatchData, setTurnInfo])

  // Find or create a match
  const findMatch = useCallback(async () => {
    if (!clientRef.current || !sessionRef.current) {
      throw new Error('Not authenticated')
    }

    try {
      const response = await clientRef.current.rpc(
        sessionRef.current,
        'find_match',
        {}
      )

      const result = response.payload as { matchId: string }
      return result.matchId
    } catch (error) {
      console.error('Failed to find match:', error)
      throw error
    }
  }, [])

  // List all available rooms
  const listRooms = useCallback(async (): Promise<RoomInfo[]> => {
    if (!clientRef.current || !sessionRef.current) {
      throw new Error('Not authenticated')
    }

    try {
      const response = await clientRef.current.rpc(
        sessionRef.current,
        'list_rooms',
        {}
      )

      const result = response.payload as ListRoomsResponse
      return result.rooms
    } catch (error) {
      console.error('Failed to list rooms:', error)
      throw error
    }
  }, [])

  // Get user's chip balance and stats
  const getChips = useCallback(async (): Promise<UserChipsData> => {
    if (!clientRef.current || !sessionRef.current) {
      throw new Error('Not authenticated')
    }

    try {
      const response = await clientRef.current.rpc(
        sessionRef.current,
        'get_chips',
        {}
      )

      return response.payload as UserChipsData
    } catch (error) {
      console.error('Failed to get chips:', error)
      throw error
    }
  }, [])

  // Claim daily reward
  const claimDailyReward = useCallback(async (): Promise<DailyRewardResponse> => {
    if (!clientRef.current || !sessionRef.current) {
      throw new Error('Not authenticated')
    }

    try {
      const response = await clientRef.current.rpc(
        sessionRef.current,
        'claim_daily_reward',
        {}
      )

      return response.payload as DailyRewardResponse
    } catch (error) {
      console.error('Failed to claim daily reward:', error)
      throw error
    }
  }, [])

  // Get leaderboard
  const getLeaderboard = useCallback(async (limit: number = 10): Promise<LeaderboardEntry[]> => {
    if (!clientRef.current || !sessionRef.current) {
      throw new Error('Not authenticated')
    }

    try {
      const response = await clientRef.current.rpc(
        sessionRef.current,
        'get_leaderboard',
        { limit }
      )

      const result = response.payload as LeaderboardResponse
      return result.leaderboard
    } catch (error) {
      console.error('Failed to get leaderboard:', error)
      throw error
    }
  }, [])

  // Join a match
  const joinMatch = useCallback(async (targetMatchId: string) => {
    if (!socketRef.current) {
      throw new Error('Socket not connected')
    }

    try {
      const match = await socketRef.current.joinMatch(targetMatchId)
      setMatchId(match.match_id)
      setConnectionState('in_match')
      console.log('Joined match:', match.match_id)
      return match
    } catch (error) {
      console.error('Failed to join match:', error)
      throw error
    }
  }, [setMatchId, setConnectionState])

  // Leave current match
  const leaveMatch = useCallback(async () => {
    if (!socketRef.current || !matchId) return

    try {
      await socketRef.current.leaveMatch(matchId)
      setMatchId(null)
      setConnectionState('connected')
      setTurnInfo(null)
      clearHandResult()
      console.log('Left match')
    } catch (error) {
      console.error('Failed to leave match:', error)
    }
  }, [matchId, setMatchId, setConnectionState, setTurnInfo, clearHandResult])

  // Send player action
  const sendAction = useCallback(async (action: PlayerAction) => {
    if (!socketRef.current || !matchId) {
      throw new Error('Not in a match')
    }

    const data = JSON.stringify(action)
    await socketRef.current.sendMatchState(matchId, OpCode.PLAYER_ACTION, data)
    console.log('Sent action:', action)

    // Clear turn info after sending action
    setTurnInfo(null)
  }, [matchId, setTurnInfo])

  // Send chat message
  const sendChat = useCallback(async (message: string) => {
    if (!socketRef.current || !matchId) {
      throw new Error('Not in a match')
    }

    const data = JSON.stringify({ message })
    await socketRef.current.sendMatchState(matchId, OpCode.CHAT_MESSAGE, data)
    console.log('Sent chat:', message)
  }, [matchId])

  // Request to become a player (for spectators)
  const requestSeat = useCallback(async () => {
    if (!socketRef.current || !matchId) {
      throw new Error('Not in a match')
    }

    await socketRef.current.sendMatchState(matchId, OpCode.REQUEST_SEAT, '{}')
    console.log('Requested seat')
  }, [matchId])

  // Attempt to reconnect to a match
  const attemptReconnect = useCallback(async () => {
    const { reconnectInfo } = useGameStore.getState()

    if (!reconnectInfo || !reconnectInfo.matchId) {
      console.log('No reconnect info, skipping reconnection')
      setReconnectInfo(null)
      setConnectionState('disconnected')
      return
    }

    if (reconnectInfo.retries >= RECONNECT_MAX_RETRIES) {
      console.log('Max reconnect retries reached')
      setReconnectInfo(null)
      setConnectionState('disconnected')
      setMatchId(null)
      setTurnInfo(null)
      setLastError('Connection lost. Please rejoin the match manually.')
      return
    }

    isReconnectingRef.current = true
    const retryDelay = Math.min(
      RECONNECT_INITIAL_DELAY_MS * Math.pow(2, reconnectInfo.retries),
      RECONNECT_MAX_DELAY_MS
    )

    console.log(`Reconnecting attempt ${reconnectInfo.retries + 1}/${RECONNECT_MAX_RETRIES} in ${retryDelay}ms`)
    setReconnectInfo({ ...reconnectInfo, retries: reconnectInfo.retries + 1 })

    reconnectTimeoutRef.current = setTimeout(async () => {
      try {
        // Reconnect socket if needed
        if (!socketRef.current || !socketRef.current) {
          await connectSocket()
        }

        // Rejoin the match
        if (socketRef.current && reconnectInfo.matchId) {
          await socketRef.current.joinMatch(reconnectInfo.matchId)
          console.log('Reconnected to match:', reconnectInfo.matchId)
          setConnectionState('in_match')
          setReconnectInfo(null)
          isReconnectingRef.current = false
          addChatMessage('system', 'System', 'Reconnected successfully!')
        }
      } catch (error) {
        console.error('Reconnection failed:', error)
        isReconnectingRef.current = false
        // Try again
        attemptReconnect()
      }
    }, retryDelay)
  }, [connectSocket, setConnectionState, setMatchId, setTurnInfo, setLastError, setReconnectInfo, addChatMessage])

  // Effect to trigger reconnection when reconnectInfo changes
  useEffect(() => {
    const { reconnectInfo, connectionState } = useGameStore.getState()
    if (reconnectInfo && connectionState === 'reconnecting' && !isReconnectingRef.current) {
      attemptReconnect()
    }
  }, [reconnectInfo, attemptReconnect])

  // Cleanup reconnection timeout on unmount
  useEffect(() => {
    return () => {
      if (reconnectTimeoutRef.current) {
        clearTimeout(reconnectTimeoutRef.current)
      }
    }
  }, [])

  // Cancel reconnection
  const cancelReconnect = useCallback(() => {
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current)
      reconnectTimeoutRef.current = null
    }
    isReconnectingRef.current = false
    setReconnectInfo(null)
    setConnectionState('disconnected')
    setMatchId(null)
    setTurnInfo(null)
  }, [setReconnectInfo, setConnectionState, setMatchId, setTurnInfo])

  // Disconnect everything
  const disconnect = useCallback(() => {
    // Cancel any pending reconnection
    if (reconnectTimeoutRef.current) {
      clearTimeout(reconnectTimeoutRef.current)
      reconnectTimeoutRef.current = null
    }
    isReconnectingRef.current = false

    if (socketRef.current) {
      socketRef.current.disconnect(false)
      socketRef.current = null
    }
    sessionRef.current = null
    reset()
  }, [reset])

  return {
    connectionState,
    reconnectInfo,
    // Authentication
    authenticate,
    authenticateGuest,
    authenticateEmail,
    registerEmail,
    // Socket
    connectSocket,
    // Match
    findMatch,
    listRooms,
    joinMatch,
    leaveMatch,
    // Game actions
    sendAction,
    sendChat,
    requestSeat,
    // User data
    getChips,
    claimDailyReward,
    getLeaderboard,
    // Connection
    disconnect,
    cancelReconnect,
  }
}
