import { useState, useEffect } from 'react'

// Chip animation types
interface ChipFlight {
  id: string
  fromX: number
  fromY: number
  toX: number
  toY: number
  amount: number
  startTime: number
}

// Player positions (same as Table.tsx)
const PLAYER_POSITIONS = [
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

// Pot center position (approximate center of table)
const POT_CENTER = { x: 50, y: 45 }

// Animation duration in ms
const FLIGHT_DURATION = 600

interface ChipAnimationProps {
  // Rotate offset to map player seats to visual positions
  rotateOffset: number
}

// Chip stack component
function ChipStack({ amount, small = false }: { amount: number; small?: boolean }) {
  const chipCount = Math.min(5, Math.ceil(amount / 100))
  const size = small ? 'w-4 h-4' : 'w-6 h-6'
  const stackOffset = small ? 2 : 3

  return (
    <div className="relative" style={{ width: small ? 16 : 24, height: small ? 16 + (chipCount - 1) * stackOffset : 24 + (chipCount - 1) * stackOffset }}>
      {Array.from({ length: chipCount }).map((_, i) => (
        <div
          key={i}
          className={`absolute ${size} rounded-full border-2 border-white shadow-lg`}
          style={{
            backgroundColor: getChipColor(amount),
            bottom: i * stackOffset,
            left: 0,
            boxShadow: '0 2px 4px rgba(0,0,0,0.5)',
          }}
        >
          <div className="absolute inset-1 rounded-full border border-white/30" />
        </div>
      ))}
    </div>
  )
}

// Get chip color based on amount
function getChipColor(amount: number): string {
  if (amount >= 1000) return '#1a1a2e' // Black - high value
  if (amount >= 500) return '#9b59b6'  // Purple
  if (amount >= 100) return '#27ae60'  // Green
  if (amount >= 50) return '#3498db'   // Blue
  if (amount >= 25) return '#e74c3c'   // Red
  return '#f1c40f' // Yellow - low value
}

// Global store for chip animations
let chipFlightsStore: ChipFlight[] = []
let listeners: Set<() => void> = new Set()

function notifyListeners() {
  listeners.forEach(fn => fn())
}

export function triggerBetAnimation(playerSeatIndex: number, amount: number, rotateOffset: number) {
  const visualPosition = (playerSeatIndex - rotateOffset + 9) % 9
  const from = PLAYER_POSITIONS[visualPosition] || PLAYER_POSITIONS[0]

  const flight: ChipFlight = {
    id: `bet-${Date.now()}-${Math.random()}`,
    fromX: from.x,
    fromY: from.y,
    toX: POT_CENTER.x,
    toY: POT_CENTER.y,
    amount,
    startTime: Date.now(),
  }

  chipFlightsStore = [...chipFlightsStore, flight]
  notifyListeners()

  // Auto-remove after animation
  setTimeout(() => {
    chipFlightsStore = chipFlightsStore.filter(f => f.id !== flight.id)
    notifyListeners()
  }, FLIGHT_DURATION + 100)
}

export function triggerWinAnimation(winnerSeatIndex: number, amount: number, rotateOffset: number) {
  const visualPosition = (winnerSeatIndex - rotateOffset + 9) % 9
  const to = PLAYER_POSITIONS[visualPosition] || PLAYER_POSITIONS[0]

  // Create multiple chip stacks flying to winner
  const numStacks = Math.min(5, Math.ceil(amount / 200))

  for (let i = 0; i < numStacks; i++) {
    const delay = i * 100

    setTimeout(() => {
      const flight: ChipFlight = {
        id: `win-${Date.now()}-${Math.random()}`,
        fromX: POT_CENTER.x + (i - 2) * 3, // Spread out from center
        fromY: POT_CENTER.y,
        toX: to.x,
        toY: to.y,
        amount: amount / numStacks,
        startTime: Date.now(),
      }

      chipFlightsStore = [...chipFlightsStore, flight]
      notifyListeners()

      // Auto-remove after animation
      setTimeout(() => {
        chipFlightsStore = chipFlightsStore.filter(f => f.id !== flight.id)
        notifyListeners()
      }, FLIGHT_DURATION + 100)
    }, delay)
  }
}

export default function ChipAnimation({ rotateOffset: _rotateOffset }: ChipAnimationProps) {
  const [flights, setFlights] = useState<ChipFlight[]>([])

  useEffect(() => {
    const updateFlights = () => setFlights([...chipFlightsStore])
    listeners.add(updateFlights)
    return () => { listeners.delete(updateFlights) }
  }, [])

  return (
    <div className="absolute inset-0 pointer-events-none overflow-hidden">
      <style>{`
        @keyframes chip-fly {
          0% {
            transform: translate(0, 0) scale(1);
            opacity: 1;
          }
          50% {
            transform: translate(var(--mid-x), var(--mid-y)) scale(1.2);
            opacity: 1;
          }
          100% {
            transform: translate(var(--end-x), var(--end-y)) scale(0.8);
            opacity: 0.8;
          }
        }

        @keyframes chip-arrive {
          0% {
            transform: scale(0.8);
            opacity: 0.8;
          }
          50% {
            transform: scale(1.3);
            opacity: 1;
          }
          100% {
            transform: scale(1);
            opacity: 0;
          }
        }
      `}</style>

      {flights.map(flight => {
        // Calculate offset from start position
        const deltaX = flight.toX - flight.fromX
        const deltaY = flight.toY - flight.fromY

        // Arc motion - go up first then down
        const arcHeight = Math.abs(deltaY) * 0.3 + 10

        return (
          <div
            key={flight.id}
            className="absolute"
            style={{
              left: `${flight.fromX}%`,
              top: `${flight.fromY}%`,
              transform: `translate(-50%, -50%)`,
              '--end-x': `${deltaX}vw`,
              '--end-y': `${deltaY}vh`,
              '--mid-x': `${deltaX * 0.5}vw`,
              '--mid-y': `${deltaY * 0.5 - arcHeight}vh`,
              animation: `chip-fly ${FLIGHT_DURATION}ms ease-out forwards`,
            } as React.CSSProperties}
          >
            <ChipStack amount={flight.amount} />
          </div>
        )
      })}
    </div>
  )
}
