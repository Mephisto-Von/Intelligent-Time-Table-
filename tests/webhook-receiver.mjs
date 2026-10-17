/** Tiny webhook receiver used as the notification sink during tests.
 *
 *  Usage: node tests/webhook-receiver.mjs <listen-port> <log-file>
 *  Every POSTed JSON body is appended as a line to the log file, and
 *  tests/http/common.py reads that file back to assert real deliveries.
 */
import http from 'node:http'
import fs from 'node:fs'

const port = Number(process.argv[2] || 9099)
const log = process.argv[3] || '/tmp/timetablemaster-webhook.log'
fs.writeFileSync(log, '')

http
  .createServer((req, res) => {
    let body = ''
    req.on('data', (c) => (body += c))
    req.on('end', () => {
      try {
        fs.appendFileSync(log, body + '\n')
      } catch (e) {
        console.error('record failed:', e)
      }
      res.writeHead(200, { 'content-type': 'application/json' })
      res.end('{"ok":true}')
    })
  })
  .listen(port, () => console.log(`webhook receiver on :${port}, logging to ${log}`))
