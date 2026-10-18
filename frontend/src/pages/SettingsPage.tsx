import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api, setToken } from '../api'
import type { AuditItem, NotifyStatus, School, SetupAll, UserRow } from '../api'
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
  initialsOf,
  plural,
  toast,
} from '../ui'

export default function SettingsPage() {
  const { user, school, isAdmin, reload } = useAuth()
  const qc = useQueryClient()

  const [schoolForm, setSchoolForm] = useState<{ name: string; cycle_weeks: number } | null>(null)
  const [editing, setEditing] = useState<UserRow | null>(null)
  const [creating, setCreating] = useState(false)

  const users = useQuery({ queryKey: ['users'], queryFn: () => api<{ items: UserRow[] }>('/api/users') })
  const setup = useQuery({ queryKey: ['setup'], queryFn: () => api<SetupAll>('/api/setup/all') })
  const audit = useQuery({
    queryKey: ['audit'],
    queryFn: () => api<{ items: AuditItem[] }>('/api/audit?limit=50'),
    enabled: isAdmin,
  })
  const notify = useQuery({
    queryKey: ['notify-status'],
    queryFn: () => api<NotifyStatus>('/api/notifications/status'),
    enabled: isAdmin,
  })

  const testNotify = useMutation({
    mutationFn: () => api<{ delivered: string[]; enabled: boolean }>('/api/notifications/test', { body: {} }),
    onSuccess: (d) =>
      d.enabled && d.delivered.length > 0
        ? toast(`Test notification sent via ${d.delivered.join(', ')}`, 'ok')
        : toast('No notification channel is configured yet', 'info'),
    onError: (e) => toast(errMsg(e), 'err'),
  })

  const saveSchool = useMutation({
    mutationFn: (body: { name: string; cycle_weeks: number }) => api<School>('/api/school', { method: 'PUT', body }),
    onSuccess: () => {
      toast('School settings saved', 'ok')
      void reload()
      void qc.invalidateQueries()
    },
    onError: (e) => toast(errMsg(e), 'err'),
  })

  const resetDemo = useMutation({
    mutationFn: () => api<{ token: string }>('/api/demo/reset', { body: {} }),
    onSuccess: async (d) => {
      setToken(d.token)
      await reload()
      void qc.invalidateQueries()
      toast('Demo school rebuilt with fresh data', 'ok')
    },
    onError: (e) => toast(errMsg(e), 'err'),
  })

  const current = schoolForm ?? { name: school?.name ?? '', cycle_weeks: school?.cycle_weeks ?? 1 }

  return (
    <Page>
      <PageHead title="Settings" sub="School configuration, staff accounts and workspace tools." />

      <div className="grid cols-2">
        <Card title="School">
          <Field label="School name">
            <Input
              value={current.name}
              disabled={!isAdmin && user?.role !== 'scheduler'}
              onChange={(e) => setSchoolForm({ ...current, name: e.target.value })}
            />
          </Field>
          <Field
            label="Timetable cycle"
            hint="Two-week cycles are useful for schools that alternate subjects between weeks."
          >
            <Select
              value={current.cycle_weeks}
              disabled={!isAdmin && user?.role !== 'scheduler'}
              onChange={(e) => setSchoolForm({ ...current, cycle_weeks: Number(e.target.value) })}
            >
              <option value={1}>Single week</option>
              <option value={2}>Two-week cycle (A/B weeks)</option>
            </Select>
          </Field>
          <Btn
            variant="primary"
            disabled={!schoolForm || saveSchool.isPending}
            onClick={() => saveSchool.mutate(current)}
          >
            Save school settings
          </Btn>
        </Card>

        <Card title="Your account">
          <div className="row" style={{ marginBottom: 10 }}>
            <span className="avatar" style={{ width: 40, height: 40, fontSize: 15 }}>
              {initialsOf(user?.name ?? '')}
            </span>
            <div>
              <div style={{ fontWeight: 650 }}>{user?.name}</div>
              <div className="small muted">{user?.email}</div>
            </div>
            <div className="right">
              <Badge tone="info">{user?.role}</Badge>
            </div>
          </div>
          <div className="small muted">
            {user?.role === 'admin'
              ? 'Administrators manage staff accounts, school settings and demo data.'
              : user?.role === 'scheduler'
                ? 'Schedulers build and maintain the timetable and arrange cover.'
                : user?.role === 'teacher'
                  ? 'Teachers see their own timetable, request leave and cover colleagues.'
                  : 'Students see their class timetable and the school calendar.'}
          </div>
        </Card>
      </div>

      {isAdmin && (
        <Card
          title={`Staff accounts (${users.data?.items.length ?? 0})`}
          actions={
            <Btn variant="primary" size="sm" onClick={() => setCreating(true)}>
              + Add account
            </Btn>
          }
          tight
        >
          {users.isLoading ? (
            <Loading label="Loading accounts…" />
          ) : (
            <div className="table-wrap">
              <table className="tbl">
                <thead>
                  <tr>
                    <th>Name</th>
                    <th>Email</th>
                    <th>Role</th>
                    <th>Linked to</th>
                    <th style={{ textAlign: 'right' }}>Actions</th>
                  </tr>
                </thead>
                <tbody>
                  {users.data?.items.map((u) => (
                    <tr key={u.id}>
                      <td>
                        <div className="row tight">
                          <span className="avatar">{initialsOf(u.name)}</span>
                          <strong>{u.name}</strong>
                          {u.id === user?.id && <Badge tone="info">you</Badge>}
                        </div>
                      </td>
                      <td className="muted">{u.email}</td>
                      <td>
                        <Badge tone={u.role === 'admin' ? 'info' : u.role === 'scheduler' ? 'warn' : 'default'}>
                          {u.role}
                        </Badge>
                      </td>
                      <td className="small">
                        {u.teacher_name ? `Teacher · ${u.teacher_name}` : u.class_name ? `Student · ${u.class_name}` : '—'}
                      </td>
                      <td>
                        <div className="actions">
                          <Btn size="sm" onClick={() => setEditing(u)}>
                            Edit
                          </Btn>
                          <ConfirmButton
                            message={`Delete the account for ${u.name}?`}
                            disabled={u.id === user?.id}
                            onConfirm={async () => {
                              await api(`/api/users/${u.id}`, { method: 'DELETE' })
                              void qc.invalidateQueries({ queryKey: ['users'] })
                              toast('Account deleted', 'ok')
                            }}
                          >
                            Delete
                          </ConfirmButton>
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

      {isAdmin && (
        <Card title="Demo workspace">
          <div className="row">
            <div className="grow small muted">
              Rebuild the public demo school (teachers, classes, timetable, leave and cover examples) with fresh
              dates. Everyone signed into the demo account is signed out.
            </div>
            <ConfirmButton
              size="md"
              variant="danger"
              message="Rebuild the demo workspace from scratch? Existing demo data is erased."
              onConfirm={() => resetDemo.mutate()}
            >
              ♻️ Reset demo data
            </ConfirmButton>
          </div>
        </Card>
      )}

      {isAdmin && (
        <Card title="Notifications">
          <div className="row" style={{ marginBottom: 10 }}>
            {notify.data?.enabled ? (
              <Badge tone="ok">enabled</Badge>
            ) : (
              <Badge tone="warn">not configured</Badge>
            )}
            {(notify.data?.channels ?? []).map((c) => (
              <Badge key={c} tone="info">
                {c}
              </Badge>
            ))}
          </div>
          <p className="small muted" style={{ marginBottom: 10 }}>
            Notify teachers when they are assigned cover, and staff when leave is requested or
            decided. Enable a channel by setting <code>NOTIFY_WEBHOOK_URL</code> (any webhook) or{' '}
            <code>KNOCK_API_KEY</code> (email via Knock) on the server, then restart it.
          </p>
          {notify.data && (
            <div className="row tight" style={{ marginBottom: 10 }}>
              <span className="small faint">Events:</span>
              {notify.data.events.map((e) => (
                <Badge key={e.id}>{e.label}</Badge>
              ))}
            </div>
          )}
          <Btn
            size="sm"
            variant="primary"
            disabled={testNotify.isPending}
            onClick={() => testNotify.mutate()}
          >
            🔔 Send test notification
          </Btn>
        </Card>
      )}

      {isAdmin && (
        <Card title="Activity log" tight>
          {audit.isLoading ? (
            <Loading label="Loading activity…" />
          ) : (audit.data?.items.length ?? 0) === 0 ? (
            <EmptyState
              icon="🧾"
              title="No activity yet"
              desc="Timetable, cover, leave and account changes are recorded here."
            />
          ) : (
            <div className="table-wrap">
              <table className="tbl">
                <thead>
                  <tr>
                    <th>When</th>
                    <th>Who</th>
                    <th>Action</th>
                    <th>Details</th>
                  </tr>
                </thead>
                <tbody>
                  {audit.data!.items.map((a) => (
                    <tr key={a.id}>
                      <td className="nowrap muted small">
                        {new Date(a.created_at).toLocaleString()}
                      </td>
                      <td>{a.actor_name}</td>
                      <td>
                        <Badge tone="info">{a.action}</Badge>
                      </td>
                      <td className="small">{a.summary}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </Card>
      )}

      {!isAdmin && (
        <Card>
          <EmptyState
            icon="🔒"
            title="Administrator area"
            desc="Only administrators can manage staff accounts and rebuild demo data."
          />
        </Card>
      )}

      {(creating || editing) && (
        <UserModal
          row={editing}
          setup={setup.data}
          onClose={() => {
            setCreating(false)
            setEditing(null)
          }}
          onSaved={() => void qc.invalidateQueries({ queryKey: ['users'] })}
        />
      )}
    </Page>
  )
}

function UserModal({
  row,
  setup,
  onClose,
  onSaved,
}: {
  row: UserRow | null
  setup: SetupAll | undefined
  onClose: () => void
  onSaved: () => void
}) {
  const [form, setForm] = useState({
    name: row?.name ?? '',
    email: row?.email ?? '',
    password: '',
    role: (row?.role ?? 'teacher') as string,
    teacher_id: row?.teacher_id ?? '',
    class_id: row?.class_id ?? '',
  })
  const [busy, setBusy] = useState(false)

  const submit = async () => {
    if (!form.name.trim() || !form.email.trim()) return toast('Name and email are required', 'err')
    if (!row && form.password.length < 6) return toast('Password must be at least 6 characters', 'err')
    setBusy(true)
    try {
      const body = {
        name: form.name,
        email: form.email,
        role: form.role,
        password: form.password || undefined,
        teacher_id: form.role === 'teacher' ? form.teacher_id || null : null,
        class_id: form.role === 'student' ? form.class_id || null : null,
      }
      if (row) await api(`/api/users/${row.id}`, { method: 'PUT', body })
      else await api('/api/users', { body })
      toast(row ? 'Account updated' : 'Account created', 'ok')
      onSaved()
      onClose()
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
      title={row ? `Edit ${row.name}` : 'Add staff account'}
      footer={
        <>
          <Btn onClick={onClose}>Cancel</Btn>
          <Btn variant="primary" onClick={submit} disabled={busy}>
            {row ? 'Save changes' : 'Create account'}
          </Btn>
        </>
      }
    >
      <Field label="Full name">
        <Input value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} />
      </Field>
      <Field label="Email">
        <Input type="email" value={form.email} onChange={(e) => setForm({ ...form, email: e.target.value })} />
      </Field>
      <Field label={row ? 'New password' : 'Password'} hint={row ? 'Leave empty to keep the current password.' : 'At least 6 characters.'}>
        <Input
          type="password"
          value={form.password}
          onChange={(e) => setForm({ ...form, password: e.target.value })}
        />
      </Field>
      <Field label="Role">
        <Select value={form.role} onChange={(e) => setForm({ ...form, role: e.target.value })}>
          <option value="admin">Administrator — full control</option>
          <option value="scheduler">Scheduler — build timetables, arrange cover</option>
          <option value="teacher">Teacher — own timetable, leave, cover</option>
          <option value="student">Student — own class timetable</option>
        </Select>
      </Field>
      {form.role === 'teacher' && (
        <Field label="Linked teacher record" hint="Connects the account to a teacher's timetable and absences.">
          <Select value={form.teacher_id} onChange={(e) => setForm({ ...form, teacher_id: e.target.value })}>
            <option value="">Not linked</option>
            {(setup?.teachers ?? []).map((t) => (
              <option key={t.id} value={t.id}>
                {t.name}
              </option>
            ))}
          </Select>
        </Field>
      )}
      {form.role === 'student' && (
        <Field label="Class">
          <Select value={form.class_id} onChange={(e) => setForm({ ...form, class_id: e.target.value })}>
            <option value="">Not linked</option>
            {(setup?.classes ?? []).map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </Select>
        </Field>
      )}
      <div className="small faint">{plural(setup?.teachers.length ?? 0, 'teacher')} available to link.</div>
    </Modal>
  )
}
