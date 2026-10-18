import { useMemo, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api, downloadFile } from '../api'
import type {
  GenerateReport,
  Lesson,
  PublicLink,
  SetupAll,
  SlotEntry,
  SolverWeights,
  TimetableData,
} from '../api'
import { useAuth } from '../auth'
import { Page, PageHead } from '../components/Layout'
import TimetableGrid from '../components/TimetableGrid'
import type { GridMode } from '../components/TimetableGrid'
import {
  Badge,
  Btn,
  Card,
  ConfirmButton,
  EmptyState,
  Field,
  Input,
  Loading,
  Modal,
  Select,
  errMsg,
  plural,
  toast,
} from '../ui'

export default function TimetablePage() {
  const { user, isStaff } = useAuth()
  const qc = useQueryClient()

  const [mode, setMode] = useState<GridMode>(user?.role === 'teacher' ? 'teacher' : 'class')
  const [groupId, setGroupId] = useState<string>(() =>
    user?.role === 'student' ? (user.class_id ?? '') : user?.role === 'teacher' ? (user.teacher_id ?? '') : '',
  )
  const [week, setWeek] = useState(0)
  const [showGenerate, setShowGenerate] = useState(false)
  const [showReport, setShowReport] = useState<GenerateReport | null>(null)
  const [prevReport, setPrevReport] = useState<GenerateReport | null>(null)
  const [editEntry, setEditEntry] = useState<SlotEntry | null>(null)
  const [showAdd, setShowAdd] = useState(false)
  const [showShare, setShowShare] = useState(false)

  const timetable = useQuery({
    queryKey: ['timetable'],
    queryFn: () => api<TimetableData>('/api/timetable'),
  })
  const setup = useQuery({ queryKey: ['setup'], queryFn: () => api<SetupAll>('/api/setup/all') })

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ['timetable'] })
    void qc.invalidateQueries({ queryKey: ['dashboard'] })
  }

  const move = useMutation({
    mutationFn: (p: { id: string; day: number; idx: number }) =>
      api(`/api/timetable/entries/${p.id}`, { method: 'PUT', body: { week, day: p.day, idx: p.idx } }),
    onSuccess: () => {
      refresh()
      toast('Lesson moved', 'ok')
    },
    onError: (e) => toast(errMsg(e), 'err'),
  })

  const data = timetable.data
  const cycleWeeks = data?.cycle_weeks ?? 1

  const groups = useMemo(() => {
    if (!setup.data) return []
    if (mode === 'class') return setup.data.classes.map((c) => ({ id: c.id, name: c.name }))
    if (mode === 'teacher') return setup.data.teachers.map((t) => ({ id: t.id, name: t.name }))
    return setup.data.rooms.map((r) => ({ id: r.id, name: `${r.name} (${r.room_type})` }))
  }, [setup.data, mode])

  const entries = useMemo(
    () => (data?.entries ?? []).filter((e) => e.week === (cycleWeeks > 1 ? week : 0)),
    [data, week, cycleWeeks],
  )

  const unplaced = useMemo(() => {
    if (!setup.data || !data) return []
    return setup.data.lessons
      .map((l) => {
        const placed = data.entries.filter((e) => e.lesson_id === l.id).length
        return { lesson: l, placed, missing: l.periods_per_week - placed }
      })
      .filter((x) => x.missing > 0)
      .sort((a, b) => b.missing - a.missing)
  }, [setup.data, data])

  const legend = useMemo(() => {
    if (!setup.data) return []
    return setup.data.subjects.filter((s) => (data?.entries ?? []).some((e) => e.subject_id === s.id))
  }, [setup.data, data])

  if (timetable.isLoading || setup.isLoading) return <Page><Loading label="Loading timetable…" /></Page>
  if (timetable.error || setup.error)
    return (
      <Page>
        <Card>
          <EmptyState icon="⚠️" title="Could not load the timetable" desc={String(timetable.error ?? setup.error)} />
        </Card>
      </Page>
    )

  const conflictHint =
    mode === 'class' ? 'classes' : mode === 'teacher' ? 'teachers' : 'rooms'

  // Export the view the user is actually looking at (entity + cycle week).
  const exportParams = () => {
    const params = new URLSearchParams({ entity: mode })
    const id = groupId || groups[0]?.id
    if (id) params.set('id', id)
    if (cycleWeeks > 1) params.set('week', String(week))
    return params
  }
  const exportTarget = groupId || groups[0]?.id
  const exportName = groups.find((g) => g.id === exportTarget)?.name ?? mode

  return (
    <Page>
      <PageHead
        title="Timetable"
        sub={
          data
            ? `${data.entries.length} lessons placed across ${plural(groups.length, conflictHint.slice(0, -1) || conflictHint)}`
            : undefined
        }
        actions={
          <div className="tt-toolbar no-print">
            {isStaff && (
              <>
                <Btn variant="primary" onClick={() => setShowGenerate(true)}>
                  ⚡ Generate
                </Btn>
                <Btn onClick={() => setShowAdd(true)}>+ Place lesson</Btn>
                <ConfirmButton
                  variant="ghost"
                  size="md"
                  message="Clear the entire timetable? Locked lessons are cleared too."
                  onConfirm={async () => {
                    await api('/api/timetable/clear', { body: {} })
                    refresh()
                    toast('Timetable cleared', 'ok')
                  }}
                >
                  Clear
                </ConfirmButton>
              </>
            )}
            <Btn
              onClick={() =>
                downloadFile(`/api/export/timetable.csv?${exportParams()}`, `timetable-${mode}.csv`)
              }
            >
              ⬇ CSV
            </Btn>
            <Btn
              onClick={() =>
                downloadFile(`/api/export/timetable.ics?${exportParams()}`, `timetable-${mode}.ics`)
              }
            >
              📆 ICS
            </Btn>
            <Btn
              onClick={() =>
                downloadFile(`/api/export/timetable.pdf?${exportParams()}`, `timetable-${mode}.pdf`)
              }
            >
              📄 PDF
            </Btn>
            {isStaff && exportTarget && (
              <Btn onClick={() => setShowShare(true)}>🔗 Share</Btn>
            )}
            <Btn onClick={() => window.print()}>🖨 Print</Btn>
          </div>
        }
      />

      <Card className="no-print" tight>
        <div className="card-body row">
          <div className="row tight">
            <span className="small muted">View by</span>
            <Select
              className="sm"
              value={mode}
              onChange={(e) => {
                setMode(e.target.value as GridMode)
                setGroupId('')
              }}
            >
              <option value="class">Class</option>
              <option value="teacher">Teacher</option>
              <option value="room">Room</option>
            </Select>
            <Select className="sm" value={groupId} onChange={(e) => setGroupId(e.target.value)}>
              <option value="">All {mode === 'class' ? 'classes' : mode === 'teacher' ? 'teachers' : 'rooms'}</option>
              {groups.map((g) => (
                <option key={g.id} value={g.id}>
                  {g.name}
                </option>
              ))}
            </Select>
            {cycleWeeks > 1 && (
              <>
                <span className="small muted">Cycle week</span>
                <Select className="sm" value={week} onChange={(e) => setWeek(Number(e.target.value))}>
                  <option value={0}>Week 1</option>
                  <option value={1}>Week 2</option>
                </Select>
              </>
            )}
          </div>
          <div className="right row tight">
            {unplaced.length > 0 && (
              <Badge tone="warn">{plural(unplaced.reduce((a, u) => a + u.missing, 0), 'period')} unplaced</Badge>
            )}
            {isStaff && <span className="small faint">Drag a lesson to move it · click to edit</span>}
          </div>
        </div>
      </Card>

      {unplaced.length > 0 && (
        <Card title="Unplaced periods" className="no-print">
          <div className="row">
            {unplaced.slice(0, 8).map(({ lesson, placed, missing }) => (
              <span key={lesson.id} className="chip" title={`${placed} of ${lesson.periods_per_week} placed`}>
                <span className="dot" style={{ background: lesson.subject_color }} />
                {lesson.class_name} · {lesson.subject_name}
                <Badge tone="warn">{missing} left</Badge>
              </span>
            ))}
          </div>
          <div className="small muted" style={{ marginTop: 8 }}>
            These periods could not be placed automatically. Try freeing slots, adding a room of the required
            type, or lengthening the generation time.
          </div>
        </Card>
      )}

      <Card tight>
        <div className="card-body">
          {!data || data.entries.length === 0 ? (
            <EmptyState
              icon="🗓️"
              title="The timetable is empty"
              desc={
                isStaff
                  ? 'Generate a timetable from your curriculum, or place lessons by hand.'
                  : 'The timetable has not been published yet.'
              }
              action={
                isStaff ? (
                  <Btn variant="primary" onClick={() => setShowGenerate(true)}>
                    ⚡ Generate timetable
                  </Btn>
                ) : undefined
              }
            />
          ) : (
            <>
              <TimetableGrid
                slots={data.slots}
                entries={entries}
                mode={mode}
                groups={groups}
                selectedGroup={groupId || undefined}
                editable={isStaff}
                onMove={(id, day, idx) => move.mutate({ id, day, idx })}
                onEdit={(e) => setEditEntry(e)}
              />
              {legend.length > 0 && (
                <div className="legend no-print" style={{ marginTop: 14 }}>
                  {legend.map((s) => (
                    <span key={s.id} className="chip">
                      <span className="dot" style={{ background: s.color }} />
                      {s.name}
                    </span>
                  ))}
                </div>
              )}
            </>
          )}
        </div>
      </Card>

      <GenerateModal
        open={showGenerate}
        onClose={() => setShowGenerate(false)}
        onDone={(report) => {
          refresh()
          setPrevReport((prev) => showReport ?? prev)
          setShowReport(report)
          if (report.unplaced_total === 0) toast(`Timetable generated — all ${report.placed} periods placed`, 'ok')
          else toast(`Generated with ${report.unplaced_total} unplaced periods`, 'err')
        }}
        cycleWeeks={cycleWeeks}
      />

      <ReportModal report={showReport} previous={prevReport} onClose={() => setShowReport(null)} />

      {editEntry && (
        <EntryModal
          entry={editEntry}
          rooms={setup.data?.rooms ?? []}
          onClose={() => setEditEntry(null)}
          onChanged={() => {
            refresh()
            void qc.invalidateQueries({ queryKey: ['setup'] })
          }}
          canEdit={isStaff}
        />
      )}

      <AddEntryModal
        open={showAdd}
        onClose={() => setShowAdd(false)}
        setup={setup.data!}
        onDone={() => {
          refresh()
          void qc.invalidateQueries({ queryKey: ['setup'] })
        }}
      />

      {showShare && exportTarget && (
        <ShareModal
          onClose={() => setShowShare(false)}
          entity={mode}
          entityId={exportTarget}
          entityName={exportName}
        />
      )}
    </Page>
  )
}

