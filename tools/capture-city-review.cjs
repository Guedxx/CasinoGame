// Run with Playwright available on NODE_PATH; captures the running browser game.
const { chromium } = require('playwright');
const { mkdirSync } = require('node:fs');
const { resolve } = require('node:path');
(async () => {
  const directory = resolve(process.argv[2] || 'art/review/current');
  mkdirSync(directory, { recursive: true });
  const browser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: false,
    args: ['--no-sandbox', '--ozone-platform=x11', '--enable-unsafe-webgpu',
      '--use-angle=vulkan', '--enable-features=Vulkan,VulkanFromANGLE'] });
  try {
    const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
    const errors = [];
    page.on('pageerror', error => errors.push(String(error)));
    page.on('response', response => { if (response.status() >= 400) errors.push(`${response.status()} ${response.url()}`); });
    page.on('console', message => {
      console.log(message.type(), message.text());
      if (message.type() === 'error') errors.push(message.text());
    });
    const titleReady = page.waitForEvent('console', { predicate: m => m.text().includes('Title screen ready'), timeout: 180000 });
    const started = page.waitForEvent('console', { predicate: m => /Opening cutscene ready|Gameplay resumed/.test(m.text()), timeout: 180000 });
    await page.goto('http://127.0.0.1:8080');
    await titleReady;
    await page.keyboard.press('Enter');
    if ((await started).text().includes('Opening cutscene ready')) {
      const playing = page.waitForEvent('console', { predicate: m => m.text().includes('Gameplay resumed'), timeout: 10000 });
      await page.keyboard.press('Enter', { delay: 1200 });
      await playing;
    }
    await page.bringToFront();
    await page.waitForTimeout(3000);
    for (const [key, name] of [
      ['F9', '00-apartment'], ['F1', '01-entrance-wall'], ['F2', '02-corridor'],
      ['F3', '03-east-window'], ['F4', '04-bedroom-window'], ['F5', '05-building-context'],
      ['F7', '06-court-from-window'], ['F8', '07-court-detail'],
      ['F10', '08-stair-landing'],
      ['F11', '09-occupied-exterior'],
    ]) {
      const changed = page.waitForEvent('console', { predicate: m => m.text().includes('City review camera'), timeout: 10000 });
      await page.keyboard.press(key);
      await changed;
      await page.waitForTimeout(1800);
      await page.screenshot({ path: `${directory}/${name}.png` });
    }
    if (errors.length) throw new Error(JSON.stringify(errors));
    console.log(`CAPTURE_OK ${directory}: 10 screenshots; no browser or asset errors`);
  } finally {
    await browser.close();
  }
})().catch(error => { console.error(error); process.exit(1); });
