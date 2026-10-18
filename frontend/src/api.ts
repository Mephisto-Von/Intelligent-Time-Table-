// Thin typed client for the Rust backend API.

const TOKEN_KEY = 'tt_token'

export function getToken(): string | null {
  return localStorage.getItem(TOKEN_KEY)
}

export function setToken(token: string | null) {
  if (token) localStorage.setItem(TOKEN_KEY, token)
  else localStorage.removeItem(TOKEN_KEY)
}

export class ApiError extends Error {
  status: number
  constructor(status: number, message: string) {
    super(message)
    this.status = status
    this.name = 'ApiError'
  }
}

interface ApiOptions {
  method?: string
  body?: unknown
  signal?: AbortSignal
}

export async function api<T = unknown>(path: string, opts: ApiOptions = {}): Promise<T> {
  const headers: Record<string, string> = {}
  const token = getToken()
  if (token) headers.Authorization = `Bearer ${token}`
  let body: BodyInit | undefined
  if (opts.body !== undefined) {
    headers['Content-Type'] = 'application/json'
    body = JSON.stringify(opts.body)
  }
  const res = await fetch(path, {
    method: opts.method ?? (opts.body !== undefined ? 'POST' : 'GET'),
    headers,
    body,
    signal: opts.signal,
  })
  if (!res.ok) {
    let message = `${res.status} ${res.statusText}`
    try {
      const data = await res.json()
      if (typeof data?.error === 'string') message = data.error
      else if (typeof data?.message === 'string') message = data.message
    } catch {
      /* not json */
    }
    throw new ApiError(res.status, message)
  }
  if (res.status === 204) return undefined as T
  const text = await res.text()
  return (text ? JSON.parse(text) : undefined) as T
}

export async function downloadFile(path: string, filename: string) {
  const token = getToken()
  const res = await fetch(path, { headers: token ? { Authorization: `Bearer ${token}` } : {} })
  if (!res.ok) throw new ApiError(res.status, res.statusText)
  const blob = await res.blob()
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  document.body.appendChild(a)
  a.click()
  a.remove()
  URL.revokeObjectURL(url)
}

// ---------------------------------------------------------------- types

export type Role = 'admin' | 'scheduler' | 'teacher' | 'student'

export interface User {
  id: string
  account_id: string
  school_id: string
  email: string
  name: string
  role: Role
  teacher_id: string | null
  class_id: string | null
}

export interface School {
  id: string
  name: string
  cycle_weeks: number
}

export interface MeResponse {
  user: User
  school: School
}

export interface LoginResponse {
  token: string
  user: User
}

export interface Slot {
  id: string
  day: number
  idx: number
  start_time: string
  end_time: string
  label: string
  is_break: boolean
}

export interface Subject {
  id: string
  name: string
  code: string
  color: string
}

export interface UnavailabilitySlot {
  day: number
  idx: number
}

export interface Teacher {
  id: string
  name: string
  initials: string
  email: string | null
  max_weekly: number
  max_daily: number
  subject_ids: string[]
  unavailable: UnavailabilitySlot[]
  lesson_count?: number
}

export interface SchoolClass {
  id: string
  name: string
  grade: string
  size: number
}

export interface Room {
  id: string
  name: string
  capacity: number
  room_type: string
}

export interface Lesson {
  id: string
  class_id: string
  class_name: string
  subject_id: string
  subject_name: string
  subject_code: string
  subject_color: string
  teacher_id: string
  teacher_name: string
  teacher_initials: string
  room_id: string | null
  room_name: string | null
  required_room_type: string | null
  periods_per_week: number
  max_per_day: number
}

export interface SlotEntry {
  id: string
  lesson_id: string
  week: number
  day: number
  idx: number
  room_id: string | null
  room_name: string | null
  locked: boolean
  class_id: string
  class_name: string
  subject_id: string
  subject_name: string
  subject_code: string
  subject_color: string
  teacher_id: string
  teacher_name: string
  teacher_initials: string
}

export interface TimetableData {
  cycle_weeks: number
  slots: Slot[]
  entries: SlotEntry[]
}

export interface BellSlotDraft {
  day: number
  idx: number
  start_time: string
  end_time: string
  label: string
  is_break: boolean
}

export interface SetupAll {
  school: School
  subjects: Subject[]
  teachers: Teacher[]
  classes: SchoolClass[]
  rooms: Room[]
  slots: Slot[]
  lessons: Lesson[]
}

