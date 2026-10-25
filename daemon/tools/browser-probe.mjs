#!/usr/bin/env -S nix develop .#browser --command node
// Local HTTP media, normal native browser MPRIS and PulseAudio/PipeWire output.
// No extension, injected stream properties, browser audio interception or MPRIS fixture.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import http from 'node:http';
import net from 'node:net';
import { spawn, execFileSync } from 'node:child_process';
import { setTimeout as delay } from 'node:timers/promises';
const directory = process.env.PYXAMP_PROBE_DIRECTORY;
if (!directory || process.env.PYXAMP_BROWSER_PROBE_PRIVATE !== '1') throw Error('Run through browser-probe private session');
const executable = process.argv[2];
const externalUrl = process.env.PYXAMP_BROWSER_URL;
if (externalUrl && new URL(externalUrl).protocol !== 'https:') throw Error('Native site probe requires an explicit HTTPS URL');
const save = (name, value) => fs.writeFileSync(`${directory}/${name}.json`, JSON.stringify(value, null, 2) + '\n');
function wav(frequency) {
  const rate = 48000, samples = rate * 30, bytes = Buffer.alloc(44 + samples * 2);
  bytes.write('RIFF'); bytes.writeUInt32LE(bytes.length - 8, 4); bytes.write('WAVEfmt ', 8);
  bytes.writeUInt32LE(16, 16); bytes.writeUInt16LE(1, 20); bytes.writeUInt16LE(1, 22);
  bytes.writeUInt32LE(rate, 24); bytes.writeUInt32LE(rate * 2, 28); bytes.writeUInt16LE(2, 32); bytes.writeUInt16LE(16, 34);
  bytes.write('data', 36); bytes.writeUInt32LE(samples * 2, 40);
  for (let i = 0; i < samples; i++) bytes.writeInt16LE(Math.round(Math.sin(2 * Math.PI * frequency * i / rate) * 655 * (0.75 + 0.25 * Math.sin(2 * Math.PI * 2 * i / rate))), 44 + i * 2);
  return bytes;
}
const media = new Map([[750, wav(750)], [6000, wav(6000)]]);
const server = http.createServer((request, response) => {
  const url = new URL(request.url, 'http://localhost');
  const frequency = Number(url.searchParams.get('frequency') || 750);
  if (!media.has(frequency)) { response.writeHead(404).end(); return; }
  if (url.pathname === '/tone.wav') { response.writeHead(200, { 'Content-Type': 'audio/wav', 'Content-Length': media.get(frequency).length }); response.end(media.get(frequency)); return; }
  response.writeHead(200, { 'Content-Type': 'text/html' });
  response.end(`<!doctype html><title>Native browser tone ${frequency}</title>
<audio id="audio" controls autoplay loop src="/tone.wav?frequency=${frequency}"></audio>
<script>
navigator.mediaSession.metadata = new MediaMetadata({title: 'Native browser tone ${frequency}', artist: 'Pyxamp private capture investigation'});
const audio = document.getElementById('audio');
navigator.mediaSession.setActionHandler('play', () => audio.play());
navigator.mediaSession.setActionHandler('pause', () => audio.pause());
audio.play();
</script>`);
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
const port = server.address().port;
const profile = `${directory}/browser-profile`;
const log = fs.openSync(`${directory}/browser.log`, 'w');
const version = execFileSync(executable, ['--version'], { encoding: 'utf8' }).trim();
save('browser', { executable, version });
const browser = spawn(executable, [`--user-data-dir=${profile}`, '--ozone-platform=x11', '--disable-gpu', '--no-first-run', '--no-default-browser-check', '--disable-background-networking', '--disable-component-update', '--password-store=basic', '--remote-debugging-port=0', '--autoplay-policy=no-user-gesture-required', externalUrl || `http://127.0.0.1:${port}/?frequency=750`], { stdio: ['ignore', log, log] });
fs.writeFileSync(`${directory}/unrelated.wav`, wav(12000));
const unrelated = externalUrl ? undefined : spawn('pw-play', ['--target=pyxamp-browser-sink', `${directory}/unrelated.wav`], { stdio: ['ignore', log, log] });
let ws, socket;
const summaries = [];
const frames = [], states = [];
let currentState;
try {
  for (let i = 0; i < 100 && !fs.existsSync(`${profile}/DevToolsActivePort`); i++) { if (browser.exitCode !== null) throw Error(`Browser exited ${browser.exitCode}`); await delay(100); }
  const [debugPort, debugPath] = fs.readFileSync(`${profile}/DevToolsActivePort`, 'utf8').trim().split('\n');
  ws = new WebSocket(`ws://127.0.0.1:${debugPort}${debugPath}`);
  await new Promise((resolve, reject) => { ws.addEventListener('open', resolve, { once: true }); ws.addEventListener('error', reject, { once: true }); });
  let serial = 0;
  const pending = new Map();
  ws.addEventListener('message', event => { const message = JSON.parse(event.data); const waiter = pending.get(message.id); if (waiter) { pending.delete(message.id); if (message.error) waiter.reject(Error(JSON.stringify(message.error))); else waiter.resolve(message.result); } });
  const call = (method, params = {}, sessionId) => new Promise((resolve, reject) => { const id = ++serial; pending.set(id, {resolve, reject}); ws.send(JSON.stringify({ id, method, params, ...(sessionId ? {sessionId} : {}) })); });
  socket = net.createConnection(`${process.env.XDG_RUNTIME_DIR}/pyxamp/control.sock`);
  socket.setEncoding('utf8'); let buffered = '';
  socket.on('data', data => { buffered += data; const lines = buffered.split('\n'); buffered = lines.pop(); for (const line of lines) { const message = JSON.parse(line); if (message._tag === 'state') { currentState = message.payload; states.push(currentState); } else if (message._tag === 'visualizer') frames.push(message.payload); } });
  await new Promise((resolve, reject) => { socket.once('connect', resolve); socket.once('error', reject); });
  socket.write('{"_tag":"hello","payload":{"version":1}}\n');
  await delay(5000);
  async function snapshot(stage) {
    const dump = JSON.parse(execFileSync('pw-dump', [], { encoding: 'utf8' }));
    save(`${stage}-pipewire`, dump);
    const clients = new Map(dump.filter(item => item.type === 'PipeWire:Interface:Client').map(item => [item.id, item.info?.props || {}]));
    const nodes = dump.filter(item => item.type === 'PipeWire:Interface:Node').map(item => ({ id: item.id, state: item.info?.state, ...clients.get(item.info?.props?.['client.id']), ...item.info?.props })).filter(node => node['media.class'] === 'Stream/Output/Audio');
    const target = currentState?.player?._tag === 'following' ? currentState.player.payload.target : undefined;
    const ownerProcess = target ? execFileSync('dbus-send', ['--session', '--print-reply', '--dest=org.freedesktop.DBus', '/org/freedesktop/DBus', 'org.freedesktop.DBus.GetConnectionUnixProcessID', `string:${target.owner}`], { encoding: 'utf8' }) : null;
    const ownerPid = Number(ownerProcess?.match(/uint32 (\d+)/)?.[1]);
    const ancestry = pid => {
      const chain = [];
      for (let i = 0; i < 64 && pid > 1; i++) {
        try {
          const status = fs.readFileSync(`/proc/${pid}/status`, 'utf8');
          const ppid = Number(status.match(/^PPid:\s+(\d+)/m)?.[1]);
          chain.push({ pid, ppid, executable: fs.readlinkSync(`/proc/${pid}/exe`), command: fs.readFileSync(`/proc/${pid}/cmdline`, 'utf8').split('\0').filter(Boolean) });
          if (pid === ownerPid) break; pid = ppid;
        } catch { break; }
      }
      return chain;
    };
    const browserNodes = nodes.filter(node => ancestry(Number(node['application.process.id'])).some(p => p.pid === ownerPid));
    const recent = frames.slice(-60);
    const current = recent.at(-1);
    const summary = {
      stage, version, browserPid: browser.pid, ownerPid, target, track: currentState?.player?.payload?.track,
      browserNodes: browserNodes.map(node => ({ id: node.id, state: node.state, pid: node['application.process.id'], name: node['media.name'], process: node['application.process.binary'], annotations: Object.keys(node).filter(key => key.startsWith('media.mpris.')), ancestry: ancestry(Number(node['application.process.id'])) })),
      unrelatedNodes: nodes.filter(node => !browserNodes.includes(node)).map(node => ({ id: node.id, name: node['application.name'], pid: node['application.process.id'] })),
      status: current?.status, reason: current?.reason,
      peaks: [2, 18, 37].map(column => ({column, min: Math.min(...recent.map(frame => frame.columns[column])), max: Math.max(...recent.map(frame => frame.columns[column]))})),
    };
    const result = { ...summary, ownerProcess, state: currentState, nodes, recentFrames: frames.slice(-3) };
    save(stage, result); summaries.push(summary); console.log(JSON.stringify(summary, null, 2));
    assert(browserNodes.every(node => !Object.keys(node).some(key => key.startsWith('media.mpris.'))), 'Proof must use unannotated native browser streams');
    if (process.env.PYXAMP_BROWSER_EXPECT_CAPTURE === '1') {
      assert.equal(currentState?.player?._tag, 'following');
      if (stage === 'youtube') {
        assert.equal(browserNodes.length, 1); assert.equal(current.status, 'ready');
        assert(recent.some(frame => frame.columns.some(value => value > 0)));
        assert(new Set(recent.map(frame => frame.columns.join(','))).size > 5, 'Native site PCM must change the spectrum, not just leave nonzero bars');
      } else if (stage === 'two-tabs') {
        assert.equal(browserNodes.length, 2); assert.equal(current.status, 'unavailable');
        assert(current.columns.every(value => value === 0)); assert(current.waveform.every(value => value === 128));
      } else {
        assert.equal(browserNodes.length, 1); assert.equal(current.status, 'ready');
        assert(current.columns[2] > 10); assert(current.columns[18] < 3); assert(current.columns[37] < 3, 'Unrelated native application leaked into browser capture');
        assert(summary.peaks[0].max - summary.peaks[0].min > 5, 'Browser PCM modulation must move the spectrum');
      }
    }
    save(`${stage}-pulse`, execFileSync('pactl', ['--format=json', 'list', 'sink-inputs'], { encoding: 'utf8' }));
    return result;
  }
  if (externalUrl) {
    const targets = await call('Target.getTargets');
    const page = targets.targetInfos.find(target => target.type === 'page');
    const {sessionId} = await call('Target.attachToTarget', {targetId: page.targetId, flatten: true});
    let diagnostics;
    for (let i = 0; i < 30; i++) {
      try {
        diagnostics = await call('Runtime.evaluate', {expression: `(() => { const video = document.querySelector('video'); if (video) { video.loop = true; video.play().catch(() => {}); } return {title: document.title, url: location.href, text: document.body?.innerText.slice(0, 2000), video: video ? {paused: video.paused, readyState: video.readyState, currentTime: video.currentTime, duration: video.duration} : null}; })()`, returnByValue: true, userGesture: true}, sessionId);
      } catch (error) { diagnostics = {error: error.message}; }
      if (currentState?.player?._tag === 'following' && frames.at(-1)?.status === 'ready') break;
      await delay(1000);
    }
    save('site-diagnostics', diagnostics); console.log('Native site diagnostics:', JSON.stringify(diagnostics));
    await delay(5000);
    await snapshot('youtube'); save('summary', summaries); save('states', states); save('frames', frames);
  } else {
  await snapshot('one-tab');
  const second = await call('Target.createTarget', { url: `http://127.0.0.1:${port}/?frequency=6000` });
  await delay(5000);
  await snapshot('two-tabs');
  await call('Target.closeTarget', {targetId: second.targetId});
  await delay(12000);
  await snapshot('one-tab-again');
  save('summary', summaries); save('states', states); save('frames', frames);
  }
} finally {
  save('states', states); save('frames', frames);
  socket?.destroy(); ws?.close(); unrelated?.kill('SIGTERM'); browser.kill('SIGTERM'); server.close();
  await delay(500); if (browser.exitCode === null) browser.kill('SIGKILL');
}
