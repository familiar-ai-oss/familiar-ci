'use strict';
// Shared by the JS kits: the writer, the error kind, the frames. Never throws, never prints.
const fs = require('fs');
const path = require('path');

const DEST = process.env.FAMILIAR_AI_REPORT || '';
const ROOT = process.cwd();

function write(obj) {
  if (!DEST) return;
  try { fs.appendFileSync(DEST, JSON.stringify(obj) + '\n'); } catch (_) { /* never raise */ }
}
function kitError(e) { write({ type: 'kit_error', message: String((e && e.stack) || e) }); }

function rel(f) {
  if (!f) return f;
  if (f.startsWith('file://')) { try { f = require('url').fileURLToPath(f); } catch (_) {} }
  return path.isAbsolute(f) ? path.relative(ROOT, f) : f;
}
function inProject(f) {
  if (!f) return false;
  if (f.startsWith('node:') || f.startsWith('internal/')) return false;
  if (f.includes('node_modules')) return false;
  const r = rel(f);
  return !r.startsWith('..') && !path.isAbsolute(r);
}
// stack -> project frames, outermost first, deepest last
function frames(stack) {
  const out = [];
  for (const ln of String(stack || '').split('\n')) {
    const m = ln.match(/^\s*at (?:(.*?) \()?(.+?):(\d+):(\d+)\)?$/);
    if (!m) continue;
    const file = m[2];
    if (!inProject(file)) continue;
    out.push({ file: rel(file), line: Number(m[3]), function: m[1] || null });
  }
  return out.reverse();
}
function strip(s) { return String(s == null ? '' : s).replace(/\x1b\[[0-9;]*m/g, ''); }
function kind(e) {
  const name = (e && (e.name || (e.constructor && e.constructor.name))) || '';
  const msg = strip(e && e.message);
  if (/ is not a function|is not defined|does not provide an export named|is not a constructor/.test(msg)) return 'missing_name';
  if (/AssertionError/.test(name) || (e && (e.matcherResult || e.code === 'ERR_ASSERTION'))) return 'assertion';
  if (/^\s*expect\(/.test(msg)) return 'assertion';
  return 'exception';
}
function error(e) {
  if (e == null) return null;
  if (typeof e === 'string') {
    const s = strip(e);
    const first = s.split('\n').find(l => l.trim()) || '';
    const m = first.match(/^\s*(\w*Error)(?: \[\w+\])?: (.*)$/);
    const pseudo = { name: m ? m[1] : '', message: m ? m[2] : first };
    if (/expect\(/.test(first)) pseudo.name = 'JestAssertionError';
    return { type: pseudo.name || null, message: pseudo.message.slice(0, 500), kind: kind(pseudo), frames: frames(s) };
  }
  return {
    type: e.name || (e.constructor && e.constructor.name) || null,
    message: strip(e.message).slice(0, 500),
    kind: kind(e),
    frames: frames(e.stack),
  };
}
function pkgVersion(name) {
  try { return require(require.resolve(name + '/package.json', { paths: [ROOT] })).version; } catch (_) { return null; }
}
module.exports = { write, kitError, rel, frames, kind, error, pkgVersion, ROOT, DEST, strip };