// ------------------------------------------------------------------ generate

function GenerateModal({
  open,
  onClose,
  onDone,
  cycleWeeks,
}: {
  open: boolean
  onClose: () => void
  onDone: (r: GenerateReport) => void
  cycleWeeks: number
}) {
  const [limit, setLimit] = useState(4000)
  const [seed, setSeed] = useState('')
  const [busy, setBusy] = useState(false)
  const [advanced, setAdvanced] = useState(false)
  const [weights, setWeights] = useState<SolverWeights>(DEFAULT_WEIGHTS)

  const run = async () => {
    setBusy(true)
    try {
      const body: Record<string, unknown> = { time_limit_ms: limit }
      if (seed.trim()) body.seed = Number(seed.trim())
      if (advanced) body.weights = weights
      const report = await api<GenerateReport>('/api/generate', { body })
      onDone(report)
      onClose()
    } catch (e) {
      toast(errMsg(e), 'err')
    } finally {
      setBusy(false)
    }
  }

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Generate timetable"
      footer={
        <>
          <Btn onClick={onClose}>Cancel</Btn>
          <Btn variant="primary" onClick={run} disabled={busy}>
            {busy ? 'Solving…' : '⚡ Generate'}
          </Btn>
        </>
      }
    >
      <p className="muted small" style={{ marginBottom: 14 }}>
        The solver places every lesson, keeping classes and teachers clash-free. Manually placed and locked
        lessons stay exactly where they are.
      </p>
      <Field label="Search effort" hint="Longer searches explore more arrangements and reduce gaps.">
        <Select value={limit} onChange={(e) => setLimit(Number(e.target.value))}>
          <option value={1000}>Quick — 1 second</option>
          <option value={4000}>Balanced — 4 seconds</option>
          <option value={10000}>Thorough — 10 seconds</option>
          <option value={20000}>Deep — 20 seconds</option>
        </Select>
      </Field>
      <Field label="Random seed" hint="Leave empty for a fresh solution each run; reuse a seed to reproduce one.">
        <Input value={seed} onChange={(e) => setSeed(e.target.value)} placeholder="e.g. 42" />
      </Field>
      <div className="small faint">
        {cycleWeeks > 1
          ? `This school uses a ${cycleWeeks}-week timetable cycle — both weeks are generated at once.`
          : 'This school uses a single-week timetable cycle.'}
      </div>

      <div style={{ marginTop: 14, borderTop: '1px solid var(--border)', paddingTop: 12 }}>
        <div className="row tight" style={{ justifyContent: 'space-between' }}>
          <strong className="small">Scoring balance</strong>
          <Btn size="sm" variant="ghost" onClick={() => setAdvanced((a) => !a)}>
            {advanced ? 'Hide advanced' : 'Tune advanced'}
          </Btn>
        </div>
        {advanced && (
          <div style={{ marginTop: 10 }}>
            <p className="muted small" style={{ marginBottom: 10 }}>
              Higher values make the solver care more about that goal. Hard constraints (clashes,
              limits, rooms) are never traded away.
            </p>
            {WEIGHT_FIELDS.map((f) => (
              <div key={f.key} style={{ marginBottom: 8 }}>
                <div className="row tight" style={{ justifyContent: 'space-between' }}>
                  <span className="small">{f.label}</span>
                  <span className="mono small">{weights[f.key]}</span>
                </div>
                <input
                  type="range"
                  min={f.min}
                  max={f.max}
                  step={f.step}
                  value={weights[f.key]}
                  style={{ width: '100%' }}
                  onChange={(e) =>
                    setWeights({ ...weights, [f.key]: Number(e.target.value) })
                  }
                />
              </div>
            ))}
            <Btn size="sm" onClick={() => setWeights(DEFAULT_WEIGHTS)}>
              Reset to defaults
            </Btn>
          </div>
        )}
      </div>
    </Modal>
  )
}

