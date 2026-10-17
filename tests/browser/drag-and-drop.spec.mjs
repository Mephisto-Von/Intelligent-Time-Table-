/** Drag & drop a lesson between slots on the /timetable grid in a real
 *  (headless) browser, and confirm the move reaches the API and re-renders.
 *
 *  Usage: BASE=http://localhost:8126 node drag-and-drop.spec.mjs
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
const moves = []
page.on('request', (req) => {
  if (req.method() === 'PUT' && req.url().includes('/api/timetable/entries/')) moves.push(req.postData() ?? '')
})

// ---------------------------------------------------------------- login via UI
await page.goto(BASE + '/login', { waitUntil: 'networkidle' }).catch(() => {})
await page.fill('input[type=email]', DEMO.email)
await page.fill('input[type=password]', DEMO.password)
await page.getByRole('button', { name: /Sign in/ }).click()
await page.waitForURL((u) => !u.pathname.startsWith('/login'), { timeout: 20000 })

const plan2 = await page.evaluate(async () => {
  const token = localStorage.getItem('tt_token')
  const H = { Authorization: `Bearer ${token}` }
  const j = async (u) => (await fetch(u, { headers: H })).json()

  const tt = await j('/api/timetable')
  const entries = tt.entries.filter((e) => e.week === 0 && !e.locked)
  const slots = tt.slots.filter((s) => !s.is_break)
  const src = entries[0]
  if (!src) return { src: null }

  const classBusy = new Set(entries.map((e) => `${e.class_id}|${e.day}|${e.idx}`))
  const teacherBusy = new Set(entries.map((e) => `${e.teacher_id}|${e.day}|${e.idx}`))
  const candidates = slots
    .map((s) => `${s.day}|${s.idx}`)
    .filter((k) => !classBusy.has(`${src.class_id}|${k}`) && !teacherBusy.has(`${src.teacher_id}|${k}`) && !k.startsWith(`${src.day}|${src.idx}`))

  return {
    src,
    candidates,
    classId: src.class_id,
    fromDay: src.day,
    fromIdx: src.idx,
    entryId: src.id,
  }
})
if (!plan2 || !plan2.src) {
  console.log('FAIL no unlocked entry found to drag — the demo DB looks empty')
  await browser.close()
  process.exit(1)
}

// --------------------------------------------------------------- open the grid
await page.goto(BASE + '/timetable', { waitUntil: 'networkidle' })
await page.waitForSelector('.tt-table', { timeout: 20000 })
// .card-body select order: [View by, group, (cycle week)] -> pick the class groups[0]
const groupSelect = page.locator('.card-body select').nth(1)
const classOptions = await groupSelect.locator('option').evaluateAll((os) => os.map((o) => o.value).filter(Boolean))
check('the timetable grid offers class groups', classOptions.length > 0, JSON.stringify(classOptions))
const targetOption = classOptions.includes(plan2.classId) ? plan2.classId : classOptions[0]
await groupSelect.selectOption(targetOption)
await page.waitForTimeout(400)
check('the grid renders lessons in the class view', (await page.locator('.tt-lesson').count()) > 0)

/** Simulate HTML5 drag & drop between two (period, day) cells. */
async function dragTo(fromDay, fromIdx, toDay, toIdx) {
  return page.evaluate(([fd, fi, td, ti]) => {
    const rows = [...document.querySelectorAll('.tt-table tbody tr')]
    const findRow = (idx) => rows.find((r) => r.querySelector('.tt-period .p-idx')?.textContent.trim() === `P${idx}`)
    const cell = (row, day) => {
      const tds = [...row.querySelectorAll('td')]
      return tds[day] // td[0] is the period column, td[1..] are Mon..Fri
    }
    const srcRow = findRow(fi)
    const tgtRow = findRow(ti)
    if (!srcRow || !tgtRow) return 'row missing'
    const srcCell = cell(srcRow, fd)
    const lesson = srcCell?.querySelector('.tt-lesson')
    const tgtCell = cell(tgtRow, td)
    if (!lesson || !tgtCell) return 'cell missing'
    if (tgtCell.querySelector('.tt-lesson')) return 'occupied'
    const dt = new DataTransfer()
    lesson.dispatchEvent(new DragEvent('dragstart', { bubbles: true, cancelable: true, dataTransfer: dt }))
    tgtCell.dispatchEvent(new DragEvent('dragover', { bubbles: true, cancelable: true, dataTransfer: dt }))
    tgtCell.dispatchEvent(new DragEvent('drop', { bubbles: true, cancelable: true, dataTransfer: dt }))
    lesson.dispatchEvent(new DragEvent('dragend', { bubbles: true, cancelable: true, dataTransfer: dt }))
    return 'dispatched'
  }, [fromDay, fromIdx, toDay, toIdx])
}

