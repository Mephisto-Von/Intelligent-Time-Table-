import { useMemo, useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { api, ROOM_TYPES, DAY_NAMES } from '../api'
import type { BellSlotDraft, Lesson, SchoolClass, SetupAll, Slot, Subject, Teacher, Room, UnavailabilitySlot } from '../api'
import { Page, PageHead } from '../components/Layout'
import {
  Badge,
  Btn,
  Card,
  ConfirmButton,
  EmptyState,
  Field,
  Loading,
  Modal,
  Select,
  Input,
  Tabs,
  errMsg,
  initialsOf,
  toast,
} from '../ui'

// ------------------------------------------------------------------ helpers

function useSave() {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: (p: { path: string; method?: string; body?: unknown }) =>
      api(p.path, { method: p.method ?? (p.body !== undefined ? 'POST' : 'GET'), body: p.body }),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ['setup'] })
      void qc.invalidateQueries({ queryKey: ['dashboard'] })
      void qc.invalidateQueries({ queryKey: ['timetable'] })
    },
    onError: (e) => toast(errMsg(e), 'err'),
  })
}

export default function SetupPage() {
  const [tab, setTab] = useState('subjects')
  const { data, isLoading, error } = useQuery({
    queryKey: ['setup'],
    queryFn: () => api<SetupAll>('/api/setup/all'),
  })

  return (
    <Page>
      <PageHead
        title="School setup"
        sub="Subjects, teachers, classes, rooms, curriculum and the bell schedule."
        actions={<Badge tone="info">Changes apply on the next generation</Badge>}
      />
      {isLoading && <Loading label="Loading school data…" />}
      {error && (
        <Card>
          <EmptyState icon="⚠️" title="Could not load setup data" desc={String(error)} />
        </Card>
      )}
      {data && (
        <Card tight>
          <Tabs
            value={tab}
            onChange={setTab}
            tabs={[
              { id: 'subjects', label: `Subjects (${data.subjects.length})` },
              { id: 'teachers', label: `Teachers (${data.teachers.length})` },
              { id: 'classes', label: `Classes (${data.classes.length})` },
              { id: 'rooms', label: `Rooms (${data.rooms.length})` },
              { id: 'lessons', label: `Curriculum (${data.lessons.length})` },
              { id: 'bell', label: 'Bell schedule' },
            ]}
          />
          <div className="card-body">
            {tab === 'subjects' && <SubjectsTab data={data} />}
            {tab === 'teachers' && <TeachersTab data={data} />}
            {tab === 'classes' && <ClassesTab data={data} />}
            {tab === 'rooms' && <RoomsTab data={data} />}
            {tab === 'lessons' && <LessonsTab data={data} />}
            {tab === 'bell' && <BellTab data={data} />}
          </div>
        </Card>
      )}
    </Page>
  )
}

// ------------------------------------------------------------------ subjects

