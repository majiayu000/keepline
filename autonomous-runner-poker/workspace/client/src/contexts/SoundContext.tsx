import { createContext, useContext, useState, useCallback, type ReactNode } from 'react'
import { useSoundEffects, type SoundType } from '../hooks/useSoundEffects'

interface SoundContextValue {
  // Settings
  soundEnabled: boolean
  setSoundEnabled: (enabled: boolean) => void
  masterVolume: number
  setMasterVolume: (volume: number) => void

  // Sound playback
  playSound: (type: SoundType) => void
  playDealSound: (cardIndex?: number) => void
  playChipsSound: (amount?: number) => void

  // Initialize audio (must be called on user interaction)
  initAudio: () => void
}

const SoundContext = createContext<SoundContextValue | null>(null)

export function SoundProvider({ children }: { children: ReactNode }) {
  const [soundEnabled, setSoundEnabled] = useState(true)
  const [masterVolume, setMasterVolume] = useState(0.7)

  const { playSound, playDealSound, playChipsSound, initAudio } = useSoundEffects({
    enabled: soundEnabled,
    volume: masterVolume,
  })

  const handleSetSoundEnabled = useCallback((enabled: boolean) => {
    setSoundEnabled(enabled)
    // Save preference to localStorage
    localStorage.setItem('poker-sound-enabled', String(enabled))
  }, [])

  const handleSetMasterVolume = useCallback((volume: number) => {
    setMasterVolume(volume)
    // Save preference to localStorage
    localStorage.setItem('poker-master-volume', String(volume))
  }, [])

  // Load preferences from localStorage on mount
  useState(() => {
    const savedEnabled = localStorage.getItem('poker-sound-enabled')
    const savedVolume = localStorage.getItem('poker-master-volume')

    if (savedEnabled !== null) {
      setSoundEnabled(savedEnabled === 'true')
    }
    if (savedVolume !== null) {
      const vol = parseFloat(savedVolume)
      if (!isNaN(vol) && vol >= 0 && vol <= 1) {
        setMasterVolume(vol)
      }
    }
  })

  const value: SoundContextValue = {
    soundEnabled,
    setSoundEnabled: handleSetSoundEnabled,
    masterVolume,
    setMasterVolume: handleSetMasterVolume,
    playSound,
    playDealSound,
    playChipsSound,
    initAudio,
  }

  return (
    <SoundContext.Provider value={value}>
      {children}
    </SoundContext.Provider>
  )
}

export function useSound(): SoundContextValue {
  const context = useContext(SoundContext)
  if (!context) {
    throw new Error('useSound must be used within a SoundProvider')
  }
  return context
}
