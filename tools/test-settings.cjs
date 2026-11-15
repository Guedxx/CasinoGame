// Run against a scene-review web build. NODE_PATH must contain Playwright.
const assert = require('node:assert/strict');
const { mkdirSync, writeFileSync } = require('node:fs');
const { chromium } = require('playwright');
const directory = process.argv[2] || 'art/review/performance/unfiltered/interaction';
const url = process.argv[3] || 'http://127.0.0.1:8080/';
const storageKey = 'casino-game.graphics.v1';

async function main() {
  mkdirSync(directory, { recursive: true });
  const browser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: false,
    args: ['--no-sandbox', '--ozone-platform=x11', '--enable-unsafe-webgpu',
      '--use-angle=vulkan', '--enable-features=Vulkan,VulkanFromANGLE'] });
  const probe = await browser.newPage({ viewport: null });
  const deviceScaleFactor = await probe.evaluate(() => devicePixelRatio);
  await probe.close();
  const page = await browser.newPage({ viewport: { width: 1280, height: 720 }, deviceScaleFactor });
  const logs = [], errors = [], checks = [], modes = [];
  page.on('console', message => {
    logs.push(message.text());
    if (message.type() === 'error') errors.push(message.text());
  });
  page.on('pageerror', error => errors.push(String(error)));
  page.on('response', response => { if (response.status() >= 400) errors.push(`${response.status()} ${response.url()}`); });
  await page.addInitScript(() => {
    window.addEventListener('keydown', event => { if (/^F\d+$/.test(event.key)) event.preventDefault(); }, true);
    const views = new WeakMap();
    const view = GPUTexture.prototype.createView;
    GPUTexture.prototype.createView = function () {
      const result = view.apply(this, arguments);
      views.set(result, { width: this.width, height: this.height, samples: this.sampleCount });
      return result;
    };
    window.passes = {};
    const begin = GPUCommandEncoder.prototype.beginRenderPass;
    GPUCommandEncoder.prototype.beginRenderPass = function (descriptor) {
      window.passes[descriptor.label] = {
        count: (window.passes[descriptor.label]?.count || 0) + 1,
        targets: descriptor.colorAttachments.filter(Boolean).map(a => views.get(a.view)),
      };
      return begin.apply(this, arguments);
    };
  });
  const key = async key => { await page.keyboard.press(key, { delay: 100 }); await page.waitForTimeout(250); };
  const screenshot = name => page.screenshot({ path: `${directory}/${name}.png` });
  const ready = async () => {
    const titleReady = page.waitForEvent('console', { predicate: m => m.text().includes('Title screen ready'), timeout: 180000 });
    const started = page.waitForEvent('console', { predicate: m => /Opening cutscene ready|Gameplay resumed/.test(m.text()), timeout: 180000 });
    await page.goto(url); await titleReady; await page.keyboard.press('Enter');
    if ((await started).text().includes('Opening cutscene ready')) {
      const playing = page.waitForEvent('console', { predicate: m => m.text().includes('Gameplay resumed'), timeout: 10000 });
      await page.keyboard.press('Enter', { delay: 1200 });
      await playing;
    }
    await page.waitForTimeout(5000);
    await key('F9');
  };
  let focus = 0;
  const settings = async () => { await key('Escape'); await key('Tab'); await key('Enter'); focus = 0; };
  const select = async index => {
    while (focus !== index) { await key(focus < index ? 'Tab' : 'Shift+Tab'); focus += focus < index ? 1 : -1; }
    await key('Enter');
  };
  const saved = () => page.evaluate(key => localStorage.getItem(key), storageKey);
  const passes = async () => {
    await page.evaluate(() => { window.passes = {}; });
    await page.waitForTimeout(700);
    return page.evaluate(() => window.passes);
  };
  try {
    await ready();
    const canvas = await page.locator('canvas').evaluate(c => ({ width: c.width, height: c.height }));
    await screenshot('game-high');
    await settings();
    await screenshot('settings-high');
    const title = () => page.screenshot({ clip: { x: 350, y: 68, width: 580, height: 55 } });
    const originalTitle = await title();
    // Hit-testing must continue to agree with the displayed controls.
    await page.mouse.click(940, 210, { delay: 150 }); focus = 2;
    await page.waitForTimeout(500);
    for (const [index, quality] of ['low', 'medium', 'high', 'low', 'high'].entries()) {
      await select(['low', 'medium', 'high'].indexOf(quality));
      await page.waitForTimeout(3000);
      const captured = await passes();
      const target = captured.main_opaque_pass_3d?.targets[0];
      assert.deepEqual(target, { ...canvas, samples: 1 }, `${quality}: native resolution without MSAA`);
      for (const label of ['fxaa', 'SMAA edge detection pass', 'SMAA blending weight calculation pass', 'SMAA neighborhood blending pass']) {
        assert(!captured[label], `${quality}: ${label} must stay absent`);
      }
      assert(originalTitle.equals(await title()), `${quality}: menu heading pixels and size must stay unchanged`);
      assert.equal(await saved(), null, 'Live preview must not save');
      await screenshot(`settings-${quality}-${index}`);
      modes.push({ quality, target, activePasses: Object.keys(captured) });
    }
    checks.push('Every lighting preset renders at full resolution with one sample and without FXAA/SMAA; menu heading stays unchanged');
    await select(12); // Save High.
    assert.deepEqual(JSON.parse(await saved()), { version: 3, graphics: { lighting: 'high', texture_warping: true, vertex_jitter: true } });
    await ready();
    await settings(); await select(0); // Preview Low.
    await page.waitForTimeout(700);
    const cancelStart = logs.length;
    await key('Escape'); await page.waitForTimeout(1500);
    assert(logs.slice(cancelStart).some(line => line.includes('lighting: High')), 'Cancel must restore saved High lighting');
    await key('Enter');
    checks.push('Save and reload preserve lighting; Cancel restores saved lighting after a Low preview');
    // Old saves retain lighting and cannot reactivate removed rendering choices.
    for (const legacy of [
      { version: 1, graphics: { lighting: 'medium', textures: 'low', render_scale: 'low', msaa: true } },
      ...['fxaa', 'smaa', 'msaa4'].map(anti_aliasing => ({ version: 2, graphics: { lighting: 'medium', anti_aliasing } })),
    ]) {
      await page.evaluate(({ key, value }) => localStorage.setItem(key, JSON.stringify(value)), { key: storageKey, value: legacy });
      const start = logs.length;
      await ready();
      assert(logs.slice(start).some(line => line.includes('lighting: Medium')));
      const captured = await passes();
      assert.deepEqual(captured.main_opaque_pass_3d.targets[0], { ...canvas, samples: 1 });
      for (const label of ['fxaa', 'SMAA edge detection pass', 'SMAA blending weight calculation pass', 'SMAA neighborhood blending pass']) assert(!captured[label]);
      await settings(); await select(12);
      assert.deepEqual(JSON.parse(await saved()), { version: 3, graphics: { lighting: 'medium', texture_warping: true, vertex_jitter: true } });
      await key('Enter');
    }
    checks.push('Version-one and version-two saves retain lighting, discard removed fields and cannot enable AA');
    await settings(); await select(1);
    await page.setViewportSize({ width: 640, height: 480 });
    await page.waitForTimeout(1500); await select(10); await page.waitForTimeout(1500);
    await screenshot('small-keyboard-scroll');
    await page.mouse.wheel(0, -1000); await page.waitForTimeout(500);
    await screenshot('small-wheel-scroll');
    await key('Escape'); await key('Enter');
    checks.push('Small-window keyboard and wheel scrolling keep the remaining settings usable');
    await page.evaluate(key => localStorage.setItem(key, '{invalid'), storageKey);
    const start = logs.length;
    await ready();
    assert(logs.slice(start).some(line => line.includes('lighting: High')));
    await settings();
    await page.evaluate(() => { Storage.prototype.setItem = () => { throw new DOMException('Storage disabled', 'SecurityError'); }; });
    await select(12); await screenshot('storage-unavailable');
    assert(logs.some(line => line.includes('Cannot save graphics settings')));
    checks.push('Invalid storage defaults to High; failed writes keep settings usable for this session');
    assert.equal(errors.length, 0, errors.join('\n'));
  } finally {
    writeFileSync(`${directory}/results.json`, JSON.stringify({ deviceScaleFactor, checks, modes, errors, logs }, null, 2));
    await browser.close();
  }
  console.log(checks.join('\n'));
}
main().catch(error => { console.error(error); process.exitCode = 1; });
