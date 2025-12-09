import { ReactNode } from 'react'
import { Header } from '@/components/Header'
import { StatsBar } from '@/components/StatsBar'
import type { SessionStats } from '@/types'
import styles from './Layout.module.css'

interface LayoutProps {
  children: ReactNode
  stats: SessionStats | null
  loading?: boolean
  onSync: () => void
  syncing?: boolean
}

export function Layout({ children, stats, loading, onSync, syncing }: LayoutProps) {
  return (
    <div className={styles.layout}>
      <Header onSync={onSync} syncing={syncing} />
      <StatsBar stats={stats} loading={loading} />
      <main className={styles.main}>{children}</main>
    </div>
  )
}
