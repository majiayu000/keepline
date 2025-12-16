import { memo, useState, useCallback } from 'react'
import { Button } from '@/components/Button'
import type { TerminalApp } from '@/types'
import styles from './TerminalSelector.module.css'

interface TerminalSelectorProps {
  onSelect: (terminal: TerminalApp) => void
  loading?: boolean
  disabled?: boolean
}

const TERMINALS: { id: TerminalApp; label: string; icon: string }[] = [
  { id: 'Warp', label: 'Warp', icon: '⚡' },
  { id: 'iTerm', label: 'iTerm', icon: '🖥' },
  { id: 'Terminal', label: 'Terminal', icon: '▶' },
]

export const TerminalSelector = memo(function TerminalSelector({
  onSelect,
  loading = false,
  disabled = false,
}: TerminalSelectorProps) {
  const [expanded, setExpanded] = useState(false)
  const [selectedTerminal, setSelectedTerminal] = useState<TerminalApp | null>(null)

  const handleToggle = useCallback(() => {
    setExpanded(prev => !prev)
  }, [])

  const handleSelect = useCallback((terminal: TerminalApp) => {
    setSelectedTerminal(terminal)
    setExpanded(false)
    onSelect(terminal)
  }, [onSelect])

  if (!expanded) {
    return (
      <Button
        variant="success"
        size="sm"
        onClick={handleToggle}
        disabled={disabled || loading}
        loading={loading}
        aria-label="Open in terminal"
        aria-haspopup="menu"
      >
        {loading ? 'Opening...' : 'Recover'}
      </Button>
    )
  }

  return (
    <div className={styles.container} role="menu" aria-label="Select terminal">
      <div className={styles.label}>Open in:</div>
      <div className={styles.buttons}>
        {TERMINALS.map(terminal => (
          <Button
            key={terminal.id}
            variant={selectedTerminal === terminal.id ? 'primary' : 'secondary'}
            size="sm"
            onClick={() => handleSelect(terminal.id)}
            disabled={disabled || loading}
            loading={loading && selectedTerminal === terminal.id}
            aria-label={`Open in ${terminal.label}`}
            role="menuitem"
          >
            <span className={styles.icon} aria-hidden="true">{terminal.icon}</span>
            {terminal.label}
          </Button>
        ))}
        <Button
          variant="ghost"
          size="sm"
          onClick={handleToggle}
          disabled={loading}
          aria-label="Cancel"
        >
          ✕
        </Button>
      </div>
    </div>
  )
})