function SubjectsTab({ data }: { data: SetupAll }) {
  const save = useSave()
  const [editing, setEditing] = useState<Subject | null>(null)
  const [creating, setCreating] = useState(false)
  const [form, setForm] = useState({ name: '', code: '', color: '#4f46e5' })

  const open = (s?: Subject) => {
    setForm({ name: s?.name ?? '', code: s?.code ?? '', color: s?.color ?? '#4f46e5' })
    s ? setEditing(s) : setCreating(true)
  }

  const close = () => {
    setEditing(null)
    setCreating(false)
  }

  const submit = async () => {
    if (!form.name.trim() || !form.code.trim()) return toast('Name and code are required', 'err')
    if (editing) await save.mutateAsync({ path: `/api/subjects/${editing.id}`, method: 'PUT', body: form })
    else await save.mutateAsync({ path: '/api/subjects', body: form })
    toast(editing ? 'Subject updated' : 'Subject added', 'ok')
    close()
  }

  return (
    <>
      <div className="row" style={{ marginBottom: 12 }}>
        <Btn variant="primary" onClick={() => open()}>
          + Add subject
        </Btn>
      </div>
      {data.subjects.length === 0 ? (
        <EmptyState icon="📚" title="No subjects yet" desc="Add the subjects taught at your school." />
      ) : (
        <div className="table-wrap">
          <table className="tbl">
            <thead>
              <tr>
                <th>Subject</th>
                <th>Code</th>
                <th>Colour</th>
                <th>Lessons</th>
                <th style={{ textAlign: 'right' }}>Actions</th>
              </tr>
            </thead>
            <tbody>
              {data.subjects.map((s) => (
                <tr key={s.id}>
                  <td>
                    <span className="row tight">
                      <span className="dot" style={{ background: s.color }} />
                      <strong>{s.name}</strong>
                    </span>
                  </td>
                  <td className="mono">{s.code}</td>
                  <td className="mono">{s.color}</td>
                  <td className="num">{data.lessons.filter((l) => l.subject_id === s.id).length}</td>
                  <td>
                    <div className="actions">
                      <Btn size="sm" onClick={() => open(s)}>
                        Edit
                      </Btn>
                      <ConfirmButton
                        message={`Delete ${s.name} and all its lessons?`}
                        onConfirm={async () => {
                          await save.mutateAsync({ path: `/api/subjects/${s.id}`, method: 'DELETE' })
                          toast('Subject deleted', 'ok')
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

      <Modal
        open={creating || editing !== null}
        onClose={close}
        title={editing ? `Edit ${editing.name}` : 'Add subject'}
        footer={
          <>
            <Btn onClick={close}>Cancel</Btn>
            <Btn variant="primary" onClick={submit} disabled={save.isPending}>
              {editing ? 'Save changes' : 'Add subject'}
            </Btn>
          </>
        }
      >
        <Field label="Name">
          <Input value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} placeholder="Mathematics" />
        </Field>
        <Field label="Code" hint="Short code shown on the timetable grid.">
          <Input value={form.code} onChange={(e) => setForm({ ...form, code: e.target.value })} placeholder="MATH" />
        </Field>
        <Field label="Colour">
          <input type="color" value={form.color} onChange={(e) => setForm({ ...form, color: e.target.value })} />
        </Field>
      </Modal>
    </>
  )
}

// ------------------------------------------------------------------ teachers

function TeachersTab({ data }: { data: SetupAll }) {
  const save = useSave()
  const [editing, setEditing] = useState<Teacher | null>(null)
  const [creating, setCreating] = useState(false)
  const [form, setForm] = useState({
    name: '',
    initials: '',
    email: '',
    max_weekly: 30,
    max_daily: 8,
  })
  const [subjectIds, setSubjectIds] = useState<string[]>([])
  const [unavailable, setUnavailable] = useState<Set<string>>(new Set())

  const days = useMemo(() => [...new Set(data.slots.map((s) => s.day))].sort(), [data.slots])
  const periods = useMemo(
    () => [...new Set(data.slots.filter((s) => !s.is_break).map((s) => s.idx))].sort((a, b) => a - b),
    [data.slots],
  )

  const open = (t?: Teacher) => {
    setForm({
      name: t?.name ?? '',
      initials: t?.initials ?? '',
      email: t?.email ?? '',
      max_weekly: t?.max_weekly ?? 30,
      max_daily: t?.max_daily ?? 8,
    })
    setSubjectIds(t?.subject_ids ?? [])
    setUnavailable(new Set((t?.unavailable ?? []).map((u) => `${u.day}|${u.idx}`)))
    t ? setEditing(t) : setCreating(true)
  }

  const close = () => {
    setEditing(null)
    setCreating(false)
  }

  const submit = async () => {
    if (!form.name.trim()) return toast('Name is required', 'err')
    const body = {
      ...form,
      initials: form.initials.trim() || undefined,
      email: form.email.trim() || undefined,
      subject_ids: subjectIds,
      unavailable: [...unavailable].map((k) => {
        const [day, idx] = k.split('|').map(Number)
        return { day, idx } satisfies UnavailabilitySlot
      }),
    }
    if (editing) await save.mutateAsync({ path: `/api/teachers/${editing.id}`, method: 'PUT', body })
    else await save.mutateAsync({ path: '/api/teachers', body })
    toast(editing ? 'Teacher updated' : 'Teacher added', 'ok')
    close()
  }

  return (
    <>
      <div className="row" style={{ marginBottom: 12 }}>
        <Btn variant="primary" onClick={() => open()}>
          + Add teacher
        </Btn>
      </div>
      {data.teachers.length === 0 ? (
        <EmptyState icon="🧑‍🏫" title="No teachers yet" desc="Add teaching staff with their subjects and availability." />
      ) : (
        <div className="table-wrap">
          <table className="tbl">
            <thead>
              <tr>
                <th>Teacher</th>
                <th>Subjects</th>
                <th className="num">Lessons / week</th>
                <th className="num">Max / day</th>
                <th>Unavailable</th>
                <th style={{ textAlign: 'right' }}>Actions</th>
              </tr>
            </thead>
            <tbody>
              {data.teachers.map((t) => (
                <tr key={t.id}>
                  <td>
                    <div className="row tight">
                      <span className="avatar">{t.initials || initialsOf(t.name)}</span>
                      <span>
                        <div>
                          <strong>{t.name}</strong>
                        </div>
                        <div className="small faint">{t.email ?? '—'}</div>
                      </span>
                    </div>
                  </td>
                  <td>
                    <div className="row tight">
                      {(t.subject_ids ?? []).map((sid) => {
                        const s = data.subjects.find((x) => x.id === sid)
                        return s ? (
                          <span key={sid} className="chip" style={{ background: `${s.color}18`, borderColor: `${s.color}44` }}>
                            {s.code}
                          </span>
                        ) : null
                      })}
                      {(t.subject_ids ?? []).length === 0 && <span className="faint">—</span>}
                    </div>
                  </td>
                  <td className="num">{t.lesson_count ?? 0}</td>
                  <td className="num">{t.max_daily}</td>
                  <td className="num">{t.unavailable?.length ?? 0} periods</td>
                  <td>
                    <div className="actions">
                      <Btn size="sm" onClick={() => open(t)}>
                        Edit
                      </Btn>
                      <ConfirmButton
                        message={`Delete ${t.name}? Lessons assigned to this teacher will be removed.`}
                        onConfirm={async () => {
                          await save.mutateAsync({ path: `/api/teachers/${t.id}`, method: 'DELETE' })
                          toast('Teacher deleted', 'ok')
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

      <Modal
        open={creating || editing !== null}
        onClose={close}
        wide
        title={editing ? `Edit ${editing.name}` : 'Add teacher'}
        footer={
          <>
            <Btn onClick={close}>Cancel</Btn>
            <Btn variant="primary" onClick={submit} disabled={save.isPending}>
              {editing ? 'Save changes' : 'Add teacher'}
            </Btn>
          </>
        }
      >
        <div className="grid cols-2" style={{ gap: 0 }}>
          <Field label="Full name">
            <Input value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} placeholder="Sarah Mitchell" />
          </Field>
          <Field label="Initials" hint="Shown on the timetable. Derived automatically if empty.">
            <Input value={form.initials} onChange={(e) => setForm({ ...form, initials: e.target.value })} placeholder="SM" />
          </Field>
          <Field label="Email">
            <Input value={form.email} onChange={(e) => setForm({ ...form, email: e.target.value })} placeholder="sarah@school.edu" />
          </Field>
          <Field label="Max periods per day">
            <Input
              type="number"
              min={1}
              max={12}
              value={form.max_daily}
              onChange={(e) => setForm({ ...form, max_daily: Number(e.target.value) })}
            />
          </Field>
          <Field label="Max periods per week">
            <Input
              type="number"
              min={1}
              max={60}
              value={form.max_weekly}
              onChange={(e) => setForm({ ...form, max_weekly: Number(e.target.value) })}
            />
          </Field>
        </div>

        <Field label="Subjects they can teach" hint="Used to rank substitute teachers and keep subject specialists.">
          <div className="row tight">
            {data.subjects.map((s) => {
              const on = subjectIds.includes(s.id)
              return (
                <button
                  key={s.id}
                  type="button"
                  className="chip"
                  style={{
                    cursor: 'pointer',
                    background: on ? `${s.color}22` : 'var(--surface)',
                    borderColor: on ? s.color : 'var(--border)',
                  }}
                  onClick={() =>
                    setSubjectIds((prev) => (on ? prev.filter((x) => x !== s.id) : [...prev, s.id]))
                  }
                >
                  <span className="dot" style={{ background: s.color }} />
                  {s.name}
                </button>
              )
            })}
          </div>
        </Field>

        <Field label="Unavailable periods" hint="Click a period to mark the teacher as unavailable (e.g. part-time days).">
          <div className="avail-grid">
            {days.map((d) => (
              <div className="avail-row" key={d}>
                <span className="avail-day">{DAY_NAMES[d] ?? `Day ${d}`}</span>
                {periods.map((p) => {
                  const key = `${d}|${p}`
                  const off = unavailable.has(key)
                  return (
                    <button
                      key={p}
                      type="button"
                      className={`avail-cell ${off ? 'off' : ''}`}
                      onClick={() =>
                        setUnavailable((prev) => {
                          const next = new Set(prev)
                          off ? next.delete(key) : next.add(key)
                          return next
                        })
                      }
                    >
                      {off ? '✕' : `P${p}`}
                    </button>
                  )
                })}
              </div>
            ))}
          </div>
        </Field>
      </Modal>
    </>
  )
}

// ------------------------------------------------------------------ classes

function ClassesTab({ data }: { data: SetupAll }) {
  const save = useSave()
  const [editing, setEditing] = useState<SchoolClass | null>(null)
  const [creating, setCreating] = useState(false)
  const [form, setForm] = useState({ name: '', grade: '', size: 26 })

  const open = (c?: SchoolClass) => {
    setForm({ name: c?.name ?? '', grade: c?.grade ?? '', size: c?.size ?? 26 })
    c ? setEditing(c) : setCreating(true)
  }
  const close = () => {
    setEditing(null)
    setCreating(false)
  }

  const submit = async () => {
    if (!form.name.trim()) return toast('Name is required', 'err')
    if (editing) await save.mutateAsync({ path: `/api/classes/${editing.id}`, method: 'PUT', body: form })
    else await save.mutateAsync({ path: '/api/classes', body: form })
    toast(editing ? 'Class updated' : 'Class added', 'ok')
    close()
  }

  return (
    <>
      <div className="row" style={{ marginBottom: 12 }}>
        <Btn variant="primary" onClick={() => open()}>
          + Add class
        </Btn>
      </div>
      {data.classes.length === 0 ? (
        <EmptyState icon="🏫" title="No classes yet" desc="Add the classes or year groups that need a timetable." />
      ) : (
        <div className="table-wrap">
          <table className="tbl">
            <thead>
              <tr>
                <th>Class</th>
                <th>Year / grade</th>
                <th className="num">Students</th>
                <th className="num">Weekly periods</th>
                <th style={{ textAlign: 'right' }}>Actions</th>
              </tr>
            </thead>
            <tbody>
              {data.classes.map((c) => (
                <tr key={c.id}>
                  <td>
                    <strong>{c.name}</strong>
                  </td>
                  <td>{c.grade || '—'}</td>
                  <td className="num">{c.size}</td>
                  <td className="num">
                    {data.lessons.filter((l) => l.class_id === c.id).reduce((a, l) => a + l.periods_per_week, 0)}
                  </td>
                  <td>
                    <div className="actions">
                      <Btn size="sm" onClick={() => open(c)}>
                        Edit
                      </Btn>
                      <ConfirmButton
                        message={`Delete ${c.name} and its lessons?`}
                        onConfirm={async () => {
                          await save.mutateAsync({ path: `/api/classes/${c.id}`, method: 'DELETE' })
                          toast('Class deleted', 'ok')
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

      <Modal
        open={creating || editing !== null}
        onClose={close}
        title={editing ? `Edit ${editing.name}` : 'Add class'}
        footer={
          <>
            <Btn onClick={close}>Cancel</Btn>
            <Btn variant="primary" onClick={submit} disabled={save.isPending}>
              {editing ? 'Save changes' : 'Add class'}
            </Btn>
          </>
        }
      >
        <Field label="Name">
          <Input value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} placeholder="Grade 7A" />
        </Field>
        <Field label="Year / grade">
          <Input value={form.grade} onChange={(e) => setForm({ ...form, grade: e.target.value })} placeholder="7" />
        </Field>
        <Field label="Number of students" hint="Rooms must have enough capacity for the whole class.">
          <Input
            type="number"
            min={1}
            value={form.size}
            onChange={(e) => setForm({ ...form, size: Number(e.target.value) })}
          />
        </Field>
      </Modal>
    </>
  )
}

// ------------------------------------------------------------------ rooms

function RoomsTab({ data }: { data: SetupAll }) {
  const save = useSave()
  const [editing, setEditing] = useState<Room | null>(null)
  const [creating, setCreating] = useState(false)
  const [form, setForm] = useState({ name: '', capacity: 30, room_type: 'general' })

  const open = (r?: Room) => {
    setForm({ name: r?.name ?? '', capacity: r?.capacity ?? 30, room_type: r?.room_type ?? 'general' })
    r ? setEditing(r) : setCreating(true)
  }
  const close = () => {
    setEditing(null)
    setCreating(false)
  }

  const submit = async () => {
    if (!form.name.trim()) return toast('Name is required', 'err')
    if (editing) await save.mutateAsync({ path: `/api/rooms/${editing.id}`, method: 'PUT', body: form })
    else await save.mutateAsync({ path: '/api/rooms', body: form })
    toast(editing ? 'Room updated' : 'Room added', 'ok')
    close()
  }

  return (
    <>
      <div className="row" style={{ marginBottom: 12 }}>
        <Btn variant="primary" onClick={() => open()}>
          + Add room
        </Btn>
      </div>
      {data.rooms.length === 0 ? (
        <EmptyState icon="🚪" title="No rooms yet" desc="Add classrooms, labs and specialist rooms." />
      ) : (
        <div className="table-wrap">
          <table className="tbl">
            <thead>
              <tr>
                <th>Room</th>
                <th>Type</th>
                <th className="num">Capacity</th>
                <th style={{ textAlign: 'right' }}>Actions</th>
              </tr>
            </thead>
            <tbody>
              {data.rooms.map((r) => (
                <tr key={r.id}>
                  <td>
                    <strong>{r.name}</strong>
                  </td>
                  <td>
                    <Badge>{r.room_type}</Badge>
                  </td>
                  <td className="num">{r.capacity}</td>
                  <td>
                    <div className="actions">
                      <Btn size="sm" onClick={() => open(r)}>
                        Edit
                      </Btn>
                      <ConfirmButton
                        message={`Delete ${r.name}?`}
                        onConfirm={async () => {
                          await save.mutateAsync({ path: `/api/rooms/${r.id}`, method: 'DELETE' })
                          toast('Room deleted', 'ok')
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

      <Modal
        open={creating || editing !== null}
        onClose={close}
        title={editing ? `Edit ${editing.name}` : 'Add room'}
        footer={
          <>
            <Btn onClick={close}>Cancel</Btn>
            <Btn variant="primary" onClick={submit} disabled={save.isPending}>
              {editing ? 'Save changes' : 'Add room'}
            </Btn>
          </>
        }
      >
        <Field label="Name">
          <Input value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} placeholder="Physics Lab" />
        </Field>
        <Field label="Type" hint="Lessons can require a room type (e.g. Chemistry needs a lab).">
          <Select value={form.room_type} onChange={(e) => setForm({ ...form, room_type: e.target.value })}>
            {ROOM_TYPES.map((t) => (
              <option key={t} value={t}>
                {t}
              </option>
            ))}
          </Select>
        </Field>
        <Field label="Capacity">
          <Input
            type="number"
            min={1}
            value={form.capacity}
            onChange={(e) => setForm({ ...form, capacity: Number(e.target.value) })}
          />
        </Field>
      </Modal>
    </>
  )
}

// ------------------------------------------------------------------ lessons

interface LessonForm {
  class_id: string
  subject_id: string
  teacher_id: string
  room_id: string
  required_room_type: string
  periods_per_week: number
  max_per_day: number
}

function LessonsTab({ data }: { data: SetupAll }) {
  const save = useSave()
  const [editing, setEditing] = useState<Lesson | null>(null)
  const [creating, setCreating] = useState(false)
  const [classFilter, setClassFilter] = useState('')
  const [form, setForm] = useState<LessonForm>({
    class_id: '',
    subject_id: '',
    teacher_id: '',
    room_id: '',
    required_room_type: '',
    periods_per_week: 2,
    max_per_day: 1,
  })

  const shown = classFilter ? data.lessons.filter((l) => l.class_id === classFilter) : data.lessons
  const teacherOptions = data.teachers.filter(
    (t) => !form.subject_id || (t.subject_ids ?? []).includes(form.subject_id),
  )
  const chosenClass = data.classes.find((c) => c.id === form.class_id)

  const open = (l?: Lesson) => {
    setForm({
      class_id: l?.class_id ?? data.classes[0]?.id ?? '',
      subject_id: l?.subject_id ?? data.subjects[0]?.id ?? '',
      teacher_id: l?.teacher_id ?? '',
      room_id: l?.room_id ?? '',
      required_room_type: l?.required_room_type ?? '',
      periods_per_week: l?.periods_per_week ?? 2,
      max_per_day: l?.max_per_day ?? 1,
    })
    l ? setEditing(l) : setCreating(true)
  }
  const close = () => {
    setEditing(null)
    setCreating(false)
  }

  const submit = async () => {
    if (!form.class_id || !form.subject_id || !form.teacher_id) {
      return toast('Class, subject and teacher are required', 'err')
    }
    const body = {
      ...form,
      room_id: form.room_id || null,
      required_room_type: form.required_room_type || null,
    }
    if (editing) await save.mutateAsync({ path: `/api/lessons/${editing.id}`, method: 'PUT', body })
    else await save.mutateAsync({ path: '/api/lessons', body })
    toast(editing ? 'Lesson updated' : 'Lesson added', 'ok')
    close()
  }

  return (
    <>
      <div className="row" style={{ marginBottom: 12 }}>
        <Btn variant="primary" onClick={() => open()} disabled={!data.classes.length || !data.subjects.length}>
          + Add lesson
        </Btn>
        <div className="right row tight">
          <span className="small muted">Filter</span>
          <Select className="sm" value={classFilter} onChange={(e) => setClassFilter(e.target.value)}>
            <option value="">All classes</option>
            {data.classes.map((c) => (
              <option key={c.id} value={c.id}>
                {c.name}
              </option>
            ))}
          </Select>
        </div>
      </div>

      {shown.length === 0 ? (
        <EmptyState
          icon="📘"
          title="No lessons in the curriculum"
          desc="A lesson links a class, a subject and a teacher — and decides how many periods are needed each week."
        />
      ) : (
        <div className="table-wrap">
          <table className="tbl">
            <thead>
              <tr>
                <th>Class</th>
                <th>Subject</th>
                <th>Teacher</th>
                <th>Room rule</th>
                <th className="num">Periods / cycle</th>
                <th className="num">Max / day</th>
                <th style={{ textAlign: 'right' }}>Actions</th>
              </tr>
            </thead>
            <tbody>
              {shown.map((l) => (
                <tr key={l.id}>
                  <td>
                    <strong>{l.class_name}</strong>
                  </td>
                  <td>
                    <span className="row tight">
                      <span className="dot" style={{ background: l.subject_color }} />
                      {l.subject_name}
                    </span>
                  </td>
                  <td>{l.teacher_name}</td>
                  <td>
                    {l.room_name ? (
                      <Badge tone="info">{l.room_name}</Badge>
                    ) : l.required_room_type ? (
                      <Badge tone="warn">any {l.required_room_type}</Badge>
                    ) : (
                      <span className="faint">automatic</span>
                    )}
                  </td>
                  <td className="num">{l.periods_per_week}</td>
                  <td className="num">{l.max_per_day}</td>
                  <td>
                    <div className="actions">
                      <Btn size="sm" onClick={() => open(l)}>
                        Edit
                      </Btn>
                      <ConfirmButton
                        message={`Remove ${l.subject_name} for ${l.class_name}?`}
                        onConfirm={async () => {
                          await save.mutateAsync({ path: `/api/lessons/${l.id}`, method: 'DELETE' })
                          toast('Lesson deleted', 'ok')
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

      <Modal
        open={creating || editing !== null}
        onClose={close}
        wide
        title={editing ? 'Edit lesson' : 'Add lesson'}
        footer={
          <>
            <Btn onClick={close}>Cancel</Btn>
            <Btn variant="primary" onClick={submit} disabled={save.isPending}>
              {editing ? 'Save changes' : 'Add lesson'}
            </Btn>
          </>
        }
      >
        <div className="grid cols-2" style={{ gap: 0 }}>
          <Field label="Class">
            <Select value={form.class_id} onChange={(e) => setForm({ ...form, class_id: e.target.value })}>
              <option value="">Select class…</option>
              {data.classes.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </Select>
          </Field>
          <Field label="Subject">
            <Select
              value={form.subject_id}
              onChange={(e) => setForm({ ...form, subject_id: e.target.value, teacher_id: '', room_id: '' })}
            >
              <option value="">Select subject…</option>
              {data.subjects.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name}
                </option>
              ))}
            </Select>
          </Field>
          <Field label="Teacher" hint={teacherOptions.length ? undefined : 'No teacher is linked to this subject yet.'}>
            <Select value={form.teacher_id} onChange={(e) => setForm({ ...form, teacher_id: e.target.value })}>
              <option value="">Select teacher…</option>
              {(teacherOptions.length ? teacherOptions : data.teachers).map((t) => (
                <option key={t.id} value={t.id}>
                  {t.name}
                </option>
              ))}
            </Select>
          </Field>
          <Field label="Periods per week">
            <Input
              type="number"
              min={1}
              max={20}
              value={form.periods_per_week}
              onChange={(e) => setForm({ ...form, periods_per_week: Number(e.target.value) })}
            />
          </Field>
          <Field label="Max periods per day">
            <Input
              type="number"
              min={1}
              max={6}
              value={form.max_per_day}
              onChange={(e) => setForm({ ...form, max_per_day: Number(e.target.value) })}
            />
          </Field>
          <Field label="Fixed room">
            <Select value={form.room_id} onChange={(e) => setForm({ ...form, room_id: e.target.value })}>
              <option value="">Automatic — pick the best free room</option>
              {data.rooms
                .filter((r) => !chosenClass || r.capacity >= chosenClass.size)
                .map((r) => (
                  <option key={r.id} value={r.id}>
                    {r.name} · {r.capacity} seats · {r.room_type}
                  </option>
                ))}
            </Select>
          </Field>
          <Field
            label="Required room type"
            hint="Use this instead of a fixed room when any fitting room of a type will do."
          >
            <Select
              value={form.required_room_type}
              onChange={(e) => setForm({ ...form, required_room_type: e.target.value })}
              disabled={!!form.room_id}
            >
              <option value="">No requirement</option>
              {ROOM_TYPES.map((t) => (
                <option key={t} value={t}>
                  {t}
                </option>
              ))}
            </Select>
          </Field>
        </div>
        {chosenClass && (
          <div className="small muted">
            {chosenClass.name} has {chosenClass.size} students — only rooms with at least that capacity can be used.
          </div>
        )}
      </Modal>
    </>
  )
}

// ------------------------------------------------------------------ bell schedule

interface Draft extends BellSlotDraft {
  id?: string
}

function BellTab({ data }: { data: SetupAll }) {
  const save = useSave()
  const [draft, setDraft] = useState<Draft[] | null>(null)

  const base = useMemo<Draft[]>(
    () =>
      [...data.slots]
        .sort((a, b) => a.day - b.day || a.idx - b.idx)
        .map((s: Slot) => ({
          id: s.id,
          day: s.day,
          idx: s.idx,
          start_time: s.start_time,
          end_time: s.end_time,
          label: s.label,
          is_break: s.is_break,
        })),
    [data.slots],
  )

  const rows = draft ?? base
  const days = [...new Set(rows.map((r) => r.day))].sort((a, b) => a - b)

  const update = (day: number, idx: number, patch: Partial<Draft>) =>
    setDraft(rows.map((r) => (r.day === day && r.idx === idx ? { ...r, ...patch } : r)))

  const copyFirstDayToAll = () => {
    const first = rows.filter((r) => r.day === days[0])
    setDraft(
      rows.map((r) => {
        const src = first.find((f) => f.idx === r.idx)
        return src
          ? { ...r, start_time: src.start_time, end_time: src.end_time, label: src.label, is_break: src.is_break }
          : r
      }),
    )
  }

  const submit = async () => {
    await save.mutateAsync({
      path: '/api/bell-schedule',
      method: 'PUT',
      body: { slots: rows.map(({ day, idx, start_time, end_time, label, is_break }) => ({ day, idx, start_time, end_time, label, is_break })) },
    })
    toast('Bell schedule saved — regenerate the timetable to apply it', 'ok')
    setDraft(null)
  }

  return (
    <>
      <div className="row" style={{ marginBottom: 12 }}>
        <Btn onClick={copyFirstDayToAll}>Copy {DAY_NAMES[days[0]] ?? 'first day'} to all days</Btn>
        <div className="right row tight">
          {draft && <Btn onClick={() => setDraft(null)}>Discard</Btn>}
          <Btn variant="primary" onClick={submit} disabled={save.isPending || !draft}>
            Save bell schedule
          </Btn>
        </div>
      </div>

      <div className="table-wrap">
        <table className="tbl">
          <thead>
            <tr>
              <th>Day</th>
              <th className="num">Period</th>
              <th>Start</th>
              <th>End</th>
              <th>Label</th>
              <th>Break?</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={`${r.day}-${r.idx}`}>
                <td>{DAY_NAMES[r.day] ?? `Day ${r.day}`}</td>
                <td className="num">P{r.idx}</td>
                <td>
                  <Input
                    className="sm"
                    style={{ width: 96 }}
                    value={r.start_time}
                    onChange={(e) => update(r.day, r.idx, { start_time: e.target.value })}
                  />
                </td>
                <td>
                  <Input
                    className="sm"
                    style={{ width: 96 }}
                    value={r.end_time}
                    onChange={(e) => update(r.day, r.idx, { end_time: e.target.value })}
                  />
                </td>
                <td>
                  <Input
                    className="sm"
                    style={{ width: 150 }}
                    value={r.label}
                    placeholder={r.is_break ? 'Lunch' : '—'}
                    onChange={(e) => update(r.day, r.idx, { label: e.target.value })}
                  />
                </td>
                <td>
                  <input
                    type="checkbox"
                    checked={r.is_break}
                    onChange={(e) => update(r.day, r.idx, { is_break: e.target.checked })}
                  />
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  )
}
