#!/usr/bin/env node
/** Render-check the deployed defense deck in headless Chromium.
 *
 *  Usage: node docs/check-deck-live.cjs https://<deployment>.vercel.app/
 *
 *  Verifies the live URL the way a presenter's browser sees it: HTTP 200,
 *  all 19 slides in the DOM, the 4 team names, scale-to-fit applied, the
 *  slide counter and its #deep-link / keyboard navigation, a working .pptx
 *  download (valid zip), and zero uncaught page errors.
 *
 *  Needs Playwright, which tests/browser already provides:
 *    cd tests/browser && node ../../docs/check-deck-live.cjs <url>
 */
'use strict';

const fs = require('fs');
const path = require('path');

const TARGET = (process.argv[2] || '').replace(/\/+$/, '');
if (!TARGET) {
  console.error('usage: node docs/check-deck-live.cjs https://<deployment>.vercel.app/');
  process.exit(64);
}

let failures = 0;
function check(ok, label, detail) {
  console.log(`${ok ? 'PASS' : 'FAIL'}  ${label}${detail ? ` — ${detail}` : ''}`);
  if (!ok) failures++;
}

async function main() {
  // playwright lives in tests/browser/node_modules (this file sits in docs/),
  // so search there explicitly, then cwd, before falling back to a plain require.
  const playwrightPath = require.resolve('playwright', {
    paths: [path.join(__dirname, '..', 'tests', 'browser'), process.cwd()],
  });
  const { chromium } = require(playwrightPath);

  // --- static assets over HTTP, before opening a browser -------------------
  for (const [p, name, type] of [
    ['/', 'index.html', 'text/html'],
    ['/FYP-Presentation.pptx', 'pptx', 'application/'],
  ]) {
    const res = await fetch(TARGET + p);
    const buf = Buffer.from(await res.arrayBuffer());
    const ctype = res.headers.get('content-type') || '';
    check(res.status === 200, `GET ${p} -> 200`, `http=${res.status}`);
    check(ctype.includes(type), `GET ${p} content-type`, ctype);
    if (name === 'index.html') {
      const local = fs.readFileSync(path.join(__dirname, 'deck', 'index.html'));
      const byteOk = buf.equals(local) || (buf.length >= local.length && buf.subarray(0, local.length).equals(local));
      check(byteOk, `GET / serves the local deck byte-exact`, `${buf.length} bytes`);
    } else {
      check(buf.subarray(0, 2).toString() === 'PK', `GET ${p} is a valid zip`, `${buf.length} bytes`);
    }
  }

  // --- live render ----------------------------------------------------------
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
  const pageErrors = [];
  page.on('pageerror', (e) => pageErrors.push(String(e)));

  await page.goto(TARGET + '/', { waitUntil: 'load' });

  const slides = await page.locator('.slide').count();
  check(slides === 19, '19 slides in the DOM', `found ${slides}`);

  const bodyText = await page.locator('body').innerText();
  for (const name of ['Hira Hassan', 'Iman Aftab', 'Jawaria Sabir', 'Hamza Abdul Karim']) {
    check(bodyText.includes(name), `team name "${name}"`);
  }

  const transform = await page.locator('#deck').evaluate((el) => getComputedStyle(el).transform);
  check(transform !== 'none', 'scale-to-fit applied to #deck', transform);

  const counterText = async () => (await page.locator('#counter').innerText()).trim();
  check((await counterText()) === '1 / 19', 'counter starts at 1 / 19', await counterText());

  // #deep-link opens straight on that slide
  await page.goto(TARGET + '/#19', { waitUntil: 'load' });
  check((await counterText()) === '19 / 19', '#19 deep-link opens on 19 / 19', await counterText());

  // keyboard navigation from a deep-linked slide
  await page.goto(TARGET + '/#5', { waitUntil: 'load' });
  await page.keyboard.press('ArrowRight');
  await page.waitForTimeout(100);
  check((await counterText()) === '6 / 19', 'ArrowRight advances 5 -> 6', await counterText());

  // the .pptx download link fetches a real file
  const href = await page.locator('#dl').getAttribute('href');
  const dl = await fetch(new URL(href, TARGET + '/').href);
  const dlBuf = Buffer.from(await dl.arrayBuffer());
  check(dl.status === 200 && dlBuf.subarray(0, 2).toString() === 'PK', 'pptx download link serves a zip', `http=${dl.status} ${dlBuf.length} bytes`);

  await browser.close();

  check(pageErrors.length === 0, 'zero uncaught page errors', pageErrors.join('; ') || 'none');

  console.log(failures === 0 ? '\nALL CHECKS PASSED' : `\n${failures} CHECK(S) FAILED`);
  process.exit(failures === 0 ? 0 : 1);
}

main().catch((e) => {
  console.error('check-deck-live crashed:', e);
  process.exit(2);
});