const DEFAULT_WEIGHTS: SolverWeights = {
  same_subject_per_day: 4,
  class_gap: 1.5,
  teacher_gap: 0.4,
  teacher_balance: 0.05,
  week_balance: 0.4,
  unplaced: 1000,
}

const WEIGHT_FIELDS: { key: keyof SolverWeights; label: string; min: number; max: number; step: number }[] = [
  { key: 'class_gap', label: 'Avoid gaps in class days', min: 0, max: 10, step: 0.5 },
  { key: 'teacher_gap', label: 'Avoid gaps in teacher days', min: 0, max: 5, step: 0.1 },
  { key: 'same_subject_per_day', label: 'Spread a subject across the week', min: 0, max: 10, step: 0.5 },
  { key: 'teacher_balance', label: 'Balance teacher load across days', min: 0, max: 1, step: 0.01 },
  { key: 'week_balance', label: 'Balance across cycle weeks', min: 0, max: 2, step: 0.1 },
  { key: 'unplaced', label: 'Priority of placing every period', min: 100, max: 2000, step: 100 },
]

function ReportModal({
  report,
  previous,
  onClose,
}: {
  report: GenerateReport | null
  previous: GenerateReport | null
  onClose: () => void
}) {
  if (!report) return null
  const pct = report.requested ? Math.round((report.placed / report.requested) * 100) : 100
  const delta = previous ? report.placed - previous.placed : 0
  const scoreDelta = previous ? report.score - previous.score : 0
  return (
    <Modal open onClose={onClose} title="Generation report" footer={<Btn onClick={onClose}>Done</Btn>}>
      <div className="row" style={{ marginBottom: 14, alignItems: 'stretch' }}>
        <div className="stat" style={{ flex: 1 }}>
          <div className="stat-label">Placed</div>
          <div className="stat-value">{report.placed}</div>
          <div className="stat-hint">of {report.requested} periods</div>
        </div>
        <div className="stat" style={{ flex: 1 }}>
          <div className="stat-label">Coverage</div>
          <div className="stat-value">{pct}%</div>
          <div className="progress" style={{ marginTop: 6 }}>
            <div style={{ width: `${pct}%` }} />
          </div>
        </div>
        <div className="stat" style={{ flex: 1 }}>
          <div className="stat-label">Search</div>
          <div className="stat-value">{(report.iterations / 1000).toFixed(0)}k</div>
          <div className="stat-hint">moves in {report.elapsed_ms} ms</div>
        </div>
      </div>

      {previous && (
        <div className="row tight" style={{ marginBottom: 12 }}>
          <Badge tone={delta >= 0 ? 'ok' : 'warn'}>
            {delta >= 0 ? '+' : ''}
            {delta} periods vs previous run
          </Badge>
          <Badge tone={scoreDelta <= 0 ? 'ok' : 'warn'}>
            quality {scoreDelta <= 0 ? 'improved' : 'worsened'}
          </Badge>
          <span className="small faint">lower quality score is better</span>
        </div>
      )}

      {report.unplaced.length > 0 ? (
        <>
          <h3 style={{ marginBottom: 6 }}>Unplaced periods</h3>
          <div className="list">
            {report.unplaced.map((u, i) => (
              <div className="list-item" key={i} style={{ padding: '8px 0' }}>
                <div className="grow">
                  <div className="title">
                    {u.class_name} · {u.subject_name}
                  </div>
                  <div className="sub">{u.count} periods — {u.dominant}</div>
                </div>
              </div>
            ))}
          </div>
        </>
      ) : (
        <div className="row">
          <Badge tone="ok">✅ Every requested period was placed</Badge>
        </div>
      )}

      <h3 style={{ margin: '16px 0 6px' }}>Quality scores (lower is better)</h3>
      <div className="row tight">
        {Object.entries(report.soft).map(([k, v]) => (
          <span key={k} className="chip">
            {k.replace(/_/g, ' ')}: <strong>{Math.round(v)}</strong>
          </span>
        ))}
      </div>
    </Modal>
  )
}

