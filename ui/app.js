'use strict';
(() => {
  const send = (m) => window.ipc.postMessage(JSON.stringify(m));

  let S = null;
  const ui = {
    net: 'TRON', start: '', end: '', count: 1,
    proof: '', key: '', keyPrev: '',
    epoch: -1, viewKey: null,
    menuOpen: false, netsOpen: false,
    copied: '', copyTimer: 0,
    toast: null, toastTimer: 0,
    fading: false, showChecks: false, cancelId: null,
    addMore: false, pending: '',
    seen: {},
  };
  const T = (ru, en) => (S && !S.ru ? en : ru);
  const MAIN_NETS = ['TRON', 'ETH', 'BTC-P2PKH'];
  const ICON_OF = { TRON: 'tron', ETH: 'eth', BSC: 'bnb', POLYGON: 'pol', ARBITRUM: 'arb', OPTIMISM: 'op', BASE: 'base', 'AVAX-C': 'avax',
    LINEA: 'linea', ZKSYNC: 'zksync', HYPEREVM: 'hype', 'BTC-P2PKH': 'btc', 'BTC-P2SH': 'btc', 'BTC-P2WPKH': 'btc', 'BTC-P2TR': 'btc',
    'LTC-P2PKH': 'ltc', 'LTC-P2SH': 'ltc', 'LTC-P2WPKH': 'ltc', DOGE: 'doge', DASH: 'dash', XRP: 'xrp', COSMOS: 'atom', OSMOSIS: 'osmo',
    CELESTIA: 'tia', DYDX: 'dydx', AKASH: 'akt', JUNO: 'juno', STARGAZE: 'stars', NOBLE: 'noble' };
  const SVG = 'http://www.w3.org/2000/svg';
  const ICON = {
    copy: 'M9 9h11v11H9zM5 15H4V4h11v1',
    check: 'M5 12.5l4.5 4.5L19 7.5',
    dl: 'M12 3v12m-5-5 5 5 5-5M5 21h14',
    lock: 'M7 11V8a5 5 0 0 1 10 0v3M5 11h14v10H5z',
    eye: 'M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12zM12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z',
    eyeoff: 'M3 3l18 18M10.6 5.1A10 10 0 0 1 12 5c6.4 0 10 7 10 7a17 17 0 0 1-3.2 4.1M6.6 6.6C3.7 8.4 2 12 2 12s3.6 7 10 7a9.6 9.6 0 0 0 5.4-1.6M9.9 9.9a3 3 0 0 0 4.2 4.2',
    clip: 'M9 4h6v3H9zM8 5.5H6a1 1 0 0 0-1 1V20a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1V6.5a1 1 0 0 0-1-1h-2',
    chev: 'm6 9 6 6 6-6',
    x: 'M6 6l12 12M18 6 6 18',
    arrow: 'M5 12h14m-5-5 5 5-5 5',
    back: 'M19 12H5m6-6-6 6 6 6',
  };

  function el(tag, cls, ...kids) {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    for (const k of kids) {
      if (k === null || k === undefined || k === false) continue;
      e.append(k instanceof Node ? k : document.createTextNode(String(k)));
    }
    return e;
  }
  function btn(cls, kids, onClick, disabled) {
    const b = el('button', cls, ...(Array.isArray(kids) ? kids : [kids]));
    b.type = 'button';
    if (disabled) b.disabled = true;
    b.addEventListener('click', onClick);
    return b;
  }
  function icon(name, cls) {
    const s = document.createElementNS(SVG, 'svg');
    s.setAttribute('viewBox', '0 0 24 24');
    if (cls) s.setAttribute('class', cls);
    const p = document.createElementNS(SVG, 'path');
    p.setAttribute('d', ICON[name]);
    s.append(p);
    return s;
  }
  function okIcon() {
    const s = document.createElementNS(SVG, 'svg');
    s.setAttribute('viewBox', '0 0 24 24');
    s.setAttribute('class', 'okicon');
    const c = document.createElementNS(SVG, 'circle');
    c.setAttribute('cx', '12'); c.setAttribute('cy', '12'); c.setAttribute('r', '10.5');
    const p = document.createElementNS(SVG, 'path');
    p.setAttribute('d', 'M7.5 12.3l3 3 6-6.3');
    s.append(c, p);
    return s;
  }

  function pl(n, one, few, many) {
    const d = n % 10, h = n % 100;
    if (d === 1 && h !== 11) return one;
    if (d >= 2 && d <= 4 && (h < 12 || h > 14)) return few;
    return many;
  }
  function plural(n) {
    if (!S || !S.ru) return n === 1 ? 'address' : 'addresses';
    return pl(n, 'адрес', 'адреса', 'адресов');
  }
  function checksDone(n) {
    return T(`${pl(n, 'Пройдена', 'Пройдены', 'Пройдены')} все ${n} ${pl(n, 'проверка', 'проверки', 'проверок')} · `, `All ${n} checks passed · `);
  }
  function netOf(tag) { return S.nets.find((n) => n.tag === tag) || S.nets[0]; }
  function netByLabel(label) { return S.nets.find((n) => n.label === label) || null; }
  function netIcon(tag, size) {
    const name = ICON_OF[tag] || 'btc';
    const im = el('img', 'nic' + (name === 'linea' || name === 'zksync' ? ' light' : ''));
    im.src = 'net/' + name + '.svg';
    im.alt = '';
    im.width = size || 16;
    im.height = size || 16;
    return im;
  }
  function netShort(label) { const i = label.indexOf(' ('); return i > 0 ? label.slice(0, i) : label; }
  function wantedBody(net, s) {
    s = s.trim();
    if (s.startsWith(net.fixed)) return s.slice(net.fixed.length);
    if (net.evm && s.startsWith('0X')) return s.slice(2);
    return s;
  }
  function addrSpan(a, cls) {
    return el('span', cls, el('span', 'hl', a.head), a.mid, el('span', 'hl', a.tail));
  }
  function previewOf(net, start, end) {
    const head = net.fixed + start;
    const dots = Math.max(3, net.len - head.length - end.length);
    return [el('span', 'hl', head), el('span', 'dt', '·'.repeat(dots)), el('span', 'hl', end)];
  }
  function fitPreview(box, net, start, end) {
    const chars = Math.max(net.len, net.fixed.length + start.length + end.length + 3);
    box.style.fontSize = Math.max(14, Math.min(34, Math.floor(610 / (chars * 0.6)))) + 'px';
  }
  function kicker() {
    const o = S.order;
    if (!o) return null;
    return el('div', 'kicker', `#${o.id} · ${netShort(o.net)} · ${o.count} ${plural(o.count)}`);
  }
  function flash(what) {
    ui.copied = what;
    clearTimeout(ui.copyTimer);
    ui.copyTimer = setTimeout(() => { ui.copied = ''; render(); }, what === 'msg' ? 2000 : 1500);
  }
  function toast(text, bad) {
    ui.toast = { text, bad: !!bad, id: Math.random() };
    clearTimeout(ui.toastTimer);
    ui.toastTimer = setTimeout(() => { ui.toast = null; renderLayer(); }, 1900);
    renderLayer();
  }
  function checksList(checks) {
    const box = el('div', 'checks');
    for (const c of checks) {
      box.append(el('div', 'check ' + c.mark, el('span', 'm', c.mark === 'ok' ? '✓' : c.mark === 'warn' ? '!' : '✖'), el('span', 't', c.text)));
    }
    return box;
  }
  function setFoot(kind, text) {
    const foot = document.getElementById('foot');
    if (!text) { foot.replaceChildren(); return; }
    foot.replaceChildren(icon('lock', kind === 'bad' ? 'lk-bad' : 'lk-warn'), el('span', null, text));
  }

  const stepEls = [];
  function renderSteps() {
    const box = document.getElementById('stepper');
    const titles = [T('Заявка', 'Request'), T('Проверка', 'Verify'), T('Кошельки', 'Wallets')];
    if (!stepEls.length) {
      for (let i = 0; i < 3; i++) {
        if (i) { const line = el('span', 'st-line', el('i')); box.append(line); stepEls.push({ line }); }
        const b = el('button', 'st', el('span', 'c'), el('span', 't'));
        b.type = 'button';
        b.addEventListener('click', () => { closePops(); send({ a: 'step', i }); });
        box.append(b);
        stepEls.push({ b });
      }
    }
    const btns = stepEls.filter((s) => s.b);
    const lines = stepEls.filter((s) => s.line);
    btns.forEach((s, i) => {
      const avail = i === 0 || S.orders.length > 0;
      const cur = S.step === i;
      const done = !!S.done[i];
      s.b.disabled = !avail || S.busy;
      s.b.className = 'st' + (avail ? ' avail' : '') + (cur ? ' cur' : '') + (done ? ' done' : '');
      s.b.querySelector('.c').textContent = done && !cur ? '✓' : String(i + 1);
      s.b.querySelector('.t').textContent = titles[i];
    });
    lines.forEach((s, i) => s.line.classList.toggle('on', !!S.done[i]));
  }

  function renderTitle() {
    const b = document.getElementById('menuBtn');
    document.getElementById('menuLabel').textContent = S.order ? '#' + S.order.id : T('Заявки', 'Requests');
    b.title = S.order ? S.order.path : '';
    b.classList.toggle('open', ui.menuOpen);
    const lang = document.getElementById('lang');
    if (!ui.fading) lang.classList.toggle('en', !S.ru);
    for (const x of lang.querySelectorAll('button')) x.classList.toggle('on', (x.dataset.ru === '1') === !lang.classList.contains('en'));
  }

  function charProblem(net, start, end) {
    const bad = [];
    for (const c of start + end) if (!net.alpha.includes(c) && !bad.includes(c)) bad.push(c);
    if (bad.length) return T('Эти символы запрещены: ', 'These characters are forbidden: ') + bad.map((c) => (c === ' ' ? T('пробел', 'space') : c)).join(' ');
    if (start.length > 20 || end.length > 20) return T('Не больше 20 символов в начале и в конце адреса.', 'At most 20 characters at the start and at the end.');
    return '';
  }

  function pickNet(tag) {
    if (ui.net !== tag) { ui.start = ''; ui.end = ''; if (S.create_msg) S.create_msg = null; }
    ui.net = tag;
    ui.netsOpen = false;
    render();
  }

  let liveCreate = null;
  function newRequest(main) {
    const scr = el('div', 'screen');
    const net = netOf(ui.net);
    scr.append(el('div', 'hint', T('Выберите сеть и придумайте начало или конец адреса', 'Choose a network and make up the start or the end of the address')));
    const preview = el('div', 'preview');
    scr.append(preview);

    const chips = el('div', 'chips');
    for (const tag of MAIN_NETS) {
      const n = netOf(tag);
      chips.append(btn('chip' + (ui.net === tag ? ' on' : ''), [netIcon(tag), netShort(n.label).replace('Bitcoin Legacy', 'Bitcoin')], () => pickNet(tag)));
    }
    const other = !MAIN_NETS.includes(ui.net);
    const more = btn('chip' + (other ? ' on' : '') + (ui.netsOpen ? ' open' : ''),
      [other ? netIcon(net.tag) : null, other ? netShort(net.label) : T('Другие', 'Others'), icon('chev')].filter(Boolean), (e) => { e.stopPropagation(); ui.netsOpen = !ui.netsOpen; render(); });
    chips.append(more);
    if (ui.netsOpen) {
      const pop = el('div', 'pop netpop');
      pop.addEventListener('click', (e) => e.stopPropagation());
      for (const n of S.nets.filter((x) => !MAIN_NETS.includes(x.tag))) {
        pop.append(btn('netrow' + (n.tag === ui.net ? ' on' : ''), [netIcon(n.tag, 18), el('span', 'nl', n.label), el('span', 'np', n.fixed + '…')],
          () => pickNet(n.tag)));
      }
      chips.append(pop);
    }
    scr.append(chips);

    const fields = el('div', 'fields');
    const start = el('input');
    start.dataset.key = 'start';
    start.value = ui.start;
    start.spellcheck = false;
    start.placeholder = T('Pet', 'Pet');
    const end = el('input');
    end.dataset.key = 'end';
    end.value = ui.end;
    end.spellcheck = false;
    end.placeholder = '777';
    const num = el('input');
    num.dataset.key = 'count';
    num.value = ui.count;
    num.inputMode = 'numeric';
    const status = el('div', 'status');
    const create = btn('primary', S.saving_request ? T('Сохраняю файл…', 'Saving the file…') : T('Создать заявку', 'Create request'),
      () => send({ a: 'create', net: ui.net, start: ui.start, end: ui.end, count: ui.count }));

    const update = () => {
      const body = wantedBody(net, ui.start);
      const tail = ui.end.trim();
      preview.replaceChildren(...previewOf(net, body, tail));
      fitPreview(preview, net, body, tail);
      const problem = charProblem(net, body, tail);
      const serverErr = S.create_msg && !S.create_msg.ok ? S.create_msg.text : '';
      status.className = 'status' + (problem || serverErr ? ' tall' : '');
      if (problem || serverErr) status.replaceChildren(el('span', 'err', problem || serverErr));
      else status.replaceChildren();
      create.disabled = S.busy || !!problem || (!body && !tail);
    };
    liveCreate = update;
    start.addEventListener('input', () => { ui.start = start.value; if (S.create_msg) S.create_msg = null; update(); });
    end.addEventListener('input', () => { ui.end = end.value; if (S.create_msg) S.create_msg = null; update(); });
    const setCount = (v) => { ui.count = Math.max(1, Math.min(100, v | 0 || 1)); num.value = ui.count; update(); };
    num.addEventListener('input', () => { const v = parseInt(num.value, 10); if (v >= 1) setCount(v); });
    num.addEventListener('blur', () => setCount(parseInt(num.value, 10)));
    for (const inp of [start, end, num]) inp.addEventListener('keydown', (e) => { if (e.key === 'Enter' && !create.disabled) create.click(); });

    fields.append(
      el('label', 'fld', el('span', 'lbl', T('Начало', 'Start')), el('div', 'box', el('span', 'pre', net.fixed), start)),
      el('label', 'fld', el('span', 'lbl', T('Конец · необязательно', 'End · optional')), el('div', 'box', end)),
      el('div', 'fld', el('span', 'lbl', T('Кол-во', 'Count')),
        el('div', 'box qty', btn('', '−', () => setCount(ui.count - 1)), num, btn('plus', '+', () => setCount(ui.count + 1)))));
    scr.append(fields, status, el('div', 'gap18', create));
    if (S.orders.length && S.new_mode) scr.append(el('div', 'gap18', btn('tbtn', T('Отмена', 'Cancel'), () => send({ a: 'cancel_new' }), S.busy)));
    main.append(scr);
    update();
    setFoot('warn', T('Секретная половина ключа создаётся на вашем компьютере и не покидает его', 'The secret key half is created on your computer and never leaves it'));
  }

  function requestDone(main) {
    const o = S.order;
    const scr = el('div', 'screen');
    const net = netByLabel(o.net);
    scr.append(kicker());
    if (net) {
      const start = o.start === '—' ? '' : wantedBody(net, o.start);
      const end = o.end === '—' ? '' : o.end;
      const p = el('div', 'preview', ...previewOf(net, start, end));
      p.style.margin = '16px 0 12px';
      fitPreview(p, net, start, end);
      scr.append(p);
    }
    scr.append(el('div', 'sub', T('Скопируйте сообщение и вставьте его в заказ на сайте', 'Copy the message and paste it into the order on the site')));
    const good = ui.copied === 'msg';
    scr.append(el('div', 'gap28', btn('primary' + (good ? ' good' : ''), [icon(good ? 'check' : 'copy'), good ? T('Скопировано', 'Copied') : T('Копировать сообщение', 'Copy message')],
      () => { send({ a: 'copy_message' }); flash('msg'); })));
    const code = o.code || '';
    const shortCode = code.length > 40 ? code.slice(0, 18) + '…' + code.slice(-8) : code;
    scr.append(el('div', 'codeline', el('span', null, shortCode),
      el('span', 'qmark', '?', el('span', 'tip', T('Здесь только публичные части ваших половин ключа — по одной на каждый адрес. По ним можно искать адрес, но получить ключ нельзя.',
        'It holds only the public parts of your key halves — one per address. They are enough to search for the address, but not to get the key.')))));
    scr.append(el('div', 'nextwrap',
      btn('nextbtn' + (ui.copied === 'msg' ? ' glow' : ''), [T('Перейти к проверке адресов', 'Go to the address check'), icon('arrow')], () => send({ a: 'step', i: 1 }))));
    main.append(scr);
    setFoot('warn', T('Не удаляйте файл заявки до получения кошельков', 'Do not delete the request file until you receive the wallets'));
  }

  function noOrders(main) {
    const scr = el('div', 'screen');
    scr.append(el('div', 'h', T('Сначала создайте заявку', 'Create a request first')),
      el('div', 'sub', T('Или откройте файл заявки через меню слева вверху.', 'Or open a request file from the menu at the top left.')),
      el('div', 'gap18', btn('tbtn', [icon('back'), T('К заявке', 'To the request')], () => send({ a: 'step', i: 0 }))));
    main.append(scr);
    setFoot('', '');
  }

  function codesInput(scr, key, kind, expected) {
    const wrap = el('div', 'codes');
    const ta = el('textarea');
    ta.dataset.key = key;
    ta.value = ui[key];
    ta.spellcheck = false;
    ta.placeholder = kind + '…\n' + kind + '…';
    const counter = el('div', 'counter');
    const count = () => {
      const n = (ui[key].match(new RegExp(kind + '\\d:', 'gi')) || []).length;
      const dots = Math.min(Math.max(expected, 1), 10);
      const filled = expected > 10 ? Math.round(Math.min(n, expected) / expected * 10) : Math.min(n, dots);
      counter.className = 'counter' + (expected && n === expected ? ' full' : '');
      const kids = [];
      for (let i = 0; i < dots; i++) kids.push(el('i', i < filled ? 'f' : null));
      kids.push(el('span', null, expected ? `${n} / ${expected}` : String(n)));
      counter.replaceChildren(...kids);
    };
    ta.addEventListener('input', () => { ui[key] = ta.value; count(); });
    const paste = btn('paste', [icon('clip'), T('Вставить', 'Paste')], () => send({ a: 'paste', target: key }));
    wrap.append(ta, el('div', 'bar', paste, counter));
    scr.append(wrap);
    count();
    return ta;
  }

  function failLines(outcome) {
    if (!outcome || outcome.passed) return [el('div', 'errline')];
    const fails = outcome.checks.filter((c) => c.mark === 'fail');
    const out = [el('div', 'errline', outcome.summary)];
    if (fails.length) { const l = checksList(fails); l.classList.add('faillist'); out.push(l); }
    return out;
  }

  function verifyPage(main) {
    if (!S.orders.length) return noOrders(main);
    const v = S.verify;
    const scr = el('div', 'screen');
    if (v && v.passed && v.found.length) {
      const n = v.found.length;
      scr.append(kicker(), el('div', 'gap18', okIcon()),
        el('div', 'h', n === 1 ? T('Адрес настоящий', 'The address is genuine') : T(`Все ${n} ${plural(n)} настоящие`, `All ${n} addresses are genuine`)),
        el('div', 'sub', T('Сверьте их с заказом на сайте и оплачивайте', 'Compare them with the order on the site and pay')));
      const list = el('div', 'list');
      v.found.forEach((a, i) => {
        const done = ui.copied === 'v' + i;
        const row = el('div', 'arow' + (ui.enter ? ' in' : ''), el('span', 'n', String(i + 1).padStart(2, '0')), addrSpan(a, 'a'), icon(done ? 'check' : 'copy', 'ic' + (done ? ' done' : '')));
        row.style.animationDelay = (0.25 + Math.min(i, 12) * 0.07) + 's';
        row.addEventListener('click', () => { send({ a: 'copy_found', i }); flash('v' + i); toast(T('Адрес скопирован', 'Address copied')); render(); });
        list.append(row);
      });
      scr.append(list);
      const more = el('div', 'more', checksDone(v.checks.length));
      const lnk = el('span', 'lnk', ui.showChecks ? T('скрыть', 'hide') : T('подробнее', 'details'));
      lnk.addEventListener('click', () => { ui.showChecks = !ui.showChecks; render(); });
      more.append(lnk);
      scr.append(more);
      if (ui.showChecks) scr.append(checksList(v.checks));
      scr.append(el('div', 'gap24', btn('primary', [T('Я оплатил', 'I have paid'), icon('arrow')], () => send({ a: 'step', i: 2 }))));
      main.append(scr);
      setFoot('warn', T('Деньги на сами адреса не отправляйте до сборки кошельков', 'Do not send money to the addresses before the wallets are assembled'));
      return;
    }
    scr.append(kicker(), el('div', 'h', T('Проверка до оплаты', 'Check before paying')),
      el('div', 'sub', T('Вставьте коды VGPROOF от продавца — в любом виде', 'Paste the VGPROOF codes from the seller — in any form')));
    const ta = codesInput(scr, 'proof', 'VGPROOF', S.order ? S.order.count : 0);
    const go = () => { if (!ui.proof.trim()) return; ui.pending = 'verify'; send({ a: 'verify', text: ui.proof }); render(); };
    ta.addEventListener('keydown', (e) => { if (e.key === 'Enter' && e.ctrlKey) { e.preventDefault(); go(); } });
    scr.append(...failLines(v));
    const busy = ui.pending === 'verify';
    scr.append(el('div', 'gap18', btn('primary', busy ? [el('span', 'spin'), T('Проверяю…', 'Checking…')] : T('Проверить', 'Check'), go, busy)));
    main.append(scr);
    setFoot('warn', T('Программа сама вычисляет каждый адрес из вашей половины ключа', 'The program computes every address from your key half itself'));
  }

  function walletPage(main) {
    if (!S.orders.length && !S.wallets.length) return noOrders(main);
    const ws = S.wallets;
    const a = S.assemble;
    const scr = el('div', 'screen wide');
    const total = S.order ? S.order.count : ws.length;
    const missing = Math.max(0, total - ws.length);
    if (!ws.length || ui.addMore) {
      scr.append(kicker(), el('div', 'h', ui.addMore ? T(`Недостающие ключи: ${missing}`, `Missing keys: ${missing}`) : T('Сборка кошельков', 'Assembling the wallets')),
        el('div', 'sub', T('Вставьте коды VGKEY, которые продавец прислал после оплаты', 'Paste the VGKEY codes the seller sent after payment')));
      const ta = codesInput(scr, 'key', 'VGKEY', ui.addMore ? missing : total);
      const verified = !!S.done[1];
      const go = () => {
        if (!ui.key.trim() || !verified) return;
        const text = ui.addMore ? ui.keyPrev + '\n' + ui.key : ui.key;
        ui.keyPrev = text;
        ui.pending = 'assemble';
        send({ a: 'assemble', text });
        render();
      };
      ta.addEventListener('keydown', (e) => { if (e.key === 'Enter' && e.ctrlKey) { e.preventDefault(); go(); } });
      if (!verified) scr.append(el('div', 'errline', T('Сначала проверьте адреса на шаге 2', 'Check the addresses in step 2 first')));
      else scr.append(...failLines(ws.length ? null : a));
      const busy = ui.pending === 'assemble';
      scr.append(el('div', 'gap18', btn('primary', busy ? [el('span', 'spin'), T('Собираю…', 'Assembling…')] : T('Собрать кошельки', 'Assemble the wallets'), go, busy || !verified)));
      if (ui.addMore) scr.append(el('div', 'gap18', btn('tbtn', [icon('back'), T('Назад к кошелькам', 'Back to the wallets')], () => { ui.addMore = false; render(); })));
      main.append(scr);
      setFoot('bad', T('Ключ даёт полный доступ к кошельку. Не отправляйте его никому, даже продавцу.', 'A key gives full access to the wallet. Never send it to anyone, not even the seller.'));
      return;
    }
    const n = ws.length;
    const h = el('div', 'h', n === 1 ? T('Кошелёк готов', 'The wallet is ready') : T(`Кошельки готовы: ${n}`, `The wallets are ready: ${n}`));
    h.style.margin = '14px 0 22px';
    scr.append(kicker(), h);
    if (missing > 0 && a && !a.passed) {
      scr.append(el('div', null, el('span', 'capsule', T(`Не хватает ключей: ${missing}`, `Keys missing: ${missing}`),
        btn('', T('Добавить', 'Add'), () => { ui.addMore = true; ui.key = ''; render(); }))));
    }
    const list = el('div', 'list');
    ws.forEach((w, i) => {
      const ka = ui.copied === 'a' + i, kk = ui.copied === 'k' + i;
      const key = el('div', 'k' + (S.show_keys ? ' shown' : ''), S.show_keys && w.key ? w.key : '•'.repeat(64));
      const row = el('div', 'wrow' + (ui.enter ? ' in' : ''), addrSpan(w, 'a'), key, el('div', 'bt',
        btn('mini' + (ka ? ' done' : ''), [icon(ka ? 'check' : 'copy'), T('адрес', 'address')], () => { send({ a: 'copy_address', i }); flash('a' + i); render(); }),
        btn('mini' + (kk ? ' done' : ''), [icon(kk ? 'check' : 'copy'), T('ключ', 'key')], () => { send({ a: 'copy_key', i }); flash('k' + i); render(); })));
      row.style.animationDelay = (0.2 + Math.min(i, 12) * 0.05) + 's';
      list.append(row);
    });
    scr.append(list);
    if (a) {
      const fails = a.checks.filter((c) => c.mark === 'fail').length;
      const more = el('div', 'more', fails ? T(`Проверок: ${a.checks.length}, с ошибками: ${fails} · `, `Checks: ${a.checks.length}, failed: ${fails} · `)
        : checksDone(a.checks.length));
      const lnk = el('span', 'lnk', ui.showChecks ? T('скрыть', 'hide') : T('подробнее', 'details'));
      lnk.addEventListener('click', () => { ui.showChecks = !ui.showChecks; render(); });
      more.append(lnk);
      scr.append(more);
      if (ui.showChecks) {
        scr.append(checksList(a.checks));
        const how = el('div', 'checks');
        ws.forEach((w, i) => how.append(el('div', 'how', `${String(i + 1).padStart(2, '0')}  ${w.a} + ${w.b} = ${w.k}`)));
        how.prepend(el('div', 'check', el('span', 'm', '·'), el('span', 't', T('Как получился ключ: ваша половина + половина продавца = ключ кошелька', 'How the key was made: your half + the seller\'s half = the wallet key'))));
        scr.append(how);
      }
    }
    if (S.wallet_msg && !S.wallet_msg.ok) scr.append(el('div', 'errline', S.wallet_msg.text));
    const saved = !!S.wallets_saved;
    scr.append(el('div', 'btnrow',
      btn('secondary', [icon(S.show_keys ? 'eyeoff' : 'eye'), S.show_keys ? T('Скрыть ключи', 'Hide keys') : T('Показать ключи', 'Show keys')], () => send({ a: 'toggle_keys' })),
      btn('primary' + (saved ? ' good' : ''), [icon(saved ? 'check' : 'dl'), saved ? T('Сохранено', 'Saved') : T('Сохранить в файл…', 'Save to a file…')], () => send({ a: 'save_wallets' }), S.busy)));
    main.append(scr);
    setFoot('bad', T('Ключ даёт полный доступ к кошельку. Не отправляйте его никому, даже продавцу.', 'A key gives full access to the wallet. Never send it to anyone, not even the seller.'));
  }

  function closePops() { ui.menuOpen = false; ui.netsOpen = false; }

  function orderStatus(o, i) {
    if (o.assembled) {
      if (i === S.selected && S.wallets.length && !S.wallets_saved) return ['#f0a8a8', T('не сохранено', 'not saved')];
      return ['#6ee0a0', T('готово', 'done')];
    }
    if (o.verified) return ['#e3c993', T('ждёт ключей', 'awaits keys')];
    return ['#5d5f63', T('ждёт проверки', 'awaits check')];
  }

  function menu() {
    const pop = el('div', 'pop menu');
    pop.addEventListener('click', (e) => e.stopPropagation());
    if (!S.orders.length && !S.missing.length) pop.append(el('div', 'mempty', T('Пока нет заявок', 'No requests yet')));
    S.orders.forEach((o, i) => {
      const [color, status] = orderStatus(o, i);
      const dot = el('span', 'dot');
      dot.style.background = color;
      const x = btn('x', icon('x'), (e) => { e.stopPropagation(); ui.cancelId = o.id; ui.menuOpen = false; render(); }, S.busy);
      x.title = T('Отменить заявку', 'Cancel the request');
      const nb = netByLabel(o.net);
      const row = el('div', 'orow' + (i === S.selected && !S.new_mode ? ' on' : ''), dot, nb ? netIcon(nb.tag, 14) : null, el('span', 'id', '#' + o.id),
        el('span', 'nt', netShort(o.net) + (o.count > 1 ? ' ×' + o.count : '')), el('span', 'ss', status), x);
      row.addEventListener('click', () => { ui.menuOpen = false; send({ a: 'select', i }); render(); });
      pop.append(row);
    });
    S.missing.forEach((m, i) => {
      const dot = el('span', 'dot');
      dot.style.background = '#e3c993';
      const row = el('div', 'orow lost', dot, el('span', 'nt', m.name), el('span', 'ss', m.exists ? T('Файл повреждён', 'File damaged') : T('Файл не найден', 'File not found')),
        el('div', 'acts', btn('', T('Найти…', 'Find…'), () => { ui.menuOpen = false; send({ a: 'locate', i }); }, S.busy),
          btn('', T('Убрать', 'Remove'), () => send({ a: 'forget_missing', i }), S.busy)));
      row.title = m.path;
      pop.append(row);
    });
    pop.append(el('div', 'msep'),
      btn('mitem acc', T('+ Новая заявка', '+ New request'), () => { ui.menuOpen = false; send({ a: 'new' }); render(); }, S.busy),
      btn('mitem', T('Открыть файл заявки…', 'Open request file…'), () => { ui.menuOpen = false; send({ a: 'open_file' }); render(); }, S.busy));
    if (S.wallets.length) {
      pop.append(btn('mitem', T('Убрать ключи с экрана', 'Remove the keys from the screen'), () => { ui.menuOpen = false; send({ a: 'close_wallets' }); render(); }));
    }
    return pop;
  }

  function cancelModal() {
    const o = S.orders.find((x) => x.id === ui.cancelId);
    if (!o) { ui.cancelId = null; return null; }
    const text = o.assembled ? T('Кошельки уже собраны — убедитесь, что ключи сохранены.', 'The wallets are already assembled — make sure the keys are saved.')
      : o.verified ? T('Если вы уже оплатили, не отменяйте — без файла заявки кошельки не собрать.', 'If you have already paid, do not cancel — the wallets cannot be assembled without the request file.')
        : T('Файл заявки будет удалён навсегда.', 'The request file will be deleted for good.');
    return el('div', 'modal', el('div', 'dialog',
      el('div', 'dh', T(`Отменить заявку #${o.id}?`, `Cancel request #${o.id}?`)),
      el('div', 'dt', text),
      el('div', 'col',
        btn('dbad', T('Отменить заявку', 'Cancel the request'), () => { const id = ui.cancelId; ui.cancelId = null; send({ a: 'cancel_order', id }); render(); }, S.busy),
        btn('dplain', T('Отмена', 'Back'), () => { ui.cancelId = null; render(); }))));
  }

  function closingModal() {
    return el('div', 'modal', el('div', 'dialog',
      el('div', 'dh', T('Ключи не сохранены', 'The keys are not saved')),
      el('div', 'dt', T('После закрытия их можно будет собрать только заново — по кодам VGKEY и файлу заявки.', 'After closing they can only be assembled again — from the VGKEY codes and the request file.')),
      el('div', 'col',
        btn('primary', [icon('dl'), T('Сохранить в файл…', 'Save to a file…')], () => send({ a: 'save_wallets' }), S.busy),
        btn('dline', T('Закрыть без сохранения', 'Close without saving'), () => send({ a: 'close_confirm' })),
        btn('dplain', T('Отмена', 'Back'), () => send({ a: 'close_cancel' })))));
  }

  function renderLayer() {
    const layer = document.getElementById('layer');
    const kids = [];
    if (S && ui.menuOpen) {
      const shade = el('div', 'shade');
      shade.addEventListener('click', () => { ui.menuOpen = false; render(); });
      kids.push(shade, menu());
    }
    if (S && ui.cancelId) { const m = cancelModal(); if (m) kids.push(m); }
    if (S && S.closing) kids.push(closingModal());
    if (ui.toast && (!ui.toastEl || ui.toastEl.dataset.id !== String(ui.toast.id))) {
      const t = el('div', 'toast', ui.toast.bad ? icon('x') : icon('check'), el('span', null, ui.toast.text));
      if (ui.toast.bad) t.firstChild.style.stroke = '#f0a8a8';
      t.dataset.id = String(ui.toast.id);
      ui.toastEl = t;
    }
    if (!ui.toast) ui.toastEl = null;
    for (const c of [...layer.children]) if (c !== ui.toastEl) c.remove();
    if (ui.toastEl && ui.toastEl.parentNode !== layer) layer.append(ui.toastEl);
    for (const k of kids) layer.insertBefore(k, ui.toastEl && ui.toastEl.parentNode === layer ? ui.toastEl : null);
  }

  function render() {
    if (!S) return;
    document.documentElement.lang = S.ru ? 'ru' : 'en';
    if (S.form_epoch !== ui.epoch) {
      if (ui.epoch !== -1) { ui.start = ''; ui.end = ''; ui.count = 1; }
      ui.epoch = S.form_epoch;
    }
    const act = document.activeElement;
    const focusKey = act && act.dataset ? act.dataset.key : null;
    const selStart = act && 'selectionStart' in act ? act.selectionStart : null;
    const selEnd = act && 'selectionEnd' in act ? act.selectionEnd : null;
    const main = document.getElementById('main');
    const scroll = main.scrollTop;
    const key = [S.step, S.new_mode, S.selected, S.order && S.order.id, S.wallets.length > 0,
      S.verify && S.verify.passed, ui.addMore].join('|');
    ui.enter = key !== ui.viewKey;

    renderTitle();
    renderSteps();
    liveCreate = null;
    main.replaceChildren();
    if (S.step === 0) (S.order ? requestDone : newRequest)(main);
    else if (S.step === 1) verifyPage(main);
    else walletPage(main);
    renderLayer();
    main.scrollTop = scroll;

    if (ui.enter) {
      ui.viewKey = key;
      const scr = main.firstElementChild;
      if (scr) scr.classList.add('in');
    }
    if (focusKey) {
      const f = document.querySelector(`[data-key="${focusKey}"]`);
      if (f) {
        f.focus();
        try { if (selStart !== null) f.setSelectionRange(selStart, selEnd); } catch (_) { }
      }
    }
  }

  function noticeMessages(state) {
    for (const k of ['copy_msg', 'open_msg', 'wallet_msg']) {
      const m = state[k];
      const sig = m ? (m.ok ? '1' : '0') + m.text : '';
      if (sig && sig !== ui.seen[k]) {
        if (m.ok) toast(m.text);
        else if (k === 'open_msg') toast(m.text, true);
      }
      ui.seen[k] = sig;
    }
  }

  window.vb = {
    render(state) {
      const prev = S;
      const stepChanged = prev && prev.step !== state.step;
      const langChanged = prev && prev.ru !== state.ru;
      if (prev && (prev.verify !== state.verify || prev.assemble !== state.assemble)) ui.pending = '';
      if (state.wallets.length && (!prev || prev.wallets.length !== state.wallets.length)) { ui.key = ''; ui.addMore = false; }
      if (!state.wallets.length) ui.addMore = false;
      if (!prev || prev.selected !== state.selected || prev.new_mode !== state.new_mode) { ui.proof = ''; ui.key = ''; ui.keyPrev = ''; ui.showChecks = false; }
      S = state;
      if (prev) noticeMessages(state); else for (const k of ['copy_msg', 'open_msg', 'wallet_msg']) ui.seen[k] = state[k] ? (state[k].ok ? '1' : '0') + state[k].text : '';
      document.body.classList.toggle('max', !!state.maximized);
      render();
      if (stepChanged) { document.getElementById('main').scrollTop = 0; ui.showChecks = false; }
      if (langChanged || ui.fading) {
        ui.fading = false;
        requestAnimationFrame(() => { for (const e of document.querySelectorAll('.fadable')) e.classList.remove('fading'); renderTitle(); });
      }
    },
    maximized(max) { document.body.classList.toggle('max', max); },
    pasted(target, text) {
      if (target !== 'proof' && target !== 'key') return;
      ui[target] = String(text || '');
      render();
      const f = document.querySelector(`[data-key="${target}"]`);
      if (f) f.focus();
    },
  };

  document.getElementById('titlebar').addEventListener('mousedown', (e) => {
    if (e.button !== 0 || e.target.closest('button')) return;
    if (e.detail === 2) send({ a: 'maximize' });
    else send({ a: 'drag' });
  });
  document.getElementById('menuBtn').addEventListener('click', (e) => {
    e.stopPropagation();
    ui.menuOpen = !ui.menuOpen;
    ui.netsOpen = false;
    render();
  });
  document.getElementById('w-min').addEventListener('click', () => send({ a: 'minimize' }));
  document.getElementById('w-max').addEventListener('click', () => send({ a: 'maximize' }));
  document.getElementById('w-close').addEventListener('click', () => send({ a: 'close' }));
  for (const b of document.querySelectorAll('#lang button')) {
    b.addEventListener('click', () => {
      const ru = b.dataset.ru === '1';
      if (!S || S.ru === ru || ui.fading) return;
      ui.fading = true;
      document.getElementById('lang').classList.toggle('en', !ru);
      for (const x of document.querySelectorAll('#lang button')) x.classList.toggle('on', x === b);
      for (const e of document.querySelectorAll('.fadable')) e.classList.add('fading');
      setTimeout(() => send({ a: 'lang', ru }), 200);
    });
  }
  document.addEventListener('click', () => {
    if (ui.netsOpen) { ui.netsOpen = false; render(); }
  });
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape' && (ui.menuOpen || ui.netsOpen || ui.cancelId)) {
      ui.menuOpen = false; ui.netsOpen = false; ui.cancelId = null; render();
    }
  });
  for (const e of document.querySelectorAll('.edge')) {
    e.addEventListener('mousedown', (ev) => { if (ev.button === 0) { ev.preventDefault(); send({ a: 'resize', dir: e.dataset.dir }); } });
  }
  document.addEventListener('contextmenu', (e) => {
    if (!e.target.closest('input, textarea, .a, .k.shown, .check')) e.preventDefault();
  });
  document.addEventListener('keydown', (e) => {
    const k = e.key.toLowerCase();
    if (e.key === 'F5' || e.key === 'F3' || e.key === 'F7' || (e.ctrlKey && ['r', 'p', 'f', 'g', 'j', 'u', 's', '+', '-', '=', '0'].includes(k))) e.preventDefault();
  });
  document.addEventListener('dragover', (e) => e.preventDefault());
  document.addEventListener('drop', (e) => e.preventDefault());

  send({ a: 'ready' });
})();
