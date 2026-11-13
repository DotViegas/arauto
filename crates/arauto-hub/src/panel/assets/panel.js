// The script of the panel of the Arauto hub. It only helps the screen: every step of a guide
// is a page of the hub and works without it. It never writes markup, never reads a cookie and
// never talks to anything but the clipboard of whoever clicks. It has no sentence of its
// own: what it says comes from the page, in the language of the page.
(function () {
  'use strict';
  var root = document.documentElement;
  root.classList.add('js');

  // How long the mascot dances before the block shows.
  var PREPARING_MS = 3200;

  function two(n) { return (n < 10 ? '0' : '') + n; }
  function clock(seconds) { return two(Math.floor(seconds / 60)) + ':' + two(seconds % 60); }

  function ready(run) {
    if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', run);
    else run();
  }

  // The editor of an access: what cannot be marked is greyed out, and the sentence under the
  // form says what the form means while it is filled. The pieces of the sentence come from
  // the page; the hub checks it all again on saving.
  function editor() {
    var form = document.getElementById('editor-form');
    var said = document.getElementById('editor-sentence');
    if (!form || !said) return;
    function piece(name) { return form.getAttribute('data-' + name) || ''; }
    function field(name) { return form.querySelector('input[name="' + name + '"]'); }
    var lines = parseInt(piece('lines'), 10) || 0;
    var all = form.querySelector('input[name="scope"][value="all"]');
    var read = field('read'), send = field('send'), modify = field('modify');
    var wake = field('wake'), start = field('start'), wait = field('wait');

    function update() {
      wake.disabled = !read.checked;
      if (wake.disabled) wake.checked = false;
      start.disabled = !(send.checked && all.checked);
      if (start.disabled) start.checked = false;
      wait.disabled = !wake.checked;

      var text;
      var none = !(read.checked || send.checked || modify.checked || wake.checked);
      if (!all.checked) {
        text = piece('chosen');
      } else if (none) {
        text = piece('none');
      } else {
        var seconds = parseInt(wait.value, 10);
        if (isNaN(seconds)) seconds = 0;
        var parts = [
          piece(read.checked ? 'read' : 'read-not'),
          piece(send.checked ? 'send' : 'send-not'),
          piece(modify.checked ? 'modify' : 'modify-not'),
          wake.checked ? piece('woken').split('{seconds}').join(String(seconds)) : piece('woken-not')
        ];
        if (start.checked) parts.push(piece('starts'));
        if (lines > 0) parts.push(piece('own-rules'));
        text = piece('head') + ' ' + parts.join(' · ') + '.';
      }
      said.textContent = text;
    }

    form.addEventListener('change', update);
    form.addEventListener('input', update);
    update();
  }

  ready(function () {
    var guide = document.getElementById('guide');
    if (!guide) return;

    // The time left of an invite or of a request, counted down on the screen. The deadline
    // itself is the one of the hub: this is only what the admin sees.
    Array.prototype.forEach.call(guide.querySelectorAll('[data-seconds]'), function (node) {
      var left = parseInt(node.getAttribute('data-seconds'), 10);
      if (isNaN(left) || left < 0) return;
      node.textContent = clock(left);
      setInterval(function () {
        left = Math.max(0, left - 1);
        node.textContent = clock(left);
      }, 1000);
    });

    editor();

    var block = document.getElementById('guide-block');
    if (guide.classList.contains('preparing')) {
      var title = document.getElementById('guide-preparing');
      if (title) title.focus();
      setTimeout(function () {
        guide.classList.remove('preparing');
        if (block) block.focus();
      }, PREPARING_MS);
    }
    if (!block) return;

    var text = document.getElementById('guide-text');
    var said = document.getElementById('guide-copied');
    block.setAttribute('role', 'button');
    block.setAttribute('tabindex', '0');

    function done(copied) {
      block.classList.remove('copied');
      void block.offsetWidth; // the animation starts again at every click
      block.classList.add('copied');
      if (said) {
        said.textContent = guide.getAttribute(copied ? 'data-copied' : 'data-selected') || '';
        said.hidden = false;
      }
      if (!copied) {
        var range = document.createRange();
        range.selectNodeContents(text);
        var selection = window.getSelection();
        selection.removeAllRanges();
        selection.addRange(range);
      }
      guide.classList.add('was-copied');
    }

    function copy() {
      var write = navigator.clipboard && navigator.clipboard.writeText;
      if (!write) { done(false); return; }
      // A browser that does not answer in 0.8 s leaves the text selected, to copy by hand.
      var answered = false;
      setTimeout(function () { if (!answered) { answered = true; done(false); } }, 800);
      navigator.clipboard.writeText(text.textContent).then(
        function () { if (!answered) { answered = true; done(true); } },
        function () { if (!answered) { answered = true; done(false); } });
    }

    block.addEventListener('click', copy);
    block.addEventListener('keydown', function (event) {
      if (event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        copy();
      }
    });
  });
})();
