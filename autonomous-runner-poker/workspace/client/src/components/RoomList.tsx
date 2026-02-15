import { useState, useEffect } from 'react'
import type { RoomInfo } from '../types/poker'

interface RoomListProps {
  rooms: RoomInfo[]
  isLoading: boolean
  onJoinRoom: (matchId: string) => void
  onRefresh: () => void
  onQuickMatch: () => void
}

function getPhaseDisplay(phase: string): { label: string; color: string } {
  switch (phase) {
    case 'waiting':
      return { label: 'Waiting', color: 'text-gray-400' }
    case 'pre_flop':
    case 'flop':
    case 'turn':
    case 'river':
      return { label: 'In Game', color: 'text-green-400' }
    case 'showdown':
      return { label: 'Showdown', color: 'text-yellow-400' }
    default:
      return { label: phase, color: 'text-gray-400' }
  }
}

export default function RoomList({
  rooms,
  isLoading,
  onJoinRoom,
  onRefresh,
  onQuickMatch,
}: RoomListProps) {
  const [lastRefresh, setLastRefresh] = useState<Date>(new Date())

  // Update last refresh time when rooms change
  useEffect(() => {
    if (!isLoading) {
      setLastRefresh(new Date())
    }
  }, [rooms, isLoading])

  return (
    <div className="w-full max-w-4xl mx-auto">
      {/* Header */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-2 md:gap-0 mb-4 md:mb-6">
        <h2 className="text-xl md:text-2xl font-bold text-white">Poker Rooms</h2>
        <div className="flex items-center gap-2 md:gap-4">
          <span className="text-gray-400 text-xs md:text-sm hidden xs:inline">
            {lastRefresh.toLocaleTimeString()}
          </span>
          <button
            onClick={onRefresh}
            disabled={isLoading}
            className="px-3 md:px-4 py-2 bg-gray-700 text-white rounded-lg hover:bg-gray-600 active:bg-gray-500
                       disabled:opacity-50 disabled:cursor-not-allowed transition-colors text-sm md:text-base
                       touch-manipulation"
          >
            {isLoading ? '...' : 'Refresh'}
          </button>
        </div>
      </div>

      {/* Quick Match Button */}
      <div className="mb-4 md:mb-6">
        <button
          onClick={onQuickMatch}
          className="w-full py-3 md:py-4 bg-poker-gold text-black font-bold rounded-lg
                     hover:bg-yellow-400 active:bg-yellow-500 transition-colors text-base md:text-lg
                     touch-manipulation"
        >
          Quick Match
        </button>
      </div>

      {/* Room List */}
      {isLoading && rooms.length === 0 ? (
        <div className="text-center py-8 md:py-12">
          <div className="animate-spin w-10 md:w-12 h-10 md:h-12 border-4 border-poker-gold border-t-transparent rounded-full mx-auto mb-4" />
          <p className="text-gray-400 text-sm md:text-base">Loading rooms...</p>
        </div>
      ) : rooms.length === 0 ? (
        <div className="text-center py-8 md:py-12 bg-black/30 rounded-lg">
          <p className="text-gray-400 text-base md:text-lg mb-2 md:mb-4">No rooms available</p>
          <p className="text-gray-500 text-sm md:text-base">Tap "Quick Match" to create a new table</p>
        </div>
      ) : (
        <div className="space-y-2 md:space-y-3">
          {rooms.map((room) => {
            const phaseInfo = getPhaseDisplay(room.phase)
            const isFull = room.players >= room.maxPlayers
            const spotsLeft = room.maxPlayers - room.players

            return (
              <div
                key={room.matchId}
                className="bg-black/40 border border-gray-700 rounded-lg p-3 md:p-4 hover:border-poker-gold/50 active:border-poker-gold/70 transition-colors"
              >
                <div className="flex items-start md:items-center justify-between gap-2 md:gap-4">
                  {/* Room Info */}
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2 md:gap-3 mb-1 md:mb-2">
                      <h3 className="text-white font-semibold text-sm md:text-lg truncate">{room.label}</h3>
                      <span className={`text-xs md:text-sm flex-shrink-0 ${phaseInfo.color}`}>
                        {phaseInfo.label}
                      </span>
                    </div>
                    <div className="flex flex-wrap items-center gap-2 md:gap-4 text-xs md:text-sm">
                      <div className="flex items-center gap-1">
                        <span className="text-gray-500">Players:</span>
                        <span className={`font-mono ${isFull ? 'text-red-400' : 'text-white'}`}>
                          {room.players}/{room.maxPlayers}
                        </span>
                      </div>
                      <div className="flex items-center gap-1">
                        <span className="text-gray-500">Blinds:</span>
                        <span className="text-poker-gold font-mono">${room.blinds}</span>
                      </div>
                      {room.spectators > 0 && (
                        <div className="flex items-center gap-1">
                          <svg className="w-3 h-3 md:w-4 md:h-4 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
                            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
                          </svg>
                          <span className="text-gray-400">{room.spectators}</span>
                        </div>
                      )}
                      {!isFull && (
                        <span className="text-green-400 hidden xs:inline">
                          {spotsLeft} {spotsLeft === 1 ? 'spot' : 'spots'}
                        </span>
                      )}
                    </div>
                  </div>

                  {/* Join Button */}
                  <div className="flex flex-col xs:flex-row gap-1 md:gap-2 flex-shrink-0">
                    {!isFull && (
                      <button
                        onClick={() => onJoinRoom(room.matchId)}
                        className="px-4 md:px-6 py-2 rounded-lg font-semibold transition-colors touch-manipulation
                          bg-green-600 text-white hover:bg-green-500 active:bg-green-700 text-sm md:text-base"
                      >
                        Join
                      </button>
                    )}
                    {isFull && (
                      <button
                        onClick={() => onJoinRoom(room.matchId)}
                        className="px-4 md:px-6 py-2 rounded-lg font-semibold transition-colors touch-manipulation
                          bg-gray-600 text-white hover:bg-gray-500 active:bg-gray-700 text-sm md:text-base"
                        title="Table is full - join as spectator"
                      >
                        Watch
                      </button>
                    )}
                  </div>
                </div>
              </div>
            )
          })}
        </div>
      )}

      {/* Room Count */}
      {rooms.length > 0 && (
        <div className="mt-4 text-center text-gray-500 text-sm">
          {rooms.length} {rooms.length === 1 ? 'room' : 'rooms'} available
        </div>
      )}
    </div>
  )
}
