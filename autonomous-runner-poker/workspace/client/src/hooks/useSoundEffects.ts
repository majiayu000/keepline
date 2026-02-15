import { useCallback, useRef, useEffect } from 'react'

export type SoundType =
  | 'deal'      // Card dealing
  | 'check'     // Check action
  | 'call'      // Call action
  | 'bet'       // Bet/Raise action
  | 'fold'      // Fold action
  | 'allIn'     // All-in action
  | 'chips'     // Chips moving
  | 'win'       // Winning hand
  | 'turn'      // Your turn notification
  | 'tick'      // Timer tick (last 5 seconds)
  | 'join'      // Player joined
  | 'leave'     // Player left

// Sound definitions - frequency, duration, type, and volume
interface SoundConfig {
  frequencies: number[]
  durations: number[]
  types: OscillatorType[]
  volumes: number[]
  delays: number[]
}

const SOUND_CONFIGS: Record<SoundType, SoundConfig> = {
  deal: {
    frequencies: [800, 600],
    durations: [0.05, 0.03],
    types: ['square', 'square'],
    volumes: [0.15, 0.1],
    delays: [0, 0.02],
  },
  check: {
    frequencies: [440, 550],
    durations: [0.08, 0.08],
    types: ['sine', 'sine'],
    volumes: [0.15, 0.12],
    delays: [0, 0.06],
  },
  call: {
    frequencies: [523, 659],
    durations: [0.1, 0.1],
    types: ['sine', 'sine'],
    volumes: [0.15, 0.15],
    delays: [0, 0.08],
  },
  bet: {
    frequencies: [392, 523, 659],
    durations: [0.08, 0.08, 0.12],
    types: ['sine', 'sine', 'sine'],
    volumes: [0.12, 0.15, 0.15],
    delays: [0, 0.06, 0.12],
  },
  fold: {
    frequencies: [300, 200],
    durations: [0.1, 0.15],
    types: ['triangle', 'triangle'],
    volumes: [0.15, 0.1],
    delays: [0, 0.08],
  },
  allIn: {
    frequencies: [392, 523, 659, 784],
    durations: [0.1, 0.1, 0.1, 0.2],
    types: ['sine', 'sine', 'sine', 'sine'],
    volumes: [0.12, 0.15, 0.18, 0.2],
    delays: [0, 0.08, 0.16, 0.24],
  },
  chips: {
    frequencies: [1200, 1000, 800],
    durations: [0.03, 0.03, 0.03],
    types: ['square', 'square', 'square'],
    volumes: [0.08, 0.1, 0.08],
    delays: [0, 0.03, 0.06],
  },
  win: {
    frequencies: [523, 659, 784, 1047],
    durations: [0.15, 0.15, 0.15, 0.3],
    types: ['sine', 'sine', 'sine', 'sine'],
    volumes: [0.15, 0.18, 0.2, 0.25],
    delays: [0, 0.12, 0.24, 0.36],
  },
  turn: {
    frequencies: [880, 1100, 880],
    durations: [0.08, 0.1, 0.08],
    types: ['sine', 'sine', 'sine'],
    volumes: [0.2, 0.25, 0.2],
    delays: [0, 0.1, 0.2],
  },
  tick: {
    frequencies: [1000],
    durations: [0.05],
    types: ['square'],
    volumes: [0.15],
    delays: [0],
  },
  join: {
    frequencies: [440, 550, 660],
    durations: [0.08, 0.08, 0.1],
    types: ['sine', 'sine', 'sine'],
    volumes: [0.1, 0.12, 0.15],
    delays: [0, 0.06, 0.12],
  },
  leave: {
    frequencies: [660, 550, 440],
    durations: [0.08, 0.08, 0.1],
    types: ['sine', 'sine', 'sine'],
    volumes: [0.15, 0.12, 0.1],
    delays: [0, 0.06, 0.12],
  },
}

interface UseSoundEffectsOptions {
  enabled?: boolean
  volume?: number // 0-1 master volume
}

