import { useCallback,useEffect,useState } from 'react'
import { ledgerRequest } from '@/services/api'
import { getWebSocketManager } from '@/services/websocket'
import type { LedgerConfig,LedgerDetail } from '../../../../../domain/ledger/types'
import { openNativeLedger,quitNativeApp,setNativeCounts } from './native'
import styles from './LedgerPage.module.css'
export function MenubarPage({ token }: { token: string }) {
  const [rows,setRows] = useState<LedgerDetail[]>([])
  const [error,setError] = useState('')
  const [quitting,setQuitting] = useState(false)
  const refresh = useCallback(async () => {
    const response = await ledgerRequest<LedgerDetail[]>('/ledger?hours=24')
    if (!response.success) throw new Error(response.error ?? 'Ledger unavailable')
    const data = response.data ?? []; setRows(data)
    const needsYou = data.filter(d => d.state === 'needs_input' || d.offPlan.length || d.state === 'review').length
    const running = data.filter(d => d.state === 'running').length
    await setNativeCounts(needsYou,running)
    setError('')
  },[])
  useEffect(() => {
    const manager = getWebSocketManager(); manager.connect(token)
    const unsubscribe = manager.onMessage(m => { if (m.type.startsWith('ledger:') || m.type === 'sessions:update') void refresh().catch(e => setError(String(e))) })
    void refresh().catch(e => setError(String(e)))
    const timer = window.setInterval(() => { void refresh().catch(e => setError(String(e))) },30000)
    const quit = () => setQuitting(true); window.addEventListener('keepline:quit',quit)
    return () => { unsubscribe(); window.clearInterval(timer); window.removeEventListener('keepline:quit',quit) }
  },[token,refresh])
  const focus = async () => {
    const response = await ledgerRequest<LedgerConfig>('/settings/ledger')
    if (!response.success || !response.data) { setError(response.error ?? 'Settings unavailable'); return }
    const cfg = response.data
    const saved = await ledgerRequest('/settings/ledger',{ method: 'PUT',body: JSON.stringify({ ...cfg,focus: { ...cfg.focus,until: new Date(Date.now()+cfg.focus.minutes*60000).toISOString() } }) })
    if (!saved.success) setError(saved.error ?? 'Could not start focus')
  }
  const needs = rows.filter(d => d.state === 'needs_input' || d.offPlan.length || d.state === 'review')
  const running = rows.filter(d => d.state === 'running' && !needs.includes(d))
  return <section className={styles.page} style={{ padding: 16 }}><div className={styles.header}><h2>Keepline</h2><button onClick={() => void openNativeLedger().catch(e => setError(String(e)))}>Open</button></div>
    {error && <p className={styles.error}>{error}</p>}
    <h3>Needs you · {needs.length}</h3>{needs.slice(0,8).map(d => <button key={d.sessionId} onClick={() => void openNativeLedger(d.sessionId,d.offPlan.length ? 'off-plan' : d.state === 'needs_input' ? 'current-step' : 'requirements').catch(e => setError(String(e)))}>{d.title} · {d.state}</button>)}
    <h3>Running · {running.length}</h3>{running.slice(0,6).map(d => <button key={d.sessionId} onClick={() => void openNativeLedger(d.sessionId).catch(e => setError(String(e)))}>{d.title} · {d.progress.done}/{d.progress.total}</button>)}
    <div className={styles.actions}><button onClick={() => void focus().catch(e => setError(String(e)))}>Focus</button><button onClick={() => setQuitting(true)}>Quit</button></div>
    {quitting && <div className={styles.card}><strong>Stop monitoring when quitting?</strong><button onClick={() => void quitNativeApp(true).catch(e => setError(String(e)))}>Stop monitoring and quit</button><button onClick={() => void quitNativeApp(false).catch(e => setError(String(e)))}>Keep monitoring and quit</button><button onClick={() => setQuitting(false)}>Cancel</button></div>}
  </section>
}
