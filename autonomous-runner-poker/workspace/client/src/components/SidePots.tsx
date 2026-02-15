import type { Pot, Player } from '../types/poker'

interface SidePotsProps {
  pots: Pot[]
  players: Player[]
  totalPot: number
}

export default function SidePots({ pots, players, totalPot }: SidePotsProps) {
  // Only show side pots view if there are multiple pots
  if (!pots || pots.length <= 1) {
    return (
      <div className="text-center bg-black/50 px-4 py-2 rounded-lg">
        <div className="text-poker-gold text-2xl font-bold font-mono">
          Pot: ${totalPot.toLocaleString()}
        </div>
      </div>
    )
  }

  // Helper function to get player name by ID
  const getPlayerName = (playerId: string): string => {
    const player = players.find(p => p.id === playerId)
    return player?.name || 'Unknown'
  }

  // Format eligible players list (show max 3 names, then "+N more")
  const formatEligiblePlayers = (eligiblePlayers: string[]): string => {
    if (eligiblePlayers.length === 0) return ''

    const names = eligiblePlayers.map(getPlayerName)
    if (names.length <= 3) {
      return names.join(', ')
    }
    return `${names.slice(0, 3).join(', ')} +${names.length - 3} more`
  }

  return (
    <div className="text-center bg-black/60 px-4 py-3 rounded-lg min-w-[200px]">
      {/* Total pot header */}
      <div className="text-poker-gold text-2xl font-bold font-mono mb-2">
        Total: ${totalPot.toLocaleString()}
      </div>

      {/* Individual pots */}
      <div className="flex flex-col gap-1.5">
        {pots.map((pot, index) => {
          const isMainPot = index === 0
          const potLabel = isMainPot ? 'Main Pot' : `Side Pot ${index}`

          return (
            <div
              key={index}
              className={`
                flex items-center justify-between gap-3 px-3 py-1.5 rounded
                ${isMainPot
                  ? 'bg-poker-gold/20 border border-poker-gold/40'
                  : 'bg-blue-500/20 border border-blue-500/40'}
              `}
            >
              {/* Pot label and amount */}
              <div className="flex items-center gap-2">
                <span className={`text-xs font-semibold uppercase ${isMainPot ? 'text-poker-gold' : 'text-blue-400'}`}>
                  {potLabel}
                </span>
                <span className="text-white font-mono font-bold">
                  ${pot.amount.toLocaleString()}
                </span>
              </div>

              {/* Eligible players tooltip */}
              {pot.eligiblePlayers.length > 0 && (
                <div className="relative group">
                  <span className="text-white/60 text-xs cursor-help">
                    ({pot.eligiblePlayers.length} players)
                  </span>

                  {/* Tooltip */}
                  <div className="absolute bottom-full left-1/2 transform -translate-x-1/2 mb-2
                                  opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none z-50">
                    <div className="bg-black/90 text-white text-xs px-2 py-1 rounded whitespace-nowrap">
                      {formatEligiblePlayers(pot.eligiblePlayers)}
                    </div>
                  </div>
                </div>
              )}
            </div>
          )
        })}
      </div>
    </div>
  )
}
