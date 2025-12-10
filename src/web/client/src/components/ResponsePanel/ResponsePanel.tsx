import { useState, useRef, useCallback, memo, useMemo } from 'react'
import ReactMarkdown, { Components } from 'react-markdown'
import styles from './ResponsePanel.module.css'

interface ResponsePanelProps {
  content: string
  defaultHeight?: number
}

// Extract markdown components outside to avoid recreation on each render
const createMarkdownComponents = (styles: Record<string, string>): Components => ({
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
})

export const ResponsePanel = memo(function ResponsePanel({
  content,
  defaultHeight = 200
}: ResponsePanelProps) {
  const [height, setHeight] = useState(defaultHeight)
  const panelRef = useRef<HTMLDivElement>(null)
  const dragState = useRef({ startY: 0, startHeight: 0 })

  // Memoize markdown components
  const markdownComponents = useMemo(() => createMarkdownComponents(styles), [])

  // Use ref-based approach to avoid height in dependency array
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault()
    dragState.current.startY = e.clientY
    dragState.current.startHeight = height

    const handleMouseMove = (e: MouseEvent) => {
      const delta = e.clientY - dragState.current.startY
      const newHeight = Math.max(100, Math.min(600, dragState.current.startHeight + delta))
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
        <ReactMarkdown components={markdownComponents}>
          {content}
        </ReactMarkdown>
      </div>
      <div
        className={styles.resizeHandle}
        onMouseDown={handleMouseDown}
        role="separator"
        aria-label="Resize panel"
        aria-orientation="horizontal"
      >
        <span className={styles.resizeIcon}>⋯</span>
      </div>
    </div>
  )
})
