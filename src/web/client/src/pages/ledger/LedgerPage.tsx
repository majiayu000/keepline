import { getWebSocketManager } from '@/services/websocket'
import { getAutostart,isNativeApp,setAutostart } from './native'
import { useCallback, useEffect, useRef, useState } from 'react'
import { ledgerRequest } from '@/services/api'
import type { WorkItem } from '@/types/work-item'
import type { LedgerConfig, LedgerDetail, RequirementItem } from '../../../../../domain/ledger/types'
import { DEFAULT_LEDGER_CONFIG,ledgerNeedsAttention,confirmedRequirement } from '../../../../../domain/ledger/types'
import styles from './LedgerPage.module.css'

type View = 'overview' | 'goals' | 'review' | 'ledger-settings'
interface Todo extends WorkItem { readyToComplete: boolean; checklist: Array<{ id: string; text: string; satisfied: boolean }>; sessions: Array<{ runtime_session_id: string; title: string; status: string }> }
interface Goal extends WorkItem { todos: Todo[]; progress: { done: number; total: number; active: number }; weeklyMovement: number; stale: boolean }
interface Review { open: LedgerDetail[]; accepted: LedgerDetail[]; offPlan: Array<{ id: string; title: string }>; corrections: unknown[]; unattributedRuntimeShare: number; goals: Goal[] }
interface Props { view: View; onOpenSession: (id: string) => void }