// ------------------------------------------------------------------ entry edit

function EntryModal({
  entry,
  rooms,
  onClose,
  onChanged,
  canEdit,
}: {
  entry: SlotEntry
  rooms: { id: string; name: string; capacity: number; room_type: string }[]
  onClose: () => void
  onChanged: () => void
  canEdit: boolean
}) {
  const [room, setRoom] = useState(entry.room_id ?? '')
  const [busy, setBusy] = useState(false)

  const patch = async (body: Record<string, unknown>, msg: string) => {
    setBusy(true)
    try {
      await api(`/api/timetable/entries/${entry.id}`, { method: 'PUT', body })
      onChanged()
      toast(msg, 'ok')
    } catch (e) {
      toast(errMsg(e), 'err')
    } finally {
      setBusy(false)
    }
  }

  return (
    <Modal
      open
      onClose={onClose}
      title={`${entry.subject_name} · ${entry.class_name}`}
      footer={
        <>
          {canEdit && (
            <ConfirmButton
              variant="danger"
              size="md"
              message="Remove this lesson from the timetable?"
              onConfirm={async () => {
                await api(`/api/timetable/entries/${entry.id}`, { method: 'DELETE' })
                onChanged()
                toast('Lesson removed from the timetable', 'ok')
                onClose()
              }}
            >
              Remove
            </ConfirmButton>
          )}
          <Btn onClick={onClose}>Close</Btn>
        </>
      }
    >
      <div className="row" style={{ marginBottom: 12 }}>
        <Badge tone="info">
          P{entry.idx} · {entry.teacher_name}
        </Badge>
        {entry.locked && <Badge tone="warn">🔒 locked — survives regeneration</Badge>}
        {!entry.locked && <Badge>auto-placed</Badge>}
      </div>

      {canEdit && (
        <>
          <Field label="Room">
            <div className="inline-form">
              <Select value={room} onChange={(e) => setRoom(e.target.value)}>
                <option value="">No room assigned</option>
                {rooms.map((r) => (
                  <option key={r.id} value={r.id}>
                    {r.name} · {r.capacity} seats · {r.room_type}
                  </option>
                ))}
              </Select>
              <Btn
                disabled={busy || room === (entry.room_id ?? '')}
                onClick={() => patch({ room_id: room || null }, 'Room updated')}
              >
                Save room
              </Btn>
            </div>
          </Field>
          <div className="row">
            <Btn
              disabled={busy}
              onClick={() => patch({ locked: !entry.locked }, entry.locked ? 'Lesson unlocked' : 'Lesson locked')}
            >
              {entry.locked ? '🔓 Unlock lesson' : '🔒 Lock lesson'}
            </Btn>
            <span className="small faint">Locked lessons keep their slot when the timetable is regenerated.</span>
          </div>
        </>
      )}
    </Modal>
  )
}

