import styles from './Spinner.module.css'

interface SpinnerProps {
  size?: 'sm' | 'md' | 'lg'
  className?: string
}

export function Spinner({ size = 'md', className = '' }: SpinnerProps) {
  return (
    <div className={`${styles.spinner} ${styles[size]} ${className}`}>
      <div className={styles.inner} />
    </div>
  )
}
