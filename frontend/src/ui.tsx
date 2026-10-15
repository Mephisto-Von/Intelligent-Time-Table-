import { useEffect, useState } from 'react'
import type { ButtonHTMLAttributes, InputHTMLAttributes, ReactNode, SelectHTMLAttributes } from 'react'
import { ApiError } from './api'

// ------------------------------------------------------------- formatting helpers

export function todayISO(): string {
  const d = new Date()
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${d.getFullYear()}-${m}-${day}`
}

export function parseISO(iso: string): Date {
  const [y, m, d] = iso.split('-').map(Number)
  return new Date(y, (m || 1) - 1, d || 1)
}

export function addDays(iso: string, days: number): string {
  const d = parseISO(iso)
  d.setDate(d.getDate() + days)
  const m = String(d.getMonth() + 1).padStart(2, '0')
  const day = String(d.getDate()).padStart(2, '0')
  return `${d.getFullYear()}-${m}-${day}`
}

const WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat']
const MONTHS = [
  'January', 'February', 'March', 'April', 'May', 'June',
  'July', 'August', 'September', 'October', 'November', 'December',
]

export function fmtDate(iso: string): string {
  if (!iso) return ''
  const d = parseISO(iso)
  return `${WEEKDAYS[d.getDay()]}, ${d.getDate()} ${MONTHS[d.getMonth()]} ${d.getFullYear()}`
}

export function fmtDateShort(iso: string): string {
  if (!iso) return ''
  const d = parseISO(iso)
  return `${d.getDate()} ${MONTHS[d.getMonth()].slice(0, 3)}`
}

export function monthLabel(year: number, month: number): string {
  return `${MONTHS[month]} ${year}`
}

export function initialsOf(name: string): string {
  return name
    .split(/\s+/)
    .filter(Boolean)
    .slice(0, 2)
    .map((p) => p[0]?.toUpperCase() ?? '')
    .join('')
}

export function errMsg(e: unknown): string {
  if (e instanceof ApiError) return e.message
  if (e instanceof Error) return e.message
  return String(e)
}

export function plural(n: number, word: string, suffix = 's'): string {
  return `${n} ${word}${n === 1 ? '' : suffix}`
}

// ------------------------------------------------------------- toasts

type ToastKind = 'ok' | 'err' | 'info'
interface ToastItem {
  id: number
  kind: ToastKind
  msg: string
}

let pushToast: ((t: ToastItem) => void) | null = null
let nextToastId = 1

export function toast(msg: string, kind: ToastKind = 'info') {
  pushToast?.({ id: nextToastId++, kind, msg })
}

export function Toaster() {
  const [items, setItems] = useState<ToastItem[]>([])
  useEffect(() => {
    pushToast = (t) => {
      setItems((prev) => [...prev, t])
      window.setTimeout(() => setItems((prev) => prev.filter((x) => x.id !== t.id)), 4200)
    }
    return () => {
      pushToast = null
    }
  }, [])
  return (
    <div className="toasts">
      {items.map((t) => (
        <div key={t.id} className={`toast ${t.kind}`}>
          {t.msg}
        </div>
      ))}
    </div>
  )
}

// ------------------------------------------------------------- primitives

interface BtnProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'default' | 'primary' | 'danger' | 'ghost'
  size?: 'md' | 'sm'
  block?: boolean
}

export function Btn({ variant = 'default', size = 'md', block, className = '', children, ...rest }: BtnProps) {
  const cls = [
    'btn',
    variant !== 'default' ? variant : '',
    size === 'sm' ? 'sm' : '',
    block ? 'block' : '',
    className,
  ]
    .filter(Boolean)
    .join(' ')
  return (
    <button type="button" className={cls} {...rest}>
      {children}
    </button>
  )
}

export function Card({
  title,
  actions,
  children,
  className = '',
  tight,
}: {
  title?: ReactNode
  actions?: ReactNode
  children: ReactNode
  className?: string
  tight?: boolean
}) {
  return (
    <section className={`card ${className}`}>
      {(title || actions) && (
        <div className="card-head">
          {title ? (typeof title === 'string' ? <h2>{title}</h2> : title) : null}
          {actions}
        </div>
      )}
      <div className={`card-body ${tight ? 'tight' : ''}`}>{children}</div>
    </section>
  )
}

export function Modal({
  open,
  onClose,
  title,
  children,
  footer,
  wide,
}: {
  open: boolean
  onClose: () => void
  title: ReactNode
  children: ReactNode
  footer?: ReactNode
  wide?: boolean
}) {
  useEffect(() => {
    if (!open) return
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onClose()
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [open, onClose])
  if (!open) return null
  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className={`modal ${wide ? 'wide' : ''}`}>
        <div className="modal-head">
          <h3>{title}</h3>
          <button className="icon-btn" onClick={onClose} aria-label="Close">
            ✕
          </button>
        </div>
        <div className="modal-body">{children}</div>
        {footer && <div className="modal-foot">{footer}</div>}
      </div>
    </div>
  )
}

export function Field({
  label,
  hint,
  children,
}: {
  label: ReactNode
  hint?: ReactNode
  children: ReactNode
}) {
  return (
    <div className="field">
      <label>{label}</label>
      {children}
      {hint ? <div className="hint">{hint}</div> : null}
    </div>
  )
}

export function Input(props: InputHTMLAttributes<HTMLInputElement>) {
  const { className = '', ...rest } = props
  return <input className={`input ${className}`} {...rest} />
}

export function Select(props: SelectHTMLAttributes<HTMLSelectElement>) {
  const { className = '', children, ...rest } = props
  return (
    <select className={`select ${className}`} {...rest}>
      {children}
    </select>
  )
}

export function Badge({
  children,
  tone = 'default',
}: {
  children: ReactNode
  tone?: 'default' | 'ok' | 'warn' | 'err' | 'info'
}) {
  return <span className={`badge ${tone === 'default' ? '' : tone}`}>{children}</span>
}

export function Spinner({ lg }: { lg?: boolean }) {
  return <span className={`spinner ${lg ? 'lg' : ''}`} />
}

export function Loading({ label = 'Loading…' }: { label?: string }) {
  return (
    <div className="center-box">
      <div className="row">
        <Spinner />
        <span className="muted">{label}</span>
      </div>
    </div>
  )
}

export function EmptyState({
  icon = '📭',
  title,
  desc,
  action,
}: {
  icon?: string
  title: string
  desc?: string
  action?: ReactNode
}) {
  return (
    <div className="empty">
      <div className="empty-icon">{icon}</div>
      <div className="empty-title">{title}</div>
      {desc && <div className="empty-desc">{desc}</div>}
      {action}
    </div>
  )
}

export function Tabs({
  tabs,
  value,
  onChange,
}: {
  tabs: { id: string; label: ReactNode }[]
  value: string
  onChange: (id: string) => void
}) {
  return (
    <div className="tabs">
      {tabs.map((t) => (
        <button
          key={t.id}
          className={`tab ${value === t.id ? 'active' : ''}`}
          onClick={() => onChange(t.id)}
        >
          {t.label}
        </button>
      ))}
    </div>
  )
}

export function ConfirmButton({
  onConfirm,
  children,
  message,
  variant = 'ghost',
  size = 'sm',
  disabled,
}: {
  onConfirm: () => void
  children: ReactNode
  message: string
  variant?: BtnProps['variant']
  size?: BtnProps['size']
  disabled?: boolean
}) {
  return (
    <Btn
      variant={variant}
      size={size}
      disabled={disabled}
      onClick={() => {
        if (window.confirm(message)) onConfirm()
      }}
    >
      {children}
    </Btn>
  )
}
