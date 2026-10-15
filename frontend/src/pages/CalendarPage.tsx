import { useMemo, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api, EVENT_KINDS } from '../api'
import type { CalendarEvent } from '../api'
import { useAuth } from '../auth'
import { Page, PageHead } from '../components/Layout'
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
  fmtDate,
  monthLabel,
  parseISO,
  todayISO,
  toast,
} from '../ui'

const KIND_STYLE: Record<string, { bg: string; fg: string; label: string }> = {
  holiday: { bg: '#fee2e2', fg: '#b91c1c', label: 'Holiday' },
  exam: { bg: '#fef3c7', fg: '#b45309', label: 'Exam' },
  activity: { bg: '#eef2ff', fg: '#4338ca', label: 'Activity' },
  other: { bg: '#e2e8f0', fg: '#334155', label: 'Other' },
}

export default function CalendarPage() {
  const { isStaff } = useAuth()
  const qc = useQueryClient()
  const today = todayISO()
  const now = parseISO(today)
  const [ym, setYm] = useState({ y: now.getFullYear(), m: now.getMonth() })
  const [editing, setEditing] = useState<CalendarEvent | null>(null)
  const [draftDate, setDraftDate] = useState<string | null>(null)

  const events = useQuery({
    queryKey: ['calendar'],
    queryFn: () => api<{ items: CalendarEvent[] }>('/api/calendar'),
  })

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ['calendar'] })
    void qc.invalidateQueries({ queryKey: ['dashboard'] })
    void qc.invalidateQueries({ queryKey: ['cover'] })
  }

  const items = events.data?.items ?? []

  const weeks = useMemo(() => {
    const first = new Date(ym.y, ym.m, 1)
    const offset = (first.getDay() + 6) % 7 // Monday-first
    const start = new Date(ym.y, ym.m, 1 - offset)
    const out: string[][] = []
    for (let w = 0; w < 6; w++) {
      const row: string[] = []
      for (let d = 0; d < 7; d++) {
        const cur = new Date(start.getFullYear(), start.getMonth(), start.getDate() + w * 7 + d)
        const mm = String(cur.getMonth() + 1).padStart(2, '0')
        const dd = String(cur.getDate()).padStart(2, '0')
        row.push(`${cur.getFullYear()}-${mm}-${dd}`)
      }
      out.push(row)
    }
    return out
  }, [ym])

  const eventsOn = (date: string) =>
    items.filter((e) => e.start_date <= date && date <= (e.end_date || e.start_date))

  const shift = (delta: number) => {
    const d = new Date(ym.y, ym.m + delta, 1)
    setYm({ y: d.getFullYear(), m: d.getMonth() })
  }

  return (
    <Page>
      <PageHead
        title="Calendar"
        sub="Holidays, exams and school activities — the solver avoids placing lessons on closures."
        actions={
          isStaff ? (
            <Btn variant="primary" onClick={() => setDraftDate(today)}>
              + Add event
            </Btn>
          ) : undefined
        }
      />

      <Card tight>
        <div className="card-head">
          <Btn size="sm" onClick={() => shift(-1)}>
            ←
          </Btn>
          <h2 style={{ textAlign: 'center' }}>{monthLabel(ym.y, ym.m)}</h2>
          <Btn size="sm" onClick={() => shift(1)}>
            →
          </Btn>
          <div className="topbar-spacer" />
          <div className="row tight">
            {Object.entries(KIND_STYLE).map(([k, v]) => (
              <span key={k} className="chip" style={{ background: v.bg, color: v.fg, borderColor: 'transparent' }}>
                {v.label}
              </span>
            ))}
          </div>
        </div>

        {events.isLoading ? (
          <Loading label="Loading calendar…" />
        ) : (
          <>
            <div className="cal-head">
              {['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'].map((d) => (
                <div key={d}>{d}</div>
              ))}
            </div>
            <div className="cal-grid">
              {weeks.flat().map((date) => {
                const d = parseISO(date)
                const outside = d.getMonth() !== ym.m
                const dayEvents = eventsOn(date)
                return (
                  <div
                    key={date}
                    className={`cal-cell ${outside ? 'outside' : ''} ${date === today ? 'today' : ''}`}
                    onDoubleClick={() => isStaff && setDraftDate(date)}
                    title={isStaff ? 'Double-click to add an event' : undefined}
                  >
                    <div className="cal-daynum">{d.getDate()}</div>
                    {dayEvents.map((e) => {
                      const st = KIND_STYLE[e.kind] ?? KIND_STYLE.other
                      return (
                        <div
                          key={e.id + date}
                          className="cal-event"
                          style={{ background: st.bg, color: st.fg }}
                          onClick={() => isStaff && setEditing(e)}
                        >
                          {e.title}
                        </div>
                      )
                    })}
                  </div>
                )
              })}
            </div>
          </>
        )}
      </Card>

      <Card title={`Events (${items.length})`} tight>
        {items.length === 0 ? (
          <EmptyState
            icon="📅"
            title="No calendar events"
            desc="Add holidays so lessons are not scheduled on closed days."
          />
        ) : (
          <div className="table-wrap">
            <table className="tbl">
              <thead>
                <tr>
                  <th>Event</th>
                  <th>Dates</th>
                  <th>Type</th>
                  <th>Notes</th>
                  {isStaff && <th style={{ textAlign: 'right' }}>Actions</th>}
                </tr>
              </thead>
              <tbody>
                {[...items]
                  .sort((a, b) => a.start_date.localeCompare(b.start_date))
                  .map((e) => (
                    <tr key={e.id}>
                      <td>
                        <strong>{e.title}</strong>
                      </td>
                      <td className="nowrap">
                        {fmtDate(e.start_date)}
                        {e.end_date !== e.start_date && <> → {fmtDate(e.end_date)}</>}
                      </td>
                      <td>
                        <Badge tone={e.kind === 'holiday' ? 'err' : e.kind === 'exam' ? 'warn' : 'info'}>
                          {KIND_STYLE[e.kind]?.label ?? e.kind}
                        </Badge>
                      </td>
                      <td className="muted small">{e.notes || '—'}</td>
                      {isStaff && (
                        <td>
                          <div className="actions">
                            <Btn size="sm" onClick={() => setEditing(e)}>
                              Edit
                            </Btn>
                            <ConfirmButton
                              message={`Delete "${e.title}"?`}
                              onConfirm={async () => {
                                await api(`/api/calendar/${e.id}`, { method: 'DELETE' })
                                refresh()
                                toast('Event deleted', 'ok')
                              }}
                            >
                              Delete
                            </ConfirmButton>
                          </div>
                        </td>
                      )}
                    </tr>
                  ))}
              </tbody>
            </table>
          </div>
        )}
      </Card>

      {(editing || draftDate) && (
        <EventModal
          event={editing}
          defaultDate={draftDate ?? today}
          onClose={() => {
            setEditing(null)
            setDraftDate(null)
          }}
          onSaved={refresh}
        />
      )}
    </Page>
  )
}