export interface GenerateReport {
  placed: number
  requested: number
  unplaced_total: number
  iterations: number
  elapsed_ms: number
  score: number
  seed: number
  locked_kept: number
  soft: Record<string, number>
  unplaced: {
    lesson_id: string
    class_name: string
    subject_name: string
    teacher_name: string
    count: number
    dominant: string
  }[]
}

export interface DashboardData {
  school_name: string
  counts: { subjects: number; teachers: number; classes: number; rooms: number; lessons: number }
  placed: number
  requested: number
  pending_leave: number
  today: { date: string; sessions_due: number; covered: number }
  gaps: { class_name: string; lesson_id: string; subject_name: string; missing: number }[]
  upcoming_events: {
    id: string
    title: string
    kind: string
    start_date: string
    end_date: string
  }[]
}

export interface LeaveItem {
  id: string
  teacher_id: string
  teacher_name: string
  teacher_initials: string
  start_date: string
  end_date: string
  part: 'full' | 'am' | 'pm'
  reason: string | null
  status: 'pending' | 'approved' | 'rejected' | 'cancelled'
  days: number
  decided_at: string | null
  created_at: string
}

export interface LeaveImpact {
  leave_id: string
  teacher_id: string
  part: 'full' | 'am' | 'pm'
  sessions: {
    date: string
    day: number
    day_name: string
    idx: number
    start_time: string
    end_time: string
    lesson_id: string
    subject_name: string
    subject_code: string
    class_name: string
    room_name: string | null
    substitution: { id: string; substitute_name: string } | null
  }[]
}

export interface CoverCandidate {
  teacher_id: string
  name: string
  initials: string
  score: number
  teaches_subject: boolean
  daily_load: number
  cover_count: number
  reason: string | null
}

export interface CoverPeriod {
  lesson_id: string
  day: number
  day_name: string
  idx: number
  class_name: string
  subject_id: string
  subject_name: string
  subject_code: string
  subject_color: string
  room_name: string | null
  absent_teacher_id: string
  absent_teacher_name: string
  absent_part: string
  candidates: CoverCandidate[]
  blocked: { teacher_id: string; name: string; reason: string }[]
  substitution: {
    id: string
    substitute_teacher_id: string
    substitute_name: string
    substitute_initials: string
  } | null
}

export interface CoverData {
  date: string
  day: number
  day_name: string
  week: number
  is_school_day: boolean
  is_holiday: boolean
  events: { id: string; title: string; kind: string; start_date: string; end_date: string }[]
  absences: { leave_id: string; teacher_id: string; teacher_name: string; teacher_initials: string; part: string }[]
  periods: CoverPeriod[]
}

export interface CoverHistoryItem {
  id: string
  date: string
  day_name?: string
  idx: number
  class_name: string
  subject_name: string
  absent_teacher_name: string
  substitute_teacher_id: string
  substitute_name: string
  status: string
  created_at: string
}

export interface CalendarEvent {
  id: string
  title: string
  start_date: string
  end_date: string
  kind: string
  notes: string | null
}

export interface UserRow {
  id: string
  name: string
  email: string
  role: Role
  teacher_id: string | null
  teacher_name: string | null
  class_id: string | null
  class_name: string | null
  created_at: string
}

export interface SolverWeights {
  same_subject_per_day: number
  class_gap: number
  teacher_gap: number
  teacher_balance: number
  week_balance: number
  unplaced: number
}

export interface AuditItem {
  id: string
  actor_id: string | null
  actor_name: string
  action: string
  entity: string
  entity_id: string | null
  summary: string
  created_at: string
}

export interface PublicLink {
  id: string
  entity: 'class' | 'teacher' | 'room'
  entity_id: string
  entity_name: string | null
  label: string
  token: string
  path: string
  created_at: string
}

export interface PublicTimetable {
  school_name: string
  cycle_weeks: number
  entity: string
  entity_id: string
  entity_name: string | null
  label: string
  slots: Slot[]
  entries: SlotEntry[]
}

export interface NotifyStatus {
  enabled: boolean
  channels: string[]
  available_channels: string[]
  events: { id: string; label: string }[]
}

export const ROOM_TYPES = ['general', 'lab', 'computer', 'art', 'music', 'gym', 'library', 'outdoor']
export const LEAVE_PARTS = [
  { value: 'full', label: 'Full day' },
  { value: 'am', label: 'Morning only' },
  { value: 'pm', label: 'Afternoon only' },
]
export const EVENT_KINDS = [
  { value: 'holiday', label: 'Holiday (school closed)' },
  { value: 'activity', label: 'School activity' },
  { value: 'exam', label: 'Exam period' },
  { value: 'other', label: 'Other' },
]
export const DAY_NAMES = ['', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday']
