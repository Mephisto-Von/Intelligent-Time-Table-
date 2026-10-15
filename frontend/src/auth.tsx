import { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react'
import type { ReactNode } from 'react'
import { api, setToken } from './api'
import type { LoginResponse, MeResponse, School, User } from './api'

interface SignupInput {
  account_name: string
  school_name: string
  name: string
  email: string
  password: string
}

interface AuthContextValue {
  user: User | null
  school: School | null
  ready: boolean
  isStaff: boolean
  isAdmin: boolean
  login: (email: string, password: string) => Promise<void>
  signup: (input: SignupInput) => Promise<void>
  demoLogin: () => Promise<void>
  logout: () => void
  reload: () => Promise<void>
}

const AuthContext = createContext<AuthContextValue | null>(null)

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<User | null>(null)
  const [school, setSchool] = useState<School | null>(null)
  const [ready, setReady] = useState(false)

  const reload = useCallback(async () => {
    if (!localStorage.getItem('tt_token')) {
      setUser(null)
      setSchool(null)
      return
    }
    try {
      const me = await api<MeResponse>('/api/auth/me')
      setUser(me.user)
      setSchool(me.school)
    } catch {
      setToken(null)
      setUser(null)
      setSchool(null)
    }
  }, [])

  useEffect(() => {
    reload().finally(() => setReady(true))
  }, [reload])

  const login = useCallback(async (email: string, password: string) => {
    const res = await api<LoginResponse>('/api/auth/login', { body: { email, password } })
    setToken(res.token)
    await reload()
  }, [reload])

  const signup = useCallback(async (input: SignupInput) => {
    const res = await api<LoginResponse>('/api/auth/signup', { body: input })
    setToken(res.token)
    await reload()
  }, [reload])

  const demoLogin = useCallback(async () => {
    const res = await api<LoginResponse & { school_id: string }>('/api/demo/seed', {
      body: {},
    })
    setToken(res.token)
    await reload()
  }, [reload])

  const logout = useCallback(() => {
    setToken(null)
    setUser(null)
    setSchool(null)
  }, [])

  const value = useMemo<AuthContextValue>(
    () => ({
      user,
      school,
      ready,
      isStaff: user?.role === 'admin' || user?.role === 'scheduler',
      isAdmin: user?.role === 'admin',
      login,
      signup,
      demoLogin,
      logout,
      reload,
    }),
    [user, school, ready, login, signup, demoLogin, logout, reload],
  )

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>
}

export function useAuth(): AuthContextValue {
  const ctx = useContext(AuthContext)
  if (!ctx) throw new Error('useAuth must be used inside AuthProvider')
  return ctx
}
