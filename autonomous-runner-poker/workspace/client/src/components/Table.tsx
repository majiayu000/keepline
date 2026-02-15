import { useState, useEffect, useRef, useMemo, useCallback } from 'react'
import type { GameState, GamePhase } from '../types/poker'
import { HAND_RANK_DISPLAY } from '../types/poker'
import Player from './Player'
import Card from './Card'
import BettingControls from './BettingControls'
import SidePots from './SidePots'
import { SpectatorList } from './SpectatorList'
import ChipAnimation, { triggerBetAnimation, triggerWinAnimation } from './ChipAnimation'
import VictoryEffect, { WinnerGlow } from './VictoryEffect'
import Chat from './Chat'
import { useGameStore } from '../store/gameStore'
import { useNakama } from '../hooks/useNakama'
import { useSound } from '../contexts/SoundContext'

interface TableProps {
  gameState: GameState | null
}

// Track which community cards are newly dealt for animation
function useNewCommunityCards(_phase: GamePhase, cardCount: number) {
  const prevCountRef = useRef<number>(cardCount)
  const [newCardIndices, setNewCardIndices] = useState<number[]>([])

  useEffect(() => {
    const prevCount = prevCountRef.current

    // Detect new cards being dealt
    if (cardCount > prevCount) {
      // Cards from prevCount to cardCount-1 are new
      const newIndices: number[] = []
      for (let i = prevCount; i < cardCount; i++) {
        newIndices.push(i)
      }
      setNewCardIndices(newIndices)

      // Clear animation flags after animation completes
      setTimeout(() => {
        setNewCardIndices([])
      }, 1000)
    }

    prevCountRef.current = cardCount
  }, [cardCount])

  return newCardIndices
}

// Player positions around an oval table (9 max)
// Position 0 is always at the bottom for the current player
// Desktop positions (percentage-based)
const PLAYER_POSITIONS_DESKTOP = [
  { x: 50, y: 92 },   // 0: Bottom center (current player)
  { x: 15, y: 78 },   // 1: Bottom left
  { x: 5, y: 50 },    // 2: Left
  { x: 15, y: 22 },   // 3: Top left
  { x: 35, y: 8 },    // 4: Top left center
  { x: 65, y: 8 },    // 5: Top right center
  { x: 85, y: 22 },   // 6: Top right
  { x: 95, y: 50 },   // 7: Right
  { x: 85, y: 78 },   // 8: Bottom right
]

// Mobile positions - adjusted for portrait screens
const PLAYER_POSITIONS_MOBILE = [
  { x: 50, y: 88 },   // 0: Bottom center (current player)
  { x: 8, y: 72 },    // 1: Bottom left
  { x: 3, y: 45 },    // 2: Left
  { x: 8, y: 20 },    // 3: Top left
  { x: 30, y: 5 },    // 4: Top left center
  { x: 70, y: 5 },    // 5: Top right center
  { x: 92, y: 20 },   // 6: Top right
  { x: 97, y: 45 },   // 7: Right
  { x: 92, y: 72 },   // 8: Bottom right
]

// Hook to detect mobile screen
function useIsMobile() {
  const [isMobile, setIsMobile] = useState(false)

  useEffect(() => {
    const checkMobile = () => {
      setIsMobile(window.innerWidth < 768)
    }
    checkMobile()
    window.addEventListener('resize', checkMobile)
    return () => window.removeEventListener('resize', checkMobile)
  }, [])

  return isMobile
}

