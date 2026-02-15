import { useState, useEffect, useRef } from 'react'
import { useNakama } from '../hooks/useNakama'
import { useGameStore } from '../store/gameStore'
import { useSound } from '../contexts/SoundContext'
import { useKeyboardShortcuts } from '../hooks/useKeyboardShortcuts'
import KeyboardShortcutsHelp from './KeyboardShortcutsHelp'

interface BettingControlsProps {
  isMobile?: boolean
}

export default function BettingControls({ isMobile = false }: BettingControlsProps) {
  const { sendAction } = useNakama()
  const { turnInfo, currentPlayer, gameState } = useGameStore()
  const { playSound, playChipsSound } = useSound()

  const [raiseAmount, setRaiseAmount] = useState(0)
  const [timeLeft, setTimeLeft] = useState(0)
  const [showShortcuts, setShowShortcuts] = useState(false)
  const hasPlayedTurnSound = useRef(false)

  // Enable keyboard shortcuts when it's the player's turn
  const { shortcuts } = useKeyboardShortcuts({
    enabled: !!turnInfo,
    currentRaiseAmount: raiseAmount,
    onRaiseAmountChange: setRaiseAmount,
  })

  // Initialize raise amount when turn info changes
  useEffect(() => {
    if (turnInfo) {
      setRaiseAmount(turnInfo.minRaise || turnInfo.minBet)
      setTimeLeft(turnInfo.timeoutSeconds)

      // Play turn notification sound once when it becomes our turn
      if (!hasPlayedTurnSound.current) {
        playSound('turn')
        hasPlayedTurnSound.current = true
      }
    } else {
      hasPlayedTurnSound.current = false
    }
  }, [turnInfo, playSound])

  // Countdown timer with tick sounds
  useEffect(() => {
    if (timeLeft <= 0) return

    const timer = setInterval(() => {
      setTimeLeft(prev => {
        const newTime = Math.max(0, prev - 1)
        // Play tick sound in last 5 seconds
        if (newTime > 0 && newTime <= 5) {
          playSound('tick')
        }
        return newTime
      })
    }, 1000)

    return () => clearInterval(timer)
  }, [timeLeft, playSound])

  if (!turnInfo || !currentPlayer || !gameState) {
    return null
  }

  const {
    canCheck,
    canCall,
    canBet,
    canRaise,
    callAmount,
    minBet,
    minRaise,
    maxBet,
  } = turnInfo

  const playerChips = currentPlayer.chips

  const handleFold = () => {
    playSound('fold')
    sendAction({ action: 'fold' })
  }

  const handleCheck = () => {
    playSound('check')
    sendAction({ action: 'check' })
  }

  const handleCall = () => {
    playSound('call')
    playChipsSound(callAmount)
    sendAction({ action: 'call' })
  }

  const handleBet = () => {
    playSound('bet')
    playChipsSound(raiseAmount)
    sendAction({ action: 'bet', amount: raiseAmount })
  }

  const handleRaise = () => {
    playSound('bet')
    playChipsSound(raiseAmount)
    sendAction({ action: 'raise', amount: raiseAmount })
  }

  const handleAllIn = () => {
    playSound('allIn')
    playChipsSound(playerChips)
    sendAction({ action: 'all_in' })
  }

  // Determine slider range
  const sliderMin = canRaise ? minRaise : minBet
  const sliderMax = maxBet

  // Timer color based on time remaining
  const timerColor = timeLeft <= 5 ? 'text-red-500' : timeLeft <= 10 ? 'text-yellow-400' : 'text-white'

  // Mobile-specific classes
  const containerClasses = isMobile
    ? 'bg-black/95 rounded-t-xl p-3 flex flex-col gap-2 w-full safe-area-inset-bottom'
    : 'bg-black/80 rounded-lg p-4 flex flex-col gap-3 min-w-[300px]'

  const buttonBaseClasses = isMobile
    ? 'px-3 py-3 text-sm rounded-lg font-bold active:scale-95 transition-all touch-manipulation min-w-[60px]'
    : 'px-4 py-2 rounded hover:opacity-90 transition-colors font-bold'

  return (
    <div className={containerClasses}>
      {/* Timer - more compact on mobile */}
      <div className={`text-center font-bold ${timerColor} ${isMobile ? 'text-sm' : ''}`}>
        {isMobile ? `${timeLeft}s` : `Time: ${timeLeft}s`}
      </div>

      {/* Action buttons - horizontal scroll on mobile */}
      <div className={`flex gap-2 ${isMobile ? 'overflow-x-auto pb-1 -mx-1 px-1' : 'flex-wrap justify-center'}`}>
        <button
          onClick={handleFold}
          className={`${buttonBaseClasses} bg-red-600 text-white flex-shrink-0`}
          title="Press F"
        >
          Fold
          {!isMobile && <span className="ml-1 text-xs text-red-300 opacity-70">[F]</span>}
        </button>

        {canCheck && (
          <button
            onClick={handleCheck}
            className={`${buttonBaseClasses} bg-blue-600 text-white flex-shrink-0`}
            title="Press C"
          >
            Check
            {!isMobile && <span className="ml-1 text-xs text-blue-300 opacity-70">[C]</span>}
          </button>
        )}

        {canCall && callAmount > 0 && (
          <button
            onClick={handleCall}
            className={`${buttonBaseClasses} bg-green-600 text-white flex-shrink-0`}
            title="Press C"
          >
            {isMobile ? `Call $${callAmount}` : `Call $${callAmount}`}
            {!isMobile && <span className="ml-1 text-xs text-green-300 opacity-70">[C]</span>}
          </button>
        )}

        {canBet && (
          <button
            onClick={handleBet}
            className={`${buttonBaseClasses} bg-yellow-600 text-white flex-shrink-0`}
            title="Press B"
          >
            {isMobile ? `Bet $${raiseAmount}` : `Bet $${raiseAmount}`}
            {!isMobile && <span className="ml-1 text-xs text-yellow-300 opacity-70">[B]</span>}
          </button>
        )}

        {canRaise && (
          <button
            onClick={handleRaise}
            className={`${buttonBaseClasses} bg-orange-600 text-white flex-shrink-0`}
            title="Press R"
          >
            {isMobile ? `Raise $${raiseAmount}` : `Raise to $${raiseAmount}`}
            {!isMobile && <span className="ml-1 text-xs text-orange-300 opacity-70">[R]</span>}
          </button>
        )}

        <button
          onClick={handleAllIn}
          className={`${buttonBaseClasses} bg-purple-600 text-white flex-shrink-0`}
          title="Press A"
        >
          {isMobile ? `All-In $${playerChips}` : `All In ($${playerChips})`}
          {!isMobile && <span className="ml-1 text-xs text-purple-300 opacity-70">[A]</span>}
        </button>
      </div>

      {/* Bet/Raise slider */}
      {(canBet || canRaise) && sliderMax > sliderMin && (
        <div className="flex items-center gap-2">
          <span className={`text-white ${isMobile ? 'text-xs min-w-[35px]' : 'text-sm min-w-[40px]'}`}>${sliderMin}</span>
          <input
            type="range"
            min={sliderMin}
            max={sliderMax}
            value={raiseAmount}
            onChange={(e) => setRaiseAmount(Number(e.target.value))}
            className={`flex-1 bg-gray-600 rounded-lg appearance-none cursor-pointer touch-manipulation ${isMobile ? 'h-3' : 'h-2'}`}
            style={isMobile ? { WebkitAppearance: 'none' } : undefined}
          />
          <span className={`text-white text-right ${isMobile ? 'text-xs min-w-[35px]' : 'text-sm min-w-[40px]'}`}>${sliderMax}</span>
        </div>
      )}

      {/* Quick bet buttons */}
      {(canBet || canRaise) && (
        <div className={`flex gap-2 justify-center ${isMobile ? 'flex-wrap' : ''}`}>
          <button
            onClick={() => setRaiseAmount(sliderMin)}
            className={`${isMobile ? 'px-3 py-2 text-xs' : 'px-2 py-1 text-xs'} bg-gray-600 text-white rounded active:bg-gray-500 touch-manipulation`}
          >
            Min {!isMobile && <span className="text-gray-400">[1]</span>}
          </button>
          <button
            onClick={() => setRaiseAmount(Math.max(sliderMin, Math.floor(gameState.pot * 0.5)))}
            className={`${isMobile ? 'px-3 py-2 text-xs' : 'px-2 py-1 text-xs'} bg-gray-600 text-white rounded active:bg-gray-500 touch-manipulation`}
          >
            1/2 Pot {!isMobile && <span className="text-gray-400">[2]</span>}
          </button>
          <button
            onClick={() => setRaiseAmount(Math.max(sliderMin, gameState.pot))}
            className={`${isMobile ? 'px-3 py-2 text-xs' : 'px-2 py-1 text-xs'} bg-gray-600 text-white rounded active:bg-gray-500 touch-manipulation`}
          >
            Pot {!isMobile && <span className="text-gray-400">[3]</span>}
          </button>
          <button
            onClick={() => setRaiseAmount(sliderMax)}
            className={`${isMobile ? 'px-3 py-2 text-xs' : 'px-2 py-1 text-xs'} bg-gray-600 text-white rounded active:bg-gray-500 touch-manipulation`}
          >
            Max {!isMobile && <span className="text-gray-400">[4]</span>}
          </button>
        </div>
      )}

      {/* Keyboard shortcuts toggle - hidden on mobile */}
      {!isMobile && (
        <div className="flex justify-center mt-1">
          <button
            onClick={() => setShowShortcuts(!showShortcuts)}
            className="text-xs text-gray-400 hover:text-gray-200 flex items-center gap-1"
          >
            <span className="text-base">⌨️</span>
            {showShortcuts ? 'Hide shortcuts' : 'Keyboard shortcuts'}
          </button>
        </div>
      )}

      {/* Keyboard shortcuts help - desktop only */}
      {!isMobile && showShortcuts && (
        <KeyboardShortcutsHelp shortcuts={shortcuts} onClose={() => setShowShortcuts(false)} />
      )}
    </div>
  )
}
