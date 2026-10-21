// NODE_PATH must contain Playwright. TRACE=1 records separate Chrome traces; --self-test needs no browser.
const { appendFileSync, mkdirSync, writeFileSync } = require('node:fs');
const { resolve } = require('node:path');
const assert = require('node:assert/strict');

function summarize({times, start, end, duplicates = 0}) {
  assert(times.length > 2, 'No game frames captured');
  assert(times.every(Number.isFinite) && Number.isFinite(start) && Number.isFinite(end));
  const intervals = times.slice(1).map((time, index) => time - times[index]);
  assert(intervals.every(ms => ms >= 0), 'Acquisition timestamps must not decrease');
  const sorted = [...intervals].sort((a, b) => a - b);
  assert(end > start && times[0] >= start && times.at(-1) <= end);
  assert(Number.isInteger(duplicates) && duplicates >= 0 && duplicates < times.length);
  const seconds = (end - start) / 1000;
  const leading = times[0] - start;
  const trailing = end - times.at(-1);
  return { frames: times.length, seconds, fps: times.length / seconds,
    unique_textures: times.length - duplicates, duplicate_acquisitions: duplicates,
    unique_texture_fps: (times.length - duplicates) / seconds,
    p95_ms: sorted[Math.floor(sorted.length * 0.95)],
    leading_gap_ms: leading, trailing_gap_ms: trailing,
    max_gap_ms: Math.max(leading, trailing, sorted.at(-1)) };
}
assert.deepEqual(summarize({times: [10, 20, 30], start: 0, end: 100, duplicates: 1}), {
  frames: 3, seconds: 0.1, fps: 30, unique_textures: 2, duplicate_acquisitions: 1,
  unique_texture_fps: 20, p95_ms: 10, leading_gap_ms: 10, trailing_gap_ms: 70, max_gap_ms: 70,
});
assert.equal(summarize({times: [10, 10, 30], start: 0, end: 40}).p95_ms, 20);
assert.throws(() => summarize({times: [10, 9, 30], start: 0, end: 100}));
assert.throws(() => summarize({times: [10, 20, 30], start: 11, end: 100}));
assert.throws(() => summarize({times: [10, 20, 30], start: 0, end: 100, duplicates: 3}));

async function saveTrace(session, path) {
  const completed = new Promise(resolve => session.once('Tracing.tracingComplete', resolve));
  await session.send('Tracing.end');
  const { stream } = await completed;
  assert(stream, 'Chrome did not return a trace stream');
  writeFileSync(path, '');
  try {
    for (;;) {
      const { data, base64Encoded, eof } = await session.send('IO.read', { handle: stream, size: 1024 * 1024 });
      appendFileSync(path, base64Encoded ? Buffer.from(data, 'base64') : data);
      if (eof) break;
    }
  } finally {
    await session.send('IO.close', { handle: stream });
  }
}

