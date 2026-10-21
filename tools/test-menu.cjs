// Run with Playwright on NODE_PATH against a running web build.
const assert = require('node:assert/strict');
const { mkdirSync, writeFileSync } = require('node:fs');
const { chromium } = require('playwright');
const directory = process.argv[2] || 'art/review/title-menu';
const url = process.argv[3] || 'http://127.0.0.1:8080/';

(async () => {
  mkdirSync(directory, { recursive: true });
  const browser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: false, args: ['--no-sandbox'] });
  const probe = await browser.newPage({ viewport: null });
  const deviceScaleFactor = await probe.evaluate(() => devicePixelRatio);
  await probe.close();
  const page = await browser.newPage({ viewport: { width: 1280, height: 720 }, deviceScaleFactor });
  const logs = [], errors = [], checks = [];
  page.on('console', message => {
    logs.push(message.text());
    if (/ready|error|panicked/i.test(message.text())) console.log(message.text());
    if (message.type() === 'error') errors.push(message.text());
  });
  page.on('pageerror', error => { errors.push(String(error)); console.error(String(error)); });
  page.on('response', response => { if (response.status() >= 400) errors.push(`${response.status()} ${response.url()}`); });
  const key = async key => { await page.keyboard.press(key, { delay: 100 }); await page.waitForTimeout(300); };
  const screenshot = name => page.screenshot({ path: `${directory}/${name}.png` });
  try {
    const ready = page.waitForEvent('console', { predicate: m => m.text().includes('Title screen ready'), timeout: 180000 });
    await page.goto(url); await ready; await page.waitForTimeout(5000);
    assert.equal(await page.title(), 'Let It Ride');
    await screenshot('title-1280');
    await key('Escape');
    assert(!logs.some(line => line.includes('First-person player ready')));
    await key('Tab'); await key('Enter');
    await screenshot('title-settings');
    await key('Shift+Tab'); await key('Enter'); // Save settings, returning to title.
    assert(await page.evaluate(() => localStorage.getItem('casino-game.graphics.v1')));
    assert(!logs.some(line => line.includes('First-person player ready')));
    checks.push('Launch shows the title; Escape does not start gameplay; title Settings saves and returns without spawning a player');
    await key('Tab'); await key('Enter'); await key('Escape');
    await page.setViewportSize({ width: 640, height: 480 });
    await page.waitForTimeout(1500); await screenshot('title-640');
    await page.setViewportSize({ width: 1280, height: 720 });
    await page.waitForTimeout(1000);
    const started = page.waitForEvent('console', { predicate: m => /Opening cutscene ready|Gameplay resumed/.test(m.text()), timeout: 180000 });
    await page.mouse.click(900, 350, { delay: 100 });
    if ((await started).text().includes('Opening cutscene ready')) {
      const playing = page.waitForEvent('console', { predicate: m => m.text().includes('Gameplay resumed'), timeout: 10000 });
      await page.keyboard.press('Enter', { delay: 1200 });
      await playing;
    }
    await page.waitForTimeout(1500);
    await screenshot('started');
    await page.bringToFront();
    await page.mouse.click(400, 360); await page.waitForTimeout(1000);
    assert(await page.evaluate(() => !!document.pointerLockElement), 'Gameplay must still capture the mouse');
    await key('Escape'); await screenshot('paused');
    assert(!(await page.evaluate(() => !!document.pointerLockElement)));
    await key('Tab'); await key('Enter'); await key('Escape');
    const resumed = page.waitForEvent('console', { predicate: m => m.text().includes('Gameplay resumed'), timeout: 10000 });
    await key('Enter'); await resumed;
    assert.equal(logs.filter(line => line.includes('First-person player ready')).length, 1);
    checks.push('Clicking Start enters gameplay; mouse capture, pause, settings cancellation and resume preserve the same player');
    assert.equal(errors.length, 0, errors.join('\n'));
  } finally {
    writeFileSync(`${directory}/results.json`, JSON.stringify({ deviceScaleFactor, checks, errors, logs }, null, 2));
    await browser.close();
  }
  console.log(checks.join('\n'));
})().catch(error => { console.error(error); process.exitCode = 1; });
