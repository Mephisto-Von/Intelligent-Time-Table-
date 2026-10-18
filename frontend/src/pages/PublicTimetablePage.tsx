import { useState } from 'react'
import { useParams } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import { api } from '../api'
import type { PublicTimetable } from '../api'
import TimetableGrid from '../components/TimetableGrid'
import type { GridMode } from '../components/TimetableGrid'
import { Badge, Card, EmptyState, Loading, Select, initialsOf } from '../ui'

/**
 * Anonymous, read-only timetable reached through a share token. Lives outside
 * the authenticated app shell so it can be embedded or linked publicly.
 */
export default function PublicTimetablePage() {
  const { token = '' } = useParams()
  const [week, setWeek] = useState(0)

  const { data, isLoading, error } = useQuery({
    queryKey: ['public-timetable', token],
    queryFn: () => api<PublicTimetable>(`/api/public/timetable/${token}`),
    retry: false,
  })

  const mode = (data?.entity ?? 'class') as GridMode
  const cycleWeeks = data?.cycle_weeks ?? 1
  const entries = (data?.entries ?? []).filter((e) => e.week === (cycleWeeks > 1 ? week : 0))
  const groupName = data?.entity_name ?? data?.school_name ?? 'Timetable'

  return (
    <div className="public-wrap">
      <header className="public-head">
        <div className="brand">
          <div className="brand-mark">TM</div>
          <div>
            <div className="brand-name">{groupName}</div>
            <div className="brand-sub">{data?.school_name ?? 'TimetableMaster'}</div>
          </div>
        </div>
        <div className="row tight">
          <Badge tone="info">{mode}</Badge>
          {cycleWeeks > 1 && (
            <Select className="sm" value={week} onChange={(e) => setWeek(Number(e.target.value))}>
              <option value={0}>Week A</option>
              <option value={1}>Week B</option>
            </Select>
          )}
        </div>
      </header>

      <main className="page">
        {isLoading && <Loading label="Loading timetable…" />}
        {error && (
          <Card>
            <EmptyState
              icon="🔗"
              title="This timetable link is not available"
              desc="The link may have been revoked or mistyped. Ask the school for a new one."
            />
          </Card>
        )}
        {data && (
          <Card tight>
            <div className="card-head">
              <h2>
                {data.entity_name ?? 'Timetable'}
                {data.label ? ` · ${data.label}` : ''}
              </h2>
              <div className="topbar-spacer" />
              <span className="small faint">Read-only · {initialsOf(data.school_name)}</span>
            </div>
            <div className="card-body">
              {entries.length === 0 ? (
                <EmptyState
                  icon="🗓️"
                  title="Nothing scheduled yet"
                  desc="This timetable has not been published for the selected week."
                />
              ) : (
                <TimetableGrid
                  slots={data.slots}
                  entries={entries}
                  mode={mode}
                  groups={[{ id: data.entity_id, name: data.entity_name ?? 'Timetable' }]}
                  selectedGroup={data.entity_id}
                  editable={false}
                />
              )}
            </div>
          </Card>
        )}
      </main>
    </div>
  )
}
