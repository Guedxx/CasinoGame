// Run with: node --experimental-vm-modules tools/test-web-startup.cjs
const assert = require('node:assert/strict');
const { readFileSync } = require('node:fs');
const vm = require('node:vm');
const html = readFileSync(`${__dirname}/../web/index.html`, 'utf8');
const language = html.match(/<html lang="([^"]+)"/)[1];
const source = html.match(/<script type="module">([\s\S]*?)<\/script>/)[1];

async function start(gpu, init = () => {}, importError, browser = {}) {
  const status = { hidden: false, textContent: 'Loading' };
  const listeners = {};
  let imported = false;
  let importedPath;
  const document = browser.document ?? { documentElement: { lang: language } };
  const context = vm.createContext({
    document: Object.assign(document, { querySelector: () => status }),
    navigator: { gpu, ...browser.navigator },
    window: { addEventListener: (name, listener) => { listeners[name] = listener; } },
    console: { error() {}, warn() {} },
  });
  const module = new vm.SourceTextModule(source, {
    context,
    importModuleDynamically: async path => {
      imported = true;
      importedPath = path;
      if (importError) throw importError;
      const game = new vm.SyntheticModule(['default'], function () {
        this.setExport('default', init);
      }, { context });
      await game.link(() => {});
      await game.evaluate();
      return game;
    },
  });
  await module.link(() => {});
  await module.evaluate();
  return { status, listeners, imported, importedPath, document };
}

(async () => {
  assert.equal(language, 'pt-BR');
  const missing = await start(undefined);
  assert.match(missing.status.textContent, /WebGPU indisponível/);
  assert.equal(missing.imported, false);
  const unavailable = await start({ requestAdapter: async () => null });
  assert.match(unavailable.status.textContent, /Nenhum dispositivo gráfico WebGPU disponível/);
  assert.equal(unavailable.imported, false);
  const gpu = { requestAdapter: async () => ({}) };
  for (const failed of [
    await start({ requestAdapter: async () => { throw new Error('adapter failed'); } }),
    await start(gpu, () => { throw new Error('renderer failed'); }),
    await start(gpu, () => {}, new Error('module failed')),
  ]) {
    assert.equal(failed.status.hidden, false);
    assert.match(failed.status.textContent, /O jogo encontrou um erro/);
  }
  let wasmPath;
  const running = await start(gpu, options => { wasmPath = options.module_or_path; });
  assert.equal(running.importedPath, './CasinoGame.js?v=__GAME_BUILD__');
  assert.equal(wasmPath, './CasinoGame_bg.wasm?v=__GAME_BUILD__');
  assert.equal(running.status.hidden, true);
  running.document.documentElement.lang = 'en';
  running.listeners.error({ error: new Error('renderer stopped') });
  assert.equal(running.status.hidden, false);
  assert.match(running.status.textContent, /renderer stopped/);
  running.status.hidden = true;
  running.listeners.unhandledrejection({ reason: new Error('device lost') });
  assert.equal(running.status.hidden, false);
  assert.match(running.status.textContent, /device lost/);

  async function keyboardFixture({ existingFullscreen = false, deny = '', pendingFullscreen } = {}) {
    const events = {}, attributes = new Set();
    let locked = false, exited = 0;
    const root = {
      setAttribute: name => attributes.add(name),
      removeAttribute: name => attributes.delete(name),
      async requestFullscreen() {
        if (deny === 'fullscreen') throw new Error('Fullscreen denied');
        await pendingFullscreen;
        document.fullscreenElement = root;
      },
    };
    const document = {
      documentElement: root,
      pointerLockElement: {},
      fullscreenElement: existingFullscreen ? root : null,
      addEventListener: (name, callback) => { events[name] = callback; },
      async exitFullscreen() { document.fullscreenElement = null; exited++; },
    };
    const keyboard = {
      async lock(keys) {
        assert.deepEqual(Array.from(keys), ['Escape']);
        if (deny === 'keyboard') throw new Error('Keyboard capture denied');
        locked = true;
      },
      unlock() { locked = false; },
    };
    await start(gpu, () => {}, undefined, { document, navigator: { keyboard } });
    return { document, root, events, attributes, get locked() { return locked; }, get exited() { return exited; } };
  }
  const capture = await keyboardFixture();
  await capture.events.pointerlockchange();
  assert.equal(capture.locked, true);
  assert(capture.attributes.has('data-game-escape-lock'));
  capture.document.pointerLockElement = null;
  await capture.events.pointerlockchange();
  assert.equal(capture.locked, false);
  assert.equal(capture.attributes.size, 0);
  assert.equal(capture.document.fullscreenElement, null);
  assert.equal(capture.exited, 1);

  const existing = await keyboardFixture({ existingFullscreen: true });
  await existing.events.pointerlockchange();
  existing.document.pointerLockElement = null;
  await existing.events.pointerlockchange();
  assert.equal(existing.document.fullscreenElement, existing.root, 'Leave pre-existing fullscreen alone');
  assert.equal(existing.exited, 0);
  for (const deny of ['fullscreen', 'keyboard']) {
    const denied = await keyboardFixture({ deny });
    await denied.events.pointerlockchange();
    assert.equal(denied.locked, false);
    assert.equal(denied.attributes.size, 0);
    assert.equal(denied.document.fullscreenElement, null);
  }
  let finishFullscreen;
  const pendingFullscreen = new Promise(resolve => { finishFullscreen = resolve; });
  const lost = await keyboardFixture({ pendingFullscreen });
  const entering = lost.events.pointerlockchange();
  lost.document.pointerLockElement = null;
  await lost.events.pointerlockchange();
  finishFullscreen();
  await entering;
  assert.equal(lost.locked, false);
  assert.equal(lost.attributes.size, 0);
  assert.equal(lost.document.fullscreenElement, null);

  const fullscreenExit = await keyboardFixture();
  await fullscreenExit.events.pointerlockchange();
  fullscreenExit.document.fullscreenElement = null;
  fullscreenExit.events.fullscreenchange();
  assert.equal(fullscreenExit.locked, false);
  assert.equal(fullscreenExit.attributes.size, 0);
  console.log('PASS: startup, matching build URLs, Escape capture, pointer/fullscreen exit, denied APIs and capture lost during fullscreen entry');
})().catch(error => { console.error(error); process.exitCode = 1; });
