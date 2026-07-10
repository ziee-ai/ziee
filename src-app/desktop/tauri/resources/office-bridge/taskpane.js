'use strict';
// Ziee Office task pane — host-aware same-origin WSS bridge client.
//
// Ported + trimmed from the proven spike (office-spike/taskpane.html), then
// extended for the ITEM-9 pane path. Responsibilities:
//   1. detect the Office host on Office.onReady,
//   2. open a same-origin wss://localhost:44300/bridge connection presenting the
//      per-session token injected into taskpane.html (ITEM-5),
//   3. send a `register` hello carrying {host, doc_key} so the daemon broker can
//      route this pane's document (bridge/broker.rs, DEC-1),
//   4. SERVICE daemon->pane run_office_js JSON-RPC requests — the model writes an
//      Office.js body we run inside the host's {Word,Excel,PowerPoint}.run — and
//      reply with the correlated {id, result} | {id, error} (the former typed ops
//      read_document/get_selection/add_comment/set_track_changes/get_tracked_changes
//      are removed; run_office_js subsumes them), and
//   5. forward DocumentSelectionChanged + a ping so the link is observably live.

var BRIDGE_URL = 'wss://localhost:44300/bridge';

// JSON-RPC error codes for pane-side failures (surfaced by the daemon as
// OFFICE_PANE_ERROR). -32601 unknown method; -32002 host-unsupported op.
var ERR_OP_FAILED = -32001;
var ERR_UNSUPPORTED_HOST = -32002;
var ERR_TARGET_MISMATCH = -32004;
var ERR_UNKNOWN_METHOD = -32601;

// Cap on read_document output so a huge document/sheet can't materialize an
// unbounded string in the WebView, over WSS, and into the LLM tool result.
var MAX_READ_CHARS = 100000;

function log(m) {
  var el = document.getElementById('log');
  if (el) { el.textContent += m + '\n'; }
}

// The per-session token stamped into taskpane.html by the bridge listener
// (ITEM-5). Falls back to null when served un-substituted (dev).
function bridgeToken() {
  var t = window.__ZIEE_BRIDGE_TOKEN__;
  if (!t || t === '__ZIEE_BRIDGE_TOKEN__') { return null; }
  return t;
}

var ws = null;
var nextId = 1;
// The Office host name ('Word' | 'Excel' | 'PowerPoint' | 'unknown'), set on ready.
var HOST = 'unknown';
// This pane's own document URL (from register). Used to validate that a request
// actually targets THIS document — defense-in-depth against broker mis-routing.
var SELF_DOC_KEY = '';

// Last path segment, tolerant of / and \ separators (matches the daemon broker).
function baseName(p) {
  if (!p) { return ''; }
  var parts = String(p).split(/[\\/]/);
  return parts[parts.length - 1];
}

function isPathLike(p) { return /[\\/]/.test(String(p || '')); }

