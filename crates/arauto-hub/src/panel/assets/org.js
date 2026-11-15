// The map of the org chart of the Arauto hub (stage A, docs/decisoes/0043-organograma-e-niveis.md).
// It only helps the screen: the page of the hub already holds the same data as plain tables, and
// Publish is a form the page posts like every other form of the panel. This file writes no markup
// (it builds elements with the DOM and sets their text), runs no text, reads no cookie and calls
// no address: the one thing it sends is that form, once. It has no sentence of its own: every
// word it shows comes from the attributes `data-say-*` of the element `#org` or from the stored
// state in `#org-data`, written by the hub in the language of the page.
//
// What the map shows is for the admin to read. No Hermes is told about a level, a role or the
// kind of an arrow: in this version a note is still data, never an order.
(function () {
  'use strict';
  var root = document.documentElement;

  // The geometry of the map, in pixels: the height of a band of one level, the width of a card, where
  // a card starts inside its band, the gap between two cards, and how far the two arrows of a pair
  // sit from each other's line.
  var BAND = 168, CARD_W = 172, CARD_TOP = 52, GAP = 24, PAIR = 14, MOVE_STEP = 24;
  // Where each band starts and how tall it is, set at each drawing of the map.
  var bandTop = [], bandHeight = [];

  function ready(run) {
    if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', run);
    else run();
  }

  function node(tag, cls, text) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined && text !== null) e.textContent = text;
    return e;
  }

  function button(label, onclick, cls) {
    var b = node('button', cls === undefined ? 'button' : cls, label);
    b.type = 'button';
    b.addEventListener('click', onclick);
    return b;
  }

  var ARROW = String.fromCharCode(8594);

  function cloneDraft(draft) {
    return {
      levels: draft.levels.map(function (l) { return { key: l.key, name: l.name }; }),
      hermes: draft.hermes.map(function (h) { return { name: h.name, level: h.level, x: h.x, role: h.role, auto: h.auto }; }),
      links: draft.links.map(function (l) { return { from: l.from, to: l.to, wake: l.wake }; })
    };
  }

  ready(function () {
    var host = document.getElementById('org');
    var form = document.getElementById('org-form');
    var bar = document.getElementById('org-bar');
    var seed = document.getElementById('org-svg-seed');
    var live = document.getElementById('org-live');
    if (!host || !form || !bar || !seed || !live || !document.getElementById('org-data')) return;
    try {
      start(host, form, bar, seed, live);
    } catch (problem) {
      // The static tables stay on the page.
      host.hidden = true;
      root.classList.remove('org-ready');
    }
  });

  function start(host, form, bar, seed, live) {
    var NS = seed.namespaceURI;
    var NAME = new RegExp('^[a-z0-9][a-z0-9_-]{0,31}$');
    var reserved = (host.getAttribute('data-reserved') || '').split(' ');
    var maxLevels = parseInt(host.getAttribute('data-max-levels'), 10) || 12;
    var maxRole = parseInt(host.getAttribute('data-max-role'), 10) || 280;
    var maxX = parseInt(host.getAttribute('data-max-x'), 10) || 4000;
    var accessesAddress = host.getAttribute('data-accesses') || '';

    var base = null;      // the stored chart as the hub wrote it
    var info = {};        // by name of Hermes: what the hub says of it (state, numbers, the WhatsApp switch)
    var S = null;         // the draft: what Publish would write
    var hist = [];        // snapshots of the draft, for Undo
    var echoed = false;   // a refused draft was loaded: it is the draft, and the page holds still
    var submitting = false;
    var view = window.matchMedia && window.matchMedia('(max-width: 640px)').matches ? 'list' : 'map';
    var sel = null;       // the Hermes whose drawer is open
    var selLink = null;   // the arrow whose popover is open, as {from, to}
    var said = '';        // the line of the status
    var dialog = null;
    var pending = false;  // a refresh waits for the admin to leave a field

    var body = node('div', 'org-body');
    host.appendChild(body);

    // ------------------------------------------------------------------ words

    function say(name, values) {
      var text = host.getAttribute('data-say-' + name) || '';
      if (values) {
        Object.keys(values).forEach(function (key) {
          text = text.split('{' + key + '}').join(String(values[key]));
        });
      }
      return text;
    }

    // ------------------------------------------------------------------ the data

    function read() {
      var data = document.getElementById('org-data');
      if (!data) return null;
      return { version: data.getAttribute('data-version'), state: JSON.parse(data.getAttribute('data-state')) };
    }

    function adopt(stored) {
      base = {
        version: stored.version,
        levels: stored.state.levels,
        hermes: stored.state.hermes.map(function (h) {
          return { name: h.name, level: h.level, x: h.x, role: h.role || '' };
        }),
        links: stored.state.links
      };
      info = {};
      stored.state.hermes.forEach(function (h) {
        info[h.name] = { tone: h.tone, state: h.state, woken: h.woken, numbers: h.numbers };
      });
    }

    function levelIndex(draft, key) {
      for (var i = 0; i < draft.levels.length; i++) if (draft.levels[i].key === key) return i;
      return -1;
    }

    function rightOf(draft, key) {
      var found = -1;
      draft.hermes.forEach(function (h) { if (h.level === key && h.x > found) found = h.x; });
      return found < 0 ? GAP : Math.min(maxX, found + CARD_W + GAP);
    }

    // A Hermes with no stored place is shown in the lowest level, at the right of the cards that are
    // there. That is not an edit: publishing is what writes its place.
    function draftOf(stored) {
      var draft = { levels: [], hermes: [], links: [] };
      stored.levels.forEach(function (l) { draft.levels.push({ key: l.key, name: l.name }); });
      if (!draft.levels.length) draft.levels.push({ key: 'new-1', name: 'level-1' });
      stored.hermes.forEach(function (h) {
        if (h.level) draft.hermes.push({ name: h.name, level: h.level, x: h.x, role: h.role, auto: false });
      });
      var lowest = draft.levels[draft.levels.length - 1].key;
      stored.hermes.forEach(function (h) {
        if (h.level) return;
        draft.hermes.push({ name: h.name, level: lowest, x: rightOf(draft, lowest), role: h.role, auto: true });
      });
      stored.links.forEach(function (l) { draft.links.push({ from: l.from, to: l.to, wake: l.wake }); });
      return draft;
    }

    function unusedNewKey(used) {
      for (var n = 1; n <= maxLevels; n++) if (used.indexOf('new-' + n) < 0) return 'new-' + n;
      return null;
    }

    // The draft the hub refused is put back over the chart as it is now: what no longer exists goes,
    // a level it named that is gone becomes a new level with the same name.
    function draftFromEcho(wanted) {
      var draft = { levels: [], hermes: [], links: [] };
      var stored = {};
      base.levels.forEach(function (l) { stored[l.key] = true; });
      var used = [];
      wanted.levels.forEach(function (l) { if (l.key.indexOf('new-') === 0) used.push(l.key); });
      var remap = {};
      wanted.levels.forEach(function (l) {
        var key = l.key;
        if (key.indexOf('new-') !== 0 && !stored[key]) {
          key = unusedNewKey(used);
          used.push(key);
        }
        remap[l.key] = key;
        draft.levels.push({ key: key, name: l.name });
      });
      var seen = {};
      wanted.hermes.forEach(function (h) {
        if (!info[h.name] || remap[h.level] === undefined) return;
        seen[h.name] = true;
        draft.hermes.push({ name: h.name, level: remap[h.level], x: h.x, role: h.role, auto: false });
      });
      var lowest = draft.levels[draft.levels.length - 1].key;
      base.hermes.forEach(function (h) {
        if (seen[h.name]) return;
        draft.hermes.push({ name: h.name, level: lowest, x: rightOf(draft, lowest), role: h.role, auto: true });
      });
      wanted.links.forEach(function (l) {
        var ends = draft.hermes.map(function (h) { return h.name; });
        if (ends.indexOf(l.from) >= 0 && ends.indexOf(l.to) >= 0) draft.links.push({ from: l.from, to: l.to, wake: l.wake });
      });
      return draft;
    }

    function hermesOf(name) {
      for (var i = 0; i < S.hermes.length; i++) if (S.hermes[i].name === name) return S.hermes[i];
      return null;
    }

    function linkOf(from, to) {
      for (var i = 0; i < S.links.length; i++) if (S.links[i].from === from && S.links[i].to === to) return i;
      return -1;
    }

    function kindOf(link) {
      var a = levelIndex(S, hermesOf(link.from).level), b = levelIndex(S, hermesOf(link.to).level);
      return a < b ? 'instruction' : a > b ? 'request' : 'note';
    }

    // A pointer is down on the map: a redraw now would drop the drag and its first move.
    var pointerBusy = false;

    function held() { return hist.length > 0 || echoed; }

    function sync() {
      if (held()) root.setAttribute('data-hold', '');
      else root.removeAttribute('data-hold');
    }

    // ------------------------------------------------------------------ the draft and its changes

    function mutate(change) {
      hist.push(cloneDraft(S));
      if (hist.length > 100) hist.shift();
      said = '';
      change();
      render();
    }

    function diff() {
      var out = [];
      var wasName = {}, nowName = {};
      base.levels.forEach(function (l) { wasName[l.key] = l.name; });
      S.levels.forEach(function (l) { nowName[l.key] = l.name; });
      S.levels.forEach(function (l) {
        if (wasName[l.key] === undefined) out.push(say('change-level-new', { name: l.name }));
        else if (wasName[l.key] !== l.name) out.push(say('change-level-renamed', { before: wasName[l.key], after: l.name }));
      });
      base.levels.forEach(function (l) {
        if (nowName[l.key] === undefined) out.push(say('change-level-removed', { name: l.name }));
      });
      var keptBefore = base.levels.filter(function (l) { return nowName[l.key] !== undefined; }).map(function (l) { return l.key; });
      var keptNow = S.levels.filter(function (l) { return wasName[l.key] !== undefined; }).map(function (l) { return l.key; });
      if (keptBefore.join(',') !== keptNow.join(',')) {
        out.push(say('change-level-order', { names: S.levels.map(function (l) { return l.name; }).join(', ') }));
      }
      var was = {};
      base.hermes.forEach(function (h) { was[h.name] = h; });
      var places = 0;
      S.hermes.slice().sort(function (a, b) { return a.name < b.name ? -1 : a.name > b.name ? 1 : 0; }).forEach(function (h) {
        var o = was[h.name];
        if (!o) return;
        if (!o.level) out.push(say('change-placed', { name: h.name, after: nowName[h.level] }));
        else if (o.level !== h.level) out.push(say('change-level', { name: h.name, before: wasName[o.level], after: nowName[h.level] }));
        else if (o.x !== h.x) places++;
        if ((o.role || '') !== (h.role || '')) out.push(say('change-role', { name: h.name }));
      });
      if (places) out.push(say('change-places', { n: places }));
      var before = {}, now = {};
      base.links.forEach(function (l) { before[l.from + ' ' + l.to] = l; });
      S.links.forEach(function (l) { now[l.from + ' ' + l.to] = l; });
      S.links.forEach(function (l) {
        if (!before[l.from + ' ' + l.to]) {
          out.push(say(l.wake ? 'change-link-new-wake' : 'change-link-new', { from: l.from, to: l.to, kind: say('kind-' + kindOf(l)) }));
        }
      });
      S.links.forEach(function (l) {
        var o = before[l.from + ' ' + l.to];
        if (o && o.wake !== l.wake) out.push(say(l.wake ? 'change-wake-on' : 'change-wake-off', { from: l.from, to: l.to }));
      });
      base.links.forEach(function (l) {
        if (!now[l.from + ' ' + l.to]) out.push(say('change-link-removed', { from: l.from, to: l.to }));
      });
      return out;
    }

    function createsLink() {
      var before = {};
      base.links.forEach(function (l) { before[l.from + ' ' + l.to] = true; });
      return S.links.some(function (l) { return !before[l.from + ' ' + l.to]; });
    }

    // ------------------------------------------------------------------ the rules, as the hub keeps them

    function badCharacter(code) {
      return code < 32 || (code >= 127 && code <= 159) || code === 8232 || code === 8233 ||
        (code >= 8234 && code <= 8238) || (code >= 8294 && code <= 8297) || (code >= 917504 && code <= 917631);
    }

    // One line: runs of white space (new lines included) become one space, and the ends go.
    function roleOf(text) {
      var value = String(text).replace(/\s+/g, ' ').trim();
      var letters = Array.from(value);
      if (letters.length > maxRole) return null;
      for (var i = 0; i < letters.length; i++) if (badCharacter(letters[i].codePointAt(0))) return null;
      return value;
    }

    function nameOk(value) {
      return NAME.test(value) && reserved.indexOf(value) < 0;
    }

    function clampX(x) {
      return Math.max(0, Math.min(maxX, Math.round(x)));
    }

    // ------------------------------------------------------------------ the changes the admin makes

    function addLevel() {
      if (S.levels.length >= maxLevels) { said = say('rule-levels', { max: maxLevels }); render(); return; }
      var names = S.levels.map(function (l) { return l.name; });
      var n = 1;
      while (names.indexOf('level-' + n) >= 0) n++;
      var key = unusedNewKey(S.levels.map(function (l) { return l.key; }));
      mutate(function () { S.levels.push({ key: key, name: 'level-' + n }); });
    }

    function renameLevel(index, value) {
      var level = S.levels[index];
      var text = String(value).trim();
      if (text === level.name) { render(); return; }
      if (!nameOk(text)) { said = say('rule-level-name'); render(); return; }
      if (S.levels.some(function (other, at) { return at !== index && other.name === text; })) {
        said = say('level-taken'); render(); return;
      }
      mutate(function () { level.name = text; });
    }

    function moveLevel(index, step) {
      var other = index + step;
      if (other < 0 || other >= S.levels.length) return;
      mutate(function () {
        var swap = S.levels[index];
        S.levels[index] = S.levels[other];
        S.levels[other] = swap;
      });
    }

    function levelIsEmpty(key) {
      return !S.hermes.some(function (h) { return h.level === key; });
    }

    function removeLevel(index) {
      if (S.levels.length < 2 || !levelIsEmpty(S.levels[index].key)) return;
      mutate(function () { S.levels.splice(index, 1); });
    }

    function freeX(key, x, except) {
      var clash = S.hermes.some(function (h) {
        return h.name !== except && h.level === key && Math.abs(h.x - x) < CARD_W + GAP / 2;
      });
      return clash ? rightOf(S, key) : x;
    }

    function setLevel(name, key) {
      var h = hermesOf(name);
      if (h.level === key) return;
      mutate(function () { h.level = key; h.x = freeX(key, h.x, name); h.auto = false; });
    }

    function moveSide(name, step) {
      var h = hermesOf(name);
      var x = clampX(h.x + step);
      if (x === h.x) return;
      mutate(function () { h.x = x; h.auto = false; });
    }

    function setRole(name, text, field) {
      var h = hermesOf(name);
      var value = roleOf(text);
      if (value === null) { said = say('rule-role'); render(); return; }
      if (value === h.role) { if (field && field.value !== value) field.value = value; return; }
      mutate(function () { h.role = value; });
    }

    function addLink(from, to) {
      if (from === to || !hermesOf(from) || !hermesOf(to)) return;
      if (linkOf(from, to) >= 0) { said = say('link-exists'); render(); return; }
      mutate(function () {
        S.links.push({ from: from, to: to, wake: false });
        selLink = view === 'map' ? { from: from, to: to } : null;
        sel = null;
      });
    }

    function removeLink(from, to) {
      var at = linkOf(from, to);
      if (at < 0) return;
      mutate(function () { S.links.splice(at, 1); selLink = null; });
    }

    function setWake(from, to, wake) {
      var at = linkOf(from, to);
      if (at < 0 || S.links[at].wake === wake) return;
      mutate(function () { S.links[at].wake = wake; });
    }

    function wayBack(from, to) {
      if (linkOf(to, from) >= 0) removeLink(to, from);
      else addLink(to, from);
    }

    function undo() {
      if (!hist.length) return;
      S = hist.pop();
      said = '';
      if (sel !== null && !hermesOf(sel)) sel = null;
      render();
    }

    // ------------------------------------------------------------------ the toolbar

    var toolbar = (function () {
      var group = node('span', 'seg');
      group.setAttribute('role', 'group');
      group.setAttribute('aria-label', say('view'));
      var mapButton = button(say('map'), function () { view = 'map'; render(); }, 'button');
      var listButton = button(say('list'), function () { view = 'list'; render(); }, 'button');
      group.appendChild(mapButton);
      group.appendChild(listButton);
      var add = button(say('add-level'), addLevel);
      var back = button(say('undo'), undo);
      var count = node('span', 'org-count');
      var publish = button(say('publish'), openPublish, 'button primary');
      bar.appendChild(group);
      bar.appendChild(add);
      bar.appendChild(back);
      bar.appendChild(count);
      bar.appendChild(publish);
      return { map: mapButton, list: listButton, undo: back, count: count, publish: publish };
    })();

    function updateToolbar(lines) {
      toolbar.map.setAttribute('aria-pressed', view === 'map' ? 'true' : 'false');
      toolbar.list.setAttribute('aria-pressed', view === 'list' ? 'true' : 'false');
      toolbar.undo.disabled = hist.length === 0;
      toolbar.publish.disabled = lines.length === 0;
      toolbar.count.textContent = lines.length === 0 ? '' :
        lines.length === 1 ? say('draft-one') : say('draft-many', { n: lines.length });
    }

    // ------------------------------------------------------------------ Publish

    function openPublish() {
      var lines = diff();
      if (!lines.length) return;
      if (!dialog) {
        dialog = node('dialog', 'modal');
        dialog.setAttribute('aria-labelledby', 'org-publish-title');
        var head = node('div', 'modal-head');
        var title = node('h2', '', say('publish-title'));
        title.id = 'org-publish-title';
        head.appendChild(title);
        var bodyBox = node('div', 'modal-body short');
        bodyBox.appendChild(node('p', '', say('publish-note')));
        var warning = node('p', 'org-warning', say('cross-warning'));
        warning.id = 'org-publish-warning';
        bodyBox.appendChild(warning);
        var list = node('ul', 'org-changes');
        list.id = 'org-publish-changes';
        bodyBox.appendChild(list);
        var foot = node('div', 'modal-foot');
        foot.appendChild(button(say('publish'), send, 'button primary'));
        foot.appendChild(button(say('back'), function () { dialog.close(); }));
        dialog.appendChild(head);
        dialog.appendChild(bodyBox);
        dialog.appendChild(foot);
        host.parentNode.appendChild(dialog);
      }
      var changes = document.getElementById('org-publish-changes');
      while (changes.firstChild) changes.removeChild(changes.firstChild);
      lines.forEach(function (line) { changes.appendChild(node('li', '', line)); });
      document.getElementById('org-publish-warning').hidden = !createsLink();
      dialog.showModal();
    }

    function wire() {
      return {
        levels: S.levels.map(function (l) { return { key: l.key, name: l.name }; }),
        hermes: S.hermes.map(function (h) { return { name: h.name, level: h.level, x: h.x, role: h.role }; }),
        links: S.links.map(function (l) { return { from: l.from, to: l.to, wake: l.wake }; })
      };
    }

    function send() {
      if (submitting) return;
      submitting = true;
      form.querySelector('input[name="version"]').value = base.version;
      form.querySelector('input[name="draft"]').value = JSON.stringify(wire());
      HTMLFormElement.prototype.submit.call(form);
    }

    // ------------------------------------------------------------------ the map

    function chips(card, h) {
      var box = node('div', 'chips');
      var numbers = (info[h.name] && info[h.name].numbers) || [];
      numbers.forEach(function (n) {
        if (n.mode === 'answers') box.appendChild(node('span', 'chip answers', say('chip-answers', { alias: n.alias })));
        else if (n.mode === 'reads') box.appendChild(node('span', 'chip', say('chip-reads', { alias: n.alias })));
        else box.appendChild(node('span', 'chip', n.alias));
      });
      if (!numbers.length) box.appendChild(node('span', 'chip', say('no-number')));
      if (h.auto) box.appendChild(node('span', 'chip', say('not-placed')));
      card.appendChild(box);
    }

    function svgNode(tag, attributes) {
      var e = document.createElementNS(NS, tag);
      Object.keys(attributes || {}).forEach(function (key) { e.setAttribute(key, attributes[key]); });
      return e;
    }

    function curve(a, b, vertical) {
      if (vertical) {
        var mid = (a.y + b.y) / 2;
        return 'M' + a.x + ' ' + a.y + 'C' + a.x + ' ' + mid + ' ' + b.x + ' ' + mid + ' ' + b.x + ' ' + b.y;
      }
      var half = (a.x + b.x) / 2;
      return 'M' + a.x + ' ' + a.y + 'C' + half + ' ' + a.y + ' ' + half + ' ' + b.y + ' ' + b.x + ' ' + b.y;
    }

    function drawMap(box) {
      var scroll = node('div', 'org-scroll');
      var map = node('div', 'org-map');
      map.setAttribute('role', 'group');
      map.setAttribute('aria-label', say('org-chart'));
      var right = 0;
      S.hermes.forEach(function (h) { right = Math.max(right, h.x + CARD_W + GAP); });
      var bands = [];
      map.style.minWidth = Math.max(760, right) + 'px';
      S.levels.forEach(function (level, index) {
        var band = node('div', 'band');
        bands.push(band);
        var label = node('div', 'band-name');
        label.appendChild(node('span', '', String(index + 1)));
        var input = node('input');
        input.type = 'text';
        input.value = level.name;
        input.maxLength = 32;
        input.setAttribute('aria-label', say('level-name', { n: index + 1 }));
        input.setAttribute('data-fk', 'lname:' + index);
        input.addEventListener('change', function () { renameLevel(index, input.value); });
        input.addEventListener('pointerdown', function (e) { e.stopPropagation(); });
        label.appendChild(input);
        if (S.levels.length > 1 && levelIsEmpty(level.key)) {
          var drop = button(String.fromCharCode(215), function () { removeLevel(index); }, 'band-remove');
          drop.title = say('remove-level');
          drop.setAttribute('aria-label', say('remove-level'));
          drop.setAttribute('data-fk', 'ldrop:' + index);
          drop.addEventListener('pointerdown', function (e) { e.stopPropagation(); });
          label.appendChild(drop);
        }
        band.appendChild(label);
        map.appendChild(band);
      });
      var canvas = svgNode('svg', { 'class': 'org-wires', 'aria-hidden': 'true' });
      var defs = svgNode('defs');
      var marker = svgNode('marker', {
        id: 'org-arrow', viewBox: '0 0 10 10', refX: '9', refY: '5', markerWidth: '7', markerHeight: '7', orient: 'auto-start-reverse'
      });
      marker.appendChild(svgNode('path', { d: 'M0 0L10 5L0 10z', fill: 'context-stroke' }));
      defs.appendChild(marker);
      canvas.appendChild(defs);
      map.appendChild(canvas);
      var cards = {};
      S.hermes.forEach(function (h) {
        var card = node('div', 'org-card' + (sel === h.name ? ' sel' : ''));
        card.tabIndex = 0;
        card.setAttribute('role', 'button');
        card.setAttribute('data-name', h.name);
        card.setAttribute('data-fk', 'card:' + h.name);
        var at = levelIndex(S, h.level);
        var facts = info[h.name] || { tone: 'idle', state: '' };
        card.setAttribute('aria-label', say('card-label', { name: h.name, level: at + 1, state: facts.state }));
        card.style.left = h.x + 'px';
        var title = node('div', 'nm');
        var dot = node('span', 'org-dot ' + facts.tone);
        dot.title = facts.state;
        title.appendChild(dot);
        title.appendChild(node('span', '', h.name));
        card.appendChild(title);
        chips(card, h);
        var handle = node('span', 'handle');
        handle.title = say('handle');
        handle.setAttribute('aria-hidden', 'true');
        card.appendChild(handle);
        map.appendChild(card);
        cards[h.name] = card;
      });
      scroll.appendChild(map);
      box.appendChild(scroll);

      // The cards are in the page now: their height is known. A band grows to hold its tallest
      // card, so a Hermes with many numbers is not cut by the next level.
      var tall = S.levels.map(function () { return 0; });
      S.hermes.forEach(function (h) {
        var at = levelIndex(S, h.level);
        tall[at] = Math.max(tall[at], cards[h.name].offsetHeight);
      });
      bandTop = [];
      bandHeight = [];
      var below = 0;
      S.levels.forEach(function (level, index) {
        bandHeight[index] = Math.max(BAND, CARD_TOP + tall[index] + GAP + 8);
        bandTop[index] = below;
        bands[index].style.top = below + 'px';
        bands[index].style.height = bandHeight[index] + 'px';
        below += bandHeight[index];
      });
      map.style.height = below + 'px';
      var spot = {};
      S.hermes.forEach(function (h) {
        var y = bandTop[levelIndex(S, h.level)] + CARD_TOP;
        cards[h.name].style.top = y + 'px';
        spot[h.name] = { x: h.x, y: y, h: cards[h.name].offsetHeight };
      });
      function anchor(link) {
        var a = spot[link.from], b = spot[link.to], kind = kindOf(link);
        var paired = linkOf(link.to, link.from) >= 0;
        var off = paired ? (link.from < link.to ? -PAIR : PAIR) : 0;
        if (kind === 'note') {
          var toRight = a.x < b.x;
          return {
            a: { x: a.x + (toRight ? CARD_W : 0), y: a.y + a.h / 2 + off },
            b: { x: b.x + (toRight ? 0 : CARD_W), y: b.y + b.h / 2 + off },
            vertical: false
          };
        }
        var down = kind === 'instruction';
        return {
          a: { x: a.x + CARD_W / 2 + off, y: a.y + (down ? a.h + 9 : 0) },
          b: { x: b.x + CARD_W / 2 + off, y: b.y + (down ? 0 : b.h + 9) },
          vertical: true
        };
      }
      var shown = null;
      S.links.forEach(function (link) {
        var where = anchor(link);
        var d = curve(where.a, where.b, where.vertical);
        var chosen = selLink && selLink.from === link.from && selLink.to === link.to;
        var wireLine = svgNode('path', { 'class': 'wire ' + kindOf(link) + (chosen ? ' sel' : ''), d: d, 'marker-end': 'url(#org-arrow)' });
        var hit = svgNode('path', { 'class': 'hit', d: d });
        hit.addEventListener('click', function (e) {
          e.stopPropagation();
          selLink = { from: link.from, to: link.to };
          sel = null;
          render();
        });
        canvas.appendChild(wireLine);
        canvas.appendChild(hit);
        if (chosen) shown = { link: link, where: where };
      });
      if (shown) popover(map, shown.link, shown.where);

      // ---- pointers: drag a card, pull a link from its handle, click to select
      var act = null;
      pointerBusy = false;
      map.addEventListener('pointerdown', function (e) {
        if (e.button !== undefined && e.button !== 0) return;
        var card = e.target.closest ? e.target.closest('.org-card') : null;
        if (!card) {
          if (sel !== null || selLink !== null) { sel = null; selLink = null; render(); }
          return;
        }
        var h = hermesOf(card.getAttribute('data-name'));
        var rect = map.getBoundingClientRect();
        if (e.target.classList.contains('handle')) {
          var line = svgNode('path', { 'class': 'wire draw' });
          canvas.appendChild(line);
          act = { type: 'link', h: h, line: line, rect: rect };
        } else {
          act = {
            type: 'move', h: h, el: card, rect: rect, moved: false,
            sx: e.clientX, sy: e.clientY,
            dx: e.clientX - rect.left - h.x,
            dy: e.clientY - rect.top - (bandTop[levelIndex(S, h.level)] + CARD_TOP)
          };
          card.classList.add('drag');
        }
        pointerBusy = true;
        map.setPointerCapture(e.pointerId);
        e.preventDefault();
      });
      map.addEventListener('pointermove', function (e) {
        if (!act) return;
        var x = e.clientX - act.rect.left, y = e.clientY - act.rect.top;
        if (act.type === 'link') {
          var from = spot[act.h.name];
          act.line.setAttribute('d', 'M' + (from.x + CARD_W / 2) + ' ' + (from.y + from.h + 9) + 'L' + x + ' ' + y);
          return;
        }
        if (!act.moved && Math.abs(e.clientX - act.sx) + Math.abs(e.clientY - act.sy) < 4) return;
        act.moved = true;
        act.left = Math.max(0, Math.min(maxX, x - act.dx));
        act.top = Math.max(0, y - act.dy);
        act.el.style.left = act.left + 'px';
        act.el.style.top = act.top + 'px';
      });
      map.addEventListener('pointerup', function (e) {
        if (!act) return;
        var done = act;
        act = null;
        pointerBusy = false;
        if (done.type === 'link') {
          done.line.remove();
          var under = document.elementFromPoint(e.clientX, e.clientY);
          var target = under && under.closest ? under.closest('.org-card') : null;
          if (target) addLink(done.h.name, target.getAttribute('data-name'));
          return;
        }
        done.el.classList.remove('drag');
        if (!done.moved) { openDrawer(done.h.name, false); return; }
        var centre = done.top + spot[done.h.name].h / 2;
        var index = 0;
        while (index < S.levels.length - 1 && centre >= bandTop[index] + bandHeight[index]) index += 1;
        mutate(function () {
          done.h.level = S.levels[index].key;
          done.h.x = clampX(done.left);
          done.h.auto = false;
        });
      });
      map.addEventListener('pointercancel', function () {
        if (!act) return;
        if (act.line) act.line.remove();
        if (act.el) act.el.classList.remove('drag');
        act = null;
        pointerBusy = false;
        render();
      });
      map.addEventListener('keydown', function (e) {
        var card = e.target.closest ? e.target.closest('.org-card') : null;
        if (card && e.target === card && (e.key === 'Enter' || e.key === ' ')) {
          e.preventDefault();
          openDrawer(card.getAttribute('data-name'), true);
        }
      });
    }

    function popover(map, link, where) {
      var pop = node('div', 'org-pop');
      pop.style.left = Math.max(8, (where.a.x + where.b.x) / 2 - 120) + 'px';
      pop.style.top = (where.a.y + where.b.y) / 2 + 10 + 'px';
      pop.addEventListener('pointerdown', function (e) { e.stopPropagation(); });
      pop.appendChild(node('b', '', link.from + ' ' + ARROW + ' ' + link.to));
      pop.appendChild(node('span', '', say('kind-of', { kind: say('kind-' + kindOf(link)) })));
      var label = node('label', 'option');
      var check = node('input');
      check.type = 'checkbox';
      check.checked = link.wake;
      check.setAttribute('data-fk', 'popwake');
      check.addEventListener('change', function () { setWake(link.from, link.to, check.checked); });
      label.appendChild(check);
      label.appendChild(node('span', '', say('wakes-name', { name: link.to })));
      pop.appendChild(label);
      pop.appendChild(node('span', 'hint', say('wake-hint', { name: link.to })));
      var row = node('div', 'org-row');
      var back = linkOf(link.to, link.from) >= 0;
      row.appendChild(button(say(back ? 'way-back-remove' : 'way-back-create'), function () { wayBack(link.from, link.to); }));
      row.appendChild(button(say('remove'), function () { removeLink(link.from, link.to); }));
      row.appendChild(button(say('close'), function () { selLink = null; render(); }));
      pop.appendChild(row);
      map.appendChild(pop);
      // The window stays inside the map: above the arrow when there is no room below it, and the
      // map grows when there is room on neither side.
      var middle = (where.a.y + where.b.y) / 2, high = pop.offsetHeight;
      if (middle + 10 + high + 8 > map.offsetHeight) {
        if (middle - 10 - high >= 8) pop.style.top = middle - 10 - high + 'px';
        else map.style.height = middle + 10 + high + 8 + 'px';
      }
    }

    // ------------------------------------------------------------------ the drawer of a Hermes

    var focusDrawer = false;

    function openDrawer(name, focus) {
      sel = name;
      selLink = null;
      focusDrawer = focus;
      render();
    }

    function labelled(text, control) {
      var label = node('label', 'org-field');
      label.appendChild(node('span', '', text));
      label.appendChild(control);
      return label;
    }

    function levelSelect(h, fk) {
      var select = node('select');
      S.levels.forEach(function (level) {
        var option = node('option', '', level.name);
        option.value = level.key;
        if (level.key === h.level) option.selected = true;
        select.appendChild(option);
      });
      select.setAttribute('data-fk', fk);
      select.addEventListener('change', function () { setLevel(h.name, select.value); });
      return select;
    }

    function drawDrawer(box, h) {
      var drawer = node('aside', 'org-drawer');
      drawer.setAttribute('aria-label', h.name);
      drawer.appendChild(node('h3', '', h.name));
      var grid = node('div', 'org-grid');

      var area = node('textarea');
      area.rows = 3;
      area.maxLength = maxRole;
      area.value = h.role;
      area.id = 'org-role';
      area.setAttribute('data-fk', 'role');
      area.addEventListener('change', function () { setRole(h.name, area.value, area); });
      var roleBox = labelled(say('role-label'), area);
      roleBox.appendChild(node('span', 'hint', say('role-hint')));
      grid.appendChild(roleBox);

      var place = node('div', 'org-stack');
      place.appendChild(labelled(say('level'), levelSelect(h, 'dlevel')));
      var moves = node('div', 'org-row');
      moves.appendChild(button(say('move-left'), function () { moveSide(h.name, -MOVE_STEP); }));
      moves.appendChild(button(say('move-right'), function () { moveSide(h.name, MOVE_STEP); }));
      place.appendChild(moves);
      var facts = info[h.name] || { woken: false, numbers: [] };
      grid.appendChild(place);
      drawer.appendChild(grid);

      drawer.appendChild(node('h4', 'org-sub', say('numbers')));
      var numbers = node('div', 'chips');
      chips(numbers, { name: h.name, auto: false });
      drawer.appendChild(numbers.firstChild || numbers);
      if (!facts.numbers.length) drawer.appendChild(node('p', 'hint', say('no-access')));
      if (facts.numbers.length) drawer.appendChild(node('p', 'hint', say('woken-by-whatsapp') + ': ' + say(facts.woken ? 'yes' : 'no')));
      var link = node('a', 'footer-link', say('change-in-accesses'));
      link.setAttribute('href', accessesAddress);
      drawer.appendChild(link);

      drawer.appendChild(node('h4', 'org-sub', say('links-of')));
      var mine = node('ul', 'org-links');
      S.links.forEach(function (l) {
        if (l.from !== h.name && l.to !== h.name) return;
        var item = node('li');
        item.appendChild(node('span', '', l.from + ' ' + ARROW + ' ' + l.to));
        var drop = button(say('remove'), function () { removeLink(l.from, l.to); });
        drop.setAttribute('data-fk', 'dl:' + l.from + ':' + l.to);
        item.appendChild(drop);
        mine.appendChild(item);
      });
      drawer.appendChild(mine);
      var others = S.hermes.filter(function (o) { return o.name !== h.name; });
      if (others.length) {
        var pick = node('select');
        others.forEach(function (o) {
          var option = node('option', '', o.name);
          option.value = o.name;
          pick.appendChild(option);
        });
        pick.setAttribute('data-fk', 'dpick');
        var adder = node('div', 'org-row');
        adder.appendChild(labelled(say('link-to'), pick));
        adder.appendChild(button(say('add-link'), function () { addLink(h.name, pick.value); }));
        drawer.appendChild(adder);
      }
      drawer.appendChild(button(say('close'), function () { closeDrawer(true); }));
      box.appendChild(drawer);
    }

    function closeDrawer(giveFocusBack) {
      var name = sel;
      sel = null;
      selLink = null;
      render();
      if (giveFocusBack && name) focusKey('card:' + name);
    }

    // ------------------------------------------------------------------ the list: the keyboard path

    function cell(row, child, header) {
      var c = node(header ? 'th' : 'td');
      if (header) c.scope = 'col';
      if (typeof child === 'string') c.textContent = child;
      else if (child) c.appendChild(child);
      row.appendChild(c);
      return c;
    }

    function table(title, heads) {
      var wrapper = node('div', 'org-table');
      wrapper.appendChild(node('h3', 'org-sub', title));
      var t = node('table', 'org-grid-table');
      var head = node('tr');
      heads.forEach(function (text) { cell(head, text, true); });
      var thead = node('thead');
      thead.appendChild(head);
      t.appendChild(thead);
      var tbody = node('tbody');
      t.appendChild(tbody);
      var frame = node('div', 'org-scroll');
      frame.appendChild(t);
      wrapper.appendChild(frame);
      return { wrapper: wrapper, body: tbody };
    }

    function hermesSelect(fk, current, except) {
      var select = node('select');
      S.hermes.forEach(function (h) {
        if (h.name === except) return;
        var option = node('option', '', h.name);
        option.value = h.name;
        if (h.name === current) option.selected = true;
        select.appendChild(option);
      });
      select.setAttribute('data-fk', fk);
      return select;
    }

    function drawList(box) {
      var list = node('div', 'org-list');

      // Levels
      var levels = table(say('levels'), ['#', say('name'), '', '', '']);
      S.levels.forEach(function (level, index) {
        var row = node('tr');
        cell(row, String(index + 1));
        var input = node('input');
        input.type = 'text';
        input.value = level.name;
        input.maxLength = 32;
        input.setAttribute('aria-label', say('level-name', { n: index + 1 }));
        input.setAttribute('data-fk', 'lname:' + index);
        input.addEventListener('change', function () { renameLevel(index, input.value); });
        cell(row, input);
        var up = button(say('move-up'), function () { moveLevel(index, -1); });
        up.disabled = index === 0;
        up.setAttribute('data-fk', 'lup:' + index);
        cell(row, up);
        var down = button(say('move-down'), function () { moveLevel(index, 1); });
        down.disabled = index === S.levels.length - 1;
        down.setAttribute('data-fk', 'ldown:' + index);
        cell(row, down);
        var drop = null;
        if (S.levels.length > 1 && levelIsEmpty(level.key)) {
          drop = button(say('remove-level'), function () { removeLevel(index); });
          drop.setAttribute('data-fk', 'ldrop:' + index);
        }
        cell(row, drop);
        levels.body.appendChild(row);
      });
      list.appendChild(levels.wrapper);

      // Hermes
      var members = table('Hermes', ['Hermes', say('level'), say('role'), say('numbers')]);
      S.hermes.forEach(function (h) {
        var row = node('tr');
        cell(row, h.name);
        cell(row, levelSelect(h, 'hlevel:' + h.name));
        var input = node('input');
        input.type = 'text';
        input.value = h.role;
        input.maxLength = maxRole;
        input.setAttribute('aria-label', say('role-label') + ': ' + h.name);
        input.setAttribute('data-fk', 'hrole:' + h.name);
        input.addEventListener('change', function () { setRole(h.name, input.value, input); });
        cell(row, input);
        var box2 = node('div', 'chips');
        chips(box2, { name: h.name, auto: h.auto });
        cell(row, box2.firstChild || box2);
        members.body.appendChild(row);
      });
      list.appendChild(members.wrapper);

      // Links
      var links = table(say('links'), [say('from'), say('to'), say('kind'), say('wakes'), '', '']);
      if (!S.links.length) {
        var none = node('tr');
        var c = cell(none, say('no-link'));
        c.colSpan = 6;
        links.body.appendChild(none);
      }
      S.links.forEach(function (l) {
        var row = node('tr');
        cell(row, l.from);
        cell(row, l.to);
        cell(row, say('kind-' + kindOf(l)));
        var check = node('input');
        check.type = 'checkbox';
        check.checked = l.wake;
        check.setAttribute('aria-label', say('wakes-name', { name: l.to }));
        check.setAttribute('data-fk', 'lwake:' + l.from + ':' + l.to);
        check.addEventListener('change', function () { setWake(l.from, l.to, check.checked); });
        cell(row, check);
        var back = linkOf(l.to, l.from) >= 0;
        var other = button(say(back ? 'way-back-remove' : 'way-back-create'), function () { wayBack(l.from, l.to); });
        other.setAttribute('data-fk', 'lback:' + l.from + ':' + l.to);
        cell(row, other);
        var drop = button(say('remove'), function () { removeLink(l.from, l.to); });
        drop.setAttribute('data-fk', 'lrm:' + l.from + ':' + l.to);
        cell(row, drop);
        links.body.appendChild(row);
      });
      var addRow = node('tr');
      var fromPick = hermesSelect('afrom', S.hermes.length ? S.hermes[0].name : '', null);
      fromPick.setAttribute('aria-label', say('from'));
      var toPick = hermesSelect('ato', S.hermes.length > 1 ? S.hermes[1].name : '', null);
      toPick.setAttribute('aria-label', say('link-to'));
      cell(addRow, fromPick);
      cell(addRow, toPick);
      cell(addRow, '');
      var wakeNew = node('input');
      wakeNew.type = 'checkbox';
      wakeNew.setAttribute('aria-label', say('wakes'));
      cell(addRow, wakeNew);
      var addButton = button(say('add-link'), function () {
        var from = fromPick.value, to = toPick.value;
        if (from === to) return;
        if (linkOf(from, to) >= 0) { said = say('link-exists'); render(); return; }
        var wake = wakeNew.checked;
        mutate(function () { S.links.push({ from: from, to: to, wake: wake }); });
      });
      addButton.setAttribute('data-fk', 'aadd');
      cell(addRow, addButton);
      cell(addRow, '');
      links.body.appendChild(addRow);
      list.appendChild(links.wrapper);
      box.appendChild(list);
    }

    // ------------------------------------------------------------------ drawing

    function activeKey() {
      var focused = document.activeElement;
      return focused && host.contains(focused) ? focused.getAttribute('data-fk') : null;
    }

    function focusKey(key) {
      if (!key) return;
      var target = host.querySelector('[data-fk="' + key + '"]');
      if (target) target.focus();
    }

    function render() {
      var keep = activeKey();
      var lines = diff();
      sync();
      updateToolbar(lines);
      while (body.firstChild) body.removeChild(body.firstChild);
      var status = node('p', 'org-status', said);
      status.setAttribute('role', 'status');
      body.appendChild(status);
      var unplaced = S.hermes.filter(function (h) { return h.auto; }).map(function (h) { return h.name; });
      if (unplaced.length) body.appendChild(node('p', 'org-unplaced', say('unplaced', { names: unplaced.join(', ') })));
      if (view === 'map') {
        drawMap(body);
        if (sel && hermesOf(sel)) drawDrawer(body, hermesOf(sel));
      } else {
        drawList(body);
      }
      if (focusDrawer) {
        focusDrawer = false;
        var first = body.querySelector('#org-role');
        if (first) first.focus();
      } else {
        focusKey(keep);
      }
    }

    // ------------------------------------------------------------------ starting, refreshing, leaving

    var stored = read();
    if (!stored) return;
    adopt(stored);
    S = draftOf(base);
    var echo = document.getElementById('org-draft');
    if (echo) {
      S = draftFromEcho(JSON.parse(echo.getAttribute('data-draft')));
      echoed = true;
    }
    host.hidden = false;
    render();
    root.classList.add('org-ready');

    function typing() {
      var focused = document.activeElement;
      return !!(focused && host.contains(focused) && /^(INPUT|TEXTAREA|SELECT)$/.test(focused.tagName));
    }

    // The hub's own region is swapped every few seconds by the script of the panel, unless a draft
    // holds the page still: then a change made elsewhere is caught by the check at Publish.
    function refresh() {
      if (held() || pointerBusy) return;
      if (typing()) { pending = true; return; }
      pending = false;
      var fresh = read();
      if (!fresh) return;
      adopt(fresh);
      S = draftOf(base);
      if (sel !== null && !hermesOf(sel)) sel = null;
      if (selLink !== null && linkOf(selLink.from, selLink.to) < 0) selLink = null;
      render();
    }

    if (window.MutationObserver) {
      new MutationObserver(refresh).observe(live, { childList: true });
    }
    host.addEventListener('focusout', function () {
      if (pending) setTimeout(function () { if (pending) refresh(); }, 0);
    });

    // On the whole page, not only inside the map: after a link is pulled with the mouse the focus
    // is on no control of the map, and Escape still has to close the window of the new arrow.
    document.addEventListener('keydown', function (e) {
      if (e.key !== 'Escape') return;
      if (selLink !== null) {
        var back = host.querySelector('[data-fk="card:' + selLink.from + '"]');
        selLink = null;
        render();
        back = host.querySelector('[data-fk="card:' + (back ? back.getAttribute('data-name') : '') + '"]');
        if (back) back.focus();
        return;
      }
      if (sel !== null) closeDrawer(true);
    });

    window.addEventListener('beforeunload', function (e) {
      if (held() && !submitting) {
        e.preventDefault();
        e.returnValue = '';
      }
    });
  }
})();
