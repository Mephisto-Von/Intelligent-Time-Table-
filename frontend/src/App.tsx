import { Navigate, Route, Routes } from 'react-router-dom'
import Layout from './components/Layout'
import { useAuth } from './auth'
import { Loading } from './ui'
import LoginPage from './pages/LoginPage'
import DashboardPage from './pages/DashboardPage'
import TimetablePage from './pages/TimetablePage'
import SetupPage from './pages/SetupPage'
import LeavePage from './pages/LeavePage'
import CoverPage from './pages/CoverPage'
import CalendarPage from './pages/CalendarPage'
import SettingsPage from './pages/SettingsPage'
import PublicTimetablePage from './pages/PublicTimetablePage'

export default function App() {
  const { ready, user, isStaff, isAdmin } = useAuth()

  if (!ready) {
    return (
      <div className="center-box">
        <Loading label="Starting TimetableMaster…" />
      </div>
    )
  }

  return (
    <Routes>
      <Route path="/login" element={user ? <Navigate to="/" replace /> : <LoginPage />} />
      {/* Anonymous, read-only shared timetable — no session required. */}
      <Route path="/public/:token" element={<PublicTimetablePage />} />
      <Route element={user ? <Layout /> : <Navigate to="/login" replace />}>
        <Route index element={<DashboardPage />} />
        <Route path="timetable" element={<TimetablePage />} />
        <Route path="leave" element={<LeavePage />} />
        <Route path="cover" element={<CoverPage />} />
        <Route path="calendar" element={<CalendarPage />} />
        <Route path="setup" element={isStaff ? <SetupPage /> : <Navigate to="/" replace />} />
        <Route path="settings" element={isAdmin ? <SettingsPage /> : <Navigate to="/" replace />} />
        <Route path="*" element={<Navigate to="/" replace />} />
      </Route>
    </Routes>
  )
}
