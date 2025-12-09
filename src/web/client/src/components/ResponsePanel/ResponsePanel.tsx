import { useState, useRef, useCallback } from 'react'
import ReactMarkdown from 'react-markdown'
import styles from './ResponsePanel.module.css'

interface ResponsePanelProps {
  content: string
  defaultHeight?: number
}

export function ResponsePanel({ content, defaultHeight = 200 }: ResponsePanelProps) {
  const [height, setHeight] = useState(defaultHeight)
  const panelRef = useRef<HTMLDivElement>(null)
  const startY = useRef(0)
  const startHeight = useRef(0)

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault()
    startY.current = e.clientY
    startHeight.current = height

    const handleMouseMove = (e: MouseEvent) => {
      const delta = e.clientY - startY.current
      const newHeight = Math.max(100, Math.min(600, startHeight.current + delta))
      setHeight(newHeight)
    }

    const handleMouseUp = () => {
      document.removeEventListener('mousemove', handleMouseMove)
      document.removeEventListener('mouseup', handleMouseUp)
    }

    document.addEventListener('mousemove', handleMouseMove)
    document.addEventListener('mouseup', handleMouseUp)
  }, [height])

  return (
    <div className={styles.container}>
      <div
        ref={panelRef}
        className={styles.content}
        style={{ height: `${height}px` }}
      >
        <ReactMarkdown
          components={{
            code: ({ className, children, ...props }) => {
              const isInline = !className
              return isInline ? (
                <code className={styles.inlineCode} {...props}>{children}</code>
              ) : (
                <pre className={styles.codeBlock}>
                  <code className={className} {...props}>{children}</code>
                </pre>
              )
            },
            table: ({ children }) => (
              <div className={styles.tableWrapper}>
                <table className={styles.table}>{children}</table>
              </div>
            ),
          }}
        >
          {content}
        </ReactMarkdown>
      </div>
      <div className={styles.resizeHandle} onMouseDown={handleMouseDown}>
        <span className={styles.resizeIcon}>⋯</span>
      </div>
    </div>
  )
}
