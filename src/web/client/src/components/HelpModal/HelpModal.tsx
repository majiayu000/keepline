import { memo, useEffect, useRef } from 'react'
import styles from './HelpModal.module.css'

interface HelpModalProps {
  isOpen: boolean
  onClose: () => void
}

interface ShortcutItem {
  label: string
  keys: string[]
}

interface ShortcutSection {
  title: string
  shortcuts: ShortcutItem[]
}

const SHORTCUT_SECTIONS: ShortcutSection[] = [
  {
    title: 'Navigation',
    shortcuts: [
      { label: 'Focus search', keys: ['/'] },
      { label: 'Clear search / Close', keys: ['Esc'] },
    ],
  },
  {
    title: 'Actions',
    shortcuts: [
      { label: 'Refresh sessions', keys: ['r'] },
      { label: 'Sync sessions', keys: ['s'] },
      { label: 'Show help', keys: ['?'] },
    ],
  },
  {
    title: 'Themes',
    shortcuts: [
      { label: 'Cycle themes', keys: ['t'] },
      { label: 'Neon theme', keys: ['1'] },
      { label: 'Matrix theme', keys: ['2'] },
      { label: 'Sunset theme', keys: ['3'] },
      { label: 'Ocean theme', keys: ['4'] },
      { label: 'Minimal theme', keys: ['5'] },
    ],
  },
]

export const HelpModal = memo(function HelpModal({ isOpen, onClose }: HelpModalProps) {
  const modalRef = useRef<HTMLDivElement>(null)

  // Close on Escape
  useEffect(() => {
    if (!isOpen) return

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose()
      }
    }

    document.addEventListener('keydown', handleKeyDown)
    return () => document.removeEventListener('keydown', handleKeyDown)
  }, [isOpen, onClose])

  // Close on click outside
  const handleOverlayClick = (e: React.MouseEvent) => {
    if (e.target === e.currentTarget) {
      onClose()
    }
  }

  if (!isOpen) return null

  return (
    <div className={styles.overlay} onClick={handleOverlayClick}>
      <div className={styles.modal} ref={modalRef} role="dialog" aria-label="Keyboard shortcuts">
        <div className={styles.header}>
          <h2 className={styles.title}>Keyboard Shortcuts</h2>
          <button className={styles.closeButton} onClick={onClose} aria-label="Close">
            ✕
          </button>
        </div>

        <div className={styles.content}>
          {SHORTCUT_SECTIONS.map((section) => (
            <div key={section.title} className={styles.section}>
              <h3 className={styles.sectionTitle}>{section.title}</h3>
              <div className={styles.shortcutList}>
                {section.shortcuts.map((shortcut) => (
                  <div key={shortcut.label} className={styles.shortcutItem}>
                    <span className={styles.shortcutLabel}>{shortcut.label}</span>
                    <div className={styles.shortcutKeys}>
                      {shortcut.keys.map((key, index) => (
                        <span key={key}>
                          {index > 0 && <span className={styles.plus}>+</span>}
                          <kbd className={styles.key}>{key}</kbd>
                        </span>
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>

        <div className={styles.footer}>
          <p className={styles.footerText}>
            Press <kbd className={styles.key}>?</kbd> anytime to show this help
          </p>
        </div>
      </div>
    </div>
  )
})
