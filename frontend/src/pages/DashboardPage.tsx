import { useQuery } from '@tanstack/react-query'
import { Link } from 'react-router-dom'
import { api } from '../api'
import type { DashboardData } from '../api'
import { useAuth } from '../auth'
import { Page, PageHead } from '../components/Layout'
import { Badge, Btn, Card, EmptyState, Loading, fmtDate, todayISO } from '../ui'

const KIND_TONE: Record<string, 'info' | 'warn' | 'ok' | 'err' | 'default'> = {
  holiday: 'err',
  activity: 'info',
  exam: 'warn',
  other: 'default',
}

export default function DashboardPage() {
  const { user, isStaff, school } = useAuth()
  const { data, isLoading, error } = useQuery({
    queryKey: ['dashboard'],
    queryFn: () => api<DashboardData>('/api/dashboard'),
  })

  const today = todayISO()
  const pct = data && data.requested > 0 ? Math.round((data.placed / data.requested) * 100) : 0

  return (
    <Page>
      <PageHead
        title={`Welcome back, ${user?.name?.split(' ')[0] ?? ''}`}
        sub={`${school?.name ?? ''} · ${fmtDate(today)}`}
        actions={
          isStaff ? (
            <div className="row tight">
              <Link to="/timetable">
                <Btn variant="primary">⚡ Generate timetable</Btn>
              </Link>
              <Link to="/cover">
                <Btn>🔁 Today&apos;s cover</Btn>
              </Link>
            </div>
          ) : (
            <Link to="/timetable">
              <Btn variant="primary">🗓️ View my timetable</Btn>
            </Link>
          )
        }
      />

      {isLoading && <Loading label="Loading dashboard…" />}
      {error && (
        <Card>
          <EmptyState icon="⚠️" title="Could not load the dashboard" desc={String(error)} />
        </Card>
      )}

      {data && (
        <>
          <div className="grid cols-4">
            <Card className="stat">
              <div className="stat-label">Timetable coverage</div>
              <div className="stat-value">{data.requested > 0 ? `${pct}%` : '—'}</div>
              {data.requested > 0 ? (
                <>
                  <div className="progress" style={{ margin: '6px 0' }}>
                    <div style={{ width: `${pct}%` }} />
                  </div>
                  <div className="stat-hint">
                    {data.placed} of {data.requested} weekly periods placed
                  </div>
                </>
              ) : (
                <div className="stat-hint">Add lessons to the curriculum to get started</div>
              )}
            </Card>
            <Card className="stat">
              <div className="stat-label">Today&apos;s absences</div>
              <div className="stat-value">{data.today.sessions_due}</div>
              <div className="stat-hint">
                {data.today.sessions_due === 0
                  ? 'No lessons need cover today'
                  : `${data.today.covered} of ${data.today.sessions_due} covered so far`}
              </div>
            </Card>
            <Card className="stat">
              <div className="stat-label">Pending leave</div>
              <div className="stat-value">{data.pending_leave}</div>
              <div className="stat-hint">
                {data.pending_leave === 0 ? 'All requests decided' : 'Awaiting a decision'}
              </div>
            </Card>
            <Card className="stat">
              <div className="stat-label">School roster</div>
              <div className="stat-value">{data.counts.teachers}</div>
              <div className="stat-hint">
                teachers · {data.counts.classes} classes · {data.counts.rooms} rooms
              </div>
            </Card>
          </div>

          <div className="grid cols-2">
            <Card
              title="Unplaced periods"
              actions={<Badge tone={data.gaps.length === 0 ? 'ok' : 'warn'}>{data.gaps.length}</Badge>}
            >
              {data.requested === 0 ? (
                <EmptyState
                  icon="🧩"
                  title="No curriculum yet"
                  desc="Add subjects, classes, teachers and lessons in School setup, then generate the timetable."
                  action={
                    isStaff ? (
                      <Link to="/setup">
                        <Btn size="sm" variant="primary">
                          Open School setup
                        </Btn>
                      </Link>
                    ) : undefined
                  }
                />
              ) : data.gaps.length === 0 ? (
                <EmptyState
                  icon="✅"
                  title="Everything is placed"
                  desc="Every lesson has a slot in the current timetable."
                />
              ) : (
                <div className="list">
                  {data.gaps.map((g, i) => (
                    <div className="list-item" key={i}>
                      <div className="grow">
                        <div className="title">
                          {g.class_name} · {g.subject_name}
                        </div>
                        <div className="sub">{g.missing} periods could not be placed</div>
                      </div>
                      <Link to="/timetable">
                        <Btn size="sm">Fix</Btn>
                      </Link>
                    </div>
                  ))}
                </div>
              )}
            </Card>

            <Card title="Upcoming calendar">
              {data.upcoming_events.length === 0 ? (
                <EmptyState
                  icon="📅"
                  title="No upcoming events"
                  desc="Add holidays, exams or school activities in the calendar."
                  action={
                    <Link to="/calendar">
                      <Btn size="sm">Open calendar</Btn>
                    </Link>
                  }
                />
              ) : (
                <div className="list">
                  {data.upcoming_events.map((e) => (
                    <div className="list-item" key={e.id}>
                      <div className="grow">
                        <div className="title">{e.title}</div>
                        <div className="sub">
                          {fmtDate(e.start_date)}
                          {e.end_date !== e.start_date ? ` → ${fmtDate(e.end_date)}` : ''}
                        </div>
                      </div>
                      <Badge tone={KIND_TONE[e.kind] ?? 'default'}>{e.kind}</Badge>
                    </div>
                  ))}
                </div>
              )}
            </Card>
          </div>

          {isStaff && (
            <Card title="Quick actions">
              <div className="row">
                <Link to="/setup">
                  <Btn>🧩 School setup</Btn>
                </Link>
                <Link to="/leave">
                  <Btn>🌴 Leave requests</Btn>
                </Link>
                <Link to="/cover">
                  <Btn>🔁 Arrange cover</Btn>
                </Link>
                <Link to="/calendar">
                  <Btn>📅 Add calendar event</Btn>
                </Link>
              </div>
            </Card>
          )}
        </>
      )}
    </Page>
  )
}
