import { useState } from 'react'

// Preset poker emotes and expressions
export const QUICK_EMOTES = [
  // Common poker expressions
  { text: 'GG', label: 'GG', tooltip: 'Good Game' },
  { text: 'NH', label: 'NH', tooltip: 'Nice Hand' },
  { text: 'WP', label: 'WP', tooltip: 'Well Played' },
  { text: 'TY', label: 'TY', tooltip: 'Thank You' },
  { text: 'GL', label: 'GL', tooltip: 'Good Luck' },
  { text: 'LOL', label: 'LOL', tooltip: 'Laugh Out Loud' },
  // Emojis
  { text: '👍', label: '👍', tooltip: 'Thumbs Up' },
  { text: '👏', label: '👏', tooltip: 'Applause' },
  { text: '🎉', label: '🎉', tooltip: 'Celebration' },
  { text: '😎', label: '😎', tooltip: 'Cool' },
  { text: '😭', label: '😭', tooltip: 'Crying' },
  { text: '🔥', label: '🔥', tooltip: 'Fire' },
]

// Extended emotes for expanded view
export const EXTENDED_EMOTES = [
  { text: 'SHIP IT!', label: 'SHIP IT!', tooltip: 'All In!' },
  { text: 'BRB', label: 'BRB', tooltip: 'Be Right Back' },
  { text: 'SIT OUT', label: 'SIT OUT', tooltip: 'Sitting Out' },
  { text: 'BLUFF', label: 'BLUFF', tooltip: 'That was a bluff!' },
  { text: 'TILTED', label: 'TILTED', tooltip: 'On tilt!' },
  { text: 'UNLUCKY', label: 'UNLUCKY', tooltip: 'Unlucky!' },
  { text: '🃏', label: '🃏', tooltip: 'Joker' },
  { text: '💰', label: '💰', tooltip: 'Money' },
  { text: '🤑', label: '🤑', tooltip: 'Rich' },
  { text: '😤', label: '😤', tooltip: 'Frustrated' },
  { text: '🤔', label: '🤔', tooltip: 'Thinking' },
  { text: '😏', label: '😏', tooltip: 'Smirk' },
]

interface QuickEmotesProps {
  onSelectEmote: (emote: string) => void
  compact?: boolean
}

export default function QuickEmotes({ onSelectEmote, compact = true }: QuickEmotesProps) {
  const [isExpanded, setIsExpanded] = useState(false)
  const [hoveredEmote, setHoveredEmote] = useState<string | null>(null)

  const displayEmotes = compact && !isExpanded ? QUICK_EMOTES.slice(0, 6) : QUICK_EMOTES

  return (
    <div className="relative">
      {/* Main emotes row */}
      <div className="flex flex-wrap items-center gap-1">
        {displayEmotes.map((emote) => (
          <button
            key={emote.text}
            onClick={() => onSelectEmote(emote.text)}
            onMouseEnter={() => setHoveredEmote(emote.text)}
            onMouseLeave={() => setHoveredEmote(null)}
            className="relative px-1.5 py-0.5 text-xs bg-gray-700 hover:bg-gray-600
                       rounded transition-colors text-white/90 hover:text-white
                       active:scale-95 active:bg-gray-500"
            title={emote.tooltip}
          >
            {emote.label}
            {/* Tooltip */}
            {hoveredEmote === emote.text && (
              <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-1 px-2 py-1
                              bg-black text-white text-xs rounded whitespace-nowrap z-10
                              pointer-events-none shadow-lg">
                {emote.tooltip}
              </div>
            )}
          </button>
        ))}

        {/* Expand/collapse toggle button */}
        {compact && (
          <button
            onClick={() => setIsExpanded(!isExpanded)}
            className="px-1.5 py-0.5 text-xs bg-gray-700 hover:bg-gray-600
                       rounded transition-colors text-gray-400 hover:text-white"
            title={isExpanded ? 'Show less' : 'Show more'}
          >
            {isExpanded ? '−' : '+'}
          </button>
        )}
      </div>

      {/* Extended emotes panel (when expanded) */}
      {isExpanded && (
        <div className="mt-1 flex flex-wrap items-center gap-1">
          {QUICK_EMOTES.slice(6).map((emote) => (
            <button
              key={emote.text}
              onClick={() => onSelectEmote(emote.text)}
              onMouseEnter={() => setHoveredEmote(emote.text)}
              onMouseLeave={() => setHoveredEmote(null)}
              className="relative px-1.5 py-0.5 text-xs bg-gray-700 hover:bg-gray-600
                         rounded transition-colors text-white/90 hover:text-white
                         active:scale-95 active:bg-gray-500"
              title={emote.tooltip}
            >
              {emote.label}
              {hoveredEmote === emote.text && (
                <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-1 px-2 py-1
                                bg-black text-white text-xs rounded whitespace-nowrap z-10
                                pointer-events-none shadow-lg">
                  {emote.tooltip}
                </div>
              )}
            </button>
          ))}
          {EXTENDED_EMOTES.map((emote) => (
            <button
              key={emote.text}
              onClick={() => onSelectEmote(emote.text)}
              onMouseEnter={() => setHoveredEmote(emote.text)}
              onMouseLeave={() => setHoveredEmote(null)}
              className="relative px-1.5 py-0.5 text-xs bg-gray-700 hover:bg-gray-600
                         rounded transition-colors text-white/90 hover:text-white
                         active:scale-95 active:bg-gray-500"
              title={emote.tooltip}
            >
              {emote.label}
              {hoveredEmote === emote.text && (
                <div className="absolute bottom-full left-1/2 -translate-x-1/2 mb-1 px-2 py-1
                                bg-black text-white text-xs rounded whitespace-nowrap z-10
                                pointer-events-none shadow-lg">
                  {emote.tooltip}
                </div>
              )}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}

// Compact inline emote bar for quick access
export function EmoteBar({ onSelectEmote }: { onSelectEmote: (emote: string) => void }) {
  // Show only the most common emotes in a very compact format
  const topEmotes = QUICK_EMOTES.slice(0, 8)

  return (
    <div className="flex items-center gap-0.5 overflow-x-auto scrollbar-none">
      {topEmotes.map((emote) => (
        <button
          key={emote.text}
          onClick={() => onSelectEmote(emote.text)}
          className="flex-shrink-0 px-1 py-0.5 text-xs bg-gray-700/50 hover:bg-gray-600
                     rounded transition-colors text-white/80 hover:text-white
                     active:scale-95"
          title={emote.tooltip}
        >
          {emote.label}
        </button>
      ))}
    </div>
  )
}
