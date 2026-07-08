'use strict';
// Ziee Office task pane — host-aware same-origin WSS bridge client.
//
// Ported + trimmed from the proven spike (office-spike/taskpane.html). The
// spike's dev-only capability battery (the exhaustive Word/Excel/PowerPoint
// probe matrix) is intentionally dropped; this keeps only what the shipping
// bridge needs:
//   1. detect the Office host on Office.onReady,
//   2. open a same-origin wss://localhost:44300/bridge connection,
//      presenting the per-session token injected into taskpane.html (ITEM-5),
//   3. register DocumentSelectionChanged and forward selection text,
//   4. a simple JSON-RPC ping/echo so the connection is observably live.
//
// The full MCP op vocabulary (read/edit/comment/track-changes) is dispatched
// over this same socket in ITEM-9; this file is the transport skeleton.

var BRIDGE_URL = 'wss://localhost:44300/bridge';

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

// Open the same-origin WSS bridge. The token rides the WebSocket subprotocol
// (DEC-6) so it never appears in a URL/query that could leak via logs.
function openBridge(info) {
  try {
    var token = bridgeToken();
    ws = token ? new WebSocket(BRIDGE_URL, ['ziee-bridge', token])
               : new WebSocket(BRIDGE_URL);
    ws.onopen = function () {
      log('bridge open (host=' + info.host + ', token=' + (token ? 'present' : 'none') + ')');
      // Simple ping/echo so the round-trip is observable.
      send('ping', { host: String(info.host), platform: String(info.platform), at: Date.now() });
    };
    ws.onmessage = function (ev) {
      log('bridge <- ' + ev.data);
    };
    ws.onerror = function () { log('bridge error'); };
    ws.onclose = function () { log('bridge closed'); };
  } catch (e) {
    log('bridge open failed: ' + ((e && e.message) || e));
  }
}

function send(method, params) {
  if (!ws || ws.readyState !== 1) { return; }
  var msg = { jsonrpc: '2.0', id: nextId++, method: method, params: params || {} };
  ws.send(JSON.stringify(msg));
}

// Forward the current selection text on every DocumentSelectionChanged. Uses
// the host-agnostic common API so one handler covers Word/Excel/PowerPoint.
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

Office.onReady(function (info) {
  var host = info.host ? info.host.toString() : 'unknown';
  var h = document.getElementById('h');
  if (h) { h.textContent = 'ziee office bridge — host=' + host; }
  log('Office.onReady host=' + host + ' platform=' + info.platform);

  openBridge(info);

  // Register the selection-change handler (best-effort; a host without the
  // event simply reports a non-success status).
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
