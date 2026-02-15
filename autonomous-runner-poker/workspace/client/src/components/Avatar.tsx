import { useMemo } from 'react'

interface AvatarProps {
  name: string
  odid: string  // Used for deterministic color generation
  avatarUrl?: string
  size?: 'xs' | 'sm' | 'md' | 'lg'
  isOnline?: boolean
  showStatusDot?: boolean
}

// Poker-themed color palettes for avatar backgrounds
const COLOR_PALETTES = [
  ['#1a4731', '#2d6a4f'], // Green felt
  ['#7c2d12', '#c2410c'], // Orange/copper
  ['#1e3a5f', '#3b82f6'], // Blue
  ['#4c1d95', '#7c3aed'], // Purple
  ['#991b1b', '#dc2626'], // Red
  ['#065f46', '#10b981'], // Emerald
  ['#713f12', '#ca8a04'], // Gold
  ['#0f172a', '#334155'], // Slate
  ['#831843', '#db2777'], // Pink
  ['#164e63', '#06b6d4'], // Cyan
]

// Generate a deterministic hash from string
function hashString(str: string): number {
  let hash = 0
  for (let i = 0; i < str.length; i++) {
    const char = str.charCodeAt(i)
    hash = ((hash << 5) - hash) + char
    hash = hash & hash // Convert to 32bit integer
  }
  return Math.abs(hash)
}

// Generate gradient colors based on user ID
function getGradientColors(odid: string): [string, string] {
  const hash = hashString(odid)
  const palette = COLOR_PALETTES[hash % COLOR_PALETTES.length]
  return [palette[0], palette[1]]
}

// Get initials from name (max 2 characters)
function getInitials(name: string): string {
  if (!name) return '?'

  const parts = name.trim().split(/\s+/)
  if (parts.length === 1) {
    // Single word: take first 1-2 characters
    return name.slice(0, 2).toUpperCase()
  }
  // Multiple words: take first letter of first two words
  return (parts[0][0] + (parts[1]?.[0] || '')).toUpperCase()
}

const SIZE_CLASSES = {
  xs: 'w-5 h-5 text-[8px]',
  sm: 'w-7 h-7 md:w-8 md:h-8 text-[10px] md:text-xs',
  md: 'w-10 h-10 md:w-12 md:h-12 text-sm md:text-base',
  lg: 'w-16 h-16 md:w-20 md:h-20 text-xl md:text-2xl',
}

const DOT_SIZE_CLASSES = {
  xs: 'w-1.5 h-1.5 border',
  sm: 'w-2 h-2 border',
  md: 'w-2.5 h-2.5 border-2',
  lg: 'w-3.5 h-3.5 border-2',
}

export default function Avatar({
  name,
  odid,
  avatarUrl,
  size = 'sm',
  isOnline = true,
  showStatusDot = false
}: AvatarProps) {
  // Memoize gradient to avoid recalculating on every render
  const gradientStyle = useMemo(() => {
    const [color1, color2] = getGradientColors(odid)
    return {
      background: `linear-gradient(135deg, ${color1} 0%, ${color2} 100%)`,
    }
  }, [odid])

  const initials = useMemo(() => getInitials(name), [name])

  return (
    <div className="relative inline-block">
      {avatarUrl ? (
        // Custom avatar image
        <img
          src={avatarUrl}
          alt={`${name}'s avatar`}
          className={`${SIZE_CLASSES[size]} rounded-full object-cover ring-1 ring-white/20`}
          onError={(e) => {
            // Fallback to initials on image load error
            const target = e.target as HTMLImageElement
            target.style.display = 'none'
            const parent = target.parentElement
            if (parent) {
              const fallback = parent.querySelector('.avatar-fallback')
              if (fallback) {
                (fallback as HTMLElement).style.display = 'flex'
              }
            }
          }}
        />
      ) : null}

      {/* Fallback/Default avatar with initials */}
      <div
        className={`avatar-fallback ${SIZE_CLASSES[size]} rounded-full flex items-center justify-center font-bold text-white ring-1 ring-white/20 select-none
          ${avatarUrl ? 'hidden' : 'flex'}`}
        style={gradientStyle}
      >
        {initials}
      </div>

      {/* Online status dot */}
      {showStatusDot && (
        <div
          className={`absolute -bottom-0.5 -right-0.5 ${DOT_SIZE_CLASSES[size]} rounded-full border-gray-900
            ${isOnline ? 'bg-green-500' : 'bg-gray-500'}`}
        />
      )}
    </div>
  )
}

// Export utility functions for use elsewhere
export { getGradientColors, getInitials, hashString }
