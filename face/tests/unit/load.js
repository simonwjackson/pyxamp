// Loads a face/lib/*.js QML JavaScript library into Node, the way QML does:
// ".pragma library" is dropped and each ".import "x.js" as Name" becomes a
// context property holding that library.
'use strict';
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const LIB = path.resolve(__dirname, '../../lib');
const cache = new Map();

function load(name) {
  if (cache.has(name)) return cache.get(name);
  const ctx = {};
  let src = fs.readFileSync(path.join(LIB, name), 'utf8');
  src = src.replace(/^\.pragma library\s*$/m, '').replace(/^\.import "([^"]+)" as (\w+)\s*$/gm, (_, file, as) => {
    ctx[as] = load(file);
    return '';
  });
  vm.createContext(ctx);
  vm.runInContext(src, ctx, { filename: name });
  cache.set(name, ctx);
  return ctx;
}

module.exports = { load };
