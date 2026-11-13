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
    return parts.slice(0, -1).join(', ') + ' e ' + parts[parts.length - 1];
  }

  // The editor of an access: what cannot be marked is greyed out, and the sentence under the
  // form says what the form means while it is filled. The hub checks it all again on saving.
  function editor() {
    var form = document.getElementById('ed-form');
    var said = document.getElementById('ed-frase');
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
        text = 'O Hermes ' + hermes + ' só alcança as conversas escolhidas do número ' + number + ': ' +
          lines + ' hoje. O que ele pode em cada uma está na linha dela.';
      } else if (none) {
        text = 'O Hermes ' + hermes + ' não tem acesso ao número ' + number + '.';
      } else {
        var can = [], rest = [];
        if (read.checked) can.push('ler'); else rest.push('não lê');
        if (send.checked) can.push('enviar'); else rest.push('não envia');
        if (modify.checked) can.push('modificar'); else rest.push('não modifica');
        var seconds = parseInt(wait.value, 10);
        if (isNaN(seconds)) seconds = 0;
        rest.push(wake.checked ? 'é acordado depois de ' + seconds + ' s de silêncio na conversa' : 'não é acordado');
        var second = joined(rest);
        if (can.length) {
          text = 'O Hermes ' + hermes + ' pode ' + joined(can) + ' em todas as conversas do número ' + number + '. ' +
            second.charAt(0).toUpperCase() + second.slice(1) + '.';
        } else {
          text = 'O Hermes ' + hermes + ' ' + second + ' em todas as conversas do número ' + number + '.';
        }
        if (start.checked) text += ' Pode iniciar conversa com quem nunca falou com o número.';
        if (lines === 1) text += ' 1 conversa tem regra própria.';
        if (lines > 1) text += ' ' + lines + ' conversas têm regra própria.';
      }
      said.textContent = text;
    }

    form.addEventListener('change', update);
    form.addEventListener('input', update);
    update();
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

    editor();

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
