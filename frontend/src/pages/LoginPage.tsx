import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useAuth } from '../auth'
import { Btn, Input, Field, Spinner, errMsg } from '../ui'

export default function LoginPage() {
  const { login, signup, demoLogin } = useAuth()
  const navigate = useNavigate()
  const [mode, setMode] = useState<'login' | 'signup'>('login')
  const [busy, setBusy] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [form, setForm] = useState({
    email: '',
    password: '',
    account_name: '',
    school_name: '',
    name: '',
  })

  const set = (k: string) => (e: React.ChangeEvent<HTMLInputElement>) =>
    setForm((f) => ({ ...f, [k]: e.target.value }))

  async function submit(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    setBusy('submit')
    try {
      if (mode === 'login') {
        await login(form.email, form.password)
      } else {
        await signup({
          account_name: form.account_name || form.name,
          school_name: form.school_name,
          name: form.name,
          email: form.email,
          password: form.password,
        })
      }
      navigate('/', { replace: true })
    } catch (err) {
      setError(errMsg(err))
    } finally {
      setBusy(null)
    }
  }

  async function demo() {
    setError(null)
    setBusy('demo')
    try {
      await demoLogin()
      navigate('/', { replace: true })
    } catch (err) {
      setError(errMsg(err))
    } finally {
      setBusy(null)
    }
  }

  return (
    <div className="auth-wrap">
      <div className="auth-hero">
        <div className="brand-mark" style={{ width: 44, height: 44, fontSize: 19 }}>
          TM
        </div>
        <h1>
          Timetables that
          <br />
          build themselves.
        </h1>
        <p>
          Intelligent scheduling for schools and universities — automatic timetable generation,
          substitute cover, leave management and shared calendars in one place.
        </p>
        <div className="auth-features">
          <div className="auth-feature">
            <span>⚡</span>
            <span>
              <strong>Constraint solver</strong> — clash-free schedules with gap-free class days,
              room requirements and teacher availability.
            </span>
          </div>
          <div className="auth-feature">
            <span>🔁</span>
            <span>
              <strong>Cover & substitutes</strong> — lock a teacher's absence and pick ranked
              replacement teachers in seconds.
            </span>
          </div>
          <div className="auth-feature">
            <span>🌴</span>
            <span>
              <strong>Leave management</strong> — requests, approvals and instant impact preview
              on the timetable.
            </span>
          </div>
          <div className="auth-feature">
            <span>🗓️</span>
            <span>
              <strong>Drag & drop editing</strong> — fine-tune by hand; locked lessons survive every
              regeneration.
            </span>
          </div>
        </div>
      </div>

      <div className="auth-panel">
        <div className="auth-card">
          <h2>{mode === 'login' ? 'Welcome back' : 'Create your school'}</h2>
          <div className="sub">
            {mode === 'login'
              ? 'Sign in to manage your timetable.'
              : 'Set up a new school workspace in a minute.'}
          </div>

          {error && <div className="auth-error">{error}</div>}

          <form onSubmit={submit}>
            {mode === 'signup' && (
              <>
                <Field label="Your name">
                  <Input value={form.name} onChange={set('name')} placeholder="Jane Doe" required />
                </Field>
                <Field label="Organisation / school name">
                  <Input
                    value={form.school_name}
                    onChange={set('school_name')}
                    placeholder="Springfield High"
                    required
                  />
                </Field>
              </>
            )}
            <Field label="Email">
              <Input
                type="email"
                value={form.email}
                onChange={set('email')}
                placeholder="you@school.edu"
                required
              />
            </Field>
            <Field label="Password" hint={mode === 'signup' ? 'At least 6 characters.' : undefined}>
              <Input
                type="password"
                value={form.password}
                onChange={set('password')}
                placeholder="••••••••"
                required
                minLength={mode === 'signup' ? 6 : undefined}
              />
            </Field>

            <Btn variant="primary" type="submit" block disabled={busy !== null}>
              {busy === 'submit' ? <Spinner /> : mode === 'login' ? 'Sign in' : 'Create school'}
            </Btn>
          </form>

          <div className="divider">or</div>

          <Btn block onClick={demo} disabled={busy !== null}>
            {busy === 'demo' ? <Spinner /> : '🚀 Explore the live demo school'}
          </Btn>

          <div className="row" style={{ justifyContent: 'center', marginTop: 16 }}>
            <span className="small muted">
              {mode === 'login' ? 'New here?' : 'Already have an account?'}
            </span>
            <Btn
              size="sm"
              variant="ghost"
              onClick={() => {
                setMode(mode === 'login' ? 'signup' : 'login')
                setError(null)
              }}
            >
              {mode === 'login' ? 'Create a school workspace' : 'Sign in instead'}
            </Btn>
          </div>
        </div>
      </div>
    </div>
  )
}
