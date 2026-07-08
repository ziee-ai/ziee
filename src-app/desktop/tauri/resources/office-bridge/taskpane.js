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
//   4. SERVICE daemon->pane JSON-RPC requests (read_document / get_selection /
//      add_comment / set_track_changes / get_tracked_changes) via Office.js and
//      reply with the correlated {id, result} | {id, error} (ITEM-9), and
//   5. forward DocumentSelectionChanged + a ping so the link is observably live.

var BRIDGE_URL = 'wss://localhost:44300/bridge';

// JSON-RPC error codes for pane-side failures (surfaced by the daemon as
// OFFICE_PANE_ERROR). -32601 unknown method; -32002 host-unsupported op.
var ERR_OP_FAILED = -32001;
var ERR_UNSUPPORTED_HOST = -32002;
var ERR_ANCHOR_NOT_FOUND = -32003;
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
      case 'get_selection': opGetSelection(id); break;
      case 'read_document': opReadDocument(id); break;
      case 'add_comment': opAddComment(id, params); break;
      case 'set_track_changes': opSetTrackChanges(id, params); break;
      case 'get_tracked_changes': opGetTrackedChanges(id); break;
      default: replyErr(id, ERR_UNKNOWN_METHOD, 'unknown pane method: ' + method);
    }
  } catch (e) {
    replyErr(id, ERR_OP_FAILED, (e && e.message) || String(e));
  }
}

// ─────────────────────────── Office.js op handlers ───────────────────────────

// get_selection — host-agnostic (Word/Excel/PowerPoint) via the common API.
function opGetSelection(id) {
  Office.context.document.getSelectedDataAsync(Office.CoercionType.Text, function (r) {
    if (r && r.status === 'succeeded') {
      reply(id, { text: r.value || '' });
    } else {
      replyErr(id, ERR_OP_FAILED, 'get_selection failed: ' + ((r && r.error && r.error.message) || 'unknown'));
    }
  });
}

// read_document — Word: full body text; Excel: used range as TSV (DEC-4).
function opReadDocument(id) {
  if (HOST === 'Word' && typeof Word !== 'undefined') {
    Word.run(function (ctx) {
      var body = ctx.document.body;
      body.load('text');
      return ctx.sync().then(function () {
        var c = capText(body.text);
        reply(id, { text: c.text, truncated: c.truncated });
      });
    }).catch(function (e) { replyErr(id, ERR_OP_FAILED, 'read_document failed: ' + e.message); });
  } else if (HOST === 'Excel' && typeof Excel !== 'undefined') {
    Excel.run(function (ctx) {
      var used = ctx.workbook.worksheets.getActiveWorksheet().getUsedRangeOrNullObject();
      used.load('values,isNullObject');
      return ctx.sync().then(function () {
        var text = used.isNullObject ? '' : (used.values || []).map(function (row) {
          return row.join('\t');
        }).join('\n');
        var c = capText(text);
        reply(id, { text: c.text, truncated: c.truncated });
      });
    }).catch(function (e) { replyErr(id, ERR_OP_FAILED, 'read_document failed: ' + e.message); });
  } else {
    replyErr(id, ERR_UNSUPPORTED_HOST, 'read_document is not supported on host ' + HOST);
  }
}

// add_comment — Word only (comments API); anchors on the first match of anchor_text.
function opAddComment(id, params) {
  if (HOST !== 'Word' || typeof Word === 'undefined') {
    return replyErr(id, ERR_UNSUPPORTED_HOST, 'add_comment is only supported in Word');
  }
  var anchor = (params && params.anchor_text) || '';
  var commentText = (params && params.text) || '';
  Word.run(function (ctx) {
    var results = ctx.document.body.search(anchor, { matchCase: false });
    results.load('items');
    return ctx.sync().then(function () {
      if (!results.items || results.items.length === 0) {
        replyErr(id, ERR_ANCHOR_NOT_FOUND, 'anchor_text not found: ' + anchor);
        return null;
      }
      results.items[0].insertComment(commentText);
      return ctx.sync().then(function () { reply(id, { ok: true }); });
    });
  }).catch(function (e) { replyErr(id, ERR_OP_FAILED, 'add_comment failed: ' + e.message); });
}

// set_track_changes — Word only (changeTrackingMode).
function opSetTrackChanges(id, params) {
  if (HOST !== 'Word' || typeof Word === 'undefined') {
    return replyErr(id, ERR_UNSUPPORTED_HOST, 'set_track_changes is only supported in Word');
  }
  var on = !!(params && params.enabled);
  Word.run(function (ctx) {
    ctx.document.changeTrackingMode = on ? Word.ChangeTrackingMode.trackAll : Word.ChangeTrackingMode.off;
    return ctx.sync().then(function () { reply(id, { ok: true, enabled: on }); });
  }).catch(function (e) { replyErr(id, ERR_OP_FAILED, 'set_track_changes failed: ' + e.message); });
}

// get_tracked_changes — Word only (body.getTrackedChanges, Word API 1.6+).
function opGetTrackedChanges(id) {
  if (HOST !== 'Word' || typeof Word === 'undefined') {
    return replyErr(id, ERR_UNSUPPORTED_HOST, 'get_tracked_changes is only supported in Word');
  }
  Word.run(function (ctx) {
    var changes = ctx.document.body.getTrackedChanges();
    changes.load('items/type,items/text,items/author');
    return ctx.sync().then(function () {
      var out = (changes.items || []).map(function (c) {
        return { type: c.type, text: c.text, author: c.author };
      });
      reply(id, { changes: out });
    });
  }).catch(function (e) { replyErr(id, ERR_OP_FAILED, 'get_tracked_changes failed: ' + e.message); });
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
  module.exports = { baseName: baseName, isPathLike: isPathLike, normPath: normPath, sameDoc: sameDoc, capText: capText, MAX_READ_CHARS: MAX_READ_CHARS };
}
