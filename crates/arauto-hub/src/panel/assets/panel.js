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

  ready(function () {
    var guide = document.getElementById('guia');
    if (!guide) return;

    // The time left of an invite or of a request, counted down on the screen. The deadline
    // itself is the one of the hub: this is only what the admin sees.
    Array.prototype.forEach.call(guide.querySelectorAll('[data-segundos]'), function (node) {
      var left = parseInt(node.getAttribute('data-segundos'), 10);
      if (isNaN(left) || left < 0) return;
      node.textContent = clock(left);
      setInterval(function () {
        left = Math.max(0, left - 1);
        node.textContent = clock(left);
      }, 1000);
    });

    var block = document.getElementById('guia-bloco');
    if (guide.classList.contains('prepara')) {
      var title = document.getElementById('guia-preparo');
      if (title) title.focus();
      setTimeout(function () {
        guide.classList.remove('prepara');
        if (block) block.focus();
      }, PREPARING_MS);
    }
    if (!block) return;

    var text = document.getElementById('guia-texto');
    var said = document.getElementById('guia-copiado');
    block.setAttribute('role', 'button');
    block.setAttribute('tabindex', '0');

    function done(copied) {
      block.classList.remove('copiou');
      void block.offsetWidth; // the animation starts again at every click
      block.classList.add('copiou');
      if (said) {
        said.textContent = copied
          ? 'Copiado para a área de transferência.'
          : 'O texto está selecionado. Copie com Ctrl+C.';
        said.hidden = false;
      }
      if (!copied) {
        var range = document.createRange();
        range.selectNodeContents(text);
        var selection = window.getSelection();
        selection.removeAllRanges();
        selection.addRange(range);
      }
      guide.classList.add('ja-copiou');
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
