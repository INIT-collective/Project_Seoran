// Drive Seoran's pages over the DevTools Protocol for testing.
// Start Seoran with WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9333
// Usage: node tools/cdp.mjs <url-substring> <js-expression>
//   e.g. node tools/cdp.mjs seoran.chrome "document.title"
const PORT = process.env.SEORAN_CDP_PORT || 9333;

export async function pages() {
  return (await fetch(`http://127.0.0.1:${PORT}/json/list`)).json();
}

export async function connect(match, onEvent) {
  const list = await pages();
  const page = list.find((p) => p.type === 'page' && p.url.includes(match));
  if (!page) throw new Error(`no page matching ${match}: ${list.map((p) => p.url).join(', ')}`);
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((ok, fail) => { ws.onopen = ok; ws.onerror = fail; });
  let id = 0;
  const waiting = new Map();
  ws.onmessage = (ev) => {
    const msg = JSON.parse(ev.data);
    if (msg.method) onEvent?.(msg.method, msg.params);
    if (msg.id && waiting.has(msg.id)) {
      const { ok, fail } = waiting.get(msg.id);
      waiting.delete(msg.id);
      msg.error ? fail(new Error(msg.error.message)) : ok(msg.result);
    }
  };
  const call = (method, params = {}) => new Promise((ok, fail) => {
    const n = ++id;
    waiting.set(n, { ok, fail });
    ws.send(JSON.stringify({ id: n, method, params }));
  });
  const evaluate = async (expression) => {
    const r = await call('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
    return r.result.value;
  };
  return { page, call, evaluate, close: () => ws.close() };
}

if (process.argv[1]?.endsWith('cdp.mjs') && process.argv.length > 3) {
  const c = await connect(process.argv[2]);
  // "@path" reads the expression from a file (avoids shell quoting issues).
  let expr = process.argv[3];
  if (expr.startsWith('@')) expr = (await import('node:fs')).readFileSync(expr.slice(1), 'utf8');
  console.log(JSON.stringify(await c.evaluate(expr), null, 2));
  c.close();
}
