import type { Player as PlayerType, PlayerStatus } from '../types/poker'
import Card from './Card'
import Avatar from './Avatar'

interface PlayerProps {
  player: PlayerType
  isCurrentUser: boolean
  compact?: boolean // For mobile view
}

const STATUS_COLORS: Record<PlayerStatus, string> = {
  waiting: 'bg-gray-500',
  active: 'bg-green-500',
  folded: 'bg-red-500',
  all_in: 'bg-purple-500',
  sitting_out: 'bg-gray-700',
}

const STATUS_LABELS: Record<PlayerStatus, string> = {
  waiting: '',
  active: '',
  folded: 'FOLD',
  all_in: 'ALL-IN',
  sitting_out: 'AWAY',
}

const ACTION_LABELS: Record<string, string> = {
  fold: 'Folded',
  check: 'Check',
  call: 'Call',
  bet: 'Bet',
  raise: 'Raise',
  all_in: 'All-In',
}

export default function Player({ player, isCurrentUser, compact: _compact = false }: PlayerProps) {
  const { id, name, chips, bet, status, cards, isDealer, isSmallBlind, isBigBlind, isTurn, lastAction, isConnected, avatarUrl } = player

  const statusLabel = STATUS_LABELS[status]
  const actionLabel = lastAction ? ACTION_LABELS[lastAction] : null
  const isDisconnected = isConnected === false

  return (
    <div
      className={`relative flex flex-col items-center gap-0.5 md:gap-1 p-1 md:p-2 rounded-lg transition-all duration-300
                  ${isTurn ? 'ring-2 ring-poker-gold ring-offset-1 md:ring-offset-2 ring-offset-transparent animate-pulse' : ''}
                  ${isCurrentUser ? 'bg-felt-light/50' : 'bg-black/30'}
                  ${isDisconnected ? 'opacity-60' : ''}`}
    >
      {/* Disconnected overlay */}
      {isDisconnected && (
        <div className="absolute inset-0 flex items-center justify-center z-10">
          <div className="bg-red-600/90 text-white text-[10px] md:text-xs px-1 md:px-2 py-0.5 md:py-1 rounded font-bold animate-pulse">
            OFFLINE
          </div>
        </div>
      )}
      {/* Cards */}
      <div className="flex gap-0.5 md:gap-1">
        {status === 'active' || status === 'all_in' ? (
          cards.length > 0 ? (
            cards.map((card, index) => (
              <Card
                key={index}
                card={card}
                faceDown={!isCurrentUser}
                small
              />
            ))
          ) : (
            // Show card backs for other players who haven't folded
            <>
              <Card card={null} faceDown small />
              <Card card={null} faceDown small />
            </>
          )
        ) : status === 'folded' ? (
          // Dimmed cards for folded players
          <div className="flex gap-0.5 md:gap-1 opacity-30">
            <Card card={null} faceDown small />
            <Card card={null} faceDown small />
          </div>
        ) : (
          // No cards shown for waiting/sitting out
          <div className="h-10 md:h-14 flex items-center text-gray-500 text-[10px] md:text-xs">
            Waiting...
          </div>
        )}
      </div>

      {/* Player info */}
      <div className="text-center min-w-[50px] md:min-w-[80px]">
        {/* Avatar and name row */}
        <div className="flex items-center justify-center gap-1 md:gap-1.5 mb-0.5 md:mb-1">
          <Avatar
            name={name}
            odid={id}
            avatarUrl={avatarUrl}
            size="sm"
            isOnline={isConnected !== false}
            showStatusDot={isConnected === false}
          />
          <div className="flex flex-col items-start">
            {/* Player name */}
            <div className={`font-bold text-[10px] md:text-xs truncate max-w-[45px] md:max-w-[70px] ${isCurrentUser ? 'text-poker-gold' : 'text-white'}`}>
              {name}
            </div>
            {/* Chips */}
            <div className="text-poker-gold text-[9px] md:text-xs font-mono">
              ${chips.toLocaleString()}
            </div>
          </div>
        </div>

        {/* Status badges */}
        <div className="flex items-center gap-0.5 md:gap-1 justify-center flex-wrap mb-0.5 md:mb-1">
          {statusLabel && (
            <span className={`text-[9px] md:text-xs px-0.5 md:px-1 rounded text-white ${STATUS_COLORS[status]}`}>
              {statusLabel}
            </span>
          )}
          {isDealer && (
            <span className="text-[9px] md:text-xs bg-white text-black px-0.5 md:px-1 rounded font-bold">D</span>
          )}
          {isSmallBlind && (
            <span className="text-[9px] md:text-xs bg-yellow-300 text-black px-0.5 md:px-1 rounded font-bold">SB</span>
          )}
          {isBigBlind && (
            <span className="text-[9px] md:text-xs bg-orange-400 text-black px-0.5 md:px-1 rounded font-bold">BB</span>
          )}
        </div>

        {/* Last action */}
        {actionLabel && !statusLabel && (
          <div className="text-[10px] md:text-xs text-gray-400 mb-0.5 md:mb-1">
            {actionLabel}
          </div>
        )}

        {/* Current bet */}
        {bet > 0 && (
          <div className="text-white text-[9px] md:text-xs bg-black/50 px-1 md:px-2 rounded mt-0.5 md:mt-1">
            ${bet}
          </div>
        )}
      </div>
    </div>
  )
}