export function useSoundEffects(options: UseSoundEffectsOptions = {}) {
  const { enabled = true, volume = 0.7 } = options

  const audioContextRef = useRef<AudioContext | null>(null)
  const enabledRef = useRef(enabled)
  const volumeRef = useRef(volume)

  // Update refs when options change
  useEffect(() => {
    enabledRef.current = enabled
    volumeRef.current = volume
  }, [enabled, volume])

  // Initialize AudioContext lazily (must be triggered by user interaction)
  const getAudioContext = useCallback(() => {
    if (!audioContextRef.current) {
      audioContextRef.current = new AudioContext()
    }
    // Resume if suspended (browser autoplay policy)
    if (audioContextRef.current.state === 'suspended') {
      audioContextRef.current.resume()
    }
    return audioContextRef.current
  }, [])

  // Play a single tone
  const playTone = useCallback((
    frequency: number,
    duration: number,
    type: OscillatorType,
    volume: number,
    delay: number,
    ctx: AudioContext
  ) => {
    const startTime = ctx.currentTime + delay

    // Create oscillator
    const oscillator = ctx.createOscillator()
    oscillator.type = type
    oscillator.frequency.setValueAtTime(frequency, startTime)

    // Create gain node for volume control and fade out
    const gainNode = ctx.createGain()
    gainNode.gain.setValueAtTime(volume * volumeRef.current, startTime)
    gainNode.gain.exponentialRampToValueAtTime(0.001, startTime + duration)

    // Connect: oscillator -> gain -> destination
    oscillator.connect(gainNode)
    gainNode.connect(ctx.destination)

    // Schedule start and stop
    oscillator.start(startTime)
    oscillator.stop(startTime + duration)
  }, [])

  // Play a sound effect
  const playSound = useCallback((soundType: SoundType) => {
    if (!enabledRef.current) return

    const config = SOUND_CONFIGS[soundType]
    if (!config) {
      console.warn(`Unknown sound type: ${soundType}`)
      return
    }

    try {
      const ctx = getAudioContext()

      // Play all tones in the sound
      for (let i = 0; i < config.frequencies.length; i++) {
        playTone(
          config.frequencies[i],
          config.durations[i],
          config.types[i],
          config.volumes[i],
          config.delays[i],
          ctx
        )
      }
    } catch (error) {
      console.warn('Failed to play sound:', error)
    }
  }, [getAudioContext, playTone])

  // Play card deal sound with slight variation
  const playDealSound = useCallback((cardIndex: number = 0) => {
    if (!enabledRef.current) return

    try {
      const ctx = getAudioContext()

      // Vary the pitch slightly for each card
      const baseFreq = 800 - (cardIndex * 20)
      const delay = cardIndex * 0.15 // Stagger the sounds

      playTone(baseFreq, 0.05, 'square', 0.12, delay, ctx)
      playTone(baseFreq - 200, 0.03, 'square', 0.08, delay + 0.02, ctx)
    } catch (error) {
      console.warn('Failed to play deal sound:', error)
    }
  }, [getAudioContext, playTone])

  // Play chip sound with amount-based intensity
  const playChipsSound = useCallback((amount: number = 0) => {
    if (!enabledRef.current) return

    try {
      const ctx = getAudioContext()

      // More chips = more sound layers
      const layers = Math.min(Math.ceil(Math.log10(amount + 1)) + 1, 5)

      for (let i = 0; i < layers; i++) {
        const freq = 1200 - (i * 100) + (Math.random() * 100)
        playTone(freq, 0.03, 'square', 0.06 + (i * 0.02), i * 0.025, ctx)
      }
    } catch (error) {
      console.warn('Failed to play chips sound:', error)
    }
  }, [getAudioContext, playTone])

  // Cleanup
  useEffect(() => {
    return () => {
      if (audioContextRef.current) {
        audioContextRef.current.close()
        audioContextRef.current = null
      }
    }
  }, [])

  return {
    playSound,
    playDealSound,
    playChipsSound,
    // Expose for manual initialization (user gesture required)
    initAudio: getAudioContext,
  }
}
