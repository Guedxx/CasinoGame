// Run against a scene-review web build. NODE_PATH must contain Playwright.
const assert = require('node:assert/strict');
const { mkdirSync, writeFileSync } = require('node:fs');
const { chromium } = require('playwright');
const directory = process.argv[2] || 'art/review/performance/settings/interaction';
const url = process.argv[3] || 'http://127.0.0.1:8080/settings-candidate/';
const storageKey = 'casino-game.graphics.v1';

async function main() {
  mkdirSync(directory, { recursive: true });
  const browser = await chromium.launch({ executablePath: '/usr/bin/chromium', headless: false, args: ['--no-sandbox'] });
  const probe = await browser.newPage({ viewport: null });
  const deviceScaleFactor = await probe.evaluate(() => devicePixelRatio);
  await probe.close();
  const page = await browser.newPage({ viewport: { width: 1280, height: 720 }, deviceScaleFactor });
  const logs = [], errors = [], checks = [];
  page.on('console', message => {
    logs.push(message.text());
    if (message.type() === 'error') errors.push(message.text());
    if (/Graphics|ready|resumed|Texture quality/.test(message.text())) console.log(message.text());
  });
  page.on('pageerror', error => errors.push(String(error)));
  await page.addInitScript(() => {
    window.addEventListener('keydown', event => { if (/^F\d+$/.test(event.key)) event.preventDefault(); }, true);
    window.uploads = [];
    window.textures = [];
    window.bindings = [];
    const views = new WeakMap();
    const createView = GPUTexture.prototype.createView;
    GPUTexture.prototype.createView = function () {
      const view = createView.apply(this, arguments);
      views.set(view, { width: this.width, height: this.height, format: this.format });
      return view;
    };
    const bind = GPUDevice.prototype.createBindGroup;
    GPUDevice.prototype.createBindGroup = function (descriptor) {
      window.bindings.push({ label: descriptor.label, textures: descriptor.entries.map(entry => views.get(entry.resource)).filter(Boolean) });
      return bind.apply(this, arguments);
    };
    const create = GPUDevice.prototype.createTexture;
    GPUDevice.prototype.createTexture = function (descriptor) {
      const texture = create.apply(this, arguments);
      window.textures.push({ width: texture.width, height: texture.height, label: descriptor.label, format: texture.format, samples: texture.sampleCount });
      return texture;
    };
    const write = GPUQueue.prototype.writeTexture;
    GPUQueue.prototype.writeTexture = function (destination, data) {
      const bytes = ArrayBuffer.isView(data) ? new Uint8Array(data.buffer, data.byteOffset, data.byteLength) : new Uint8Array(data);
      let hash = 2166136261;
      for (const byte of bytes) hash = Math.imul(hash ^ byte, 16777619);
      window.uploads.push({ width: destination.texture.width, height: destination.texture.height,
        format: destination.texture.format, bytes: bytes.length, hash: hash >>> 0 });
      return write.apply(this, arguments);
    };
  });
  const key = async key => { await page.keyboard.press(key, { delay: 100 }); await page.waitForTimeout(500); };
  const screenshot = name => page.screenshot({ path: `${directory}/${name}.png` });
  const ready = async () => {
    const loaded = page.waitForEvent('console', { predicate: m => m.text().includes('First-person player ready'), timeout: 180000 });
    await page.goto(url); await loaded; await page.waitForTimeout(8000);
  };
  const settings = async () => { await key('Escape'); await key('Tab'); await key('Enter'); };
  const storage = () => page.evaluate(key => localStorage.getItem(key), storageKey);
  const uploads = () => page.evaluate(() => window.uploads);
  const signature = row => `${row.width}:${row.height}:${row.format}:${row.bytes}:${row.hash}`;
  try {
    await ready();
    assert.equal(await storage(), null);
    await key('F9');
    await screenshot('high-before');
    const sceneCorner = await page.screenshot({ clip: { x: 300, y: 300, width: 20, height: 20 } });
    const original = new Set((await uploads()).map(signature));
    await key('Escape'); await screenshot('pause');
    await key('Tab'); await key('Enter'); await screenshot('settings-high');
    assert(!sceneCorner.equals(await page.screenshot({ clip: { x: 300, y: 300, width: 20, height: 20 } })), 'High-quality settings panel must be visible');
    await page.mouse.click(940, 210, { delay: 150 }); // High preset in the visible panel.
    await page.waitForTimeout(500);
    await key('Shift+Tab'); await key('Shift+Tab'); // Back to Low.
    const beforeLow = (await uploads()).length;
    const beforeBindings = await page.evaluate(() => window.bindings.length);
    await key('Enter'); // Initial settings focus: Low preset.
    await page.waitForTimeout(8000);
    await screenshot('settings-low');
    assert(logs.some(line => line.includes('Graphics render scale 50%')));
    const lowWrites = (await uploads()).slice(beforeLow);
    assert(lowWrites.filter(row => row.width <= 256 && row.height <= 256).length >= 70, 'Scene textures must actually be reduced');
    assert((await page.evaluate(() => window.textures)).some(row => row.width === 800 && row.height === 450));
    const lowBindings = await page.evaluate(start => window.bindings.slice(start), beforeBindings);
    assert(lowBindings.filter(group => group.textures.some(texture => texture.width === 64 && texture.height === 64)).length >= 40, 'Materials must bind the resized texture views');
    assert.equal(await storage(), null, 'Preview must not persist');
    checks.push('Low preview changes the 3D target to 800×450 and uploads and binds reduced scene textures without saving');
    const beforeRestore = (await uploads()).length;
    await key('Escape'); await page.waitForTimeout(6000);
    const restored = (await uploads()).slice(beforeRestore).filter(row => original.has(signature(row)));
    assert(restored.length >= 70, `Cancel must restore original texture bytes; matched ${restored.length}`);
    await key('Enter'); await key('F9'); await screenshot('high-restored');
    checks.push(`Cancel restored ${restored.length} original texture uploads byte-for-byte`);
    await settings();
    await key('Tab'); await key('Space'); // Medium preset.
    await page.waitForTimeout(6000);
    await screenshot('settings-medium');
    // Wrap backwards from Medium to Low, Save, then activate Save.
    await key('Shift+Tab'); await key('Shift+Tab'); await key('Enter');
    const saved = JSON.parse(await storage());
    assert.deepEqual(saved, { version: 1, graphics: { lighting: 'medium', textures: 'medium', render_scale: 'medium', msaa: false } });
    checks.push('Mouse selection, keyboard navigation, Space activation and Save persist Medium');
    await key('Enter');
    await ready();
    await page.waitForTimeout(4000);
    await settings(); await screenshot('medium-after-reload');
    assert.equal(JSON.parse(await storage()).graphics.render_scale, 'medium');
    // Select only lighting High: first focus Low, then Medium, High, lighting Low, Medium, High.
    for (let i = 0; i < 5; i++) await key('Tab');
    await key('Enter'); await screenshot('custom');
    assert(logs.some(line => line.includes('Graphics quality Custom')));
    await page.setViewportSize({ width: 640, height: 480 });
    await page.waitForTimeout(2000);
    for (let i = 0; i < 8; i++) await key('Tab');
    await screenshot('small-keyboard-scroll');
    await page.mouse.wheel(0, -1000); await page.waitForTimeout(500);
    await screenshot('small-wheel-scroll');
    await key('Escape'); await page.waitForTimeout(2000);
    assert.equal(JSON.parse(await storage()).graphics.lighting, 'medium');
    checks.push('Individual setting produces Custom; Cancel preserves saved Medium; small-window scrolling exercised');
    await page.evaluate(key => localStorage.setItem(key, '{invalid'), storageKey);
    const beforeInvalid = logs.length;
    await ready();
    assert(logs.slice(beforeInvalid).some(line => line.includes('Graphics quality High')));
    await settings(); await screenshot('invalid-storage');
    checks.push('Invalid saved JSON falls back to High');
    // Force a real storage write failure; the menu must remain usable.
    await page.evaluate(() => { Storage.prototype.setItem = () => { throw new DOMException('Storage disabled', 'SecurityError'); }; });
    await key('Enter'); await page.waitForTimeout(5000);
    await key('Shift+Tab'); await key('Enter'); await screenshot('storage-unavailable');
    assert(logs.some(line => line.includes('Cannot save graphics settings')));
    checks.push('Storage write failure keeps session settings and displays a nonblocking notice');
    assert.equal(errors.length, 0, errors.join('\n'));
    assert.equal(logs.filter(line => line.includes('Texture quality:')).length, 0, 'Every scene texture must support resizing');
  } finally {
    writeFileSync(`${directory}/results.json`, JSON.stringify({ checks, errors, logs, uploads: await uploads().catch(() => []) }, null, 2));
    await browser.close();
  }
  console.log(checks.join('\n'));
}
main().catch(error => { console.error(error); process.exitCode = 1; });
