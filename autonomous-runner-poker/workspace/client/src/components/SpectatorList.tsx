import { useGameStore } from '../store/gameStore'

export function SpectatorList() {
  const { spectators } = useGameStore()

  if (spectators.length === 0) return null

  return (
    <div className="bg-gray-800/80 rounded-lg p-3 text-sm">
      <div className="flex items-center gap-2 mb-2 text-gray-400">
        <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 12a3 3 0 11-6 0 3 3 0 016 0z" />
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M2.458 12C3.732 7.943 7.523 5 12 5c4.478 0 8.268 2.943 9.542 7-1.274 4.057-5.064 7-9.542 7-4.477 0-8.268-2.943-9.542-7z" />
        </svg>
        <span>Spectators ({spectators.length})</span>
      </div>
      <div className="flex flex-wrap gap-2">
        {spectators.map((spectator) => (
          <span
            key={spectator.id}
            className="px-2 py-1 bg-gray-700 rounded text-gray-300 text-xs"
          >
            {spectator.name}
          </span>
        ))}
      </div>
    </div>
  )
}
