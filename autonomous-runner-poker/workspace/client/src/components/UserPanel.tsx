import { useState, useEffect } from 'react'
import { useGameStore } from '../store/gameStore'
import type { DailyRewardResponse } from '../types/poker'

interface UserPanelProps {
  onClaimDailyReward: () => Promise<DailyRewardResponse>
  onRefreshChips: () => Promise<void>
}

export function UserPanel({ onClaimDailyReward, onRefreshChips }: UserPanelProps) {
  const { username, userChips, authMode } = useGameStore()
  const [isClaimingReward, setIsClaimingReward] = useState(false)
  const [rewardMessage, setRewardMessage] = useState<string | null>(null)
  const [nextRewardTime, setNextRewardTime] = useState<number | null>(null)
  const [countdown, setCountdown] = useState<string | null>(null)

  // Countdown timer for next reward
  useEffect(() => {
    if (!nextRewardTime) return

    const updateCountdown = () => {
      const now = Date.now()
      const remaining = nextRewardTime - now

      if (remaining <= 0) {
        setCountdown(null)
        setNextRewardTime(null)
        return
      }

      const hours = Math.floor(remaining / (1000 * 60 * 60))
      const minutes = Math.floor((remaining % (1000 * 60 * 60)) / (1000 * 60))
      setCountdown(`${hours}h ${minutes}m`)
    }

    updateCountdown()
    const interval = setInterval(updateCountdown, 60000) // Update every minute

    return () => clearInterval(interval)
  }, [nextRewardTime])

  const handleClaimReward = async () => {
    setIsClaimingReward(true)
    setRewardMessage(null)

    try {
      const result = await onClaimDailyReward()
      setRewardMessage(result.message)
      setNextRewardTime(result.nextRewardTime)

      if (result.rewarded) {
        // Refresh chips to get updated balance
        await onRefreshChips()
      }
    } catch (err) {
      setRewardMessage('Failed to claim reward')
    } finally {
      setIsClaimingReward(false)
    }
  }

  const formatNumber = (num: number) => {
    return num.toLocaleString()
  }

  const winRate = userChips && userChips.handsPlayed > 0
    ? Math.round((userChips.handsWon / userChips.handsPlayed) * 100)
    : 0

  return (
    <div className="bg-felt-medium rounded-lg p-4 shadow-lg">
      {/* Header */}
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 bg-gradient-to-br from-poker-gold to-yellow-600 rounded-full flex items-center justify-center text-black font-bold">
            {username?.charAt(0).toUpperCase() || '?'}
          </div>
          <div>
            <h3 className="text-white font-medium">{username}</h3>
            <span className="text-xs text-gray-400">
              {authMode === 'guest' ? 'Guest Account' : 'Registered'}
            </span>
          </div>
        </div>
      </div>

      {/* Balance */}
      <div className="bg-felt-dark rounded-lg p-3 mb-4">
        <div className="text-gray-400 text-xs mb-1">Chip Balance</div>
        <div className="text-2xl font-bold text-poker-gold">
          ${formatNumber(userChips?.balance || 0)}
        </div>
      </div>

      {/* Stats Grid */}
      {userChips && (
        <div className="grid grid-cols-2 gap-2 mb-4">
          <div className="bg-felt-dark rounded p-2">
            <div className="text-xs text-gray-400">Total Won</div>
            <div className="text-green-400 font-medium">
              +${formatNumber(userChips.totalWon)}
            </div>
          </div>
          <div className="bg-felt-dark rounded p-2">
            <div className="text-xs text-gray-400">Total Lost</div>
            <div className="text-red-400 font-medium">
              -${formatNumber(userChips.totalLost)}
            </div>
          </div>
          <div className="bg-felt-dark rounded p-2">
            <div className="text-xs text-gray-400">Hands Played</div>
            <div className="text-white font-medium">
              {formatNumber(userChips.handsPlayed)}
            </div>
          </div>
          <div className="bg-felt-dark rounded p-2">
            <div className="text-xs text-gray-400">Win Rate</div>
            <div className="text-white font-medium">
              {winRate}%
            </div>
          </div>
        </div>
      )}

      {/* Daily Reward */}
      <div className="border-t border-gray-700 pt-4">
        <div className="flex items-center justify-between mb-2">
          <span className="text-gray-300 text-sm">Daily Reward</span>
          {countdown && (
            <span className="text-xs text-gray-400">
              Next: {countdown}
            </span>
          )}
        </div>

        <button
          onClick={handleClaimReward}
          disabled={isClaimingReward || !!countdown}
          className={`w-full py-2 rounded-lg font-medium transition-colors ${
            countdown
              ? 'bg-gray-700 text-gray-400 cursor-not-allowed'
              : 'bg-green-600 hover:bg-green-700 text-white'
          }`}
        >
          {isClaimingReward ? (
            <span className="flex items-center justify-center gap-2">
              <span className="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
              Claiming...
            </span>
          ) : countdown ? (
            `Come back in ${countdown}`
          ) : (
            'Claim 500 Chips'
          )}
        </button>

        {rewardMessage && (
          <div className={`mt-2 text-sm text-center ${
            rewardMessage.includes('received') ? 'text-green-400' : 'text-gray-400'
          }`}>
            {rewardMessage}
          </div>
        )}
      </div>

      {/* Guest warning */}
      {authMode === 'guest' && (
        <div className="mt-4 p-3 bg-yellow-900/30 border border-yellow-700/50 rounded-lg">
          <p className="text-yellow-400 text-xs">
            Guest accounts don't save progress. Create an account to keep your chips!
          </p>
        </div>
      )}
    </div>
  )
}
