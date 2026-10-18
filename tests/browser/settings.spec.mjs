/** Settings page (notifications + activity log) and the timetable Share
 *  dialog: create a public link from the UI, copy it, then revoke it and
 *  confirm removal against the API.
 *
 *  Usage: BASE=http://localhost:8126 node settings.spec.mjs
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

await page.goto(BASE + '/', { waitUntil: 'domcontentloaded' })
await page.goto(BASE + '/login', { waitUntil: 'networkidle' }).catch(() => {})
await page.fill('input[type=email]', DEMO.email)
await page.fill('input[type=password]', DEMO.password)
await page.getByRole('button', { name: /Sign in/ }).click()
await page.waitForURL((u) => !u.pathname.startsWith('/login'), { timeout: 20000 })

// ------------------------------------------------------------------- settings
await page.goto(BASE + '/settings')
await page.waitForSelector('.card', { timeout: 15000 })

const notifCard = page.locator('.card', { hasText: 'Notifications' }).first()
await notifCard.waitFor({ timeout: 10000 })
const notifText = await notifCard.innerText()
check('notifications card reports the configured channel as enabled',
  /webhook/.test(notifText) && /enabled/.test(notifText), notifText.replace(/\s+/g, ' ').slice(0, 160))
check('notifications card lists the notification events',
  (await notifCard.getByText('Substitute teacher assigned').count()) > 0)

await notifCard.getByRole('button', { name: /Send test notification/ }).click()
const toastText = await page.locator('.toast').first().innerText({ timeout: 8000 }).catch(() => '')
check('send-test-notification gives visible feedback', toastText.length > 0, toastText)

// ---------------------------------------- school settings -> activity log entry
const schoolCard = page.locator('.card').filter({ has: page.getByRole('button', { name: /Save school settings/ }) })
await schoolCard.waitFor({ timeout: 10000 })
const nameInput = schoolCard.locator('input').first()
const origName = await nameInput.inputValue()
await nameInput.fill(origName + ' (E2E)')
await schoolCard.getByRole('button', { name: /Save school settings/ }).click()

const auditCard = page.locator('.card', { hasText: 'Activity log' }).first()
await auditCard.waitFor({ timeout: 10000 })
// Earlier suites write audit rows too, and the list refetches after this save,
// so wait for *this* save's row instead of reading whatever is currently first.
const savedRow = auditCard.locator('table.tbl tbody tr', { hasText: 'Updated school settings' }).first()
const logged = await savedRow.waitFor({ timeout: 15000 }).then(() => true).catch(() => false)
check('saving school settings is written to the activity log', logged, await auditCard.innerText().catch(() => ''))
const rowText = logged ? (await savedRow.innerText()).replace(/\s+/g, ' ') : ''
const apiTop = await page.evaluate(async () => {
  const r = await fetch('/api/audit?limit=50', {
    headers: { Authorization: 'Bearer ' + localStorage.getItem('tt_token') },
  })
  const d = await r.json()
  return (d.items ?? [])[0] ?? null
})
check('the newest log entry is the change just saved', apiTop?.action === 'school.update', JSON.stringify(apiTop ?? {}))
check('the newest log entry names the actor and the change',
  !!apiTop && rowText.includes(apiTop.actor_name) && rowText.includes(apiTop.summary),
  `row="${rowText}" api=${JSON.stringify(apiTop ?? {})}`)

// restore the demo school name so the fixture is left untouched
await nameInput.fill(origName)
await schoolCard.getByRole('button', { name: /Save school settings/ }).click()
await page.waitForTimeout(400)
const restored = await page.evaluate(async () => {
  const r = await fetch('/api/school', { headers: { Authorization: 'Bearer ' + localStorage.getItem('tt_token') } })
  return (await r.json()).name
})
check('the demo school name is restored', restored === origName, `${restored} != ${origName}`)

// --------------------------------------------------------------- share dialog
await page.goto(BASE + '/timetable')
await page.waitForSelector('.tt-table', { timeout: 20000 })
const groupSelect = page.locator('.card-body select').nth(1)
await groupSelect.selectOption({ index: 1 })
await page.waitForTimeout(400)

await page.getByRole('button', { name: /Share/ }).click()
const modal = page.locator('.modal')
await modal.waitFor({ timeout: 10000 })
check('the share dialog opens', (await modal.innerText()).includes('timetable'))

// The dialog fetches existing links when it opens. Wait for that fetch to settle
// before looking for the Create button: counting it immediately can see zero (the
// "Loading links…" placeholder is still up) and then click nothing at all.
await modal.getByText('Loading links…').waitFor({ state: 'hidden', timeout: 20000 }).catch(() => {})
const createBtn = modal.getByRole('button', { name: /Create public link/ })
const linkActive = modal.getByText('Public link active')
if (!(await linkActive.isVisible().catch(() => false))) {
  const posted = page.waitForResponse(
    (r) => r.url().includes('/api/public-links') && r.request().method() === 'POST',
    { timeout: 20000 },
  )
  await createBtn.click({ timeout: 20000 })
  const res = await posted
  check('creating a link from the dialog succeeds', res.ok(), `${res.status()} ${await res.text().catch(() => '')}`)
}
await linkActive.waitFor({ timeout: 20000 })
check('the dialog shows the live link after creating one', (await modal.innerText()).includes('Public link active'))

const countLinks = async () =>
  page.evaluate(async () => {
    const r = await fetch('/api/public-links', {
      headers: { Authorization: 'Bearer ' + localStorage.getItem('tt_token') },
    })
    const d = await r.json()
    return (d.items ?? []).length
  })
const before = await countLinks()
check('the API lists the link created from the dialog', before > 0, `count=${before}`)

await modal.getByRole('button', { name: /Copy link/ }).click()
const copyToast = await page.locator('.toast').first().innerText({ timeout: 8000 }).catch(() => '')
check('copying the link gives feedback', copyToast.length > 0, copyToast)

// revoke (register the confirm handler before the click) and verify via the API
page.once('dialog', (d) => d.accept())
await modal.getByRole('button', { name: 'Revoke' }).first().click()
await page.waitForSelector('text=Create public link', { timeout: 10000 })
const after = await countLinks()
check('revoking removes the link server-side', after === before - 1, `before=${before} after=${after}`)

check('no uncaught page errors', errors.length === 0, errors.slice(0, 3))

await browser.close()
console.log('\n' + (failed ? 'SOME CHECKS FAILED' : 'ALL PASS'))
process.exit(failed ? 1 : 0)