async function main() {
  const { chromium } = require('playwright');
  const directory = resolve(process.argv[2] || 'art/review/performance');
  const url = process.argv[3] || 'http://127.0.0.1:8080';
  const rounds = Number(process.env.ROUNDS || 1);
  const sampleMs = Number(process.env.SAMPLE_MS || 8000);
  const trace = process.env.TRACE === '1';
  const setupKeys = (process.env.SETUP_KEYS || '').split(',').map(key => key.trim()).filter(Boolean);
  assert(Number.isInteger(rounds) && rounds > 0 && sampleMs >= 1000);
  mkdirSync(directory, { recursive: true });
  const browser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: false,
    args: ['--no-sandbox'] });
  let report;
  const persist = () => writeFileSync(`${directory}/results.json`, JSON.stringify(report, null, 2) + '\n');
  try {
    const browserSession = await browser.newBrowserCDPSession();
    const { gpu } = await browserSession.send('SystemInfo.getInfo');
    const viewport = { width: Number(process.env.VIEWPORT_WIDTH || 1280), height: Number(process.env.VIEWPORT_HEIGHT || 720) };
    assert(Object.values(viewport).every(n => Number.isInteger(n) && n > 0));
    const probe = await browser.newPage({ viewport: null });
    const deviceScaleFactor = await probe.evaluate(() => devicePixelRatio);
    await probe.close();
    const page = await browser.newPage({ viewport, deviceScaleFactor });
    const preset = process.env.GRAPHICS_PRESET;
    if (preset) {
      assert(['low', 'medium', 'high'].includes(preset));
      await page.addInitScript(preset => localStorage.setItem('casino-game.graphics.v1', JSON.stringify({
        version: 2, graphics: { lighting: preset, anti_aliasing: { low: 'off', medium: 'fxaa', high: 'msaa4' }[preset] }
      })), preset);
    }
    const errors = [];
    const assetRequests = [];
    const log = [];
    page.on('pageerror', error => errors.push(String(error)));
    page.on('response', response => { if (response.url().endsWith('.glb')) assetRequests.push(response.url()); if (response.status() >= 400) errors.push(`${response.status()} ${response.url()}`); });
    page.on('console', message => {
      log.push(message.text());
      if (message.type() === 'error') errors.push(message.text());
      if (/adapter|ready|preprocessing/i.test(message.text())) console.log(message.text());
    });
    await page.addInitScript(() => {
      const requestAdapter = GPU.prototype.requestAdapter;
      GPU.prototype.requestAdapter = async function (...args) {
        const adapter = await requestAdapter.apply(this, args);
        if (adapter) window.gameAdapter = { vendor: adapter.info.vendor,
          architecture: adapter.info.architecture, isFallbackAdapter: adapter.info.isFallbackAdapter };
        return adapter;
      };
      window.addEventListener('keydown', event => { if (/^F\d+$/.test(event.key)) event.preventDefault(); }, true);
      // Acquisitions measure producer throughput, not completed GPU work or presentation.
      window.gameSample = null;
      const conditions = () => ({ time: performance.now(), visibility: document.visibilityState,
        focused: document.hasFocus() });
      const recordConditions = () => {
        if (window.gameSample) window.gameSample.conditions.push(conditions());
      };
      document.addEventListener('visibilitychange', recordConditions);
      window.addEventListener('focus', recordConditions);
      window.addEventListener('blur', recordConditions);
      window.beginGameSample = () => {
        window.gameSample = { start: performance.now(), times: [], duplicates: 0,
          textures: new WeakSet(), conditions: [conditions()] };
      };
      window.endGameSample = () => {
        recordConditions();
        const { textures, ...sample } = window.gameSample;
        sample.end = performance.now();
        window.gameSample = null;
        return sample;
      };
      const acquire = GPUCanvasContext.prototype.getCurrentTexture;
      GPUCanvasContext.prototype.getCurrentTexture = function (...args) {
        const texture = acquire.apply(this, args);
        const sample = window.gameSample;
        if (sample) {
          sample.times.push(performance.now());
          if (sample.textures.has(texture)) sample.duplicates++;
          else sample.textures.add(texture);
        }
        return texture;
      };
    });
    const titleReady = page.waitForEvent('console', { predicate: m => m.text().includes('Title screen ready'), timeout: 180000 });
    const started = page.waitForEvent('console', { predicate: m => /Opening cutscene ready|Gameplay resumed/.test(m.text()), timeout: 180000 });
    await page.goto(url);
    await titleReady;
    await page.keyboard.press('Enter');
    if ((await started).text().includes('Opening cutscene ready')) {
      const playing = page.waitForEvent('console', { predicate: m => m.text().includes('Gameplay resumed'), timeout: 10000 });
      await page.keyboard.press('Enter', { delay: 1200 });
      await playing;
    }
    await page.bringToFront();
    await page.waitForTimeout(10000);
    for (const key of setupKeys) await page.keyboard.press(key);
    if (setupKeys.length) await page.waitForTimeout(3000);
    const canvas = await page.locator('canvas').evaluate(c => ({ width: c.width, height: c.height }));
    assert(canvas.width > 0 && canvas.height > 0, 'Empty render target');
    const results = [];
    report = { date: new Date().toISOString(), url, browser: browser.version(), gpu,
      adapter: await page.evaluate(() => window.gameAdapter), assetRequests, canvas, deviceScaleFactor, rounds, sampleMs,
      preset: preset || 'default',
      metric: 'Canvas acquisitions per wall-clock second; not presented FPS', trace, setupKeys,
      results, gameplaySmoke: 'not run', errors, log };
    persist();
    const views = [
      ['F9', 'apartment'], ['F1', 'entrance-wall'], ['F2', 'corridor'],
      ['F3', 'east-window'], ['F4', 'bedroom-window'], ['F5', 'building-context'],
      ['F7', 'court-from-window'], ['F8', 'court-detail'],
      ['F10', 'stair-landing'], ['F11', 'occupied-exterior'],
      ['1', 'bathroom-jump'], ['2', 'kitchen-left-edge'], ['3', 'kitchen-right-edge'],
      ['4', 'bedroom-left-edge'], ['5', 'bedroom-right-edge'], ['6', 'bathroom-standing'],
    ].filter(([, name]) => !process.env.VIEWS || process.env.VIEWS.split(',').includes(name));
    assert(views.length > 0, 'No benchmark views selected');
    for (let round = 0; round < rounds; round++) {
      for (const [key, name] of views) {
        const changed = page.waitForEvent('console', { predicate: m => m.text().includes('City review camera'), timeout: 10000 });
        await page.keyboard.press(key);
        await changed;
        await page.waitForTimeout(3000);
        const traceFile = trace ? `trace-round-${round + 1}-${name}.json` : undefined;
        if (trace) await browserSession.send('Tracing.start', {
          categories: 'devtools.timeline,disabled-by-default-devtools.timeline,disabled-by-default-devtools.timeline.frame,disabled-by-default-v8.cpu_profiler,blink,blink.user_timing,cc,gpu,viz,toplevel,benchmark',
          transferMode: 'ReturnAsStream',
        });
        await page.evaluate(tracing => {
          if (tracing) performance.mark('casino-sample-start');
          window.beginGameSample();
        }, trace);
        await page.waitForTimeout(sampleMs);
        const sample = await page.evaluate(tracing => {
          const sample = window.endGameSample();
          if (tracing) performance.mark('casino-sample-end');
          return sample;
        }, trace);
        const result = { round: round + 1, view: name, ...summarize(sample), traceFile };
        results.push({ ...result, sample });
        persist();
        console.log(JSON.stringify(result));
        if (trace) await saveTrace(browserSession, `${directory}/${traceFile}`);
        if (round === 0) {
          await page.screenshot({ path: `${directory}/${name}.png` });
          // Keep the full render target too; browser screenshots can downsample it.
          const png = await page.locator('canvas').evaluate(c => c.toDataURL('image/png'));
          assert(png.startsWith('data:image/png;base64,'), 'Canvas snapshot must be PNG');
          mkdirSync(`${directory}/canvas`, { recursive: true });
          writeFileSync(`${directory}/canvas/${name}.png`, Buffer.from(png.split(',')[1], 'base64'));
        }
      }
    }
    assert.deepEqual(await page.locator('canvas').evaluate(c => ({ width: c.width, height: c.height })), canvas);
    report.gameplaySmoke = 'running';
    persist();
    await page.keyboard.press('F6');
    await page.bringToFront();
    await page.mouse.click(640, 360);
    if (!await page.evaluate(() => document.pointerLockElement !== null)) await page.mouse.click(640, 360);
    await page.waitForFunction(() => document.pointerLockElement !== null);
    await page.keyboard.down('w');
    await page.waitForTimeout(1500);
    await page.keyboard.up('w');
    await page.keyboard.press('Space');
    await page.mouse.move(800, 360);
    await page.waitForTimeout(1000);
    await page.keyboard.press('Escape');
    await page.waitForFunction(() => document.pointerLockElement === null);
    await page.waitForTimeout(1000);
    const resumed = page.waitForEvent('console', { predicate: m => m.text().includes('Gameplay resumed') || m.text().includes('First-person player ready'), timeout: 30000 });
    await page.keyboard.press('Enter');
    await resumed;
    if (log.some(line => line.includes('Graphics quality'))) {
      assert.equal(log.filter(line => line.includes('First-person player ready')).length, 1, 'Resume must preserve the original player');
    }
    report.gameplaySmoke = 'pointer lock, walk, jump, turn, escape, resume';
    persist();
    assert.equal(errors.length, 0, errors.join('\n'));
  } catch (error) {
    if (report) {
      report.failure = String(error);
      persist();
    }
    throw error;
  } finally {
    await browser.close();
  }
}

if (process.argv[2] === '--self-test') console.log('Benchmark metric self-checks passed');
else main().catch(error => { console.error(error); process.exitCode = 1; });
