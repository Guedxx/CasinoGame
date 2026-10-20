const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const script = fs.readFileSync(`${__dirname}/../web/index.html`, 'utf8')
  .match(/<script data-audio-unlock>([\s\S]*?)<\/script>/)[1];

(async () => {
  const events = {}, warnings = [];
  class AudioContext {
    constructor(options) { this.options = options; this.state = 'suspended'; this.resumes = 0; }
    resume() { this.resumes++; this.state = 'running'; return Promise.resolve(); }
  }
  const window = {AudioContext, addEventListener: (name, callback) => { events[name] = callback; }};
  vm.runInNewContext(script, {window, console: {warn: (...args) => warnings.push(args)}});
  const audio = new window.AudioContext({sampleRate: 44100});
  assert.equal(audio.options.sampleRate, 44100);
  assert.equal(audio.resumes, 0);
  events.pointerdown();
  assert.equal(audio.state, 'running');
  events.keydown();
  assert.equal(audio.resumes, 1);
  audio.state = 'suspended';
  events.keydown();
  assert.equal(audio.resumes, 2);
  audio.state = 'closed';
  events.pointerdown();
  assert.equal(audio.resumes, 2);
  const denied = new window.AudioContext();
  denied.resume = () => Promise.reject(new Error('device unavailable'));
  events.pointerdown();
  await Promise.resolve();
  assert.equal(warnings.length, 1);
  vm.runInNewContext(script, {window: {}});
  console.log('Audio unlock: gestures, repeat/resume, closed contexts and unavailable audio passed.');
})().catch(error => {console.error(error); process.exitCode = 1;});
