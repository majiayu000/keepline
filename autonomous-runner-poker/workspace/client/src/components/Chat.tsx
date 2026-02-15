import { useState, useRef, useEffect } from 'react'
import QuickEmotes from './QuickEmotes'

export interface ChatMessage {
  id: string
  username: string
  message: string
  timestamp: number
  isSystem?: boolean
}

interface ChatProps {
  messages: ChatMessage[]
  onSendMessage: (text: string) => void
  isCollapsed?: boolean
  onToggleCollapse?: () => void
  unreadCount?: number
}

export default function Chat({
  messages,
  onSendMessage,
  isCollapsed = false,
  onToggleCollapse,
  unreadCount = 0,
}: ChatProps) {
  const [inputText, setInputText] = useState('')
  const [showEmotes, setShowEmotes] = useState(false)
  const messagesEndRef = useRef<HTMLDivElement>(null)
  const messagesContainerRef = useRef<HTMLDivElement>(null)

  // Auto-scroll to bottom when new messages arrive (if not collapsed)
  useEffect(() => {
    if (!isCollapsed && messagesEndRef.current) {
      messagesEndRef.current.scrollIntoView({ behavior: 'smooth' })
    }
  }, [messages, isCollapsed])

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (inputText.trim()) {
      onSendMessage(inputText.trim())
      setInputText('')
    }
  }

  const handleEmoteSelect = (emote: string) => {
    onSendMessage(emote)
    setShowEmotes(false)
  }

  // Format timestamp to HH:MM
  const formatTime = (timestamp: number) => {
    const date = new Date(timestamp)
    return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
  }

  // Collapsed view - just show toggle button
  if (isCollapsed) {
    return (
      <button
        onClick={onToggleCollapse}
        className="relative flex items-center gap-2 bg-black/70 hover:bg-black/80
                   text-white px-3 py-2 rounded-lg transition-colors"
      >
        <svg
          className="w-5 h-5"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            strokeLinecap="round"
            strokeLinejoin="round"
            strokeWidth={2}
            d="M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.418-4.03 8-9 8a9.863 9.863 0 01-4.255-.949L3 20l1.395-3.72C3.512 15.042 3 13.574 3 12c0-4.418 4.03-8 9-8s9 3.582 9 8z"
          />
        </svg>
        <span className="text-sm font-medium">Chat</span>
        {unreadCount > 0 && (
          <span className="absolute -top-1 -right-1 bg-red-500 text-white text-xs
                           rounded-full w-5 h-5 flex items-center justify-center font-bold">
            {unreadCount > 9 ? '9+' : unreadCount}
          </span>
        )}
      </button>
    )
  }

  // Expanded view - full chat window
  return (
    <div className="bg-black/70 rounded-lg flex flex-col h-72 w-80 backdrop-blur-sm border border-gray-700">
      {/* Header */}
      <div className="flex items-center justify-between px-3 py-2 border-b border-gray-700">
        <span className="text-white font-medium text-sm">Table Chat</span>
        {onToggleCollapse && (
          <button
            onClick={onToggleCollapse}
            className="text-gray-400 hover:text-white transition-colors p-1"
            title="Minimize chat"
          >
            <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
            </svg>
          </button>
        )}
      </div>

      {/* Messages area */}
      <div
        ref={messagesContainerRef}
        className="flex-1 overflow-y-auto p-2 space-y-1.5 scrollbar-thin scrollbar-thumb-gray-600"
      >
        {messages.length === 0 ? (
          <div className="text-gray-500 text-sm text-center py-8">
            No messages yet. Say hello!
          </div>
        ) : (
          messages.map((msg, index) => (
            <div
              key={`${msg.id}-${msg.timestamp}-${index}`}
              className={`text-sm ${msg.id === 'system' ? 'italic' : ''}`}
            >
              <span className="text-gray-500 text-xs mr-1.5">
                {formatTime(msg.timestamp)}
              </span>
              <span
                className={`font-bold ${
                  msg.id === 'system'
                    ? 'text-blue-400'
                    : 'text-poker-gold'
                }`}
              >
                {msg.username}:
              </span>
              <span className={`ml-1 ${msg.id === 'system' ? 'text-gray-400' : 'text-white'}`}>
                {msg.message}
              </span>
            </div>
          ))
        )}
        <div ref={messagesEndRef} />
      </div>

      {/* Quick emotes panel (toggleable) */}
      {showEmotes && (
        <div className="px-2 py-1.5 border-t border-gray-700 bg-gray-800/50">
          <QuickEmotes onSelectEmote={handleEmoteSelect} compact={true} />
        </div>
      )}

      {/* Input area */}
      <form onSubmit={handleSubmit} className="p-2 border-t border-gray-700">
        <div className="flex gap-2">
          {/* Emote toggle button */}
          <button
            type="button"
            onClick={() => setShowEmotes(!showEmotes)}
            className={`flex-shrink-0 w-8 h-8 flex items-center justify-center rounded
                       transition-colors ${
                         showEmotes
                           ? 'bg-poker-gold text-black'
                           : 'bg-gray-700 text-gray-300 hover:bg-gray-600 hover:text-white'
                       }`}
            title="Quick emotes"
          >
            <span className="text-lg">😊</span>
          </button>
          <input
            type="text"
            value={inputText}
            onChange={(e) => setInputText(e.target.value)}
            placeholder="Type a message..."
            maxLength={200}
            className="flex-1 px-3 py-1.5 rounded bg-gray-800 text-white text-sm
                       border border-gray-600 focus:border-poker-gold focus:outline-none
                       placeholder-gray-500"
          />
          <button
            type="submit"
            disabled={!inputText.trim()}
            className="px-3 py-1.5 bg-poker-gold text-black rounded font-bold text-sm
                       hover:bg-yellow-400 transition-colors disabled:opacity-50
                       disabled:cursor-not-allowed"
          >
            Send
          </button>
        </div>
      </form>
    </div>
  )
}