async function api<T>(path: string, method = 'GET', data?: unknown): Promise<T> {
  const response = await ledgerRequest<T>(path,{ method,...(data === undefined ? {} : { body: JSON.stringify(data) }) })
  if (!response.success) throw new Error(response.error ?? '请求失败')
  return response.data as T
}
const stateText = { running: '运行中',needs_input: '等你审批',review: '待核对',accepted: '已验收',stopped: '已停下',ended: '已结束' };
const sourceText = { fallback: '候选',work_item: '验收清单',model: 'AI 识别',user: '你确认的' };
const itemStateText = { todo: '待做',doing: '进行中',done: '有证据的完成',unverified: '未验证' };
const alertText = { needs_input: '等你审批',off_plan: '偏离已确认要求',claimed_unverified: '完成说法缺少证据',stalled: '会话停下',limited: '额度或预算用尽' };
function Activity({ row }: { row: Pick<LedgerDetail,'activity'> }) {
  return <div className={styles.activity}>{row.activity?.action && <p>最近动作：{row.activity.action}</p>}{row.activity?.evidence.map(e => <p key={e.id}>最近证据：{e.kind === 'test' ? '测试' : e.kind === 'file' ? '文件修改' : e.kind === 'pr' ? 'PR 链接' : e.kind === 'commit' ? '提交' : '执行'} · {e.value}{e.exitCode !== undefined && ` · ${e.exitCode === 0 ? '成功' : '失败'}`}</p>)}{row.activity?.lastMessage && <p>最后汇报（仅记录）：{row.activity.lastMessage}</p>}{!row.activity?.action && !row.activity?.lastMessage && <p>暂无可读取的执行活动</p>}</div>
}
function Progress({ done, total }: { done: number; total: number }) {
  if (!total) return null
  return <div className={styles.actions}><div className={styles.progress} aria-label={`已完成 ${done} / ${total} 项`}>{Array.from({ length: total },(_,i) => <span key={i} className={`${styles.segment} ${i < done ? styles.done : ''}`} />)}</div><span>{done}/{total}</span></div>
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
    try { await action(); await load() } catch (e) { setError(e instanceof Error ? e.message : '操作失败') } finally { setBusy(false) }
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
  const groupKey = (row: LedgerDetail) => group === 'project' ? row.projectRoot : group === 'goal' ? (goals.find(g => g.id === todos.find(t => t.id === row.workItemId)?.parentId)?.title ?? '未关联目标') : ledgerNeedsAttention(row) ? '需要你' : row.state === 'running' ? '正在执行' : '已结束'
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
  const title = { overview: '进度账',goals: '目标',review: '回顾','ledger-settings': '进度账设置' }[view]
  const itemPath = (row: LedgerDetail) => {
    const todo = todos.find(t => t.id === row.workItemId); const goal = goals.find(g => g.id === todo?.parentId)
    return todo ? `${goal ? `${goal.title} › ` : ''}${todo.title}` : '未关联目标'
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
    <div className={styles.header}><div><h2>{title}</h2><p className={styles.subtle}>实际执行记录 · 由你确认要求和验收</p></div><div className={styles.actions}>
      <button disabled={busy} onClick={() => void perform(load)}>刷新</button>
      <button onClick={() => void perform(async () => { setSettings(await api<LedgerConfig>('/settings/ledger','PUT',{ ...settings,focus: { ...settings.focus,until: new Date(Date.now()+settings.focus.minutes*60000).toISOString() } })) })}>专注 {settings.focus.minutes} 分钟</button>
      {settings.focus.until && <button onClick={() => void perform(async () => { setSettings(await api<LedgerConfig>('/settings/ledger','PUT',{ ...settings,focus: { ...settings.focus,until: new Date().toISOString() } })) })}>结束专注</button>}
    </div></div>
    {error && <div role="alert" className={styles.error}>{error}</div>}
    {onboarding && <div className={styles.card}><h3>{rows.length} 个最近会话</h3><p>可选：安装 hook 以识别审批请求。只有开启 AI 识别时，才会把用户原话发送给所选识别方式。</p><div className={styles.actions}><button onClick={() => void perform(() => navigator.clipboard.writeText('keepline hooks install'))}>复制 hook 安装命令</button><button onClick={() => { setOnboarding(false); localStorage.setItem('ledger-onboarded','1') }}>跳过设置</button></div></div>}
    {view === 'overview' && !detail && <>
      <div className={styles.toolbar}><input ref={searchRef} aria-label="搜索进度账" placeholder="搜索原话或项目…" value={search} onChange={e => setSearch(e.target.value)} /><select aria-label="分组方式" value={group} onChange={e => setGroup(e.target.value)}><option value="urgency">需要你优先</option><option value="goal">目标</option><option value="project">项目</option></select><span className={styles.subtle}>J/K 切换 · 回车查看 · O 打开 agent · A 验收 · C 后续提示</span></div>
      {!filtered.length && <div className={styles.card}>这个时间段暂无会话。记录来源：~/.claude/projects 与 ~/.codex/sessions；可在进度账设置检查排除项与保留时间。</div>}
      <div className={styles.rows} onPointerEnter={() => { hovering.current = true }} onPointerLeave={() => { hovering.current = false; if (queued.current) { setRows(queued.current); queued.current = null } }}>
        {filtered.map((row,i) => <div key={row.sessionId} className={`${styles.row} ${cursor === i ? styles.selected : ''}`}><button onClick={() => { setSelectedId(row.sessionId); setItemFilter(null) }}>{(i === 0 || groupKey(filtered[i-1]) !== groupKey(row)) && <h3>{groupKey(row)}</h3>}<div className={styles.rowTitle}><strong>{row.title}</strong><span className={`${styles.badge} ${row.state === 'needs_input' || row.offPlan.length ? styles.attention : ''}`}>{stateText[row.state]}</span>{row.possiblyWaiting && <span className={`${styles.badge} ${styles.possible}`}>可能在等待</span>}{row.offPlan.length > 0 && <span className={styles.badge}>偏离要求</span>}</div><p className={styles.subtle}>{group === 'project' ? row.projectRoot : group === 'goal' ? itemPath(row) : ledgerNeedsAttention(row) ? '需要你' : row.state === 'running' ? '正在执行' : '已结束'} · {itemPath(row)} · {row.runtimeId}</p><Activity row={row} /></button><Progress {...row.progress} /></div>)}
      </div>
    </>}
    {view === 'overview' && detail && <>
      <div className={styles.toolbar}><button onClick={() => setSelectedId(null)}>← 总览</button><h3>{detail.title}</h3><span className={styles.badge}>{stateText[detail.state]}</span><button onClick={() => onOpenSession(detail.sessionId)}>打开 agent</button><Progress {...detail.progress} /></div>{detail.statusReason && <p className={styles.subtle}>{detail.statusReason}</p>}
      {!settings.judge.enabled && detail.items.filter(item => item.source === 'fallback' && !item.dropped).length > 8 && <div className={styles.card}>候选要求较多：可在进度账设置开启 AI 识别，或通过编辑要求删除不算的项。</div>}
      <div className={styles.card}><h3>实际执行活动</h3><Activity row={detail} />{!detail.progress.total && <p className={styles.subtle}>尚无已确认要求，当前仅记录活动，不判断进度或偏离。</p>}</div>
      {detail.parentSessionId && <button onClick={() => setSelectedId(detail.parentSessionId!)}>返回父会话</button>}
      {!detail.available && <div className={styles.card}>{detail.unavailableReason}. 可查看会话状态或原始记录。</div>}
      {detail.attribution && !detail.workItemId && <div className={styles.card}><h3>关联目标</h3>{detail.attribution.map(s => <button key={s.workItemId} onClick={() => void perform(() => mutateDetail('attribution',{ workItemId: s.workItemId }))}>{s.title} — {s.reasons.join('; ')}</button>)}<select aria-label="Choose todo" value="" onChange={e => e.target.value && void perform(() => mutateDetail('attribution',{ workItemId: e.target.value }))}><option value="">选择其他待办</option>{todos.map(t => <option key={t.id} value={t.id}>{t.title}</option>)}</select><button onClick={() => void perform(() => mutateDetail('attribution',{ workItemId: null }))}>不关联目标</button></div>}
      {detail.workItemId && <div className={styles.toolbar}>{itemPath(detail)}<button onClick={() => void perform(() => mutateDetail('attribution',{ workItemId: null }))}>清除归属</button></div>}
      <div className={styles.card}><h3>你的原话</h3><p className={styles.subtle}>原话默认保留为候选；编辑并确认后才参与进度判断。问题留在这里，不计入进度。</p>{detail.asks.map(ask => <div key={ask.id}><span className={styles.subtle}>{ask.kind === 'question' ? '问题 · 不计进度' : `候选要求 · ${ask.kind}`} · {new Date(ask.at).toLocaleString()}</span><pre className={styles.ask}>{ask.text}</pre></div>)}</div>
      {!!detail.importSuggestions?.length && <div className={styles.card}><h3>澄清要求</h3><p>原话较简短，可从同项目的先前会话导入要求，或关联一个待办。</p>{detail.importSuggestions.map(previous => <button key={previous.sessionId} disabled={busy} onClick={() => void perform(() => mutateDetail('import-requirements',{ fromSessionId: previous.sessionId }))}>导入要求： {previous.title}</button>)}</div>}
      {!!detail.followUpSuggestions?.length && <div className={styles.card}><h3>建议后续待办</h3><p className={styles.subtle}>这些是 agent 的后续建议；预览并保存后才会创建待办。</p>{detail.followUpSuggestions.map(text => <div key={text} className={styles.item}><span>{text}</span><button onClick={() => setEditing({ title: text,level: 'task',parentId: todos.find(t => t.id === detail.workItemId)?.parentId ?? '',outcome: '',checklist: '',projectRoot: detail.projectRoot })}>预览新待办</button></div>)}</div>}
      <div className={styles.split}><div id="ledger-requirements" className={styles.card}><div className={styles.header}><h3>{detail.progress.total ? '已确认要求' : '候选要求 · 不计进度'}</h3>{detail.items.some(item => item.source === 'fallback') && <button disabled={busy} onClick={() => void perform(() => mutateDetail('items',{ items: detail.items },'PUT'))}>确认这些要求</button>}<div className={styles.actions}><button onClick={() => setItemEditor(structuredClone(detail.items).map(item => item.source === 'user' && item.anchors.commandFormat === 'legacy-unconfirmed' ? { ...item,anchors: { ...item.anchors,commands: [] } } : item))}>编辑要求</button>{settings.judge.enabled && <button disabled={busy} onClick={() => void perform(() => mutateDetail('redecompose',{}))}>重新识别要求</button>}</div></div>{detail.items.map(item => <button className={`${styles.item} ${itemFilter === item.id ? styles.selected : ''}`} key={item.id} onClick={() => setItemFilter(f => f === item.id ? null : item.id)}><strong>{item.title}</strong><span className={styles.subtle}>{item.dropped ? '你已放弃' : item.source === 'fallback' ? '待你确认' : itemStateText[item.status]} · {sourceText[item.source]} · {item.statusSource === 'user' ? '你设置的状态' : '实际证据匹配'} · {item.evidenceIds.length} 条证据</span>{item.source === 'user' && item.anchors.commandFormat === 'legacy-unconfirmed' && <span className={styles.subtle}>旧命令格式待确认 · 请在“编辑要求”中填写完整验收命令</span>}{item.constraints.map((c,i) => <span key={i} className={styles.subtle}>{c.kind}: {c.value}</span>)}</button>)}</div>
      <div id={detail.trail.length ? undefined : "ledger-current-step"} className={styles.card}><h3>执行轨迹 {itemFilter && '（已筛选）'}</h3>{detail.turns.map(turn => <details key={turn.id} open><summary>{{ started: '开始执行',completed: '本轮结束',aborted: '已中断' }[turn.phase] ?? turn.phase} · {new Date(turn.at).toLocaleTimeString()} · {turn.readOnlyCount} 次读取或等待调用已折叠</summary>{detail.trail.filter(s => s.turnId === turn.id && (!itemFilter || s.itemId === itemFilter)).map(step => <div id={step.callId === detail.trail.at(-1)?.callId ? "ledger-current-step" : undefined} className={styles.item} key={step.callId}><pre className={styles.step}>{step.summary}</pre><span className={styles.subtle}>{step.itemId ? detail.items.find(i => i.id === step.itemId)?.title : step.acceptedOffPlan ? '你已接受的偏离' : '未归属'} · {step.violations.join('; ')}</span><details><summary>证据（{step.evidenceIds.length})</summary>{detail.evidence.filter(e => step.evidenceIds.includes(e.id)).map(e => <pre className={styles.output} key={e.id}>{e.kind}: {e.value} {e.exitCode !== undefined ? `· exit ${e.exitCode}` : ''}</pre>)}</details><select aria-label="Assign step" value={step.itemId ?? ''} onChange={e => void perform(() => mutateDetail('corrections',{ callId: step.callId,itemId: e.target.value || undefined }))}><option value="">未归属</option>{detail.items.filter(confirmedRequirement).map(i => <option key={i.id} value={i.id}>{i.title}</option>)}</select><button disabled={!step.itemId} onClick={() => { const paths = window.prompt('相似步骤的路径（例如 src/ledger/**）'); if (paths?.trim() && step.itemId) void perform(() => mutateDetail('corrections',{ callId: step.callId,itemId: step.itemId,rule: { itemId: step.itemId,matcher: { paths: [paths.trim()],commands: [] } } })) }}>后续相似步骤也这样归属</button></div>)}</details>)}</div></div>
      {detail.offPlan.map((run,index) => <div id={index === 0 ? "ledger-off-plan" : undefined} className={styles.card} key={run.id}><strong>可能偏离 · {run.callIds.length} 步</strong><div className={styles.actions}><button onClick={() => void perform(() => mutateDetail('corrections',{ callIds: run.callIds,acceptOffPlan: true }))}>接受这段操作，不再提醒</button><button onClick={() => void perform(() => openPrompt(detail,run.id))}>生成纠正提示</button></div></div>)}
      <div className={styles.card}><h3>汇报与证据</h3><table className={styles.table}><thead><tr><th>agent 原话</th><th>实际执行证据</th></tr></thead><tbody>{detail.claims.map((claim,i) => <tr key={i}><td>{claim.text}</td><td>{claim.evidenceIds.length ? detail.evidence.filter(e => claim.evidenceIds.includes(e.id)).map(e => <div key={e.id}>{e.kind}: {e.value}</div>) : <button onClick={() => void perform(() => openPrompt(detail))}>暂无匹配证据（仅记录）</button>}</td></tr>)}</tbody></table></div>
      {(detail.state === 'review' || detail.state === 'ended' && detail.progress.total > 0 && detail.turns.find(t => t.id === detail.turnId)?.phase === 'completed') && <div className={styles.card}><h3>核对这一轮</h3><div className={styles.actions}><button disabled={detail.progress.done !== detail.progress.total || busy} onClick={() => void perform(() => mutateDetail('acceptances',{ decision: 'accepted' }))}>验收通过</button><button onClick={() => { const reason = window.prompt('为什么放弃剩余要求？'); if (reason?.trim()) void perform(() => mutateDetail('acceptances',{ decision: 'accepted_with_gaps',reason,droppedItemIds: detail.items.filter(i => i.status !== 'done' || !i.evidenceIds.length).map(i => i.id) })) }}>接受并放弃剩余项</button><button onClick={() => void perform(async () => { await mutateDetail('acceptances',{ decision: 'follow_up' }); await openPrompt(detail) })}>生成后续提示</button></div></div>}
      {detail.acceptances.length > 0 && <div className={styles.card}>{detail.acceptances.map(a => <div key={a.turnId}>{a.decision} · {a.reason} · {new Date(a.at).toLocaleString()}</div>)}</div>}
      {!!detail.subagents?.length && <div className={styles.card}><h3>子任务 · {detail.subagents.length}</h3>{detail.subagents.map(child => <button key={child.sessionId} onClick={() => setSelectedId(child.sessionId)}><strong>{child.title}</strong> · {stateText[child.state]}<Activity row={child} /></button>)}</div>}
    </>}
    {view === 'goals' && <><button onClick={() => editWorkItem()}>+ 新建目标</button>{goals.map(goal => <div className={styles.card} key={goal.id}><div className={styles.header}><h3>{goal.title} {goal.stale && <span className={styles.attention}>· 暂无进展</span>}</h3><Progress {...goal.progress} /></div><p>{goal.outcome}</p><span className={styles.subtle}>{goal.progress.active} active todos · {goal.weeklyMovement} completed this week</span><div className={styles.actions}><button onClick={() => editWorkItem(goal)}>编辑目标</button><button onClick={() => editWorkItem(undefined,goal.id)}>+ Todo</button></div>{goal.todos.map(todo => <div className={styles.item} key={todo.id}><strong>{todo.title}</strong><span className={styles.subtle}>{todo.status}</span>{todo.checklist.map(c => <div key={c.id}>{c.satisfied ? '✓' : '○'} {c.text}</div>)}{todo.sessions.map(s => <button key={s.runtime_session_id} onClick={() => onOpenSession(s.runtime_session_id)}>{s.title} · {s.status}</button>)}<div className={styles.actions}><button onClick={() => editWorkItem(todo)}>编辑待办</button>{(['codex','claude-code'] as const).map(runtime => <button key={runtime} onClick={() => setPrompt({ dispatch: todo,runtime,text: `${todo.title}\n${todo.body ?? ''}\n\nAcceptance criteria:\n${todo.acceptance?.map(c => `- ${c.text}`).join('\n') ?? ''}` })}>交给 {runtime}</button>)}{todo.readyToComplete && <button onClick={() => void perform(() => api(`/goals/todos/${todo.id}/complete`,'POST'))}>验收清单已有证据，可标记完成</button>}</div></div>)}</div>)}</>}
    {view === 'review' && <><div className={styles.toolbar}><input aria-label="回顾日期" type="date" value={date} onChange={e => setDate(e.target.value)} /><label><input type="checkbox" checked={weekly} onChange={e => setWeekly(e.target.checked)} /> 从此日期开始的一周</label></div>{review && <><div className={styles.card}><h3>仍需处理</h3>{review.open.map(row => <div className={styles.item} key={row.sessionId}><strong>{row.title}</strong><p>{row.items.filter(i => i.status !== 'done').map(i => i.title).join(' · ')}</p><div className={styles.actions}><button onClick={() => void perform(() => openPrompt(row))}>生成后续提示</button><button onClick={() => void perform(() => api(`/ledger/${encodeURIComponent(row.sessionId)}/carry-over`,'POST'))}>转为明日待办</button></div></div>)}</div><div className={styles.card}><h3>已验收的工作</h3>{review.accepted.map(row => <div key={row.sessionId}>{row.title}<p className={styles.subtle}>{row.evidence.filter(e => e.exitCode === 0).map(e => e.value).slice(0,3).join(' · ')}</p></div>)}</div><div className={styles.card}><h3>偏离与纠正</h3><p>{review.offPlan.length} 段偏离 · {review.corrections.length}次纠正</p>{review.offPlan.map(run => <div key={run.id}>{run.title}</div>)}</div>{weekly && <div className={styles.card}><h3>本周进展</h3><p>未关联目标的运行时间： {(review.unattributedRuntimeShare*100).toFixed(1)}%</p>{review.goals.map(g => <div key={g.id}>{g.title}: {g.weeklyMovement} 条已完成待办</div>)}</div>}</>}</>}
    {view === 'ledger-settings' && <div className={`${styles.card} ${styles.form}`}>
      {isNativeApp() && <label className={styles.toggle}><input type="checkbox" checked={autostart} onChange={e => { const enabled = e.target.checked; void perform(async () => { await setAutostart(enabled); setAutostartState(enabled) }) }} />登录后启动 Keepline</label>}
      {(['enabled','constraints','nativeNotifications'] as const).map(key => <label className={styles.toggle} key={key}><input type="checkbox" checked={settings[key]} onChange={e => setSettings(s => ({ ...s,[key]: e.target.checked }))} />{key}</label>)}
      <label className={styles.toggle}><input type="checkbox" checked={settings.judge.enabled} onChange={e => setSettings(s => ({ ...s,judge: { ...s.judge,enabled: e.target.checked } }))} />AI 要求识别与拆分（默认关闭）</label><p className={styles.subtle}>每条新的候选原话最多识别一次。工具调用、agent 汇报和重复扫描不触发模型；完成仍以执行证据为准。可手动重新识别。</p>
      <label>识别方式<select value={settings.judge.backend} onChange={e => setSettings(s => ({ ...s,judge: { ...s.judge,backend: e.target.value as LedgerConfig['judge']['backend'] } }))}>{(['cli-claude','cli-codex','local','sdk'] as const).map(b => <option key={b} value={b}>{{ 'cli-claude': 'Claude CLI','cli-codex': 'Codex CLI',local: '本地服务',sdk: 'SDK' }[b]}</option>)}</select></label>
      <label>模型<input value={settings.judge.model ?? ''} onChange={e => setSettings(s => ({ ...s,judge: { ...s.judge,model: e.target.value || null } }))} /></label>
      <label>偏离敏感度<select value={settings.deviation} onChange={e => setSettings(s => ({ ...s,deviation: e.target.value as LedgerConfig['deviation'] }))}>{(['off','conservative','sensitive'] as const).map(b => <option key={b} value={b}>{{ off: '关闭',conservative: '保守',sensitive: '敏感' }[b]}</option>)}</select></label>
      {Object.keys(settings.alerts).map(key => <label className={styles.toggle} key={key}><input type="checkbox" checked={settings.alerts[key as keyof LedgerConfig['alerts']]} onChange={e => setSettings(s => ({ ...s,alerts: { ...s.alerts,[key]: e.target.checked } }))} />系统通知：{alertText[key as keyof typeof alertText]}</label>)}
      {(['retentionDays','staleGoalDays','stalledAfterSeconds','alertCoalesceSeconds'] as const).map(key => <label key={key}>{{ retentionDays: '记录保留天数',staleGoalDays: '目标停滞提醒天数',stalledAfterSeconds: '停滞判断秒数',alertCoalesceSeconds: '重复提醒合并秒数' }[key]}<input type="number" min="1" value={settings[key]} onChange={e => setSettings(s => ({ ...s,[key]: Number(e.target.value) }))} /></label>)}
      <label>专注分钟数<input type="number" min="1" value={settings.focus.minutes} onChange={e => setSettings(s => ({ ...s,focus: { ...s.focus,minutes: Number(e.target.value) } }))} /></label>
      <label>排除项目（每行一个路径）<textarea value={settings.exclude.projects.join('\n')} onChange={e => setSettings(s => ({ ...s,exclude: { ...s.exclude,projects: e.target.value.split('\n') } }))} /></label>
      <label>排除 agent 类型<select multiple value={settings.exclude.runtimes} onChange={e => setSettings(s => ({ ...s,exclude: { ...s.exclude,runtimes: Array.from(e.target.selectedOptions,o => o.value) } }))}><option value="codex">Codex</option><option value="claude-code">Claude Code</option></select></label>
      <button disabled={busy} onClick={() => void perform(saveSettings)}>保存设置</button>
    </div>}
    {prompt && <div className={styles.dialog} role="dialog" aria-modal="true" aria-label={prompt.dispatch ? '预览交给 agent 的任务' : '编辑后续提示'}><div className={styles.card}><h3>{prompt.dispatch ? `交给 ${prompt.runtime}` : '复制提示'}</h3><textarea rows={14} value={prompt.text} onChange={e => setPrompt({ ...prompt,text: e.target.value })} /><div className={styles.actions}><button onClick={() => void perform(() => navigator.clipboard.writeText(prompt.text))}>复制</button>{prompt.sessionId && <button onClick={() => void perform(async () => { await navigator.clipboard.writeText(prompt.text); onOpenSession(prompt.sessionId!); setPrompt(null) })}>复制并打开 agent</button>}{prompt.dispatch && <button disabled={busy} onClick={() => void perform(async () => { await api(`/goals/todos/${prompt.dispatch!.id}/dispatch`,'POST',{ runtimeId: prompt.runtime,cwd: prompt.dispatch!.projectRoot,prompt: prompt.text,idempotencyKey: crypto.randomUUID() }); setPrompt(null) })}>启动 agent</button>}<button onClick={() => setPrompt(null)}>关闭</button></div></div></div>}
    {editing && <div className={styles.dialog} role="dialog" aria-modal="true" aria-label="编辑目标或待办"><div className={`${styles.card} ${styles.form}`}><h3>{editing.id ? '编辑' : '新建'} {editing.level === 'goal' ? '目标' : '待办'}</h3><label>标题<input value={editing.title} onChange={e => setEditing({ ...editing,title: e.target.value })} /></label><label>项目目录<input value={editing.projectRoot} onChange={e => setEditing({ ...editing,projectRoot: e.target.value })} /></label>{editing.level === 'goal' ? <label>成功标准<textarea value={editing.outcome} onChange={e => setEditing({ ...editing,outcome: e.target.value })} /></label> : <><label>目标<select value={editing.parentId} onChange={e => setEditing({ ...editing,parentId: e.target.value })}><option value="">不关联目标</option>{goals.map(g => <option value={g.id} key={g.id}>{g.title}</option>)}</select></label><label>验收清单（每行一项）<textarea rows={8} value={editing.checklist} onChange={e => setEditing({ ...editing,checklist: e.target.value })} /></label></>}<div className={styles.actions}><button disabled={!editing.title.trim() || busy} onClick={() => void perform(saveWorkItem)}>保存</button><button onClick={() => setEditing(null)}>取消</button></div></div></div>}
    {itemEditor !== null && <div className={styles.dialog} role="dialog" aria-modal="true" aria-label="编辑并确认要求"><div className={`${styles.card} ${styles.form}`}><h3>编辑并确认要求</h3><p className={styles.subtle}>可用路径和关键词关联步骤。验收命令须与直接执行的命令一致；后续失败或文件编辑后需要重新验证。保存即确认这些要求，已删除的项不会重新出现。</p>{itemEditor.map((item,index) => {
      const update = (patch: Partial<RequirementItem>) => setItemEditor(items => items!.map((value,i) => i === index ? { ...value,...patch } : value))
      const lines = (text: string) => text.split('\n')
      return <fieldset className={styles.item} key={item.id}><legend>要求 {index+1}</legend><label>要求标题<input value={item.title} onChange={e => update({ title: e.target.value })} /></label><label>状态<select value={item.status} onChange={e => update({ status: e.target.value as RequirementItem['status'] })}>{['todo','doing','done','unverified'].map(status => <option key={status} value={status}>{itemStateText[status as keyof typeof itemStateText]}</option>)}</select></label>{item.source === 'user' && item.anchors.legacyCommands?.length ? <div className={styles.subtle}><p>{item.anchors.commandFormat === 'legacy-unconfirmed' ? '旧数据未记录命令格式，暂不用于自动验收。原文本仅保留步骤归属；请填写实际需要执行的完整命令，再明确确认。' : '原命令模式仅用于步骤归属，不作为验收证据。'}</p><pre>{item.anchors.legacyCommands.join('\n')}</pre></div> : null}{(['paths','commands','keywords'] as const).map(kind => <label key={kind}>{kind === 'paths' ? '文件路径或匹配模式' : kind === 'commands' ? '验收命令（完整文本）' : '关键词'}（每行一个）<textarea rows={2} value={item.anchors[kind].join('\n')} onChange={e => update({ anchors: { ...item.anchors,[kind]: lines(e.target.value) } })} /></label>)}{item.anchors.commandFormat === 'legacy-unconfirmed' && <button disabled={!item.anchors.commands.some(command => command.trim())} onClick={() => update({ anchors: { ...item.anchors,commandFormat: 'literal-v2' } })}>确认以上是完整验收命令</button>}<label className={styles.toggle}><input type="checkbox" checked={item.constraints.some(c => c.kind === 'no_public_api_change')} onChange={e => update({ constraints: [...item.constraints.filter(c => c.kind !== 'no_public_api_change'),...(e.target.checked ? [{ kind: 'no_public_api_change' as const }] : [])] })} />保持公共接口不变</label><label>禁止修改的路径（每行一个）<textarea rows={2} value={item.constraints.filter(c => c.kind === 'path_forbidden').map(c => c.value).join('\n')} onChange={e => update({ constraints: [...item.constraints.filter(c => c.kind !== 'path_forbidden'),...lines(e.target.value).map(value => ({ kind: 'path_forbidden' as const,value }))] })} /></label><label>需要保留的标签或文字（每行一个）<textarea rows={2} value={item.constraints.filter(c => c.kind === 'preserve_text').map(c => c.value).join('\n')} onChange={e => update({ constraints: [...item.constraints.filter(c => c.kind !== 'preserve_text'),...lines(e.target.value).map(value => ({ kind: 'preserve_text' as const,value }))] })} /></label><button onClick={() => setItemEditor(items => items!.filter((_,i) => i !== index))}>删除这项要求</button></fieldset>
    })}<button onClick={() => setItemEditor(items => [...items!,{ id: crypto.randomUUID(),ordinal: items!.length,title: '',anchors: { paths: [],commands: [],keywords: [] },constraints: [],source: 'user',status: 'todo',statusSource: 'user',evidenceIds: [] }])}>补充要求</button><div className={styles.actions}><button disabled={busy || itemEditor.some(item => !item.title.trim() || item.anchors.commandFormat === 'legacy-unconfirmed' && item.anchors.commands.some(command => command.trim()))} onClick={() => void perform(async () => { await mutateDetail('items',{ items: itemEditor.map(item => ({ ...item,id: detail?.items.some(existing => existing.id === item.id) ? item.id : undefined,anchors: { ...item.anchors,paths: item.anchors.paths.map(value => value.trim()).filter(Boolean),commands: item.anchors.commands.map(value => value.trim()).filter(Boolean),keywords: item.anchors.keywords.map(value => value.trim()).filter(Boolean) },constraints: item.constraints.filter(c => c.kind !== 'path_forbidden' || c.value?.trim()).map(c => ({ ...c,...(c.value !== undefined ? { value: c.value.trim() } : {}) })) })) },'PUT'); setItemEditor(null) })}>保存并确认</button><button onClick={() => setItemEditor(null)}>取消</button></div></div></div>}
  </section>
}
