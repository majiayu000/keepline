import { useEffect, useState } from 'react'
import type { Card as CardType, RankNumber } from '../types/poker'
import { getRankDisplay } from '../types/poker'

interface CardProps {
  card: CardType | null
  faceDown?: boolean
  small?: boolean
  animate?: 'deal' | 'flip' | 'highlight' | 'none'
  animationDelay?: number // ms
}

const SUIT_SYMBOLS: Record<string, string> = {
  hearts: '\u2665',
  diamonds: '\u2666',
  clubs: '\u2663',
  spades: '\u2660',
}

const SUIT_COLORS: Record<string, string> = {
  hearts: 'text-red-500',
  diamonds: 'text-red-500',
  clubs: 'text-gray-900',
  spades: 'text-gray-900',
}

// Animation keyframes as inline styles (to avoid Tailwind config changes)
const ANIMATION_STYLES = `
@keyframes card-deal {
  0% {
    transform: translateY(-100px) translateX(-50px) rotate(-15deg) scale(0.5);
    opacity: 0;
  }
  50% {
    opacity: 1;
  }
  100% {
    transform: translateY(0) translateX(0) rotate(0deg) scale(1);
    opacity: 1;
  }
}

@keyframes card-flip {
  0% {
    transform: rotateY(180deg) scale(0.9);
  }
  50% {
    transform: rotateY(90deg) scale(1.05);
  }
  100% {
    transform: rotateY(0deg) scale(1);
  }
}

@keyframes card-highlight {
  0%, 100% {
    box-shadow: 0 0 0 rgba(234, 179, 8, 0);
  }
  50% {
    box-shadow: 0 0 20px rgba(234, 179, 8, 0.8);
  }
}

@keyframes card-pop {
  0% {
    transform: scale(1);
  }
  50% {
    transform: scale(1.1);
  }
  100% {
    transform: scale(1);
  }
}
`

export default function Card({
  card,
  faceDown = false,
  small = false,
  animate = 'none',
  animationDelay = 0,
}: CardProps) {
  const [isAnimating, setIsAnimating] = useState(animate !== 'none')
  const [showCard, setShowCard] = useState(animate === 'none')

  // Responsive sizes: small cards are used in player hands, normal for community cards
  // On mobile (< md), we use smaller sizes
  const baseSize = small
    ? 'w-8 h-11 md:w-10 md:h-14'
    : 'w-10 h-14 md:w-14 md:h-20'
  const fontSize = small
    ? 'text-[10px] md:text-xs'
    : 'text-xs md:text-sm'
  const centerSuitSize = small
    ? 'text-lg md:text-xl'
    : 'text-xl md:text-2xl'

  // Handle animation delay
  useEffect(() => {
    if (animate !== 'none' && animationDelay > 0) {
      setShowCard(false)
      const timer = setTimeout(() => {
        setShowCard(true)
        setIsAnimating(true)
      }, animationDelay)
      return () => clearTimeout(timer)
    } else if (animate !== 'none') {
      setShowCard(true)
      setIsAnimating(true)
    } else {
      setShowCard(true)
      setIsAnimating(false)
    }
  }, [animate, animationDelay])

  // Clear animation after it completes
  useEffect(() => {
    if (isAnimating) {
      const timer = setTimeout(() => {
        setIsAnimating(false)
      }, 600) // Animation duration
      return () => clearTimeout(timer)
    }
  }, [isAnimating])

  // Hidden before animation starts
  if (!showCard) {
    return <div className={`${baseSize}`} />
  }

  // Animation style based on type
  const getAnimationStyle = () => {
    if (!isAnimating || animate === 'none') return {}

    switch (animate) {
      case 'deal':
        return {
          animation: 'card-deal 0.4s ease-out forwards',
        }
      case 'flip':
        return {
          animation: 'card-flip 0.5s ease-in-out forwards',
        }
      case 'highlight':
        return {
          animation: 'card-highlight 1s ease-in-out infinite',
        }
      default:
        return {}
    }
  }

  // Face down card (back)
  if (faceDown || !card) {
    return (
      <>
        <style>{ANIMATION_STYLES}</style>
        <div
          className={`${baseSize} rounded-lg bg-gradient-to-br from-blue-800 to-blue-900
                      border-2 border-white shadow-lg flex items-center justify-center
                      transition-transform duration-200 hover:scale-105`}
          style={getAnimationStyle()}
        >
          <div className="w-3/4 h-3/4 bg-blue-700 rounded border border-blue-600
                         bg-[radial-gradient(ellipse_at_center,_var(--tw-gradient-stops))]
                         from-blue-600 to-blue-800" />
        </div>
      </>
    )
  }

  const suitSymbol = SUIT_SYMBOLS[card.suit]
  const suitColor = SUIT_COLORS[card.suit]
  const rankDisplay = getRankDisplay(card.rank as RankNumber)

  return (
    <>
      <style>{ANIMATION_STYLES}</style>
      <div
        className={`${baseSize} rounded-lg bg-white border-2 border-gray-300 shadow-lg
                    flex flex-col justify-between p-1 transition-transform duration-200
                    hover:scale-105 hover:shadow-xl`}
        style={getAnimationStyle()}
      >
        {/* Top left */}
        <div className={`${fontSize} ${suitColor} font-bold leading-none`}>
          <div>{rankDisplay}</div>
          <div>{suitSymbol}</div>
        </div>

        {/* Center suit */}
        <div className={`${centerSuitSize} ${suitColor} text-center`}>
          {suitSymbol}
        </div>

        {/* Bottom right (rotated) */}
        <div className={`${fontSize} ${suitColor} font-bold leading-none self-end rotate-180`}>
          <div>{rankDisplay}</div>
          <div>{suitSymbol}</div>
        </div>
      </div>
    </>
  )
}