export default function Table({ gameState }: TableProps) {
  const { currentPlayer, turnInfo, winners, showdownPlayers, userId, lastBetAnimation, clearBetAnimation, chatMessages } = useGameStore()
  const { leaveMatch, sendChat } = useNakama()
  const { playSound, playDealSound } = useSound()
  const isMobile = useIsMobile()
  const PLAYER_POSITIONS = isMobile ? PLAYER_POSITIONS_MOBILE : PLAYER_POSITIONS_DESKTOP
  const prevWinnersRef = useRef<number>(0)
  const [showVictoryEffect, setShowVictoryEffect] = useState(false)

  // Chat state
  const [isChatCollapsed, setIsChatCollapsed] = useState(true)
  const [unreadCount, setUnreadCount] = useState(0)
  const lastReadMessageCount = useRef<number>(0)

  // Track unread messages when chat is collapsed
  useEffect(() => {
    if (isChatCollapsed) {
      // Count new messages since last read
      const newMessages = chatMessages.length - lastReadMessageCount.current
      if (newMessages > 0) {
        setUnreadCount(prev => prev + newMessages)
      }
    } else {
      // Chat is open, mark all as read
      setUnreadCount(0)
    }
    lastReadMessageCount.current = chatMessages.length
  }, [chatMessages.length, isChatCollapsed])

  // Reset unread when opening chat
  const handleToggleChat = useCallback(() => {
    if (isChatCollapsed) {
      // Opening chat - clear unread
      setUnreadCount(0)
    }
    setIsChatCollapsed(prev => !prev)
  }, [isChatCollapsed])

  // Handle sending chat message
  const handleSendChat = useCallback((message: string) => {
    sendChat(message)
  }, [sendChat])

  // Track new community cards for animations
  const communityCardCount = gameState?.communityCards?.length ?? 0
  const phase = gameState?.phase ?? 'waiting'
  const newCardIndices = useNewCommunityCards(phase, communityCardCount)

  // Play deal sound for new community cards
  useEffect(() => {
    if (newCardIndices.length > 0) {
      newCardIndices.forEach((_, idx) => {
        setTimeout(() => playDealSound(idx), idx * 150)
      })
    }
  }, [newCardIndices, playDealSound])

  // Calculate rotate offset for chip animations
  const currentUserSeatIndex = currentPlayer?.seatIndex ?? 0
  const rotateOffset = currentUserSeatIndex

  // Handle bet animations
  useEffect(() => {
    if (lastBetAnimation) {
      triggerBetAnimation(lastBetAnimation.seatIndex, lastBetAnimation.amount, rotateOffset)
      clearBetAnimation()
    }
  }, [lastBetAnimation, rotateOffset, clearBetAnimation])

  // Play win sound and trigger effects when winners are announced
  useEffect(() => {
    if (winners.length > 0 && prevWinnersRef.current === 0) {
      playSound('win')
      setShowVictoryEffect(true)

      // Trigger win chip animations for each winner
      winners.forEach((winner, index) => {
        const player = gameState?.players.find(p => p.id === winner.id)
        if (player) {
          setTimeout(() => {
            triggerWinAnimation(player.seatIndex, winner.amount, rotateOffset)
          }, 300 + index * 200)
        }
      })
    }
    prevWinnersRef.current = winners.length
  }, [winners, playSound, rotateOffset, gameState?.players])

  if (!gameState) {
    return (
      <div className="relative w-full h-[50vh] md:h-[600px] flex items-center justify-center">
        <div className="text-white text-lg md:text-xl">Waiting for game state...</div>
      </div>
    )
  }

  const { players, communityCards, pot, pots, currentBet, handNumber } = gameState

  // Rotate players so current user is always at position 0 (bottom)
  const rotatedPlayers = [...players].sort((a, b) => {
    const aOffset = (a.seatIndex - currentUserSeatIndex + 9) % 9
    const bOffset = (b.seatIndex - currentUserSeatIndex + 9) % 9
    return aOffset - bOffset
  })

  // Phase display names
  const phaseNames: Record<string, string> = {
    waiting: 'Waiting for Players',
    pre_flop: 'Pre-Flop',
    flop: 'Flop',
    turn: 'Turn',
    river: 'River',
    showdown: 'Showdown',
  }

  // Check if we have a winner to display
  const hasWinner = winners.length > 0

  // Calculate winner positions for victory effect
  const winnerPositions = useMemo(() => {
    return winners.map(winner => {
      const player = players.find(p => p.id === winner.id)
      if (!player) return { x: window.innerWidth / 2, y: window.innerHeight / 2 }

      const visualPosition = (player.seatIndex - currentUserSeatIndex + 9) % 9
      const pos = PLAYER_POSITIONS[visualPosition] || PLAYER_POSITIONS[0]
      return {
        x: (pos.x / 100) * window.innerWidth,
        y: (pos.y / 100) * 700, // Match table height
      }
    })
  }, [winners, players, currentUserSeatIndex])

  return (
    <div className="relative w-full h-[calc(100vh-120px)] min-h-[400px] md:h-[700px]">
      {/* Chip flying animations */}
      <ChipAnimation rotateOffset={rotateOffset} />

      {/* Victory celebration effect */}
      <VictoryEffect
        active={showVictoryEffect}
        winnerPositions={winnerPositions}
        onComplete={() => setShowVictoryEffect(false)}
      />

      {/* Leave button */}
      <button
        onClick={leaveMatch}
        className="absolute top-1 md:top-2 right-1 md:right-2 z-10 px-2 md:px-3 py-1 bg-red-600 text-white text-xs md:text-sm rounded hover:bg-red-700 active:bg-red-800"
      >
        Leave
      </button>

      {/* Hand number & phase */}
      <div className="absolute top-1 md:top-2 left-1 md:left-2 z-10 text-white text-xs md:text-sm">
        <div>Hand #{handNumber}</div>
        <div className="text-poker-gold">{phaseNames[phase] || phase}</div>
      </div>

      {/* Table felt (oval) */}
      <div className="absolute inset-2 md:inset-10 top-10 md:top-16 bg-felt rounded-[50%] border-4 md:border-8 border-amber-900 shadow-2xl">
        {/* Table center info */}
        <div className="absolute inset-0 flex flex-col items-center justify-center">
          {/* Winner announcement */}
          {hasWinner && (
            <div className="absolute top-2 md:top-4 bg-black/80 px-2 md:px-4 py-1 md:py-2 rounded-lg z-20 animate-bounce max-w-[90%]">
              <div className="text-poker-gold text-sm md:text-lg font-bold text-center">
                {winners.map((w, i) => (
                  <div key={i} className="truncate">
                    {w.name} wins ${w.amount}
                    {w.handDescription && (
                      <span className="text-white text-xs md:text-sm ml-1 md:ml-2">({w.handDescription})</span>
                    )}
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* Community cards */}
          <div className="flex gap-1 md:gap-2 mb-2 md:mb-4">
            {communityCards.length > 0 ? (
              communityCards.map((card, index) => (
                <Card
                  key={`${phase}-${index}`}
                  card={card}
                  animate={newCardIndices.includes(index) ? 'deal' : 'none'}
                  animationDelay={newCardIndices.includes(index) ? (index - newCardIndices[0]) * 150 : 0}
                  small={isMobile}
                />
              ))
            ) : (
              <div className="text-felt-light text-sm md:text-lg">
                {phase === 'waiting'
                  ? `Waiting (${players.length}/2 min)`
                  : phase === 'pre_flop'
                  ? 'Pre-Flop'
                  : ''}
              </div>
            )}
          </div>

          {/* Pot info with side pots */}
          <SidePots pots={pots} players={players} totalPot={pot} />
          {currentBet > 0 && (
            <div className="text-white text-xs md:text-sm mt-1 bg-black/50 px-2 md:px-3 py-1 rounded">
              To call: ${currentBet}
            </div>
          )}

          {/* Showdown hands display */}
          {showdownPlayers.length > 0 && (
            <div className="absolute bottom-2 md:bottom-4 left-1/2 transform -translate-x-1/2 flex gap-2 md:gap-4 bg-black/80 px-2 md:px-4 py-1 md:py-2 rounded-lg max-w-[95%] overflow-x-auto">
              {showdownPlayers.map((sp, idx) => (
                <div key={idx} className="text-center flex-shrink-0">
                  <div className="text-white text-[10px] md:text-xs mb-1 truncate max-w-[60px] md:max-w-none">{sp.name}</div>
                  <div className="flex gap-0.5 md:gap-1">
                    {sp.cards.map((card, cardIdx) => (
                      <Card key={cardIdx} card={card} small />
                    ))}
                  </div>
                  <div className="text-poker-gold text-[10px] md:text-xs mt-1">
                    {HAND_RANK_DISPLAY[sp.handRank]}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* Players */}
      {rotatedPlayers.map((player, index) => {
        const position = PLAYER_POSITIONS[index % PLAYER_POSITIONS.length]
        const isWinner = winners.some(w => w.id === player.id)
        return (
          <div
            key={player.id}
            className="absolute transform -translate-x-1/2 -translate-y-1/2"
            style={{
              left: `${position.x}%`,
              top: `${position.y}%`,
            }}
          >
            <WinnerGlow active={isWinner}>
              <Player player={player} isCurrentUser={player.id === userId} />
            </WinnerGlow>
          </div>
        )
      })}

      {/* Betting controls (only if it's current player's turn) */}
      {turnInfo && currentPlayer && (
        <div className="fixed md:absolute bottom-0 md:bottom-2 left-0 md:left-1/2 right-0 md:right-auto md:transform md:-translate-x-1/2 z-20">
          <BettingControls isMobile={isMobile} />
        </div>
      )}

      {/* Spectator list - hidden on mobile when betting controls are visible */}
      <div className={`absolute bottom-2 left-1 md:left-2 z-10 max-w-[120px] md:max-w-xs ${turnInfo ? 'hidden md:block' : ''}`}>
        <SpectatorList />
      </div>

      {/* Chat - hidden on mobile when betting controls are visible */}
      <div className={`absolute bottom-2 right-1 md:right-2 z-10 ${turnInfo ? 'hidden md:block' : ''}`}>
        <Chat
          messages={chatMessages}
          onSendMessage={handleSendChat}
          isCollapsed={isChatCollapsed}
          onToggleCollapse={handleToggleChat}
          unreadCount={unreadCount}
        />
      </div>
    </div>
  )
}
