import { useCallback,useEffect,useState } from 'react'
import { ledgerRequest } from '@/services/api'
import { getWebSocketManager } from '@/services/websocket'
import { ledgerNeedsAttention } from '../../../../../domain/ledger/types'
import type { LedgerConfig,LedgerDetail } from '../../../../../domain/ledger/types'
import { openNativeLedger,quitNativeApp,setNativeCounts } from './native'
import styles from './LedgerPage.module.css'
export function MenubarPage({ token }: { token: string }) {
  const [rows,setRows] = useState<LedgerDetail[]>([])
  const [error,setError] = useState('')
  const [quitting,setQuitting] = useState(false)
  const refresh = useCallback(async () => {
    const response = await ledgerRequest<LedgerDetail[]>('/ledger?hours=24')
    if (!response.success) throw new Error(response.error ?? '暂时无法读取进度账')
    const data = response.data ?? []; setRows(data)
    const needsYou = data.filter(d => ledgerNeedsAttention(d)).length
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
  const needs = rows.filter(d => ledgerNeedsAttention(d))
  const running = rows.filter(d => d.state === 'running' && !needs.includes(d))
  return <section className={styles.page} style={{ padding: 16 }}><div className={styles.header}><h2>Keepline</h2><button onClick={() => void openNativeLedger().catch(e => setError(String(e)))}>打开</button></div>
    {error && <p className={styles.error}>{error}</p>}
    <h3>需要你 · {needs.length}</h3>{needs.slice(0,8).map(d => <button key={d.sessionId} onClick={() => void openNativeLedger(d.sessionId,d.offPlan.length ? 'off-plan' : d.state === 'needs_input' ? 'current-step' : 'requirements').catch(e => setError(String(e)))}>{d.title} · {d.state === 'needs_input' ? '等你审批' : d.state === 'review' ? '待核对' : '可能偏离'}</button>)}
    <h3>正在执行 · {running.length}</h3>{running.slice(0,6).map(d => <button key={d.sessionId} onClick={() => void openNativeLedger(d.sessionId).catch(e => setError(String(e)))}>{d.title} · {d.progress.total ? `${d.progress.done}/${d.progress.total}` : d.activity?.action ?? '记录执行活动'}</button>)}
    <div className={styles.actions}><button onClick={() => void focus().catch(e => setError(String(e)))}>专注</button><button onClick={() => setQuitting(true)}>退出</button></div>
    {quitting && <div className={styles.card}><strong>退出时停止监控吗？</strong><button onClick={() => void quitNativeApp(true).catch(e => setError(String(e)))}>停止监控并退出</button><button onClick={() => void quitNativeApp(false).catch(e => setError(String(e)))}>保持监控并退出</button><button onClick={() => setQuitting(false)}>取消</button></div>}
  </section>
}
