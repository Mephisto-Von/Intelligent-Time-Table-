import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from '../api'
import type { CoverData, CoverHistoryItem, SetupAll } from '../api'
import { useAuth } from '../auth'
import { Page, PageHead } from '../components/Layout'
import {
  Badge,
  Btn,
  Card,
  ConfirmButton,
  EmptyState,
  Loading,
  Select,
  Tabs,
  addDays,
  errMsg,
  fmtDate,
  initialsOf,
  plural,
  toast,
  todayISO,
} from '../ui'

export default function CoverPage() {
  const { isStaff } = useAuth()
  const qc = useQueryClient()
  const [date, setDate] = useState(todayISO())
  const [tab, setTab] = useState<'day' | 'history'>('day')

  const cover = useQuery({
    queryKey: ['cover', date],
    queryFn: () => api<CoverData>(`/api/cover?date=${date}`),
    enabled: tab === 'day',
  })
  const setup = useQuery({ queryKey: ['setup'], queryFn: () => api<SetupAll>('/api/setup/all') })
  const history = useQuery({
    queryKey: ['cover-history'],
    queryFn: () => api<{ items: CoverHistoryItem[] }>('/api/cover/history'),
    enabled: tab === 'history',
  })

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ['cover'] })
    void qc.invalidateQueries({ queryKey: ['cover-history'] })
    void qc.invalidateQueries({ queryKey: ['dashboard'] })
  }

  const assign = useMutation({
    mutationFn: (p: { lesson_id: string; day: number; idx: number; substitute_teacher_id: string }) =>
      api('/api/cover/assign', {
        body: { date, lesson_id: p.lesson_id, day: p.day, idx: p.idx, substitute_teacher_id: p.substitute_teacher_id },
      }),
    onSuccess: () => {
      refresh()
      toast('Substitute assigned', 'ok')
    },
    onError: (e) => toast(errMsg(e), 'err'),
  })

  const cancelSub = useMutation({
    mutationFn: (id: string) => api(`/api/cover/${id}`, { method: 'DELETE' }),
    onSuccess: () => {
      refresh()
      toast('Substitution cancelled', 'ok')
    },
    onError: (e) => toast(errMsg(e), 'err'),
  })

  const data = cover.data
  const uncovered = data?.periods.filter((p) => !p.substitution).length ?? 0

  return (
    <Page>
      <PageHead
        title="Cover & substitutes"
        sub="Absent teachers, their lessons, and the best-ranked replacements — all in one screen."
        actions={
          <div className="row tight no-print">
            <Btn size="sm" onClick={() => setDate(addDays(date, -1))}>
              ←
            </Btn>
            <input
              className="input sm"
              type="date"
              style={{ width: 150 }}
              value={date}
              onChange={(e) => setDate(e.target.value)}
            />
            <Btn size="sm" onClick={() => setDate(addDays(date, 1))}>
              →
            </Btn>
            <Btn size="sm" variant="ghost" onClick={() => setDate(todayISO())}>
              Today
            </Btn>
          </div>
        }
      />

      <Card tight className="no-print">
        <Tabs
          value={tab}
          onChange={(t) => setTab(t as 'day' | 'history')}
          tabs={[
            { id: 'day', label: data ? `${fmtDate(date)} · ${uncovered} uncovered` : 'Day view' },
            { id: 'history', label: 'Substitution history' },
          ]}
        />
      </Card>

      {tab === 'day' && (
        <>
          {cover.isLoading && <Loading label="Checking absences…" />}
          {data && !data.is_school_day && (
            <Card>
              <EmptyState
                icon="🏖️"
                title={`No school on ${fmtDate(date)}`}
                desc="This date is outside the school week — no lessons need cover."
              />
            </Card>
          )}
          {data && data.is_school_day && (
            <>
              <Card
                title={`Absences on ${fmtDate(date)}`}
                actions={
                  <div className="row tight">
                    {data.is_holiday && <Badge tone="err">Holiday</Badge>}
                    {data.events.map((e) => (
                      <Badge key={e.id} tone="info">
                        {e.title}
                      </Badge>
                    ))}
                  </div>
                }
              >
                {data.absences.length === 0 ? (
                  <div className="row">
                    <Badge tone="ok">✅ Every teacher is present</Badge>
                    <span className="small muted">No absences recorded for this day.</span>
                  </div>
                ) : (
                  <div className="row">
                    {data.absences.map((a) => (
                      <span className="chip" key={a.leave_id}>
                        <span className="avatar" style={{ width: 22, height: 22, fontSize: 10 }}>
                          {a.teacher_initials}
                        </span>
                        {a.teacher_name}
                        <Badge tone="warn">{a.part === 'full' ? 'all day' : a.part}</Badge>
                      </span>
                    ))}
                  </div>
                )}
              </Card>

              {data.periods.length === 0 ? (
                <Card>
                  <EmptyState
                    icon="✨"
                    title="Nothing to cover"
                    desc="No absent teacher has lessons scheduled on this day."
                  />
                </Card>
              ) : (
                <div className="grid cols-2">
                  {data.periods.map((p) => (
                    <div className="cover-period" key={`${p.lesson_id}-${p.day}-${p.idx}`}>
                      <div className="cover-head">
                        <Badge tone="info">P{p.idx}</Badge>
                        <div className="grow">
                          <div className="p-title" style={{ color: p.subject_color }}>
                            {p.subject_name} · {p.class_name}
                          </div>
                          <div className="small muted">
                            {p.absent_teacher_name} is away
                            {p.absent_part !== 'full' ? ` (${p.absent_part})` : ''}
                            {p.room_name ? ` · ${p.room_name}` : ''}
                          </div>
                        </div>
                        {p.substitution ? (
                          <Badge tone="ok">covered</Badge>
                        ) : (
                          <Badge tone="warn">needs cover</Badge>
                        )}
                      </div>

                      {p.substitution ? (
                        <div className="cand" style={{ background: 'var(--ok-soft)', borderColor: '#a7f3d0' }}>
                          <span className="avatar">{p.substitution.substitute_initials}</span>
                          <div className="grow">
                            <div className="c-name">{p.substitution.substitute_name}</div>
                            <div className="c-sub">covering this lesson</div>
                          </div>
                          {isStaff && (
                            <ConfirmButton
                              message={`Cancel ${p.substitution.substitute_name}'s substitution?`}
                              onConfirm={() => cancelSub.mutate(p.substitution!.id)}
                            >
                              Cancel
                            </ConfirmButton>
                          )}
                        </div>
                      ) : (
                        <>
                          {p.candidates.slice(0, 4).map((c) => (
                            <div className="cand" key={c.teacher_id}>
                              <span className="avatar">{c.initials || initialsOf(c.name)}</span>
                              <div className="grow">
                                <div className="c-name">{c.name}</div>
                                <div className="c-sub">
                                  {c.reason ?? 'available'}
                                  {c.daily_load > 0 ? ` · ${plural(c.daily_load, 'period')} today` : ' · free all day'}
                                  {c.cover_count > 0 ? ` · covered ${c.cover_count}× this cycle` : ''}
                                </div>
                              </div>
                              {c.teaches_subject && <Badge tone="ok">subject expert</Badge>}
                              <Badge tone={c.score > 100 ? 'info' : 'default'}>{Math.round(c.score)}</Badge>
                              {isStaff && (
                                <Btn
                                  size="sm"
                                  variant="primary"
                                  onClick={() =>
                                    assign.mutate({
                                      lesson_id: p.lesson_id,
                                      day: p.day,
                                      idx: p.idx,
                                      substitute_teacher_id: c.teacher_id,
                                    })
                                  }
                                >
                                  Assign
                                </Btn>
                              )}
                            </div>
                          ))}

                          {isStaff && (
                            <div className="inline-form">
                              <Select
                                className="sm"
                                defaultValue=""
                                onChange={(e) => {
                                  if (!e.target.value) return
                                  assign.mutate({
                                    lesson_id: p.lesson_id,
                                    day: p.day,
                                    idx: p.idx,
                                    substitute_teacher_id: e.target.value,
                                  })
                                  e.target.value = ''
                                }}
                              >
                                <option value="">Assign someone else…</option>
                                {(setup.data?.teachers ?? [])
                                  .filter(
                                    (t) =>
                                      t.id !== p.absent_teacher_id &&
                                      !p.candidates.some((c) => c.teacher_id === t.id),
                                  )
                                  .map((t) => (
                                    <option key={t.id} value={t.id}>
                                      {t.name}
                                    </option>
                                  ))}
                              </Select>
                            </div>
                          )}

                          {p.blocked.length > 0 && (
                            <details>
                              <summary className="blocked-list" style={{ cursor: 'pointer' }}>
                                {plural(p.blocked.length, 'teacher')} unavailable
                              </summary>
                              <div className="blocked-list" style={{ marginTop: 6, display: 'grid', gap: 2 }}>
                                {p.blocked.map((b) => (
                                  <div key={b.teacher_id}>
                                    {b.name} — {b.reason}
                                  </div>
                                ))}
                              </div>
                            </details>
                          )}
                        </>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </>
          )}
        </>
      )}

      {tab === 'history' && (
        <Card title="Substitution history" tight>
          {history.isLoading ? (
            <Loading label="Loading history…" />
          ) : (history.data?.items.length ?? 0) === 0 ? (
            <EmptyState
              icon="🔁"
              title="No substitutions yet"
              desc="Assigned cover appears here with the absent teacher and their replacement."
            />
          ) : (
            <div className="table-wrap">
              <table className="tbl">
                <thead>
                  <tr>
                    <th>Date</th>
                    <th>Period</th>
                    <th>Class</th>
                    <th>Subject</th>
                    <th>Absent</th>
                    <th>Substitute</th>
                    <th>Status</th>
                    <th style={{ textAlign: 'right' }}>Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {history.data!.items.map((h) => (
                    <tr key={h.id}>
                      <td className="nowrap">{fmtDate(h.date)}</td>
                      <td>P{h.idx}</td>
                      <td>{h.class_name}</td>
                      <td>{h.subject_name}</td>
                      <td className="muted">{h.absent_teacher_name}</td>
                      <td>
                        <strong>{h.substitute_name}</strong>
                      </td>
                      <td>
                        <Badge tone={h.status === 'assigned' ? 'ok' : 'default'}>{h.status}</Badge>
                      </td>
                      <td>
                        <div className="actions">
                          {isStaff && h.status === 'assigned' && (
                            <ConfirmButton message="Cancel this substitution?" onConfirm={() => cancelSub.mutate(h.id)}>
                              Cancel
                            </ConfirmButton>
                          )}
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </Card>
      )}
    </Page>
  )
}
