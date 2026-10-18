import { useMemo, useState } from 'react'
import type { Slot, SlotEntry } from '../api'
import { EmptyState } from '../ui'

export type GridMode = 'class' | 'teacher' | 'room'

interface Group {
  id: string
  name: string
}

interface Props {
  slots: Slot[]
  entries: SlotEntry[]
  mode: GridMode
  groups: Group[]
  selectedGroup?: string
  editable: boolean
  onMove?: (entryId: string, day: number, idx: number) => void
  onEdit?: (entry: SlotEntry) => void
  showRoom?: boolean
}

function groupKeyOf(e: SlotEntry, mode: GridMode): string | undefined {
  if (mode === 'class') return e.class_id
  if (mode === 'teacher') return e.teacher_id
  return e.room_id ?? undefined
}

export default function TimetableGrid({
  slots,
  entries,
  mode,
  groups,
  selectedGroup,
  editable,
  onMove,
  onEdit,
  showRoom = true,
}: Props) {
  const [dragging, setDragging] = useState<string | null>(null)
  const [hover, setHover] = useState<string | null>(null)

  const { days, periods, slotAt } = useMemo(() => {
    const daySet = new Set<number>()
    const periodMap = new Map<number, Slot>()
    const at = new Map<string, Slot>()
    for (const s of slots) {
      daySet.add(s.day)
      periodMap.set(s.idx, s)
      at.set(`${s.day}|${s.idx}`, s)
    }
    return {
      days: [...daySet].sort((a, b) => a - b),
      periods: [...periodMap.keys()].sort((a, b) => a - b).map((i) => periodMap.get(i)!),
      slotAt: at,
    }
  }, [slots])

  const cellMap = useMemo(() => {
    const map = new Map<string, SlotEntry>()
    for (const e of entries) {
      const g = groupKeyOf(e, mode)
      if (!g) continue
      map.set(`${g}|${e.day}|${e.idx}`, e)
    }
    return map
  }, [entries, mode])

  const shownGroups = selectedGroup ? groups.filter((g) => g.id === selectedGroup) : groups

  if (days.length === 0 || periods.length === 0) {
    return (
      <EmptyState
        icon="🗓️"
        title="No bell schedule yet"
        desc="Add periods to the bell schedule in School setup before building the timetable."
      />
    )
  }
  if (shownGroups.length === 0) {
    return <EmptyState icon="🏫" title="Nothing to show" desc="Create classes, teachers or rooms first." />
  }

  const dayLabel = (day: number) =>
    ['', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'][day] ?? `D${day}`

  return (
    <div>
      {shownGroups.map((group) => {
        const groupEntries = entries.filter((e) => groupKeyOf(e, mode) === group.id).length
        return (
          <div className="tt-group" key={group.id}>
            <div className="tt-group-title">
              <span>{group.name}</span>
              <span className="faint small">{groupEntries} periods / cycle</span>
            </div>
            <table className="tt-table">
              <thead>
                <tr>
                  <th style={{ width: 108 }}>Period</th>
                  {days.map((d) => (
                    <th key={d}>{dayLabel(d)}</th>
                  ))}
                </tr>
              </thead>
              <tbody>
                {periods.map((p) => {
                  // Hide periods that are breaks on every day (e.g. lunch) when empty
                  const anyBreak = days.every((d) => slotAt.get(`${d}|${p.idx}`)?.is_break)
                  const anyLesson = days.some((d) => cellMap.has(`${group.id}|${d}|${p.idx}`))
                  if (anyBreak && !anyLesson) {
                    return (
                      <tr key={p.idx}>
                        <td className="tt-period">
                          <div className="p-time">{p.start_time}</div>
                        </td>
                        <td colSpan={days.length} style={{ padding: 0 }}>
                          <div className="tt-cell break" style={{ minHeight: 30 }}>
                            {slotAt.get(`${days[0]}|${p.idx}`)?.label || 'Break'}
                          </div>
                        </td>
                      </tr>
                    )
                  }
                  return (
                    <tr key={p.idx}>
                      <td className="tt-period">
                        <div className="p-idx">P{p.idx}</div>
                        <div className="p-time">
                          {p.start_time}–{p.end_time}
                        </div>
                      </td>
                      {days.map((d) => {
                        const slot = slotAt.get(`${d}|${p.idx}`)
                        const key = `${group.id}|${d}|${p.idx}`
                        const cell = cellMap.get(key)
                        const isHover = hover === key
                        if (!slot) {
                          return <td key={d} style={{ background: 'var(--surface-2)' }} />
                        }
                        if (slot.is_break && !cell) {
                          return (
                            <td key={d} style={{ padding: 0 }}>
                              <div className="tt-cell break">{slot.label || 'Break'}</div>
                            </td>
                          )
                        }
                        return (
                          <td
                            key={d}
                            style={{ padding: 3 }}
                            onDragOver={(ev) => {
                              if (!editable || !dragging || cell) return
                              ev.preventDefault()
                              setHover(key)
                            }}
                            onDragLeave={() => setHover((h) => (h === key ? null : h))}
                            onDrop={(ev) => {
                              ev.preventDefault()
                              setHover(null)
                              // Drops are only accepted on empty cells (see onDragOver),
                              // so bail out when the target already holds a lesson.
                              if (!editable || cell) return
                              const id = ev.dataTransfer.getData('text/plain') || dragging
                              if (id) onMove?.(id, d, p.idx)
                            }}
                          >
                            <div className={`tt-cell ${isHover ? 'drop-target' : ''}`}>
                              {cell ? (
                                <div
                                  className={`tt-lesson ${editable ? 'draggable' : ''} ${
                                    dragging === cell.id ? 'dragging' : ''
                                  } ${cell.locked ? 'locked' : ''}`}
                                  draggable={editable}
                                  onDragStart={(ev) => {
                                    ev.dataTransfer.setData('text/plain', cell.id)
                                    ev.dataTransfer.effectAllowed = 'move'
                                    setDragging(cell.id)
                                  }}
                                  onDragEnd={() => {
                                    setDragging(null)
                                    setHover(null)
                                  }}
                                  onClick={() => onEdit?.(cell)}
                                  title={`${cell.subject_name} · ${cell.class_name} · ${cell.teacher_name}${
                                    cell.room_name ? ` · ${cell.room_name}` : ''
                                  }${cell.locked ? ' (locked)' : ''}`}
                                  style={{
                                    borderLeftColor: cell.subject_color,
                                    background: `${cell.subject_color}18`,
                                  }}
                                >
                                  <div className="l-subject" style={{ color: cell.subject_color }}>
                                    {cell.subject_code || cell.subject_name}
                                  </div>
                                  <div className="l-meta">
                                    {mode === 'teacher' ? cell.class_name : mode === 'room' ? cell.class_name : cell.teacher_initials || cell.teacher_name}
                                  </div>
                                  {showRoom && mode !== 'room' && cell.room_name && (
                                    <div className="l-meta faint">{cell.room_name}</div>
                                  )}
                                  {cell.locked && <span className="l-badge">🔒</span>}
                                </div>
                              ) : null}
                            </div>
                          </td>
                        )
                      })}
                    </tr>
                  )
                })}
              </tbody>
            </table>
          </div>
        )
      })}
    </div>
  )
}