function useSaveEvent() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (p: { id?: string; body: unknown }) =>
      p.id
        ? api(`/api/calendar/${p.id}`, { method: 'PUT', body: p.body })
        : api('/api/calendar', { body: p.body }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['calendar'] })
      void qc.invalidateQueries({ queryKey: ['dashboard'] })
      void qc.invalidateQueries({ queryKey: ['cover'] })
    },
    onError: (e) => toast(errMsg(e), 'err'),
  })
}

function EventModal({
  event,
  defaultDate,
  onClose,
  onSaved,
}: {
  event: CalendarEvent | null
  defaultDate: string
  onClose: () => void
  onSaved: () => void
}) {
  const save = useSaveEvent()
  const [form, setForm] = useState({
    title: event?.title ?? '',
    start_date: event?.start_date ?? defaultDate,
    end_date: event?.end_date ?? defaultDate,
    kind: event?.kind ?? 'activity',
    notes: event?.notes ?? '',
  })

  const submit = async () => {
    if (!form.title.trim()) return toast('Give the event a title', 'err')
    const body = { ...form, end_date: form.end_date || form.start_date, notes: form.notes || undefined }
    await save.mutateAsync({ id: event?.id, body })
    toast(event ? 'Event updated' : 'Event added', 'ok')
    onSaved()
    onClose()
  }

  return (
    <Modal
      open
      onClose={onClose}
      title={event ? `Edit ${event.title}` : 'Add calendar event'}
      footer={
        <>
          <Btn onClick={onClose}>Cancel</Btn>
          <Btn variant="primary" onClick={submit} disabled={save.isPending}>
            {event ? 'Save changes' : 'Add event'}
          </Btn>
        </>
      }
    >
      <Field label="Title">
        <Input
          value={form.title}
          onChange={(e) => setForm({ ...form, title: e.target.value })}
          placeholder="Sports Day"
        />
      </Field>
      <div className="grid cols-2" style={{ gap: 0 }}>
        <Field label="Start">
          <Input
            type="date"
            value={form.start_date}
            onChange={(e) => setForm({ ...form, start_date: e.target.value, end_date: form.end_date || e.target.value })}
          />
        </Field>
        <Field label="End">
          <Input type="date" value={form.end_date} onChange={(e) => setForm({ ...form, end_date: e.target.value })} />
        </Field>
      </div>
      <Field label="Type" hint="Holidays close the school: the solver treats them as non-teaching days.">
        <Select value={form.kind} onChange={(e) => setForm({ ...form, kind: e.target.value })}>
          {EVENT_KINDS.map((k) => (
            <option key={k.value} value={k.value}>
              {k.label}
            </option>
          ))}
        </Select>
      </Field>
      <Field label="Notes (optional)">
        <Input value={form.notes} onChange={(e) => setForm({ ...form, notes: e.target.value })} />
      </Field>
      <div className="small faint">Tip: double-click any day in the calendar to add an event there.</div>
    </Modal>
  )
}
