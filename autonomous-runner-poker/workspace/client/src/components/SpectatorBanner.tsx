import { useGameStore } from '../store/gameStore'
import { useNakama } from '../hooks/useNakama'

export function SpectatorBanner() {
  const { isSpectator, spectators, gameState } = useGameStore()
  const { requestSeat } = useNakama()

  // Check if there are available seats
  const maxPlayers = 9 // Default max players
  const currentPlayers = gameState?.players?.length || 0
  const hasAvailableSeat = currentPlayers < maxPlayers

  const handleRequestSeat = async () => {
    try {
      await requestSeat()
    } catch (error) {
      console.error('Failed to request seat:', error)
    }
  }

  if (!isSpectator) return null

  return (
    <div className="fixed top-0 left-0 right-0 z-50 bg-yellow-500/90 text-black px-4 py-2">
      <div className="max-w-4xl mx-auto flex items-center justify-between">
        <div className="flex items-center gap-2">
          <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
          </svg>
          <span className="font-semibold">Spectator Mode</span>
          <span className="text-sm opacity-80">
            {spectators.length > 1
              ? `(${spectators.length} watching)`
              : '(watching)'}
          </span>
        </div>

        {hasAvailableSeat && (
          <button
            onClick={handleRequestSeat}
            className="bg-green-600 hover:bg-green-700 text-white px-4 py-1 rounded font-medium transition-colors"
          >
            Join as Player
          </button>
        )}

        {!hasAvailableSeat && (
          <span className="text-sm opacity-80">
            Table is full - waiting for available seat
          </span>
        )}
      </div>
    </div>
  )
}
