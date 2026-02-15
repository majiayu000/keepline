import { useEffect, useCallback, useRef } from 'react'
import { useGameStore } from '../store/gameStore'
import { useNakama } from './useNakama'
import { useSound } from '../contexts/SoundContext'

interface KeyboardShortcutsOptions {
  enabled?: boolean
  onRaiseAmountChange?: (amount: number) => void
  currentRaiseAmount?: number
}

interface KeyboardShortcutsReturn {
  shortcuts: ShortcutInfo[]
}

interface ShortcutInfo {
  key: string
  description: string
  action: string
}

const SHORTCUTS: ShortcutInfo[] = [
  { key: 'F', description: 'Fold', action: 'fold' },
  { key: 'C', description: 'Call / Check', action: 'call_check' },
  { key: 'B', description: 'Bet', action: 'bet' },
  { key: 'R', description: 'Raise', action: 'raise' },
  { key: 'A', description: 'All-In', action: 'all_in' },
  { key: '1', description: 'Min bet', action: 'min' },
  { key: '2', description: '1/2 Pot', action: 'half_pot' },
  { key: '3', description: 'Pot', action: 'pot' },
  { key: '4', description: 'Max bet', action: 'max' },
  { key: '↑', description: 'Increase bet', action: 'increase' },
  { key: '↓', description: 'Decrease bet', action: 'decrease' },
]

export function useKeyboardShortcuts(options: KeyboardShortcutsOptions = {}): KeyboardShortcutsReturn {
  const { enabled = true, onRaiseAmountChange, currentRaiseAmount } = options

  const { sendAction } = useNakama()
  const { turnInfo, currentPlayer, gameState } = useGameStore()
  const { playSound, playChipsSound } = useSound()

  // Use ref to avoid stale closures in event handler
  const stateRef = useRef({ turnInfo, currentPlayer, gameState, currentRaiseAmount })

  useEffect(() => {
    stateRef.current = { turnInfo, currentPlayer, gameState, currentRaiseAmount }
  }, [turnInfo, currentPlayer, gameState, currentRaiseAmount])

  const handleKeyDown = useCallback((event: KeyboardEvent) => {
    // Don't trigger shortcuts when typing in input fields
    const target = event.target as HTMLElement
    if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable) {
      return
    }

    const { turnInfo, currentPlayer, gameState, currentRaiseAmount: raiseAmount } = stateRef.current

    // Only process shortcuts if it's the player's turn
    if (!turnInfo || !currentPlayer || !gameState) {
      return
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
    const sliderMin = canRaise ? minRaise : minBet
    const sliderMax = maxBet

    const key = event.key.toUpperCase()

    switch (key) {
      case 'F':
        // Fold
        event.preventDefault()
        playSound('fold')
        sendAction({ action: 'fold' })
        break

      case 'C':
        // Call or Check
        event.preventDefault()
        if (canCall && callAmount > 0) {
          playSound('call')
          playChipsSound(callAmount)
          sendAction({ action: 'call' })
        } else if (canCheck) {
          playSound('check')
          sendAction({ action: 'check' })
        }
        break

      case 'B':
        // Bet
        event.preventDefault()
        if (canBet && raiseAmount !== undefined) {
          playSound('bet')
          playChipsSound(raiseAmount)
          sendAction({ action: 'bet', amount: raiseAmount })
        }
        break

      case 'R':
        // Raise
        event.preventDefault()
        if (canRaise && raiseAmount !== undefined) {
          playSound('bet')
          playChipsSound(raiseAmount)
          sendAction({ action: 'raise', amount: raiseAmount })
        }
        break

      case 'A':
        // All-In
        event.preventDefault()
        playSound('allIn')
        playChipsSound(playerChips)
        sendAction({ action: 'all_in' })
        break

      case '1':
        // Min bet
        event.preventDefault()
        if ((canBet || canRaise) && onRaiseAmountChange) {
          onRaiseAmountChange(sliderMin)
        }
        break

      case '2':
        // 1/2 Pot
        event.preventDefault()
        if ((canBet || canRaise) && onRaiseAmountChange) {
          const halfPot = Math.floor(gameState.pot * 0.5)
          const amount = Math.max(sliderMin, Math.min(halfPot, sliderMax))
          onRaiseAmountChange(amount)
        }
        break

      case '3':
        // Pot
        event.preventDefault()
        if ((canBet || canRaise) && onRaiseAmountChange) {
          const potAmount = Math.min(gameState.pot, sliderMax)
          const amount = Math.max(sliderMin, potAmount)
          onRaiseAmountChange(amount)
        }
        break

      case '4':
        // Max bet
        event.preventDefault()
        if ((canBet || canRaise) && onRaiseAmountChange) {
          onRaiseAmountChange(sliderMax)
        }
        break

      case 'ARROWUP':
        // Increase bet amount
        event.preventDefault()
        if ((canBet || canRaise) && onRaiseAmountChange && raiseAmount !== undefined) {
          // Increase by big blind or 10% of current, whichever is larger
          const step = Math.max(gameState.bigBlind || 20, Math.floor(raiseAmount * 0.1))
          const newAmount = Math.min(raiseAmount + step, sliderMax)
          onRaiseAmountChange(newAmount)
        }
        break

      case 'ARROWDOWN':
        // Decrease bet amount
        event.preventDefault()
        if ((canBet || canRaise) && onRaiseAmountChange && raiseAmount !== undefined) {
          const step = Math.max(gameState.bigBlind || 20, Math.floor(raiseAmount * 0.1))
          const newAmount = Math.max(raiseAmount - step, sliderMin)
          onRaiseAmountChange(newAmount)
        }
        break
    }
  }, [enabled, sendAction, playSound, playChipsSound, onRaiseAmountChange])

  useEffect(() => {
    if (!enabled) return

    window.addEventListener('keydown', handleKeyDown)
    return () => window.removeEventListener('keydown', handleKeyDown)
  }, [enabled, handleKeyDown])

  return { shortcuts: SHORTCUTS }
}

export type { ShortcutInfo }
