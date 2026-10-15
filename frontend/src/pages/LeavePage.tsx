import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api, LEAVE_PARTS } from '../api'
import type { LeaveImpact, LeaveItem, SetupAll } from '../api'
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
  Tabs,
  errMsg,
  fmtDate,
  plural,
  todayISO,
  toast,
} from '../ui'

const STATUS_TONE: Record<string, 'ok' | 'warn' | 'err' | 'default'> = {
  approved: 'ok',
  pending: 'warn',
  rejected: 'err',
  cancelled: 'default',
}

export default function LeavePage() {
  const { user, isStaff } = useAuth()
  const qc = useQueryClient()
  const [status, setStatus] = useState('')
  const [showCreate, setShowCreate] = useState(false)
  const [impactOf, setImpactOf] = useState<LeaveItem | null>(null)

  const leave = useQuery({
    queryKey: ['leave', status],
    queryFn: () => api<{ items: LeaveItem[] }>(`/api/leave${status ? `?status=${status}` : ''}`),
  })
  const setup = useQuery({ queryKey: ['setup'], queryFn: () => api<SetupAll>('/api/setup/all') })

  const refresh = () => {
    void qc.invalidateQueries({ queryKey: ['leave'] })
    void qc.invalidateQueries({ queryKey: ['dashboard'] })
    void qc.invalidateQueries({ queryKey: ['cover'] })
  }

  const decide = useMutation({
    mutationFn: (p: { id: string; approve: boolean }) =>
      api(`/api/leave/${p.id}/decide`, { body: { approve: p.approve } }),
    onSuccess: (_d, v) => {
      refresh()
      toast(v.approve ? 'Leave approved — cover suggestions updated' : 'Leave rejected', 'ok')
    },
    onError: (e) => toast(errMsg(e), 'err'),
  })

  const cancel = useMutation({
    mutationFn: (id: string) => api(`/api/leave/${id}/cancel`, { body: {} }),
    onSuccess: () => {
      refresh()
      toast('Leave cancelled', 'ok')
    },
    onError: (e) => toast(errMsg(e), 'err'),
  })

  const items = leave.data?.items ?? []
  const myTeacherId = user?.teacher_id ?? null

  return (
    <Page>
      <PageHead
        title="Leave"
        sub={
          isStaff
            ? 'Approve absences — approved leave automatically appears in Cover with substitute suggestions.'
            : 'Your leave requests and their status.'
        }
        actions={
          <Btn variant="primary" onClick={() => setShowCreate(true)}>
            + Request leave
          </Btn>
        }
      />

      <Card className="no-print" tight>
        <div className="card-body row">
          <Tabs
            value={status}
            onChange={setStatus}
            tabs={[
              { id: '', label: 'All' },
              { id: 'pending', label: 'Pending' },
              { id: 'approved', label: 'Approved' },
              { id: 'rejected', label: 'Rejected' },
            ]}
          />
        </div>
      </Card>

      <Card title={`${plural(items.length, 'request')}`} tight>
        {leave.isLoading ? (
          <Loading label="Loading leave requests…" />
        ) : items.length === 0 ? (
          <EmptyState
            icon="🌴"
            title="No leave requests"
            desc="Teachers can request leave here; approved absences flow straight into cover planning."
          />
        ) : (
          <div className="table-wrap">
            <table className="tbl">
              <thead>
                <tr>
                  <th>Teacher</th>
                  <th>Dates</th>
                  <th>Part</th>
                  <th className="num">Days</th>
                  <th>Reason</th>
                  <th>Status</th>
                  <th style={{ textAlign: 'right' }}>Actions</th>
                </tr>
              </thead>
              <tbody>
                {items.map((l) => (
                  <tr key={l.id}>
                    <td>
                      <div className="row tight">
                        <span className="avatar">{l.teacher_initials}</span>
                        <strong>{l.teacher_name}</strong>
                      </div>
                    </td>
                    <td className="nowrap">
                      {fmtDate(l.start_date)}
                      {l.end_date !== l.start_date && <> → {fmtDate(l.end_date)}</>}
                    </td>
                    <td>
                      <Badge>{LEAVE_PARTS.find((p) => p.value === l.part)?.label ?? l.part}</Badge>
                    </td>
                    <td className="num">{l.days}</td>
                    <td className="muted small">{l.reason || '—'}</td>
                    <td>
                      <Badge tone={STATUS_TONE[l.status] ?? 'default'}>{l.status}</Badge>
                    </td>
                    <td>
                      <div className="actions">
                        <Btn size="sm" onClick={() => setImpactOf(l)}>
                          Impact
                        </Btn>
                        {isStaff && l.status === 'pending' && (
                          <>
                            <Btn size="sm" variant="primary" onClick={() => decide.mutate({ id: l.id, approve: true })}>
                              Approve
                            </Btn>
                            <Btn size="sm" onClick={() => decide.mutate({ id: l.id, approve: false })}>
                              Reject
                            </Btn>
                          </>
                        )}
                        {((isStaff && (l.status === 'pending' || l.status === 'approved')) ||
                          (!isStaff && myTeacherId === l.teacher_id && l.status === 'pending')) && (
                          <ConfirmButton message="Cancel this leave request?" onConfirm={() => cancel.mutate(l.id)}>
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

      <RequestModal
        open={showCreate}
        onClose={() => setShowCreate(false)}
        setup={setup.data}
        isStaff={isStaff}
        selfTeacherId={myTeacherId}
        onDone={refresh}
      />

      <ImpactModal leave={impactOf} onClose={() => setImpactOf(null)} />
    </Page>
  )
}

function RequestModal({
  open,
  onClose,
  setup,
  isStaff,
  selfTeacherId,
  onDone,
}: {
  open: boolean
  onClose: () => void
  setup: SetupAll | undefined
  isStaff: boolean
  selfTeacherId: string | null
  onDone: () => void
}) {
  const today = todayISO()
  const [teacherId, setTeacherId] = useState(selfTeacherId ?? '')
  const [start, setStart] = useState(today)
  const [end, setEnd] = useState(today)
  const [part, setPart] = useState('full')
  const [reason, setReason] = useState('')
  const [busy, setBusy] = useState(false)

  const submit = async () => {
    setBusy(true)
    try {
      await api('/api/leave', {
        body: {
          teacher_id: teacherId || undefined,
          start_date: start,
          end_date: end,
          part,
          reason: reason || undefined,
        },
      })
      toast('Leave request submitted', 'ok')
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
      title="Request leave"
      footer={
        <>
          <Btn onClick={onClose}>Cancel</Btn>
          <Btn variant="primary" onClick={submit} disabled={busy}>
            Submit request
          </Btn>
        </>
      }
    >
      {isStaff && (
        <Field label="Teacher" hint="Staff can record leave on behalf of a teacher; it is approved automatically.">
          <Select value={teacherId} onChange={(e) => setTeacherId(e.target.value)}>
            <option value="">Select teacher…</option>
            {(setup?.teachers ?? []).map((t) => (
              <option key={t.id} value={t.id}>
                {t.name}
              </option>
            ))}
          </Select>
        </Field>
      )}
      <div className="grid cols-2" style={{ gap: 0 }}>
        <Field label="From">
          <Input type="date" value={start} onChange={(e) => setStart(e.target.value)} />
        </Field>
        <Field label="To">
          <Input type="date" value={end} onChange={(e) => setEnd(e.target.value)} />
        </Field>
      </div>
      <Field label="Which part of the day?">
        <Select value={part} onChange={(e) => setPart(e.target.value)}>
          {LEAVE_PARTS.map((p) => (
            <option key={p.value} value={p.value}>
              {p.label}
            </option>
          ))}
        </Select>
      </Field>
      <Field label="Reason (optional)">
        <Input value={reason} onChange={(e) => setReason(e.target.value)} placeholder="Medical appointment" />
      </Field>
    </Modal>
  )
}

function ImpactModal({ leave, onClose }: { leave: LeaveItem | null; onClose: () => void }) {
  const { data, isLoading } = useQuery({
    queryKey: ['leave-impact', leave?.id],
    enabled: leave !== null,
    queryFn: () => api<LeaveImpact>(`/api/leave/${leave!.id}/impact`),
  })

  if (!leave) return null

  return (
    <Modal
      open
      onClose={onClose}
      wide
      title={`Impact of ${leave.teacher_name}'s leave`}
      footer={<Btn onClick={onClose}>Close</Btn>}
    >
      {isLoading || !data ? (
        <Loading label="Checking the timetable…" />
      ) : data.sessions.length === 0 ? (
        <EmptyState
          icon="✅"
          title="No lessons affected"
          desc="This teacher teaches nothing during the requested period."
        />
      ) : (
        <>
          <p className="muted small" style={{ marginBottom: 12 }}>
            {plural(data.sessions.length, 'lesson')} would need cover while {leave.teacher_name} is away.
            Approving the leave makes them appear on the Cover page for the affected days.
          </p>
          <div className="table-wrap">
            <table className="tbl">
              <thead>
                <tr>
                  <th>Date</th>
                  <th>Period</th>
                  <th>Class</th>
                  <th>Subject</th>
                  <th>Room</th>
                </tr>
              </thead>
              <tbody>
                {data.sessions.map((s, i) => (
                  <tr key={i}>
                    <td className="nowrap">{fmtDate(s.date)}</td>
                    <td>
                      P{s.idx} · {s.start_time}
                    </td>
                    <td>{s.class_name}</td>
                    <td>{s.subject_name}</td>
                    <td className="muted">{s.room_name ?? '—'}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      )}
    </Modal>
  )
}
