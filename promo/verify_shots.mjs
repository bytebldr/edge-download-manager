import { spawn } from 'node:child_process';
import { writeFile, mkdir } from 'node:fs/promises';
import { pathToFileURL } from 'node:url';

const EDGE_PATH = "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe";
const PORT = 9222;
const userDataDir = "C:\\Users\\Suvesh\\AppData\\Local\\Temp\\edge-cdp-" + Date.now();

console.log('Launching headless Edge on port', PORT);
const edgeProc = spawn(EDGE_PATH, [
  `--remote-debugging-port=${PORT}`,
  `--user-data-dir=${userDataDir}`,
  '--headless=new',
  '--no-sandbox',
  '--disable-gpu-sandbox',
  '--window-size=1920,1080',
  '--autoplay-policy=no-user-gesture-required'
]);

// Wait for CDP to be ready
let targets = null;
for (let i = 0; i < 30; i++) {
  try {
    const res = await fetch(`http://127.0.0.1:${PORT}/json/list`);
    if (res.ok) {
      targets = await res.json();
      break;
    }
  } catch (e) {}
  await new Promise(r => setTimeout(r, 200));
}

if (!targets || !targets.length) {
  console.error('Failed to connect to Edge CDP endpoint');
  edgeProc.kill();
  process.exit(1);
}

const wsUrl = targets[0].webSocketDebuggerUrl;
console.log('Connected to CDP:', wsUrl);

const ws = new WebSocket(wsUrl);
await new Promise(res => ws.onopen = res);

let msgId = 1;
const pending = new Map();
ws.onmessage = (event) => {
  const data = JSON.parse(event.data);
  if (data.id && pending.has(data.id)) {
    const { resolve, reject } = pending.get(data.id);
    pending.delete(data.id);
    if (data.error) reject(data.error);
    else resolve(data.result);
  } else if (data.method === 'Runtime.consoleAPICalled') {
    console.log('[BROWSER CONSOLE]', data.params.type, data.params.args.map(a => a.value || a.description).join(' '));
  } else if (data.method === 'Runtime.exceptionThrown') {
    console.error('[BROWSER EXCEPTION]', data.params.exceptionDetails);
  }
};

function send(method, params = {}) {
  return new Promise((resolve, reject) => {
    const id = msgId++;
    pending.set(id, { resolve, reject });
    ws.send(JSON.stringify({ id, method, params }));
  });
}

await send('Runtime.enable');
await send('Page.enable');

const fileUrl = pathToFileURL("C:/Users/Suvesh/Desktop/projects/EDM/promo/index.html").href + '?clean=1';
console.log('Navigating to:', fileUrl);
await send('Page.navigate', { url: fileUrl });

// Wait for OPENER.ready
console.log('Waiting for OPENER.ready...');
let ready = false;
for (let i = 0; i < 40; i++) {
  const res = await send('Runtime.evaluate', {
    expression: 'Boolean(window.OPENER && window.OPENER.ready)',
    returnByValue: true
  });
  if (res.result && res.result.value === true) {
    ready = true;
    break;
  }
  await new Promise(r => setTimeout(r, 250));
}

if (!ready) {
  console.error('Timed out waiting for OPENER.ready');
} else {
  console.log('OPENER is ready!');
}

await mkdir('promo/shots', { recursive: true });

const keyTimes = [2.0, 7.5, 14.0, 20.5, 26.5];
for (const t of keyTimes) {
  console.log(`Capturing frame at ${t}s...`);
  await send('Runtime.evaluate', {
    expression: `window.OPENER.seek(${t});`,
    awaitPromise: true
  });
  // Wait 150ms for render
  await new Promise(r => setTimeout(r, 150));
  
  const shot = await send('Page.captureScreenshot', { format: 'png' });
  const buf = Buffer.from(shot.data, 'base64');
  const filename = `promo/shots/scene_${t.toFixed(1).replace('.', '_')}s.png`;
  await writeFile(filename, buf);
  console.log(`Saved screenshot -> ${filename}`);
}

ws.close();
edgeProc.kill();
console.log('Visual verification complete!');
