import { useState } from 'react'
import { useSound } from '../contexts/SoundContext'

interface SoundSettingsProps {
  compact?: boolean
}

export function SoundSettings({ compact = false }: SoundSettingsProps) {
  const { soundEnabled, setSoundEnabled, masterVolume, setMasterVolume, playSound } = useSound()
  const [showSettings, setShowSettings] = useState(false)

  const handleToggle = () => {
    const newEnabled = !soundEnabled
    setSoundEnabled(newEnabled)
    if (newEnabled) {
      // Play a test sound when enabling
      setTimeout(() => playSound('check'), 100)
    }
  }

  const handleVolumeChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const value = parseFloat(e.target.value)
    setMasterVolume(value)
  }

  // Compact mode - just an icon button
  if (compact) {
    return (
      <div className="relative">
        <button
          onClick={() => setShowSettings(!showSettings)}
          className="p-2 rounded-lg bg-black/30 hover:bg-black/50 transition-colors"
          title={soundEnabled ? 'Sound On' : 'Sound Off'}
        >
          {soundEnabled ? (
            <svg className="w-5 h-5 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                d="M15.536 8.464a5 5 0 010 7.072m2.828-9.9a9 9 0 010 12.728M5.586 15H4a1 1 0 01-1-1v-4a1 1 0 011-1h1.586l4.707-4.707C10.923 3.663 12 4.109 12 5v14c0 .891-1.077 1.337-1.707.707L5.586 15z" />
            </svg>
          ) : (
            <svg className="w-5 h-5 text-gray-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                d="M5.586 15H4a1 1 0 01-1-1v-4a1 1 0 011-1h1.586l4.707-4.707C10.923 3.663 12 4.109 12 5v14c0 .891-1.077 1.337-1.707.707L5.586 15z" />
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                d="M17 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2" />
            </svg>
          )}
        </button>

        {/* Dropdown settings */}
        {showSettings && (
          <div className="absolute right-0 mt-2 w-48 bg-gray-800 rounded-lg shadow-xl p-3 z-50">
            <div className="flex items-center justify-between mb-3">
              <span className="text-white text-sm">Sound</span>
              <button
                onClick={handleToggle}
                className={`w-10 h-5 rounded-full transition-colors relative ${
                  soundEnabled ? 'bg-green-500' : 'bg-gray-600'
                }`}
              >
                <span
                  className={`absolute top-0.5 w-4 h-4 bg-white rounded-full transition-transform ${
                    soundEnabled ? 'translate-x-5' : 'translate-x-0.5'
                  }`}
                />
              </button>
            </div>

            <div className="space-y-2">
              <span className="text-white text-sm">Volume</span>
              <input
                type="range"
                min="0"
                max="1"
                step="0.1"
                value={masterVolume}
                onChange={handleVolumeChange}
                disabled={!soundEnabled}
                className="w-full h-2 bg-gray-600 rounded-lg appearance-none cursor-pointer
                           disabled:opacity-50 disabled:cursor-not-allowed"
              />
              <div className="text-gray-400 text-xs text-right">
                {Math.round(masterVolume * 100)}%
              </div>
            </div>
          </div>
        )}
      </div>
    )
  }

  // Full mode - expanded settings panel
  return (
    <div className="flex items-center gap-3 bg-black/30 px-4 py-2 rounded-lg">
      <button
        onClick={handleToggle}
        className="flex items-center gap-2 text-white hover:text-poker-gold transition-colors"
      >
        {soundEnabled ? (
          <>
            <svg className="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                d="M15.536 8.464a5 5 0 010 7.072m2.828-9.9a9 9 0 010 12.728M5.586 15H4a1 1 0 01-1-1v-4a1 1 0 011-1h1.586l4.707-4.707C10.923 3.663 12 4.109 12 5v14c0 .891-1.077 1.337-1.707.707L5.586 15z" />
            </svg>
            <span className="text-sm">Sound On</span>
          </>
        ) : (
          <>
            <svg className="w-5 h-5 text-gray-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                d="M5.586 15H4a1 1 0 01-1-1v-4a1 1 0 011-1h1.586l4.707-4.707C10.923 3.663 12 4.109 12 5v14c0 .891-1.077 1.337-1.707.707L5.586 15z" />
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2}
                d="M17 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2" />
            </svg>
            <span className="text-sm text-gray-400">Sound Off</span>
          </>
        )}
      </button>

      {soundEnabled && (
        <input
          type="range"
          min="0"
          max="1"
          step="0.1"
          value={masterVolume}
          onChange={handleVolumeChange}
          className="w-20 h-2 bg-gray-600 rounded-lg appearance-none cursor-pointer"
          title={`Volume: ${Math.round(masterVolume * 100)}%`}
        />
      )}
    </div>
  )
}
