/** Anonymous public share page: create a link as staff, open /public/:token
 *  without signing in, verify the read-only grid, then revoked -> not available.
 *
 *  Usage: BASE=http://localhost:8126 node public-page.spec.mjs
 */
import { chromium } from 'playwright'

const BASE = (process.env.BASE || 'http://localhost:8126').replace(/\/$/, '')
const DEMO = { email: 'admin@demo.school', password: 'demo1234' }

let failed = false
const check = (name, cond, extra = '') => {
  if (!cond) failed = true
  console.log(`${cond ? 'PASS' : 'FAIL'} ${name}${cond ? '' : '  -> ' + extra}`)
}

const browser = await chromium.launch()
const page = await browser.newPage()
const errors = []
page.on('pageerror', (e) => errors.push(String(e)))

// staff session (the API calls run inside the page so cookies/token line up)
await page.goto(BASE + '/', { waitUntil: 'domcontentloaded' })
await page.goto(BASE + '/login', { waitUntil: 'networkidle' }).catch(() => {})
await page.fill('input[type=email]', DEMO.email)
await page.fill('input[type=password]', DEMO.password)
await page.getByRole('button', { name: /Sign in/ }).click()
await page.waitForURL((u) => !u.pathname.startsWith('/login'), { timeout: 20000 })

// create a public link for the first class through the API
const link = await page.evaluate(async () => {
  const token = localStorage.getItem('tt_token')
  const j = async (u, opts = {}) => {
    const r = await fetch(u, { ...opts, headers: { Authorization: `Bearer ${token}`, ...(opts.headers || {}) } })
    return { status: r.status, body: await r.json().catch(() => ({})) }
  }
  const classes = (await j('/api/setup/all')).body.classes
  const classId = classes[0].id
  const created = await j('/api/public-links', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ entity: 'class', entity_id: classId }),
  })
  const timetable = (await j('/api/timetable')).body
  const expected = timetable.entries.filter((e) => e.class_id === classId && e.week === 0)
  return {
    status: created.status,
    path: created.body.path,
    id: created.body.id,
    expected: expected.length,
    className: classes[0].name,
  }
})
check('staff can create a public share link', link?.path?.startsWith('/public/') && link.status < 300, JSON.stringify(link ?? {}))

// anonymous visit
const anon = await browser.newPage()
const anonErrors = []
anon.on('pageerror', (e) => anonErrors.push(String(e)))
await anon.goto(BASE + link.path, { waitUntil: 'networkidle' })
check('the public page renders a read-only timetable', (await anon.locator('.tt-table').count()) === 1, await anon.title())
check('the anonymous page carries the class name', (await anon.locator('.public-head').innerText()).includes(link.className), await anon.locator('.public-head').innerText())
const rendered = await anon.locator('.tt-lesson').count()
check('the anonymous grid shows exactly the shared week', rendered === link.expected, `${rendered} vs ${link.expected}`)
const editControls = await anon.locator('.tt-lesson[draggable="true"]').count()
check('the public grid is not draggable/editable', editControls === 0, `${editControls} draggable lessons`)
const loginVisible = await anon.locator('input[type=email]').count()
check('no sign-in form is shown to anonymous visitors', loginVisible === 0, `${loginVisible} login fields`)

// a revoked/unknown token shows the friendly state
await anon.goto(BASE + '/public/definitely-not-a-token', { waitUntil: 'networkidle' })
check('unknown tokens show the not-available state', (await anon.innerText('body')).includes('This timetable link is not available'))

// revoke and confirm the link dies
await page.evaluate(async (id) => {
  const token = localStorage.getItem('tt_token')
  await fetch(`/api/public-links/${id}`, { method: 'DELETE', headers: { Authorization: `Bearer ${token}` } })
}, link.id)
await anon.goto(BASE + link.path, { waitUntil: 'networkidle' })
check('revoking the link breaks the public page', (await anon.innerText('body')).includes('This timetable link is not available'))

check('no uncaught page errors (staff)', errors.length === 0, errors.slice(0, 3))
check('no uncaught page errors (anonymous)', anonErrors.length === 0, anonErrors.slice(0, 3))

await browser.close()
console.log('\n' + (failed ? 'SOME CHECKS FAILED' : 'ALL PASS'))
process.exit(failed ? 1 : 0)
