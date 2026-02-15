import { useState, useEffect, useCallback, useRef } from 'react'

interface Particle {
  id: string
  x: number
  y: number
  vx: number
  vy: number
  color: string
  size: number
  life: number
  maxLife: number
  type: 'confetti' | 'spark' | 'star'
  rotation: number
  rotationSpeed: number
}

interface VictoryEffectProps {
  active: boolean
  winnerPositions?: { x: number; y: number }[]
  onComplete?: () => void
}

// Colors for confetti
const CONFETTI_COLORS = [
  '#FFD700', // Gold
  '#FFA500', // Orange
  '#FF6347', // Tomato
  '#00CED1', // Dark Turquoise
  '#9370DB', // Medium Purple
  '#3CB371', // Medium Sea Green
  '#FF69B4', // Hot Pink
  '#00BFFF', // Deep Sky Blue
]

// Colors for sparkles
const SPARK_COLORS = ['#FFD700', '#FFF8DC', '#FFFACD', '#FFFFFF']

export default function VictoryEffect({ active, winnerPositions = [], onComplete }: VictoryEffectProps) {
  const [particles, setParticles] = useState<Particle[]>([])
  const animationRef = useRef<number | null>(null)
  const startTimeRef = useRef<number>(0)

  const createParticle = useCallback((x: number, y: number): Particle => {
    const type = Math.random() < 0.6 ? 'confetti' : Math.random() < 0.7 ? 'spark' : 'star'
    const angle = Math.random() * Math.PI * 2
    const speed = 2 + Math.random() * 4

    return {
      id: `${Date.now()}-${Math.random()}`,
      x,
      y,
      vx: Math.cos(angle) * speed * (Math.random() < 0.5 ? 1 : -1),
      vy: -Math.abs(Math.sin(angle) * speed) - 2, // Always start going up
      color: type === 'spark' || type === 'star'
        ? SPARK_COLORS[Math.floor(Math.random() * SPARK_COLORS.length)]
        : CONFETTI_COLORS[Math.floor(Math.random() * CONFETTI_COLORS.length)],
      size: type === 'confetti' ? 8 + Math.random() * 6 : type === 'star' ? 10 + Math.random() * 8 : 3 + Math.random() * 3,
      life: 1,
      maxLife: 2 + Math.random(),
      type,
      rotation: Math.random() * 360,
      rotationSpeed: (Math.random() - 0.5) * 20,
    }
  }, [])

  const createBurst = useCallback((x: number, y: number, count: number) => {
    const newParticles: Particle[] = []
    for (let i = 0; i < count; i++) {
      newParticles.push(createParticle(x, y))
    }
    setParticles(prev => [...prev, ...newParticles])
  }, [createParticle])

  // Animation loop
  const animate = useCallback(() => {
    const elapsed = (Date.now() - startTimeRef.current) / 1000

    setParticles(prev => {
      const updated = prev
        .map(p => ({
          ...p,
          x: p.x + p.vx,
          y: p.y + p.vy,
          vy: p.vy + 0.15, // Gravity
          vx: p.vx * 0.99, // Air resistance
          rotation: p.rotation + p.rotationSpeed,
          life: p.life - 0.02,
        }))
        .filter(p => p.life > 0 && p.y < window.innerHeight + 50)

      return updated
    })

    // Stop after 3 seconds
    if (elapsed < 3) {
      animationRef.current = requestAnimationFrame(animate)
    } else if (onComplete) {
      onComplete()
    }
  }, [onComplete])

  // Start/stop effect
  useEffect(() => {
    if (active) {
      startTimeRef.current = Date.now()

      // Create initial bursts at winner positions or center
      const burstPositions = winnerPositions.length > 0
        ? winnerPositions
        : [{ x: window.innerWidth / 2, y: window.innerHeight / 2 }]

      // Multiple bursts over time
      const burstCount = 3
      for (let i = 0; i < burstCount; i++) {
        setTimeout(() => {
          burstPositions.forEach(pos => {
            const offsetX = (Math.random() - 0.5) * 100
            const offsetY = (Math.random() - 0.5) * 50
            createBurst(pos.x + offsetX, pos.y + offsetY, 25)
          })
        }, i * 400)
      }

      // Start animation
      animationRef.current = requestAnimationFrame(animate)

      return () => {
        if (animationRef.current) {
          cancelAnimationFrame(animationRef.current)
        }
      }
    } else {
      setParticles([])
      if (animationRef.current) {
        cancelAnimationFrame(animationRef.current)
      }
    }
  }, [active, winnerPositions, createBurst, animate])

  if (!active && particles.length === 0) return null

  return (
    <div className="fixed inset-0 pointer-events-none z-50 overflow-hidden">
      <svg className="absolute inset-0 w-full h-full">
        <defs>
          {/* Glow filter for stars */}
          <filter id="glow">
            <feGaussianBlur stdDeviation="2" result="coloredBlur" />
            <feMerge>
              <feMergeNode in="coloredBlur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
        </defs>

        {particles.map(p => {
          const opacity = Math.min(1, p.life * 2)

          if (p.type === 'confetti') {
            // Rectangular confetti
            return (
              <rect
                key={p.id}
                x={p.x - p.size / 2}
                y={p.y - p.size / 4}
                width={p.size}
                height={p.size / 2}
                fill={p.color}
                opacity={opacity}
                transform={`rotate(${p.rotation} ${p.x} ${p.y})`}
                rx={1}
              />
            )
          } else if (p.type === 'star') {
            // Star shape
            const points = 5
            const outerRadius = p.size / 2
            const innerRadius = outerRadius * 0.4
            const starPoints = []

            for (let i = 0; i < points * 2; i++) {
              const radius = i % 2 === 0 ? outerRadius : innerRadius
              const angle = (i * Math.PI) / points - Math.PI / 2 + (p.rotation * Math.PI) / 180
              starPoints.push(`${p.x + Math.cos(angle) * radius},${p.y + Math.sin(angle) * radius}`)
            }

            return (
              <polygon
                key={p.id}
                points={starPoints.join(' ')}
                fill={p.color}
                opacity={opacity}
                filter="url(#glow)"
              />
            )
          } else {
            // Sparkle / small circle
            return (
              <circle
                key={p.id}
                cx={p.x}
                cy={p.y}
                r={p.size / 2}
                fill={p.color}
                opacity={opacity}
                filter="url(#glow)"
              />
            )
          }
        })}
      </svg>
    </div>
  )
}

// Golden shimmer effect for winner highlight
export function WinnerGlow({ active, children }: { active: boolean; children: React.ReactNode }) {
  if (!active) return <>{children}</>

  return (
    <div className="relative">
      <style>{`
        @keyframes winner-glow {
          0%, 100% {
            box-shadow: 0 0 10px 2px rgba(255, 215, 0, 0.4),
                        0 0 20px 4px rgba(255, 215, 0, 0.2),
                        0 0 30px 6px rgba(255, 215, 0, 0.1);
          }
          50% {
            box-shadow: 0 0 20px 4px rgba(255, 215, 0, 0.6),
                        0 0 40px 8px rgba(255, 215, 0, 0.4),
                        0 0 60px 12px rgba(255, 215, 0, 0.2);
          }
        }

        @keyframes winner-pulse {
          0%, 100% { transform: scale(1); }
          50% { transform: scale(1.02); }
        }
      `}</style>
      <div
        className="rounded-lg"
        style={{
          animation: 'winner-glow 1.5s ease-in-out infinite, winner-pulse 1.5s ease-in-out infinite',
        }}
      >
        {children}
      </div>
    </div>
  )
}
