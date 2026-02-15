import { useState, useEffect, useCallback } from 'react'
import { useNakama } from './hooks/useNakama'
import { useGameStore } from './store/gameStore'
import { SoundProvider, useSound } from './contexts/SoundContext'
import { ToastProvider, useToast } from './contexts/ToastContext'
import Table from './components/Table'
import RoomList from './components/RoomList'
import { SpectatorBanner } from './components/SpectatorBanner'
import { SoundSettings } from './components/SoundSettings'
import { AuthForm } from './components/AuthForm'
import { UserPanel } from './components/UserPanel'
import ToastContainer from './components/Toast'
import { OfflineIndicator } from './components/OfflineIndicator'
import { PWAUpdatePrompt } from './components/PWAUpdatePrompt'
import type { RoomInfo } from './types/poker'

function AppContent() {
  const [isConnecting, setIsConnecting] = useState(false)
  const [rooms, setRooms] = useState<RoomInfo[]>([])
  const [isLoadingRooms, setIsLoadingRooms] = useState(false)

  const {
    connectionState,
    reconnectInfo,
    authenticateGuest,
    authenticateEmail,
    registerEmail,
    connectSocket,
    findMatch,
    listRooms,
    joinMatch,
    getChips,
    claimDailyReward,
    cancelReconnect
  } = useNakama()
  const { username, matchId, gameState, setUserChips, setAuthMode } = useGameStore()
  const { initAudio } = useSound()
  const toast = useToast()

  // Fetch user chips after authentication
  const fetchUserChips = useCallback(async () => {
    try {
      const chips = await getChips()
      setUserChips(chips)
    } catch (err) {
      console.error('Failed to fetch chips:', err)
    }
  }, [getChips, setUserChips])

  const handleGuestLogin = async () => {
    setIsConnecting(true)
    initAudio()

    try {
      await authenticateGuest()
      await connectSocket()
      setAuthMode('guest')
      await fetchUserChips()
      toast.success('Connected as guest!')
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Connection failed')
      throw err
    } finally {
      setIsConnecting(false)
    }
  }

  const handleEmailLogin = async (email: string, password: string) => {
    setIsConnecting(true)
    initAudio()

    try {
      await authenticateEmail(email, password)
      await connectSocket()
      setAuthMode('email')
      await fetchUserChips()
      toast.success('Welcome back!')
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Login failed')
      throw err
    } finally {
      setIsConnecting(false)
    }
  }

  const handleEmailRegister = async (email: string, password: string, displayName: string) => {
    setIsConnecting(true)
    initAudio()

    try {
      await registerEmail(email, password, displayName)
      await connectSocket()
      setAuthMode('email')
      await fetchUserChips()
      toast.success('Account created! You received 10,000 starting chips!')
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Registration failed')
      throw err
    } finally {
      setIsConnecting(false)
    }
  }

  const handleClaimDailyReward = async () => {
    const result = await claimDailyReward()
    if (result.rewarded) {
      toast.success(result.message)
    }
    return result
  }

  // Fetch rooms when connected
  const fetchRooms = useCallback(async () => {
    setIsLoadingRooms(true)
    try {
      const roomList = await listRooms()
      setRooms(roomList)
    } catch (err) {
      console.error('Failed to fetch rooms:', err)
    } finally {
      setIsLoadingRooms(false)
    }
  }, [listRooms])

  // Fetch rooms when we become connected
  useEffect(() => {
    if (connectionState === 'connected') {
      fetchRooms()
    }
  }, [connectionState, fetchRooms])

  const handleQuickMatch = async () => {
    try {
      toast.info('Finding a table...')
      const matchId = await findMatch()
      await joinMatch(matchId)
      toast.success('Joined table!')
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Failed to find match')
    }
  }

  const handleJoinRoom = async (roomMatchId: string) => {
    try {
      await joinMatch(roomMatchId)
      toast.success('Joined room!')
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Failed to join room')
    }
  }

  // Login screen (not connected)
  if (connectionState === 'disconnected') {
    return (
      <div className="min-h-screen flex items-center justify-center bg-felt-dark p-4">
        <AuthForm
          onGuestLogin={handleGuestLogin}
          onEmailLogin={handleEmailLogin}
          onEmailRegister={handleEmailRegister}
          isLoading={isConnecting}
        />
      </div>
    )
  }

  // Connecting screen
  if (connectionState === 'connecting') {
    return (
      <div className="min-h-screen flex items-center justify-center bg-felt-dark p-4">
        <div className="text-center">
          <h1 className="text-xl md:text-2xl text-white mb-4">Connecting...</h1>
          <div className="animate-spin w-10 md:w-12 h-10 md:h-12 border-4 border-poker-gold border-t-transparent rounded-full mx-auto" />
        </div>
      </div>
    )
  }

  // Reconnecting screen
  if (connectionState === 'reconnecting') {
    return (
      <div className="min-h-screen flex items-center justify-center bg-felt-dark p-4">
        <div className="text-center">
          <h1 className="text-xl md:text-2xl text-white mb-4">Connection Lost</h1>
          <p className="text-gray-400 mb-4 text-sm md:text-base">
            Reconnecting... Attempt {reconnectInfo?.retries || 0} of 5
          </p>
          <div className="animate-spin w-10 md:w-12 h-10 md:h-12 border-4 border-poker-gold border-t-transparent rounded-full mx-auto mb-6" />
          <button
            onClick={cancelReconnect}
            className="px-6 py-2 bg-red-600 text-white font-bold rounded-lg
                       hover:bg-red-700 active:bg-red-800 transition-colors touch-manipulation"
          >
            Cancel
          </button>
        </div>
      </div>
    )
  }

  // Connected but not in match - show lobby with room list
  if (connectionState === 'connected') {
    return (
      <div className="min-h-screen bg-felt-dark p-4 md:p-8">
        {/* Header */}
        <div className="max-w-6xl mx-auto mb-4 md:mb-8">
          <div className="flex flex-col md:flex-row md:items-center justify-between gap-2 md:gap-4">
            <h1 className="text-2xl md:text-4xl font-bold text-poker-gold font-poker text-center md:text-left">
              Texas Hold'em
            </h1>
            <div className="flex items-center justify-center md:justify-end gap-3 md:gap-4">
              <SoundSettings compact />
            </div>
          </div>
        </div>

        {/* Main content: User Panel + Room List */}
        <div className="max-w-6xl mx-auto grid grid-cols-1 lg:grid-cols-4 gap-4 md:gap-6">
          {/* User Panel - Left sidebar on desktop */}
          <div className="lg:col-span-1 order-2 lg:order-1">
            <UserPanel
              onClaimDailyReward={handleClaimDailyReward}
              onRefreshChips={fetchUserChips}
            />
          </div>

          {/* Room List - Main content */}
          <div className="lg:col-span-3 order-1 lg:order-2">
            <RoomList
              rooms={rooms}
              isLoading={isLoadingRooms}
              onJoinRoom={handleJoinRoom}
              onRefresh={fetchRooms}
              onQuickMatch={handleQuickMatch}
            />
          </div>
        </div>
      </div>
    )
  }

  // In match - show table
  return (
    <div className="min-h-screen bg-felt-dark overflow-hidden">
      {/* Spectator banner at the top */}
      <SpectatorBanner />

      <div className="p-2 md:p-4 pt-1 md:pt-2">
        <div className="text-white text-xs md:text-sm mb-1 md:mb-2 flex justify-between items-center">
          <span className="truncate max-w-[100px] md:max-w-none">#{matchId?.slice(0, 6)}</span>
          <div className="flex items-center gap-2 md:gap-4">
            <SoundSettings compact />
            <span className="truncate max-w-[80px] md:max-w-none">{username}</span>
          </div>
        </div>
        <Table gameState={gameState} />
      </div>
    </div>
  )
}

// Main App component wrapped with providers
function App() {
  return (
    <ToastProvider>
      <SoundProvider>
        <OfflineIndicator />
        <AppContent />
        <ToastContainer />
        <PWAUpdatePrompt />
      </SoundProvider>
    </ToastProvider>
  )
}

export default App