// ------------------------------------------------------------------ manual placement

function AddEntryModal({
  open,
  onClose,
  setup,
  onDone,
}: {
  open: boolean
  onClose: () => void
  setup: SetupAll
  onDone: () => void
}) {
  const [lessonId, setLessonId] = useState('')
  const [day, setDay] = useState(1)
  const [idx, setIdx] = useState(1)
  const [busy, setBusy] = useState(false)

  const days = [...new Set(setup.slots.map((s) => s.day))].sort((a, b) => a - b)
  const periods = [...new Set(setup.slots.filter((s) => !s.is_break).map((s) => s.idx))].sort((a, b) => a - b)
  const lesson: Lesson | undefined = setup.lessons.find((l) => l.id === lessonId)

  const submit = async () => {
    if (!lessonId) return toast('Pick a lesson first', 'err')
    setBusy(true)
    try {
      await api('/api/timetable/entries', { body: { lesson_id: lessonId, day, idx, locked: true } })
      toast('Lesson placed and locked', 'ok')
      onDone()
      onClose()
    } catch (e) {
      toast(errMsg(e), 'err')
    } finally {
      setBusy(false)
    }
  }

  return (
    <Modal
      open={open}
      onClose={onClose}
      title="Place a lesson by hand"
      footer={
        <>
          <Btn onClick={onClose}>Cancel</Btn>
          <Btn variant="primary" onClick={submit} disabled={busy || !lessonId}>
            Place lesson
          </Btn>
        </>
      }
    >
      <Field label="Lesson">
        <Select value={lessonId} onChange={(e) => setLessonId(e.target.value)}>
          <option value="">Select a lesson…</option>
          {setup.lessons.map((l) => (
            <option key={l.id} value={l.id}>
              {l.class_name} · {l.subject_name} · {l.teacher_name}
            </option>
          ))}
        </Select>
      </Field>
      <div className="grid cols-2" style={{ gap: 0 }}>
        <Field label="Day">
          <Select value={day} onChange={(e) => setDay(Number(e.target.value))}>
            {days.map((d) => (
              <option key={d} value={d}>
                {['', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'][d] ?? `Day ${d}`}
              </option>
            ))}
          </Select>
        </Field>
        <Field label="Period">
          <Select value={idx} onChange={(e) => setIdx(Number(e.target.value))}>
            {periods.map((p) => (
              <option key={p} value={p}>
                P{p}
              </option>
            ))}
          </Select>
        </Field>
      </div>
      <div className="small faint">
        {lesson
          ? `${lesson.periods_per_week} periods per week planned for ${lesson.class_name}.`
          : 'Hand-placed lessons are automatically locked.'}
      </div>
    </Modal>
  )
}

// ------------------------------------------------------------------ share links

function ShareModal({
  onClose,
  entity,
  entityId,
  entityName,
}: {
  onClose: () => void
  entity: GridMode
  entityId: string
  entityName: string
}) {
  const qc = useQueryClient()
  const links = useQuery({
    queryKey: ['public-links'],
    queryFn: () => api<{ items: PublicLink[] }>('/api/public-links'),
  })

  const refresh = () => void qc.invalidateQueries({ queryKey: ['public-links'] })

  const create = useMutation({
    mutationFn: () =>
      api<PublicLink>('/api/public-links', { body: { entity, entity_id: entityId } }),
    onSuccess: (l) => {
      refresh()
      void copy(l)
    },
    onError: (e) => toast(errMsg(e), 'err'),
  })

  const remove = useMutation({
    mutationFn: (id: string) => api(`/api/public-links/${id}`, { method: 'DELETE' }),
    onSuccess: () => {
      refresh()
      toast('Public link revoked', 'ok')
    },
    onError: (e) => toast(errMsg(e), 'err'),
  })

  async function copy(l: PublicLink) {
    const full = `${window.location.origin}${l.path}`
    try {
      await navigator.clipboard.writeText(full)
      toast('Public link copied to clipboard', 'ok')
    } catch {
      toast(full, 'info')
    }
  }

  const items = links.data?.items ?? []
  const current = items.find((l) => l.entity === entity && l.entity_id === entityId)

  return (
    <Modal
      open
      onClose={onClose}
      title={`Share the ${entity} timetable`}
      footer={<Btn onClick={onClose}>Done</Btn>}
    >
      <p className="muted small" style={{ marginBottom: 12 }}>
        A public link opens <strong>{entityName}</strong> as a read-only timetable — no sign-in
        needed. Revoke it any time; the link stops working immediately.
      </p>

      {links.isLoading ? (
        <Loading label="Loading links…" />
      ) : current ? (
        <div className="list">
          <div className="list-item">
            <div className="grow">
              <div className="title">Public link active</div>
              <div className="sub mono small">{`${window.location.origin}${current.path}`}</div>
            </div>
            <Badge tone="ok">live</Badge>
          </div>
          <div className="row">
            <Btn size="sm" variant="primary" onClick={() => copy(current)}>
              📋 Copy link
            </Btn>
            <a href={current.path} target="_blank" rel="noreferrer">
              <Btn size="sm">Open</Btn>
            </a>
            <ConfirmButton
              message="Revoke this public link? Anyone using it will lose access."
              onConfirm={() => remove.mutate(current.id)}
            >
              Revoke
            </ConfirmButton>
          </div>
        </div>
      ) : (
        <div className="row">
          <Btn variant="primary" onClick={() => create.mutate()} disabled={create.isPending}>
            🔗 Create public link
          </Btn>
          <span className="small faint">Only staff can create or revoke links.</span>
        </div>
      )}

      {items.length > 0 && (
        <div style={{ marginTop: 18 }}>
          <h3 style={{ marginBottom: 6 }}>All shared timetables</h3>
          <div className="table-wrap">
            <table className="tbl">
              <thead>
                <tr>
                  <th>Type</th>
                  <th>Name</th>
                  <th>Created</th>
                  <th style={{ textAlign: 'right' }}>Actions</th>
                </tr>
              </thead>
              <tbody>
                {items.map((l) => (
                  <tr key={l.id}>
                    <td>
                      <Badge>{l.entity}</Badge>
                    </td>
                    <td>{l.entity_name ?? '—'}</td>
                    <td className="muted small">{new Date(l.created_at).toLocaleDateString()}</td>
                    <td>
                      <div className="actions">
                        <Btn size="sm" onClick={() => copy(l)}>
                          Copy
                        </Btn>
                        <ConfirmButton
                          message={`Revoke the link for ${l.entity_name ?? l.entity}?`}
                          onConfirm={() => remove.mutate(l.id)}
                        >
                          Revoke
                        </ConfirmButton>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </Modal>
  )
}
