import { getWebSocketManager } from '@/services/websocket'
import { getAutostart,isNativeApp,setAutostart } from './native'
import { useCallback, useEffect, useRef, useState } from 'react'
import { ledgerRequest } from '@/services/api'
import type { WorkItem } from '@/types/work-item'
import type { LedgerConfig, LedgerDetail, RequirementItem } from '../../../../../domain/ledger/types'
import { DEFAULT_LEDGER_CONFIG } from '../../../../../domain/ledger/types'
import styles from './LedgerPage.module.css'

type View = 'overview' | 'goals' | 'review' | 'ledger-settings'
interface Todo extends WorkItem { readyToComplete: boolean; checklist: Array<{ id: string; text: string; satisfied: boolean }>; sessions: Array<{ runtime_session_id: string; title: string; status: string }> }
interface Goal extends WorkItem { todos: Todo[]; progress: { done: number; total: number; active: number }; weeklyMovement: number; stale: boolean }
interface Review { open: LedgerDetail[]; accepted: LedgerDetail[]; offPlan: Array<{ id: string; title: string }>; corrections: unknown[]; unattributedRuntimeShare: number; goals: Goal[] }
interface Props { view: View; onOpenSession: (id: string) => void }

async function api<T>(path: string, method = 'GET', data?: unknown): Promise<T> {
  const response = await ledgerRequest<T>(path,{ method,...(data === undefined ? {} : { body: JSON.stringify(data) }) })
  if (!response.success) throw new Error(response.error ?? 'Request failed')
  return response.data as T
}
function Progress({ done, total }: { done: number; total: number }) {
  return <div className={styles.actions}><div className={styles.progress} aria-label={`${done} of ${total} complete`}>{Array.from({ length: total },(_,i) => <span key={i} className={`${styles.segment} ${i < done ? styles.done : ''}`} />)}</div><span>{done}/{total}</span></div>
}
export function LedgerPage({ view, onOpenSession }: Props) {
  const [rows,setRows] = useState<LedgerDetail[]>([])
  const [goals,setGoals] = useState<Goal[]>([])
  const [todos,setTodos] = useState<WorkItem[]>([])
  const [settings,setSettings] = useState<LedgerConfig>(structuredClone(DEFAULT_LEDGER_CONFIG))
  const [detail,setDetail] = useState<LedgerDetail | null>(null)
  const [selectedId,setSelectedId] = useState<string | null>(() => new URLSearchParams(window.location.search).get('sessionId'))
  const [autostart,setAutostartState] = useState(false)
  const [error,setError] = useState('')
  const [busy,setBusy] = useState(false)
  const [search,setSearch] = useState('')
  const [group,setGroup] = useState('urgency')
  const [cursor,setCursor] = useState(0)
  const [review,setReview] = useState<Review | null>(null)
  const [date,setDate] = useState(new Date().toLocaleDateString('en-CA'))
  const [weekly,setWeekly] = useState(false)
  const [prompt,setPrompt] = useState<{ text: string; sessionId?: string; dispatch?: Todo; runtime?: 'codex' | 'claude-code' } | null>(null)
  const [editing,setEditing] = useState<{ id?: string; title: string; level: 'goal' | 'task'; parentId: string; outcome: string; checklist: string; projectRoot: string } | null>(null)
  const [itemEditor,setItemEditor] = useState<RequirementItem[] | null>(null)
  const [itemFilter,setItemFilter] = useState<string | null>(null)
  const [onboarding,setOnboarding] = useState(() => !localStorage.getItem('ledger-onboarded'))
  const hovering = useRef(false)
  const queued = useRef<LedgerDetail[] | null>(null)
  const searchRef = useRef<HTMLInputElement>(null)
  const settingsLoaded = useRef(false)
  const load = useCallback(async () => {
    const [ledger, goalData, work, cfg] = await Promise.all([api<LedgerDetail[]>('/ledger?hours=24'),api<Goal[]>('/goals'),api<{ items: WorkItem[] }>('/work-items'),api<LedgerConfig>('/settings/ledger')])
    if (hovering.current) queued.current = ledger; else setRows(ledger)
    setGoals(goalData); setTodos(work.items.filter(w => w.level !== 'goal' && w.kind === 'todo'));
    if (!settingsLoaded.current) { setSettings(cfg); settingsLoaded.current = true }
    setError('')
  },[])
  const perform = useCallback(async (action: () => Promise<unknown>) => {
    setError(''); setBusy(true)
    try { await action(); await load() } catch (e) { setError(e instanceof Error ? e.message : 'Action failed') } finally { setBusy(false) }
  },[load])
  useEffect(() => {
    void load().catch(e => setError(String(e)))
    const timer = window.setInterval(() => { void load().catch(e => setError(String(e))) },30000)
    return () => window.clearInterval(timer)
  },[load])
  useEffect(() => {
    if (!selectedId || view !== 'overview') { setDetail(null); return }
    let active = true
    const update = async () => {
      await api(`/ledger/${encodeURIComponent(selectedId)}/viewing`,'POST',{ viewed: true })
      const data = await api<LedgerDetail>(`/ledger/${encodeURIComponent(selectedId)}`)
      if (active) setDetail(data)
    }
    void update().catch(e => setError(String(e)))
    const timer = window.setInterval(() => { void update().catch(e => setError(String(e))) },15000)
    return () => { active = false; window.clearInterval(timer); void api(`/ledger/${encodeURIComponent(selectedId)}/viewing`,'POST',{ viewed: false }).catch(() => {}) }
  },[selectedId,rows,view])
  useEffect(() => {
    if (view !== 'review') return
    void api<Review>(`/ledger/review?${weekly ? 'week' : 'date'}=${date}`).then(setReview).catch(e => setError(String(e)))
  },[view,date,weekly,rows])
  const openPrompt = async (row: LedgerDetail, runId?: string) => {
    const result = await api<{ text: string }>(`/ledger/${encodeURIComponent(row.sessionId)}/${runId ? `correction?runId=${encodeURIComponent(runId)}` : 'follow-up'}`)
    setPrompt({ text: result.text,sessionId: row.sessionId })
  }
  useEffect(() => getWebSocketManager().onMessage(message => { if (message.type.startsWith('ledger:') || message.type === 'sessions:update') void load().catch(e => setError(String(e))) }),[load])
  useEffect(() => { if (isNativeApp()) void getAutostart().then(setAutostartState).catch(e => setError(String(e))) },[])
  useEffect(() => {
    const anchor = new URLSearchParams(window.location.search).get('anchor')
    if (detail && anchor) document.getElementById(`ledger-${anchor}`)?.scrollIntoView({ block: 'center' })
  },[detail?.sessionId])
  const groupKey = (row: LedgerDetail) => group === 'project' ? row.projectRoot : group === 'goal' ? (goals.find(g => g.id === todos.find(t => t.id === row.workItemId)?.parentId)?.title ?? 'Unattributed') : row.state === 'needs_input' || row.offPlan.length || row.state === 'review' ? 'Needs you' : row.state === 'accepted' ? 'Accepted today' : 'Running'
  const filtered = rows.filter(r => `${r.title} ${r.projectRoot} ${r.asks.map(a => a.text).join(' ')}`.toLowerCase().includes(search.toLowerCase())).sort((a,b) => group === 'urgency' ? 0 : groupKey(a).localeCompare(groupKey(b)))
  useEffect(() => {
    if (view !== 'overview' || prompt || editing || itemEditor) return
    const handler = (event: KeyboardEvent) => {
      if ((event.target as HTMLElement).closest('input,textarea,select')) return
      const row = filtered[cursor]
      if (event.key === '/') { event.preventDefault(); searchRef.current?.focus() }
      if (event.key.toLowerCase() === 'j') setCursor(c => Math.min(c+1,filtered.length-1))
      if (event.key.toLowerCase() === 'k') setCursor(c => Math.max(c-1,0))
      if (event.key === 'Enter' && row) setSelectedId(row.sessionId)
      if (event.key.toLowerCase() === 'o' && row) onOpenSession(row.sessionId)
      if (event.key.toLowerCase() === 'c' && row) void perform(() => openPrompt(row))
      if (event.key.toLowerCase() === 'a' && row && row.state === 'review') void perform(() => api(`/ledger/${encodeURIComponent(row.sessionId)}/acceptances`,'POST',{ decision: 'accepted' }))
    }
    document.addEventListener('keydown',handler); return () => document.removeEventListener('keydown',handler)
  },[view,prompt,editing,itemEditor,filtered,cursor,onOpenSession,perform])
  const title = { overview: 'Progress ledger',goals: 'Goals',review: 'Review','ledger-settings': 'Ledger settings' }[view]
  const itemPath = (row: LedgerDetail) => {
    const todo = todos.find(t => t.id === row.workItemId); const goal = goals.find(g => g.id === todo?.parentId)
    return todo ? `${goal ? `${goal.title} › ` : ''}${todo.title}` : 'Unattributed'
  }
  const mutateDetail = async (path: string, data: unknown, method = 'POST') => {
    if (!detail) return
    const result = await api<LedgerDetail>(`/ledger/${encodeURIComponent(detail.sessionId)}/${path}`,method,data)
    setDetail(result)
  }
  const saveWorkItem = async () => {
    if (!editing) return
    const old = [...goals,...todos].find(i => i.id === editing.id)
    const texts = editing.checklist.split('\n').map(t => t.trim()).filter(Boolean)
    const acceptance = texts.map((text,i) => ({ id: old?.acceptance?.[i]?.id ?? crypto.randomUUID(),text,completed: old?.acceptance?.[i]?.completed ?? false }))
    await api(`/work-items${editing.id ? `/${editing.id}` : ''}`,editing.id ? 'PATCH' : 'POST',{ title: editing.title,level: editing.level,parentId: editing.level === 'goal' ? null : editing.parentId || null,outcome: editing.outcome,acceptance,projectRoot: editing.projectRoot || null,kind: 'todo' })
    setEditing(null)
  }
  const editWorkItem = (item?: WorkItem, parentId = '') => setEditing({ id: item?.id,title: item?.title ?? '',level: item?.level ?? (parentId ? 'task' : 'goal'),parentId: item?.parentId ?? parentId,outcome: item?.outcome ?? '',checklist: item?.acceptance?.map(c => c.text).join('\n') ?? '',projectRoot: item?.projectRoot ?? goals.find(g => g.id === parentId)?.projectRoot ?? '' })
  const saveSettings = async () => { const saved = await api<LedgerConfig>('/settings/ledger','PUT',{ ...settings,exclude: { ...settings.exclude,projects: settings.exclude.projects.map(path => path.trim()).filter(Boolean) } }); setSettings(saved) }
  return <section className={styles.page} id={`panel-${view}`} aria-labelledby={`tab-${view}`}>
    <div className={styles.header}><div><h2>{title}</h2><p className={styles.subtle}>Evidence from tools · Acceptance by you</p></div><div className={styles.actions}>
      <button disabled={busy} onClick={() => void perform(load)}>Refresh</button>
      <button onClick={() => void perform(async () => { setSettings(await api<LedgerConfig>('/settings/ledger','PUT',{ ...settings,focus: { ...settings.focus,until: new Date(Date.now()+settings.focus.minutes*60000).toISOString() } })) })}>Focus {settings.focus.minutes} min</button>
      {settings.focus.until && <button onClick={() => void perform(async () => { setSettings(await api<LedgerConfig>('/settings/ledger','PUT',{ ...settings,focus: { ...settings.focus,until: new Date().toISOString() } })) })}>End focus</button>}
    </div></div>
    {error && <div role="alert" className={styles.error}>{error}</div>}
    {onboarding && <div className={styles.card}><h3>{rows.length} recent sessions discovered</h3><p>Optional enhanced detection: install hooks to detect permission requests. AI requirement recognition sends authored user text to your selected backend only when enabled.</p><div className={styles.actions}><button onClick={() => void perform(() => navigator.clipboard.writeText('keepline hooks install'))}>Copy hook install command</button><button onClick={() => { setOnboarding(false); localStorage.setItem('ledger-onboarded','1') }}>Skip setup</button></div></div>}
    {view === 'overview' && !detail && <>
      <div className={styles.toolbar}><input ref={searchRef} aria-label="Search ledger" placeholder="Search asks or projects…" value={search} onChange={e => setSearch(e.target.value)} /><select aria-label="Group by" value={group} onChange={e => setGroup(e.target.value)}><option value="urgency">Urgency</option><option value="goal">Goal</option><option value="project">Project</option></select><span className={styles.subtle}>J/K navigate · Enter inspect · O open agent · A accept · C follow-up</span></div>
      {!filtered.length && <div className={styles.card}>No sessions in this window. Scanned: ~/.claude/projects and ~/.codex/sessions. Check exclusions and retention in Ledger settings.</div>}
      <div className={styles.rows} onPointerEnter={() => { hovering.current = true }} onPointerLeave={() => { hovering.current = false; if (queued.current) { setRows(queued.current); queued.current = null } }}>
        {filtered.map((row,i) => <div key={row.sessionId} className={`${styles.row} ${cursor === i ? styles.selected : ''}`}><button onClick={() => { setSelectedId(row.sessionId); setItemFilter(null) }}>{(i === 0 || groupKey(filtered[i-1]) !== groupKey(row)) && <h3>{groupKey(row)}</h3>}<div className={styles.rowTitle}><strong>{row.title}</strong><span className={`${styles.badge} ${row.state === 'needs_input' || row.offPlan.length ? styles.attention : ''}`}>{row.state}</span>{row.possiblyWaiting && <span className={`${styles.badge} ${styles.possible}`}>Possibly waiting</span>}{row.offPlan.length > 0 && <span className={styles.badge}>Off-plan</span>}</div><p className={styles.subtle}>{group === 'project' ? row.projectRoot : group === 'goal' ? itemPath(row) : row.state === 'needs_input' || row.offPlan.length || row.state === 'review' ? 'Needs you' : row.state === 'accepted' ? 'Accepted today' : 'Running'} · {itemPath(row)} · {row.runtimeId}</p></button><Progress {...row.progress} /></div>)}
      </div>
    </>}
    {view === 'overview' && detail && <>
      <div className={styles.toolbar}><button onClick={() => setSelectedId(null)}>← Overview</button><h3>{detail.title}</h3><span className={styles.badge}>{detail.state}</span><button onClick={() => onOpenSession(detail.sessionId)}>Open agent</button><Progress {...detail.progress} /></div>{detail.statusReason && <p className={styles.subtle}>{detail.statusReason}</p>}
      {!settings.judge.enabled && detail.items.filter(item => item.source === 'fallback' && !item.dropped).length > 8 && <div className={styles.card}>候选要求较多：可在 Ledger settings 开启 AI 识别，或通过 Edit items 删除不算的项。</div>}
      {!detail.available && <div className={styles.card}>{detail.unavailableReason}. View session status or inspect the original transcript.</div>}
      {detail.attribution && !detail.workItemId && <div className={styles.card}><h3>Attribute this session</h3>{detail.attribution.map(s => <button key={s.workItemId} onClick={() => void perform(() => mutateDetail('attribution',{ workItemId: s.workItemId }))}>{s.title} — {s.reasons.join('; ')}</button>)}<select aria-label="Choose todo" value="" onChange={e => e.target.value && void perform(() => mutateDetail('attribution',{ workItemId: e.target.value }))}><option value="">Choose another todo</option>{todos.map(t => <option key={t.id} value={t.id}>{t.title}</option>)}</select><button onClick={() => void perform(() => mutateDetail('attribution',{ workItemId: null }))}>No goal</button></div>}
      {detail.workItemId && <div className={styles.toolbar}>{itemPath(detail)}<button onClick={() => void perform(() => mutateDetail('attribution',{ workItemId: null }))}>Clear attribution</button></div>}
      <div className={styles.card}><h3>Your words, verbatim</h3><p className={styles.subtle}>Candidate requirements are kept by default. Edit items to remove any that do not belong or add missing requirements. Questions stay here and do not count toward progress.</p>{detail.asks.map(ask => <div key={ask.id}><span className={styles.subtle}>{ask.kind === 'question' ? '问题 · 不计进度' : `候选要求 · ${ask.kind}`} · {new Date(ask.at).toLocaleString()}</span><pre className={styles.ask}>{ask.text}</pre></div>)}</div>
      {!!detail.importSuggestions?.length && <div className={styles.card}><h3>Clarify this ask</h3><p>The ask has little detail. You can import requirements from a previous session in this project, or link a todo below.</p>{detail.importSuggestions.map(previous => <button key={previous.sessionId} disabled={busy} onClick={() => void perform(() => mutateDetail('import-requirements',{ fromSessionId: previous.sessionId }))}>Import requirements: {previous.title}</button>)}</div>}
      {!!detail.followUpSuggestions?.length && <div className={styles.card}><h3>Suggested follow-up work</h3><p className={styles.subtle}>Agent proposals. Review and save a todo to add one.</p>{detail.followUpSuggestions.map(text => <div key={text} className={styles.item}><span>{text}</span><button onClick={() => setEditing({ title: text,level: 'task',parentId: todos.find(t => t.id === detail.workItemId)?.parentId ?? '',outcome: '',checklist: '',projectRoot: detail.projectRoot })}>Review new todo</button></div>)}</div>}
      <div className={styles.split}><div id="ledger-requirements" className={styles.card}><div className={styles.header}><h3>Requirements</h3><div className={styles.actions}><button onClick={() => setItemEditor(structuredClone(detail.items))}>Edit items</button>{settings.judge.enabled && <button disabled={busy} onClick={() => void perform(() => mutateDetail('redecompose',{}))}>Recognize requirements again</button>}</div></div>{detail.items.map(item => <button className={`${styles.item} ${itemFilter === item.id ? styles.selected : ''}`} key={item.id} onClick={() => setItemFilter(f => f === item.id ? null : item.id)}><strong>{item.title}</strong><span className={styles.subtle}>{item.dropped ? 'Dropped by you' : item.status} · {item.source} · {item.statusSource} · {item.evidenceIds.length} evidence</span>{item.constraints.map((c,i) => <span key={i} className={styles.subtle}>{c.kind}: {c.value}</span>)}</button>)}</div>
      <div id={detail.trail.length ? undefined : "ledger-current-step"} className={styles.card}><h3>Trail {itemFilter && '(filtered)'}</h3>{detail.turns.map(turn => <details key={turn.id} open><summary>{turn.phase} · {new Date(turn.at).toLocaleTimeString()} · {turn.readOnlyCount} read/wait calls collapsed</summary>{detail.trail.filter(s => s.turnId === turn.id && (!itemFilter || s.itemId === itemFilter)).map(step => <div id={step.callId === detail.trail.at(-1)?.callId ? "ledger-current-step" : undefined} className={styles.item} key={step.callId}><pre className={styles.step}>{step.summary}</pre><span className={styles.subtle}>{step.itemId ? detail.items.find(i => i.id === step.itemId)?.title : step.acceptedOffPlan ? 'Accepted deviation' : 'Off-plan'} · {step.violations.join('; ')}</span><details><summary>Evidence ({step.evidenceIds.length})</summary>{detail.evidence.filter(e => step.evidenceIds.includes(e.id)).map(e => <pre className={styles.output} key={e.id}>{e.kind}: {e.value} {e.exitCode !== undefined ? `· exit ${e.exitCode}` : ''}</pre>)}</details><select aria-label="Assign step" value={step.itemId ?? ''} onChange={e => void perform(() => mutateDetail('corrections',{ callId: step.callId,itemId: e.target.value || undefined }))}><option value="">Unmatched</option>{detail.items.map(i => <option key={i.id} value={i.id}>{i.title}</option>)}</select><button disabled={!step.itemId} onClick={() => { const paths = window.prompt('Path/glob for similar steps (e.g. src/ledger/**)'); if (paths?.trim() && step.itemId) void perform(() => mutateDetail('corrections',{ callId: step.callId,itemId: step.itemId,rule: { itemId: step.itemId,matcher: { paths: [paths.trim()],commands: [] } } })) }}>Apply to similar steps</button></div>)}</details>)}</div></div>
      {detail.offPlan.map((run,index) => <div id={index === 0 ? "ledger-off-plan" : undefined} className={styles.card} key={run.id}><strong>Off-plan run · {run.callIds.length} steps</strong><div className={styles.actions}><button onClick={() => void perform(() => mutateDetail('corrections',{ callIds: run.callIds,acceptOffPlan: true }))}>Reasonable, stop alerting</button><button onClick={() => void perform(() => openPrompt(detail,run.id))}>Generate correction</button></div></div>)}
      <div className={styles.card}><h3>Claims vs evidence</h3><table className={styles.table}><thead><tr><th>Agent claim</th><th>Tool evidence</th></tr></thead><tbody>{detail.claims.map((claim,i) => <tr key={i}><td>{claim.text}</td><td>{claim.evidenceIds.length ? detail.evidence.filter(e => claim.evidenceIds.includes(e.id)).map(e => <div key={e.id}>{e.kind}: {e.value}</div>) : <button onClick={() => void perform(() => openPrompt(detail))}>No evidence found — request evidence</button>}</td></tr>)}</tbody></table></div>
      {detail.state === 'review' && <div className={styles.card}><h3>Review this turn</h3><div className={styles.actions}><button disabled={detail.progress.done !== detail.progress.total || busy} onClick={() => void perform(() => mutateDetail('acceptances',{ decision: 'accepted' }))}>Accept</button><button onClick={() => { const reason = window.prompt('Why are the remaining items dropped?'); if (reason?.trim()) void perform(() => mutateDetail('acceptances',{ decision: 'accepted_with_gaps',reason,droppedItemIds: detail.items.filter(i => i.status !== 'done' || !i.evidenceIds.length).map(i => i.id) })) }}>Accept with gaps</button><button onClick={() => void perform(async () => { await mutateDetail('acceptances',{ decision: 'follow_up' }); await openPrompt(detail) })}>Generate follow-up</button></div></div>}
      {detail.acceptances.length > 0 && <div className={styles.card}>{detail.acceptances.map(a => <div key={a.turnId}>{a.decision} · {a.reason} · {new Date(a.at).toLocaleString()}</div>)}</div>}
    </>}
    {view === 'goals' && <><button onClick={() => editWorkItem()}>+ New goal</button>{goals.map(goal => <div className={styles.card} key={goal.id}><div className={styles.header}><h3>{goal.title} {goal.stale && <span className={styles.attention}>· No movement</span>}</h3><Progress {...goal.progress} /></div><p>{goal.outcome}</p><span className={styles.subtle}>{goal.progress.active} active todos · {goal.weeklyMovement} completed this week</span><div className={styles.actions}><button onClick={() => editWorkItem(goal)}>Edit goal</button><button onClick={() => editWorkItem(undefined,goal.id)}>+ Todo</button></div>{goal.todos.map(todo => <div className={styles.item} key={todo.id}><strong>{todo.title}</strong><span className={styles.subtle}>{todo.status}</span>{todo.checklist.map(c => <div key={c.id}>{c.satisfied ? '✓' : '○'} {c.text}</div>)}{todo.sessions.map(s => <button key={s.runtime_session_id} onClick={() => onOpenSession(s.runtime_session_id)}>{s.title} · {s.status}</button>)}<div className={styles.actions}><button onClick={() => editWorkItem(todo)}>Edit todo</button>{(['codex','claude-code'] as const).map(runtime => <button key={runtime} onClick={() => setPrompt({ dispatch: todo,runtime,text: `${todo.title}\n${todo.body ?? ''}\n\nAcceptance criteria:\n${todo.acceptance?.map(c => `- ${c.text}`).join('\n') ?? ''}` })}>Hand to {runtime}</button>)}{todo.readyToComplete && <button onClick={() => void perform(() => api(`/goals/todos/${todo.id}/complete`,'POST'))}>Checklist verified — mark done</button>}</div></div>)}</div>)}</>}
    {view === 'review' && <><div className={styles.toolbar}><input aria-label="Review date" type="date" value={date} onChange={e => setDate(e.target.value)} /><label><input type="checkbox" checked={weekly} onChange={e => setWeekly(e.target.checked)} /> Week from this date</label></div>{review && <><div className={styles.card}><h3>Still open</h3>{review.open.map(row => <div className={styles.item} key={row.sessionId}><strong>{row.title}</strong><p>{row.items.filter(i => i.status !== 'done').map(i => i.title).join(' · ')}</p><div className={styles.actions}><button onClick={() => void perform(() => openPrompt(row))}>Follow-up</button><button onClick={() => void perform(() => api(`/ledger/${encodeURIComponent(row.sessionId)}/carry-over`,'POST'))}>Carry to tomorrow</button></div></div>)}</div><div className={styles.card}><h3>Accepted work</h3>{review.accepted.map(row => <div key={row.sessionId}>{row.title}<p className={styles.subtle}>{row.evidence.filter(e => e.exitCode === 0).map(e => e.value).slice(0,3).join(' · ')}</p></div>)}</div><div className={styles.card}><h3>Deviations & corrections</h3><p>{review.offPlan.length} off-plan runs · {review.corrections.length} corrections</p>{review.offPlan.map(run => <div key={run.id}>{run.title}</div>)}</div>{weekly && <div className={styles.card}><h3>Weekly movement</h3><p>Unattributed runtime: {(review.unattributedRuntimeShare*100).toFixed(1)}%</p>{review.goals.map(g => <div key={g.id}>{g.title}: {g.weeklyMovement} completed todos</div>)}</div>}</>}</>}
    {view === 'ledger-settings' && <div className={`${styles.card} ${styles.form}`}>
      {isNativeApp() && <label className={styles.toggle}><input type="checkbox" checked={autostart} onChange={e => { const enabled = e.target.checked; void perform(async () => { await setAutostart(enabled); setAutostartState(enabled) }) }} />Start Keepline at login</label>}
      {(['enabled','constraints','nativeNotifications'] as const).map(key => <label className={styles.toggle} key={key}><input type="checkbox" checked={settings[key]} onChange={e => setSettings(s => ({ ...s,[key]: e.target.checked }))} />{key}</label>)}
      <label className={styles.toggle}><input type="checkbox" checked={settings.judge.enabled} onChange={e => setSettings(s => ({ ...s,judge: { ...s.judge,enabled: e.target.checked } }))} />AI requirement recognition and decomposition (off by default)</label><p className={styles.subtle}>One call per new candidate user message. Tools, agent reports and repeated scans do not trigger calls. Completion remains based on execution evidence. Explicit re-decomposition can retry.</p>
      <label>Backend<select value={settings.judge.backend} onChange={e => setSettings(s => ({ ...s,judge: { ...s.judge,backend: e.target.value as LedgerConfig['judge']['backend'] } }))}>{['cli-claude','cli-codex','local','sdk'].map(b => <option key={b}>{b}</option>)}</select></label>
      <label>Model<input value={settings.judge.model ?? ''} onChange={e => setSettings(s => ({ ...s,judge: { ...s.judge,model: e.target.value || null } }))} /></label>
      <label>Deviation<select value={settings.deviation} onChange={e => setSettings(s => ({ ...s,deviation: e.target.value as LedgerConfig['deviation'] }))}>{['off','conservative','sensitive'].map(b => <option key={b}>{b}</option>)}</select></label>
      {Object.keys(settings.alerts).map(key => <label className={styles.toggle} key={key}><input type="checkbox" checked={settings.alerts[key as keyof LedgerConfig['alerts']]} onChange={e => setSettings(s => ({ ...s,alerts: { ...s.alerts,[key]: e.target.checked } }))} />Alert: {key}</label>)}
      {(['retentionDays','staleGoalDays','stalledAfterSeconds','alertCoalesceSeconds'] as const).map(key => <label key={key}>{key}<input type="number" min="1" value={settings[key]} onChange={e => setSettings(s => ({ ...s,[key]: Number(e.target.value) }))} /></label>)}
      <label>Focus minutes<input type="number" min="1" value={settings.focus.minutes} onChange={e => setSettings(s => ({ ...s,focus: { ...s.focus,minutes: Number(e.target.value) } }))} /></label>
      <label>Excluded projects (one path per line)<textarea value={settings.exclude.projects.join('\n')} onChange={e => setSettings(s => ({ ...s,exclude: { ...s.exclude,projects: e.target.value.split('\n') } }))} /></label>
      <label>Excluded runtimes<select multiple value={settings.exclude.runtimes} onChange={e => setSettings(s => ({ ...s,exclude: { ...s.exclude,runtimes: Array.from(e.target.selectedOptions,o => o.value) } }))}><option value="codex">Codex</option><option value="claude-code">Claude Code</option></select></label>
      <button disabled={busy} onClick={() => void perform(saveSettings)}>Save settings</button>
    </div>}
    {prompt && <div className={styles.dialog} role="dialog" aria-modal="true" aria-label={prompt.dispatch ? 'Preview dispatch' : 'Edit follow-up'}><div className={styles.card}><h3>{prompt.dispatch ? `Hand to ${prompt.runtime}` : 'Copy prompt'}</h3><textarea rows={14} value={prompt.text} onChange={e => setPrompt({ ...prompt,text: e.target.value })} /><div className={styles.actions}><button onClick={() => void perform(() => navigator.clipboard.writeText(prompt.text))}>Copy</button>{prompt.sessionId && <button onClick={() => void perform(async () => { await navigator.clipboard.writeText(prompt.text); onOpenSession(prompt.sessionId!); setPrompt(null) })}>Copy and jump to agent</button>}{prompt.dispatch && <button disabled={busy} onClick={() => void perform(async () => { await api(`/goals/todos/${prompt.dispatch!.id}/dispatch`,'POST',{ runtimeId: prompt.runtime,cwd: prompt.dispatch!.projectRoot,prompt: prompt.text,idempotencyKey: crypto.randomUUID() }); setPrompt(null) })}>Launch agent</button>}<button onClick={() => setPrompt(null)}>Close</button></div></div></div>}
    {editing && <div className={styles.dialog} role="dialog" aria-modal="true" aria-label="Edit goal or todo"><div className={`${styles.card} ${styles.form}`}><h3>{editing.id ? 'Edit' : 'New'} {editing.level === 'goal' ? 'goal' : 'todo'}</h3><label>Title<input value={editing.title} onChange={e => setEditing({ ...editing,title: e.target.value })} /></label><label>Project directory<input value={editing.projectRoot} onChange={e => setEditing({ ...editing,projectRoot: e.target.value })} /></label>{editing.level === 'goal' ? <label>Success outcome<textarea value={editing.outcome} onChange={e => setEditing({ ...editing,outcome: e.target.value })} /></label> : <><label>Goal<select value={editing.parentId} onChange={e => setEditing({ ...editing,parentId: e.target.value })}><option value="">No goal</option>{goals.map(g => <option value={g.id} key={g.id}>{g.title}</option>)}</select></label><label>Acceptance checklist (one per line)<textarea rows={8} value={editing.checklist} onChange={e => setEditing({ ...editing,checklist: e.target.value })} /></label></>}<div className={styles.actions}><button disabled={!editing.title.trim() || busy} onClick={() => void perform(saveWorkItem)}>Save</button><button onClick={() => setEditing(null)}>Cancel</button></div></div></div>}
    {itemEditor !== null && <div className={styles.dialog} role="dialog" aria-modal="true" aria-label="Edit requirement items"><div className={`${styles.card} ${styles.form}`}><h3>Edit requirement items</h3><p className={styles.subtle}>Use paths, commands and keywords to match work to each requirement. Removed requirements stay removed.</p>{itemEditor.map((item,index) => {
      const update = (patch: Partial<RequirementItem>) => setItemEditor(items => items!.map((value,i) => i === index ? { ...value,...patch } : value))
      const lines = (text: string) => text.split('\n')
      return <fieldset className={styles.item} key={item.id}><legend>Requirement {index+1}</legend><label>Requirement title<input value={item.title} onChange={e => update({ title: e.target.value })} /></label><label>Status<select value={item.status} onChange={e => update({ status: e.target.value as RequirementItem['status'] })}>{['todo','doing','done','unverified'].map(status => <option key={status}>{status}</option>)}</select></label>{(['paths','commands','keywords'] as const).map(kind => <label key={kind}>{kind === 'paths' ? 'File paths or globs' : kind === 'commands' ? 'Command patterns' : 'Keywords'} (one per line)<textarea rows={2} value={item.anchors[kind].join('\n')} onChange={e => update({ anchors: { ...item.anchors,[kind]: lines(e.target.value) } })} /></label>)}<label className={styles.toggle}><input type="checkbox" checked={item.constraints.some(c => c.kind === 'no_public_api_change')} onChange={e => update({ constraints: [...item.constraints.filter(c => c.kind !== 'no_public_api_change'),...(e.target.checked ? [{ kind: 'no_public_api_change' as const }] : [])] })} />Keep public APIs unchanged</label><label>Forbidden paths (one per line)<textarea rows={2} value={item.constraints.filter(c => c.kind === 'path_forbidden').map(c => c.value).join('\n')} onChange={e => update({ constraints: [...item.constraints.filter(c => c.kind !== 'path_forbidden'),...lines(e.target.value).map(value => ({ kind: 'path_forbidden' as const,value }))] })} /></label><label>Preserved tags or text (one per line)<textarea rows={2} value={item.constraints.filter(c => c.kind === 'preserve_text').map(c => c.value).join('\n')} onChange={e => update({ constraints: [...item.constraints.filter(c => c.kind !== 'preserve_text'),...lines(e.target.value).map(value => ({ kind: 'preserve_text' as const,value }))] })} /></label><button onClick={() => setItemEditor(items => items!.filter((_,i) => i !== index))}>Remove requirement</button></fieldset>
    })}<button onClick={() => setItemEditor(items => [...items!,{ id: crypto.randomUUID(),ordinal: items!.length,title: '',anchors: { paths: [],commands: [],keywords: [] },constraints: [],source: 'user',status: 'todo',statusSource: 'user',evidenceIds: [] }])}>Add requirement</button><div className={styles.actions}><button disabled={busy || itemEditor.some(item => !item.title.trim())} onClick={() => void perform(async () => { await mutateDetail('items',{ items: itemEditor.map(item => ({ ...item,id: detail?.items.some(existing => existing.id === item.id) ? item.id : undefined,anchors: Object.fromEntries(Object.entries(item.anchors).map(([key,values]) => [key,values.map((value: string) => value.trim()).filter(Boolean)])),constraints: item.constraints.filter(c => c.kind !== 'path_forbidden' || c.value?.trim()).map(c => ({ ...c,...(c.value !== undefined ? { value: c.value.trim() } : {}) })) })) },'PUT'); setItemEditor(null) })}>Save</button><button onClick={() => setItemEditor(null)}>Cancel</button></div></div></div>}
  </section>
}