/** Wait for the move to be reflected: the API entry sits at the target slot
 *  *and* the grid has repainted that cell with a lesson. Polled from Node —
 *  page.waitForFunction cannot await an async predicate (it resolves on the
 *  first poll because the predicate returns a Promise), so it must not be used
 *  for a check that needs a fetch. */
async function waitMoved(candidate, timeout = 8000) {
  const [day, idx] = candidate.split('|').map(Number)
  const deadline = Date.now() + timeout
  for (;;) {
    const ok = await page.evaluate(async ([eid, d, i]) => {
      const token = localStorage.getItem('tt_token')
      const res = await fetch('/api/timetable', { headers: { Authorization: `Bearer ${token}` } })
      const tt = await res.json()
      const e = tt.entries.find((x) => x.id === eid)
      if (!e || e.day !== d || e.idx !== i) return false
      const rows = [...document.querySelectorAll('.tt-table tbody tr')]
      const row = rows.find((r) => r.querySelector('.tt-period .p-idx')?.textContent.trim() === `P${i}`)
      return !!row?.querySelectorAll('td')[d]?.querySelector('.tt-lesson')
    }, [plan2.entryId, day, idx])
    if (ok) return
    if (Date.now() > deadline) throw new Error(`move to ${candidate} not reflected within ${timeout}ms`)
    await page.waitForTimeout(75)
  }
}

// ------------------------------------------------------- drag until a legal move
let done = null
let attempts = 0
let attemptLog = []
for (const candidate of plan2.candidates) {
  const [day, idx] = candidate.split('|').map(Number)
  attempts++
  attemptLog.push(candidate)
  const res = await dragTo(plan2.fromDay, plan2.fromIdx, day, idx)
  if (res !== 'dispatched') {
    check(`candidate ${candidate} is a droppable empty cell`, false, res)
    continue
  }
  try {
    await waitMoved(candidate, 4000)
    done = { day, idx, attempts }
    break
  } catch {
    // the drop was refused (e.g. a conflict the test cannot predict) — try the next empty slot
  }
}
check('a drag & drop moves the lesson to an empty legal cell', !!done, `attempted ${attempts} candidate(s) ${attemptLog.join(',')}; src P${plan2.fromIdx}/day${plan2.fromDay}`)
check('each drag attempt sent exactly one move request', moves.length === attempts, `${moves.length} PUT(s) for ${attempts} attempt(s)`)
if (done) {
  const last = JSON.parse(moves[moves.length - 1] || '{}')
  check('the last move request targets the dropped slot', last.day === done.day && last.idx === done.idx, JSON.stringify(last))
}

if (done) {
  const [day, idx] = [done.day, done.idx]
  const cellText = await page.evaluate(([d, i]) => {
    const rows = [...document.querySelectorAll('.tt-table tbody tr')]
    const row = rows.find((r) => r.querySelector('.tt-period .p-idx')?.textContent.trim() === `P${i}`)
    return row?.querySelectorAll('td')[d]?.querySelector('.tt-lesson')?.textContent ?? ''
  }, [day, idx])
  check('the dropped cell shows the moved lesson', cellText.length > 0, cellText)

  const otherCell = await page.evaluate(([d, i]) => {
    const rows = [...document.querySelectorAll('.tt-table tbody tr')]
    const row = rows.find((r) => r.querySelector('.tt-period .p-idx')?.textContent.trim() === `P${i}`)
    return row?.querySelectorAll('td')[d]?.querySelector('.tt-lesson')?.textContent ?? ''
  }, [plan2.fromDay, plan2.fromIdx])
  check('the source cell no longer holds the lesson', otherCell.length === 0 || otherCell !== cellText, otherCell)
}
check('no uncaught page errors', errors.length === 0, errors.slice(0, 3))

await browser.close()
console.log('\n' + (failed ? 'SOME CHECKS FAILED' : 'ALL PASS'))
process.exit(failed ? 1 : 0)
