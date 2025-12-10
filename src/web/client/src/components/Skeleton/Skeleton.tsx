import { memo } from 'react'
import styles from './Skeleton.module.css'

interface SkeletonProps {
  width?: string | number
  height?: string | number
  variant?: 'text' | 'rect' | 'circle'
  className?: string
}

export const Skeleton = memo(function Skeleton({
  width,
  height,
  variant = 'text',
  className = ''
}: SkeletonProps) {
  return (
    <div
      className={`${styles.skeleton} ${styles[variant]} ${className}`}
      style={{ width, height }}
      aria-hidden="true"
    />
  )
})

interface SessionCardSkeletonProps {
  count?: number
}

export const SessionCardSkeleton = memo(function SessionCardSkeleton({ count = 3 }: SessionCardSkeletonProps) {
  return (
    <div className={styles.cardList}>
      {Array.from({ length: count }).map((_, i) => (
        <div key={i} className={styles.card}>
          <div className={styles.cardHeader}>
            <Skeleton width={60} height={16} />
            <Skeleton width={80} height={14} />
          </div>
          <Skeleton width="70%" height={18} className={styles.cardTitle} />
          <Skeleton width="50%" height={14} />
          <div className={styles.cardStats}>
            <Skeleton width={60} height={12} />
            <Skeleton width={80} height={12} />
            <Skeleton width={70} height={12} />
          </div>
        </div>
      ))}
    </div>
  )
})
