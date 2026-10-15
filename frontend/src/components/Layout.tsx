import { NavLink, Outlet, useLocation, useNavigate } from 'react-router-dom'
import type { ReactNode } from 'react'
import { useAuth } from '../auth'
import { Btn, Toaster, initialsOf } from '../ui'

interface NavEntry {
  to: string
  label: string
  icon: string
  roles?: string[]
  end?: boolean
}

const NAV: { section: string; items: NavEntry[] }[] = [
  {
    section: 'Plan',
    items: [
      { to: '/', label: 'Dashboard', icon: '📊', end: true },
      { to: '/timetable', label: 'Timetable', icon: '🗓️' },
      { to: '/cover', label: 'Cover & Substitutes', icon: '🔁' },
      { to: '/leave', label: 'Leave', icon: '🌴' },
      { to: '/calendar', label: 'Calendar', icon: '📅' },
    ],
  },
  {
    section: 'Configure',
    items: [
      { to: '/setup', label: 'School setup', icon: '🧩', roles: ['admin', 'scheduler'] },
      { to: '/settings', label: 'Settings', icon: '⚙️', roles: ['admin'] },
    ],
  },
]

const TITLES: Record<string, string> = {
  '/': 'Dashboard',
  '/timetable': 'Timetable',
  '/setup': 'School setup',
  '/leave': 'Leave management',
  '/cover': 'Cover & substitutes',
  '/calendar': 'Calendar',
  '/settings': 'Settings',
}

export default function Layout() {
  const { user, school, isStaff, isAdmin, logout } = useAuth()
  const navigate = useNavigate()
  const location = useLocation()

  const role = user?.role ?? 'student'
  const visible = NAV.map((group) => ({
    ...group,
    items: group.items.filter((i) => !i.roles || i.roles.includes(role)),
  })).filter((g) => g.items.length > 0)

  const title =
    TITLES[location.pathname] ??
    (location.pathname.startsWith('/timetable') ? 'Timetable' : 'TimetableMaster')

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">
          <div className="brand-mark">TM</div>
          <div>
            <div className="brand-name">TimetableMaster</div>
            <div className="brand-sub">{school?.name ?? 'Intelligent scheduling'}</div>
          </div>
        </div>
        <nav className="nav">
          {visible.map((group) => (
            <div key={group.section}>
              <div className="nav-label">{group.section}</div>
              {group.items.map((item) => (
                <NavLink
                  key={item.to}
                  to={item.to}
                  end={item.end}
                  className={({ isActive }) => `nav-item ${isActive ? 'active' : ''}`}
                >
                  <span className="nav-icon">{item.icon}</span>
                  {item.label}
                </NavLink>
              ))}
            </div>
          ))}
        </nav>
        <div className="sidebar-foot">
          {isAdmin ? 'Administrator' : isStaff ? 'Scheduler' : role === 'teacher' ? 'Teacher' : 'Student'} access
        </div>
      </aside>

      <div className="main">
        <header className="topbar no-print">
          <div className="topbar-title">{title}</div>
          <div className="topbar-spacer" />
          <div className="user-chip">
            <div className="avatar">{initialsOf(user?.name ?? '?')}</div>
            <div className="user-meta">
              <div className="user-name">{user?.name}</div>
              <div className="user-role">{user?.role}</div>
            </div>
          </div>
          <Btn
            size="sm"
            variant="ghost"
            onClick={() => {
              logout()
              navigate('/login', { replace: true })
            }}
          >
            Sign out
          </Btn>
        </header>
        <Outlet />
      </div>
      <Toaster />
    </div>
  )
}

export function PageHead({
  title,
  sub,
  actions,
}: {
  title: string
  sub?: string
  actions?: ReactNode
}) {
  return (
    <div className="page-head">
      <div className="grow">
        <h1>{title}</h1>
        {sub && <div className="page-sub">{sub}</div>}
      </div>
      {actions}
    </div>
  )
}

export function Page({ children }: { children: ReactNode }) {
  return <main className="page">{children}</main>
}
