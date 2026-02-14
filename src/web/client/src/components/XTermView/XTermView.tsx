/**
 * XTermView - xterm.js terminal instance
 *
 * Renders a terminal, pipes I/O through callbacks,
 * handles resize via FitAddon + ResizeObserver.
 */

import { useEffect, useRef } from 'react'
import { Terminal } from '@xterm/xterm'
import { FitAddon } from '@xterm/addon-fit'
import { WebLinksAddon } from '@xterm/addon-web-links'
import '@xterm/xterm/css/xterm.css'
import styles from './XTermView.module.css'

interface XTermViewProps {
  sessionId: string
  onInput: (data: string) => void
  onResize: (cols: number, rows: number) => void
  registerOutput: (sessionId: string, handler: (data: string) => void) => () => void
  disconnected?: boolean
}

export function XTermView({ sessionId, onInput, onResize, registerOutput, disconnected }: XTermViewProps) {
  const containerRef = useRef<HTMLDivElement>(null)
  const termRef = useRef<Terminal | null>(null)
  const fitRef = useRef<FitAddon | null>(null)

  useEffect(() => {
    if (!containerRef.current) return

    const term = new Terminal({
      cursorBlink: true,
      fontSize: 14,
      fontFamily: "'JetBrains Mono', 'Fira Code', 'Cascadia Code', Menlo, monospace",
      theme: {
        background: '#0a0a0f',
        foreground: '#e0e0e0',
        cursor: '#00ff88',
        selectionBackground: 'rgba(0, 255, 136, 0.2)',
        black: '#1a1a2e',
        red: '#ff5555',
        green: '#00ff88',
        yellow: '#ffcc00',
        blue: '#6699ff',
        magenta: '#cc66ff',
        cyan: '#00ccff',
        white: '#e0e0e0',
        brightBlack: '#555555',
        brightRed: '#ff7777',
        brightGreen: '#33ffaa',
        brightYellow: '#ffdd44',
        brightBlue: '#88aaff',
        brightMagenta: '#dd88ff',
        brightCyan: '#44ddff',
        brightWhite: '#ffffff',
      },
      allowProposedApi: true,
    })

    const fitAddon = new FitAddon()
    const webLinksAddon = new WebLinksAddon()

    term.loadAddon(fitAddon)
    term.loadAddon(webLinksAddon)
    term.open(containerRef.current)

    // Initial fit
    requestAnimationFrame(() => {
      fitAddon.fit()
      onResize(term.cols, term.rows)
    })

    // Pipe user input to callback
    const inputDisposable = term.onData((data) => {
      onInput(data)
    })

    // Register output handler for this session
    const unregister = registerOutput(sessionId, (data: string) => {
      term.write(data)
    })

    // ResizeObserver for auto-fit
    const observer = new ResizeObserver(() => {
      requestAnimationFrame(() => {
        fitAddon.fit()
        onResize(term.cols, term.rows)
      })
    })
    observer.observe(containerRef.current)

    termRef.current = term
    fitRef.current = fitAddon

    return () => {
      observer.disconnect()
      inputDisposable.dispose()
      unregister()
      term.dispose()
      termRef.current = null
      fitRef.current = null
    }
  }, [sessionId, onInput, onResize, registerOutput])

  return (
    <div className={styles.wrapper}>
      <div ref={containerRef} className={styles.terminal} />
      {disconnected && (
        <div className={styles.overlay}>
          <span>Disconnected - reconnecting...</span>
        </div>
      )}
    </div>
  )
}
