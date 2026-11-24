// Run against a disposable Seoran desktop profile with the loopback CDP endpoint
// enabled as described in tools/cdp.mjs. Requires Node 22+.
import assert from 'node:assert/strict';
import { createServer } from 'node:http';
import { setTimeout as delay } from 'node:timers/promises';
import { connect, pages } from './cdp.mjs';

async function waitFor(probe, label) {
  const end = Date.now() + 12000;
  while (Date.now() < end) {
    if (await probe()) return;
    await delay(30);
  }
  throw new Error('Timed out waiting for ' + label);
}

let documentRequests = 0;
const server = createServer((req, res) => {
  if (req.url !== '/fixture') { res.writeHead(404).end(); return; }
  documentRequests++;
  res.writeHead(200, { 'Content-Type': 'text/html', 'Cache-Control': 'no-store' });
  res.end(`<!doctype html><title>Layout continuity fixture</title>
    <button id="play">Start audio</button><input id="draft">
    <script>
    const marker = crypto.randomUUID();
    let audio, interruptions = 0;
    play.onclick = async () => {
      audio = new AudioContext();
      const osc = audio.createOscillator(), gain = audio.createGain();
      gain.gain.value = 0.005;
      osc.connect(gain).connect(audio.destination);
      osc.start();
      await audio.resume();
      audio.onstatechange = () => { if (audio.state !== 'running') interruptions++; };
    };
    </script>`);
});
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
const url = 'http://127.0.0.1:' + server.address().port + '/fixture';
let chrome, fixture, tab;
let counting = false;
let created = 0, dropped = 0, navigations = 0;
const pageTargets = new Set();
try {
  chrome = await connect('seoran.chrome', (method, params) => {
    if (method === 'Target.targetCreated' && params.targetInfo.type === 'page') {
      pageTargets.add(params.targetInfo.targetId);
      if (counting) created++;
    }
    if (method === 'Target.targetDestroyed' && pageTargets.delete(params.targetId) && counting) dropped++;
  });
  await chrome.call('Target.setDiscoverTargets', {discover:true});
  assert.equal(await chrome.evaluate('S?.tabPosition'), 'top', 'Start from the default top layout');
  await chrome.evaluate('send({ t: "openTab", url: ' + JSON.stringify(url) + ' })');
  await waitFor(async () => (await pages()).some(p => p.url === url), 'fixture tab');
  fixture = await connect(url, (method, params) => {
    if (counting && method === 'Page.frameNavigated' && !params.frame.parentId) navigations++;
  });
  await fixture.call('Page.enable');
  await waitFor(() => chrome.evaluate('S.tabs.find(t => t.id === S.active)?.title === "Layout continuity fixture"'), 'fixture navigation');
  tab = await chrome.evaluate('S.active');
  const button = await fixture.evaluate('const r = play.getBoundingClientRect(); ({x:r.x+r.width/2,y:r.y+r.height/2})');
  for (const type of ['mousePressed', 'mouseReleased']) {
    await fixture.call('Input.dispatchMouseEvent', { type, ...button, button: 'left', clickCount: 1 });
  }
  await waitFor(() => fixture.evaluate('audio?.state === "running"'), 'user-activated audio');
  const before = await fixture.evaluate(`draft.value = 'unsaved text';
    history.pushState({step:1}, '', '#one');
    history.pushState({step:2}, '', '#two');
    ({marker, time:audio.currentTime})`);
  const targetIds = async () => (await pages()).filter(p => p.type === 'page').map(p => p.id).sort();
  const initialTargets = await targetIds();
  assert.ok(initialTargets.every(id => pageTargets.has(id)), 'CDP discovered the starting page targets');
  const requests = documentRequests;
  counting = true;
  const samples = [];
  for (let i = 0; i < 104; i++) {
    const ms = await chrome.evaluate(`(async () => {
      const next = S.tabPosition === 'top' ? 'left' : 'top';
      document.querySelector('#menu-btn').click();
      const start = performance.now();
      document.querySelector('[data-a="switchLayout"]').click();
      await new Promise((ok, fail) => {
        const timeout = setTimeout(() => fail(Error('layout acknowledgement')), 4000);
        function frame() {
          if (document.documentElement.dataset.tabs !== next) return requestAnimationFrame(frame);
          clearTimeout(timeout); ok();
        }
        requestAnimationFrame(frame);
      });
      return performance.now() - start;
    })()`);
    if (i >= 4) samples.push(ms);
    const shell = await chrome.evaluate('({expectedWidth:innerWidth-S.railWidth, active:S.active, focus:document.activeElement.id})');
    assert.equal(shell.active, tab);
    assert.equal(shell.focus, 'menu-btn');
    await waitFor(() => fixture.evaluate('Math.abs(innerWidth - ' + shell.expectedWidth + ') <= 1'), 'page width');
    const page = await fixture.evaluate('({marker, draft:draft.value, state:audio.state, interruptions, step:history.state?.step})');
    assert.deepEqual(page, {marker:before.marker, draft:'unsaved text', state:'running', interruptions:0, step:2});
    assert.deepEqual(await targetIds(), initialTargets);
    assert.equal(documentRequests, requests);
  }
  counting = false;
  assert.deepEqual({created, dropped, navigations}, {created:0, dropped:0, navigations:0});
  await fixture.evaluate('history.back()');
  await waitFor(() => fixture.evaluate('history.state?.step === 1'), 'back history');
  await fixture.evaluate('history.forward()');
  await waitFor(() => fixture.evaluate('history.state?.step === 2'), 'forward history');
  assert.ok(await fixture.evaluate('audio.currentTime') > before.time);
  const dpi = await chrome.evaluate('devicePixelRatio');
  samples.sort((a, b) => a - b);
  console.log(JSON.stringify({ switches: samples.length, warmup:4, p95Ms:samples[94], devicePixelRatio:dpi,
    created, dropped, navigations, additionalDocumentRequests:documentRequests - requests, survivingTarget:fixture.page.id,
    audioInterruptions:await fixture.evaluate('interruptions') }));
} finally {
  if (tab && chrome) await chrome.evaluate('send({ t:"closeTab", id:' + tab + ' })').catch(() => {});
  fixture?.close();
  chrome?.close();
  server.closeAllConnections();
  server.close();
}
