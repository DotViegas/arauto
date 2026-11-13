// The script of the panel of the Arauto hub. It only helps the screen: every step of a guide
// is a page of the hub and works without it. It never writes markup, never reads a cookie and
// never talks to anything but the clipboard of whoever clicks.
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

  function joined(parts) {
    if (parts.length <= 1) return parts.join('');
    return parts.slice(0, -1).join(', ') + ' and ' + parts[parts.length - 1];
  }

  // The editor of an access: what cannot be marked is greyed out, and the sentence under the
  // form says what the form means while it is filled. The hub checks it all again on saving.
  function editor() {
    var form = document.getElementById('editor-form');
    var said = document.getElementById('editor-sentence');
    if (!form || !said) return;
    var hermes = form.getAttribute('data-hermes');
    var number = form.getAttribute('data-number');
    var lines = parseInt(form.getAttribute('data-lines'), 10) || 0;
    function field(name) { return form.querySelector('input[name="' + name + '"]'); }
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
        text = 'The Hermes ' + hermes + ' only reaches the chosen chats of the number ' + number + ': ' +
          lines + ' today. What it may do in each one is in its line.';
      } else if (none) {
        text = 'The Hermes ' + hermes + ' has no access to the number ' + number + '.';
      } else {
        var can = [], rest = [];
        if (read.checked) can.push('read'); else rest.push('does not read');
        if (send.checked) can.push('send'); else rest.push('does not send');
        if (modify.checked) can.push('modify'); else rest.push('does not modify');
        var seconds = parseInt(wait.value, 10);
        if (isNaN(seconds)) seconds = 0;
        rest.push(wake.checked ? 'is woken after ' + seconds + ' s of silence in the chat' : 'is not woken');
        if (can.length) {
          text = 'The Hermes ' + hermes + ' may ' + joined(can) + ' in every chat of the number ' + number +
            '. It ' + joined(rest) + '.';
        } else {
          text = 'The Hermes ' + hermes + ' ' + joined(rest) + ' in every chat of the number ' + number + '.';
        }
        if (start.checked) text += ' It may start a chat with someone who never talked to the number.';
        if (lines === 1) text += ' 1 chat has its own rule.';
        if (lines > 1) text += ' ' + lines + ' chats have their own rule.';
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
        said.textContent = copied
          ? 'Copied to the clipboard.'
          : 'The text is selected. Copy it with Ctrl+C.';
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