// Light path normalization for comparison: drop a file:// scheme, unify separators,
// lowercase (Office file paths are case-insensitive on Win/mac). A Windows file URL
// keeps a leading slash before the drive letter (file:///C:/x -> /c:/x); strip it so
// it matches the native COM path (C:\x -> c:/x) and a legit op isn't falsely rejected.
function normPath(p) {
  var s = String(p || '').replace(/^file:\/\//i, '').replace(/\\/g, '/').toLowerCase();
  return s.replace(/^\/([a-z]):\//, '$1:/');
}

// Whether a request target and this pane's own doc key refer to the same document.
// When BOTH are path-like, compare the full normalized paths (so two docs sharing a
// filename in different directories do NOT match); otherwise fall back to basename.
function sameDoc(target, self) {
  if (isPathLike(target) && isPathLike(self)) { return normPath(target) === normPath(self); }
  return baseName(target).toLowerCase() === baseName(self).toLowerCase();
}

// Truncate read output to the cap; returns { text, truncated }. When truncated, an
// IN-BAND marker is appended to `text` (not only the structured `truncated` flag) so
// the model can't mistake a cut body for the whole document.
function capText(s) {
  s = String(s == null ? '' : s);
  if (s.length <= MAX_READ_CHARS) { return { text: s, truncated: false }; }
  return {
    text: s.slice(0, MAX_READ_CHARS) + '\n…[truncated: document exceeds ' + MAX_READ_CHARS + ' characters]',
    truncated: true
  };
}

// String(v) that can't throw even if v.toString/valueOf throws (v is a
// model-supplied return value or thrown object, so treat it as hostile).
function safeString(v) {
  try { return String(v); } catch (_s) { return '[unstringifiable value]'; }
}

// Serialize a run_office_js return value into a model-safe, capped payload
// (DEC-7): returns { result, truncated, text } where `text` is the capped string
// form (surfaced in the readable tool-result channel) and `result` is the native
// JSON value when it serializes and fits (reply() re-serializes it identically, so
// we skip a redundant re-parse), else the capped string. `undefined` (no `return`)
// → null. A circular / non-serializable value degrades to a string. NEVER throws.
function serializeResult(value) {
  if (typeof value === 'undefined') { return { result: null, truncated: false, text: '' }; }
  var json;
  try { json = JSON.stringify(value); } catch (e) { json = undefined; }
  if (typeof json === 'undefined') {
    // Non-serializable (function / circular / BigInt / Symbol): degrade to a string.
    var cs = capText(safeString(value));
    return { result: cs.text, truncated: cs.truncated, text: cs.text };
  }
  var c = capText(json);
  // Non-truncated: hand back the native `value` (reply() serializes it identically);
  // truncated: keep the capped (partial, no longer valid-JSON) string.
  return { result: c.truncated ? c.text : value, truncated: c.truncated, text: c.text };
}

// Build a STRUCTURED error string from a thrown Office.js error (DEC-9): the name +
// message, plus the OfficeExtension.Error `.code` and `.debugInfo` when present, so
// the daemon's OFFICE_PANE_ERROR carries enough for the model to self-correct in one
// retry. Pure + node-testable. The whole body is wrapped so it NEVER throws — `e` is
// a fully model-controlled thrown value, so even a bare property read (`e.message`)
// can trip a hostile throwing getter (`throw { get message(){ throw 0 } }`); a throw
// here would escape the caller's `.catch` and swallow the reply.
function describeError(prefix, e) {
  try {
    var raw = (e && e.message != null) ? e.message : e;
    var msg = (raw == null || raw === '') ? 'unknown error' : safeString(raw);
    var name = e && e.name != null ? safeString(e.name) : '';
    var out = (name && msg.indexOf(name) !== 0)
      ? prefix + ' failed: ' + name + ': ' + msg
      : prefix + ' failed: ' + msg;
    if (e && e.code) { out += ' [code=' + safeString(e.code) + ']'; }
    if (e && e.debugInfo) {
      try { out += ' debugInfo=' + JSON.stringify(e.debugInfo); } catch (_e) { /* ignore */ }
    }
    return out;
  } catch (_d) {
    return prefix + ' failed: (unreadable error value)';
  }
}

// Open the same-origin WSS bridge. The token rides the WebSocket subprotocol
// (DEC-6) so it never appears in a URL/query that could leak via logs.
function openBridge(info) {
  try {
    var token = bridgeToken();
    ws = token ? new WebSocket(BRIDGE_URL, ['ziee-bridge', token])
               : new WebSocket(BRIDGE_URL);
    ws.onopen = function () {
      log('bridge open (host=' + info.host + ', token=' + (token ? 'present' : 'none') + ')');
      sendRegister(info);
      // Simple ping so the round-trip is observable.
      send('ping', { host: String(info.host), platform: String(info.platform), at: Date.now() });
    };
    ws.onmessage = function (ev) { handleIncoming(ev.data); };
    ws.onerror = function () { log('bridge error'); };
    ws.onclose = function () { log('bridge closed'); };
  } catch (e) {
    log('bridge open failed: ' + ((e && e.message) || e));
  }
}

// Announce this pane + its document to the daemon broker (DEC-1). The doc_key is
// the document URL (empty for a never-saved document → the broker's sole-pane
// fallback covers it).
function sendRegister(info) {
  try {
    Office.context.document.getFilePropertiesAsync(function (r) {
      var url = (r && r.status === 'succeeded' && r.value && r.value.url) ? r.value.url : '';
      SELF_DOC_KEY = url;
      send('register', { host: String(info.host), doc_key: url });
      log('registered (doc_key=' + (url || '<unsaved>') + ')');
    });
  } catch (e) {
    // getFilePropertiesAsync unavailable on this host — register without a key.
    send('register', { host: String(info.host), doc_key: '' });
  }
}

// A pane->daemon request/notification (carries our own auto-incrementing id).
function send(method, params) {
  rawSend({ jsonrpc: '2.0', id: nextId++, method: method, params: params || {} });
}

function rawSend(obj) {
  if (!ws || ws.readyState !== 1) { return; }
  ws.send(JSON.stringify(obj));
}

// Reply to a daemon->pane request, echoing its id (JSON-RPC correlation).
function reply(id, result) { rawSend({ jsonrpc: '2.0', id: id, result: result }); }
function replyErr(id, code, message) {
  rawSend({ jsonrpc: '2.0', id: id, error: { code: code, message: message } });
}

// Classify an inbound frame: a daemon->pane request (a method we service, WITH an
// id to reply to) is dispatched; anything else (our own ping's echo, events) is
// just logged.
function handleIncoming(data) {
  var msg;
  try { msg = JSON.parse(data); } catch (e) { log('bridge <- (unparsable)'); return; }
  if (msg && typeof msg.method === 'string' && msg.id !== undefined && msg.id !== null) {
    dispatchOp(msg.id, msg.method, msg.params || {});
    return;
  }
  log('bridge <- ' + data);
}

function dispatchOp(id, method, params) {
  // Defense-in-depth against broker mis-routing: if this pane has a known document
  // and the request targets a DIFFERENT one, refuse rather than act on the wrong doc.
  // (Skipped for an unsaved doc whose SELF_DOC_KEY is empty and can't be matched.)
  var target = params && params.doc_full_name;
  if (SELF_DOC_KEY && target && !sameDoc(target, SELF_DOC_KEY)) {
    return replyErr(id, ERR_TARGET_MISMATCH,
      'this task pane serves "' + baseName(SELF_DOC_KEY) + '", not "' + baseName(target) + '"');
  }
  try {
    switch (method) {
      case 'run_office_js': opRunOfficeJs(id, params); break;
      default: replyErr(id, ERR_UNKNOWN_METHOD, 'unknown pane method: ' + method);
    }
  } catch (e) {
    replyErr(id, ERR_OP_FAILED, (e && e.message) || String(e));
  }
}

// ─────────────────────────── Office.js op handlers ───────────────────────────

// run_office_js — host-agnostic, open-ended. The model writes an Office.js body; we
// run it inside the host's {Word,Excel,PowerPoint}.run(context => …), let it
// `await context.sync()` + `return` a value, and reply the serialized (capped) value
// (DEC-5/7/8). A compile or runtime error becomes a STRUCTURED replyErr (DEC-9). This
// tool is gated behind per-call user approval by the daemon (office_bridge is not in
// the approval-bypass set), so arbitrary-code risk is user-consented per call.
function opRunOfficeJs(id, params) {
  var script = params && params.script;
  if (typeof script !== 'string' || script.trim() === '') {
    return replyErr(id, ERR_OP_FAILED, 'run_office_js requires a non-empty `script`');
  }
  // Resolve the host runner (unknown/unavailable host → unsupported).
  var run;
  if (HOST === 'Word' && typeof Word !== 'undefined') { run = function (fn) { return Word.run(fn); }; }
  else if (HOST === 'Excel' && typeof Excel !== 'undefined') { run = function (fn) { return Excel.run(fn); }; }
  else if (HOST === 'PowerPoint' && typeof PowerPoint !== 'undefined') { run = function (fn) { return PowerPoint.run(fn); }; }
  else { return replyErr(id, ERR_UNSUPPORTED_HOST, 'run_office_js is not supported on host ' + HOST); }

  // Compile the script as an async function body with `context` in scope. The leading
  // + trailing newlines guard a first/last-line `//` comment in the model's script.
  var body;
  try {
    body = new Function('context', '"use strict"; return (async function () {\n' + script + '\n})();');
  } catch (e) {
    return replyErr(id, ERR_OP_FAILED, describeError('run_office_js compile', e));
  }

  run(function (context) {
    return Promise.resolve(body(context)).then(function (value) {
      var s = serializeResult(value);
      reply(id, { result: s.result, truncated: s.truncated, text: s.text });
    });
  }).catch(function (e) {
    replyErr(id, ERR_OP_FAILED, describeError('run_office_js', e));
  });
}

// Forward the current selection text on every DocumentSelectionChanged. Uses the
// host-agnostic common API so one handler covers Word/Excel/PowerPoint.
function onSelectionChanged() {
  try {
    Office.context.document.getSelectedDataAsync(Office.CoercionType.Text, function (r) {
      var text = (r && r.status === 'succeeded') ? (r.value || '') : '';
      send('selection_changed', { text: text, len: text.length, at: Date.now() });
      log('selection: "' + (text.length > 60 ? text.slice(0, 60) + '…' : text) + '"');
    });
  } catch (e) {
    log('selection read failed: ' + ((e && e.message) || e));
  }
}

// Bootstrap only inside a real Office host (guarded so the pure helpers below can be
// required + unit-tested under node, where `Office` is undefined).
if (typeof Office !== 'undefined' && Office.onReady) {
  Office.onReady(function (info) {
    HOST = info.host ? info.host.toString() : 'unknown';
    var h = document.getElementById('h');
    if (h) { h.textContent = 'ziee office bridge — host=' + HOST; }
    log('Office.onReady host=' + HOST + ' platform=' + info.platform);

    openBridge(info);

    // Register the selection-change handler (best-effort; a host without the event
    // simply reports a non-success status).
    try {
      Office.context.document.addHandlerAsync(
        Office.EventType.DocumentSelectionChanged,
        onSelectionChanged,
        function (r) { log('addHandler status=' + r.status); }
      );
    } catch (e) {
      log('addHandler failed: ' + ((e && e.message) || e));
    }
  });
}

// Export the PURE helpers for node-based unit testing (taskpane.test.mjs). No effect
// in the browser (no `module`); the Office.js op handlers still require a real host.
if (typeof module !== 'undefined' && module.exports) {
  module.exports = { baseName: baseName, isPathLike: isPathLike, normPath: normPath, sameDoc: sameDoc, capText: capText, serializeResult: serializeResult, describeError: describeError, MAX_READ_CHARS: MAX_READ_CHARS };
}
