// The script of the window that pairs a WhatsApp number. It only helps the screen: without it
// the window reloads itself every few seconds (a meta tag the page writes) and every step works.
//
// What it does: every two seconds it asks the hub for the same window again and puts in place
// the two regions that change, the QR (or the pairing code) and the state sentence. It tells the
// hub the serial of the QR it already shows, so the hub does not send the same value again.
// When the pairing ended, it puts in place the whole window, which then holds how it ended.
//
// What it does not do: it writes no markup of its own (the hub's own nodes are moved in), reads no
// cookie, keeps nothing, calls nothing but the address of the window it is on, and has no sentence of
// its own: every word on the screen comes from the page, in the language of the page.
(function () {
  'use strict';

  var ASK_EVERY_MS = 2000;
  var asking = false;
  var stopped = false;

  // Puts a node of the fresh page in the place of the one on this page. The node is imported
  // whole: it is the hub's, already escaped.
  function put(here, fresh) {
    if (!here || !fresh) return null;
    var moved = document.importNode(fresh, true);
    here.replaceWith(moved);
    return moved;
  }

  function serialNow() {
    var qr = document.getElementById('pair-qr');
    return qr ? qr.getAttribute('data-serial') || '0' : '0';
  }

  function swap(text) {
    var next = new DOMParser().parseFromString(text, 'text/html');
    var state = next.getElementById('pair-state');
    if (!state || !document.getElementById('guide')) return;
    if (state.hasAttribute('data-ended')) {
      // It ended: the whole window says how, and nothing more is asked.
      stopped = true;
      var window_ = put(document.getElementById('guide'), next.getElementById('guide'));
      var title = window_ && window_.querySelector('#guide-title');
      if (title) title.focus();
      return;
    }
    ['pair-qr', 'pair-state'].forEach(function (id) {
      var fresh = next.getElementById(id);
      var here = document.getElementById(id);
      if (!fresh || !here || fresh.hasAttribute('data-same')) return;
      if (!here.isEqualNode(fresh)) put(here, fresh);
    });
  }

  function ask() {
    if (stopped || asking || document.hidden) return;
    asking = true;
    fetch(location.pathname + location.search + '&shown=' + encodeURIComponent(serialNow()), {
      credentials: 'same-origin',
      cache: 'no-store',
      redirect: 'manual',
      headers: { 'X-Arauto-Refresh': '1' }
    })
      .then(function (answer) {
        if (answer.type === 'opaqueredirect') {
          // The panel session ended, or the number moved on: show where the hub leads.
          stopped = true;
          location.reload();
          return null;
        }
        return answer.status === 200 ? answer.text() : null;
      })
      .then(function (text) { if (text !== null && !stopped) swap(text); })
      .catch(function () { /* the hub may be restarting: ask again at the next tick */ })
      .then(function () { asking = false; });
  }

  function start() {
    if (!window.fetch || !window.DOMParser || !document.getElementById('pair-live')) return;
    setInterval(ask, ASK_EVERY_MS);
    document.addEventListener('visibilitychange', function () {
      if (!document.hidden) ask();
    });
  }

  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', start);
  else start();
})();
