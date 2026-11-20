'use strict';
(() => {
  const send = (m) => window.ipc.postMessage(JSON.stringify(m));

  let S = null;
  const ui = {
    net: 'TRON', start: '', end: '', count: 1,
    proof: '', key: '',
    epoch: -1,
    copiedFound: -1,
    showAssembleChecks: false,
    viewKey: null,
    confirmCancel: null,
  };
  const T = (ru, en) => (S && !S.ru ? en : ru);

  function el(tag, cls, ...kids) {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    for (const k of kids) {
      if (k === null || k === undefined || k === false) continue;
      e.append(k instanceof Node ? k : document.createTextNode(String(k)));
    }
    return e;
  }
  function btn(cls, text, onClick, disabled) {
    const b = el('button', cls, text);
    b.type = 'button';
    if (disabled) b.disabled = true;
    b.addEventListener('click', onClick);
    return b;
  }
  function kicker(text) { return el('div', 'kicker', text); }
  function message(m, extra) {
    if (!m) return null;
    return m.ok ? el('div', 'okmsg' + (extra ? ' ' + extra : ''), m.text) : el('div', 'err' + (extra ? ' ' + extra : ''), m.text);
  }
  function addr(a, midClass) {
    return el('span', null, el('span', 'hl', a.head), el('span', midClass || null, a.mid), el('span', 'hl', a.tail));
  }

  function checkIcon() {
    const ns = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(ns, 'svg');
    svg.setAttribute('viewBox', '0 0 24 24');
    svg.setAttribute('class', 'chk');
    const c = document.createElementNS(ns, 'circle');
    c.setAttribute('cx', '12'); c.setAttribute('cy', '12'); c.setAttribute('r', '10.5');
    const p = document.createElementNS(ns, 'path');
    p.setAttribute('d', 'M7 12.4l3.3 3.3L17.2 8.6');
    svg.append(c, p);
    return svg;
  }

  const stepEls = [];
  function renderSteps() {
    const nav = document.getElementById('steps');
    const titles = [T('Заявка', 'Request'), T('Проверка адресов', 'Verify addresses'), T('Кошельки', 'Wallets')];
    const first = !stepEls.length;
    for (let i = 0; i < 3; i++) {
      if (first) {
        const b = el('button', 'step', el('span', 'bar'), el('span', 'gbar'), el('span', 'mark'),
          el('span', 'tt', el('span', 't1'), el('span', 't2')));
        b.type = 'button';
        b.addEventListener('click', () => send({ a: 'step', i }));
        nav.append(b);
        stepEls.push({ b, done: null });
      }
      const s = stepEls[i];
      const done = !!S.done[i];
      s.b.classList.toggle('done', done);
      s.b.classList.toggle('open', S.step === i);
      s.b.querySelector('.t1').textContent = titles[i];
      s.b.querySelector('.t2').textContent = S.subs[i];
      if (s.done !== done) {
        const mark = s.b.querySelector('.mark');
        if (done) {
          const c = checkIcon();
          if (s.done === false) c.classList.add('pop');
          mark.replaceChildren(c);
        } else {
          mark.replaceChildren(el('span', 'num', '0' + (i + 1)));
        }
        s.done = done;
      }
    }
  }

  function cancelBox(o) {
    const lines = [T(`Отменить заказ #${o.id}? Файл заявки с вашей секретной половиной ключа будет удалён без возможности восстановления.`,
      `Cancel order #${o.id}? The request file with your secret key half will be deleted permanently.`)];
    if (o.assembled) {
      lines.push(T('Кошельки по этой заявке уже собраны — убедитесь, что ключи сохранены.',
        'The wallets of this request are already assembled — make sure the keys are saved.'));
    } else if (o.verified) {
      lines.push(T('Если вы уже оплатили заказ, не отменяйте его: без этого файла собрать кошельки невозможно.',
        'If you have already paid, do not cancel: the wallets cannot be assembled without this file.'));
    }
    return el('div', 'cancelbox',
      ...lines.map((t, k) => el('div', k ? 'q strong' : 'q', t)),
      el('div', 'row',
        btn('btn sm danger-btn', T('Отменить заказ', 'Cancel the order'), () => { ui.confirmCancel = null; send({ a: 'cancel_order', id: o.id }); }, S.busy),
        btn('btn sm', T('Нет', 'No'), () => { ui.confirmCancel = null; render(); })));
  }

  function renderOrders() {
    const box = document.getElementById('orders');
    box.replaceChildren();
    box.append(kicker(T('Заявки', 'Requests') + ' · ' + S.orders.length));
    const list = el('div', 'olist');
    if (S.new_mode && S.orders.length) {
      list.append(el('div', 'orow draft', el('span', 'id', T('Новая заявка', 'New request')),
        el('span', 'nt', T('Сеть и красивый адрес', 'Network and a nice address'))));
    }
    S.orders.forEach((o, i) => {
      const x = el('button', 'ox', '×');
      x.type = 'button';
      x.title = T('Отменить заказ', 'Cancel the order');
      x.disabled = S.busy;
      x.addEventListener('click', (e) => { e.stopPropagation(); ui.confirmCancel = o.id; render(); });
      const row = el('div', 'orow' + (i === S.selected ? ' on' : ''),
        el('span', 'id', '#' + o.id),
        el('span', 'nt', o.net + (o.count > 1 ? ' · ×' + o.count : '')),
        el('span', 'dt', o.date),
        x);
      row.addEventListener('click', () => { if (i !== S.selected) send({ a: 'select', i }); });
      list.append(row);
      if (ui.confirmCancel === o.id) list.append(cancelBox(o));
    });
    if (!S.orders.length && !S.missing.length) list.append(el('div', 'empty', T('Пока нет заявок', 'No requests yet')));
    S.missing.forEach((m, i) => {
      list.append(el('div', 'orow lost',
        el('span', 'id', m.name),
        el('span', 'nt', m.exists ? T('Файл повреждён', 'The file is damaged') : T('Файл не найден', 'File not found')),
        el('span', 'pth', m.path),
        el('div', 'lostbtns',
          btn('btn sm acc', T('Найти файл…', 'Find file…'), () => send({ a: 'locate', i }), S.busy),
          btn('btn sm', T('Убрать', 'Remove'), () => send({ a: 'forget_missing', i }), S.busy))));
    });
    box.append(list);
    const btns = el('div', 'obtns');
    if (!S.new_mode) btns.append(btn('btn acc', T('+  Новая заявка', '+  New request'), () => send({ a: 'new' }), S.busy));
    btns.append(btn('btn', T('Открыть файл заявки…', 'Open request file…'), () => send({ a: 'open_file' }), S.busy));
    const om = message(S.open_msg, 'msg');
    if (om) btns.append(om);
    box.append(btns);
  }

  function netOf(tag) { return S.nets.find((n) => n.tag === tag) || S.nets[0]; }
  function wantedBody(net, s) {
    s = s.trim();
    if (s.startsWith(net.fixed)) return s.slice(net.fixed.length);
    if (net.evm && s.startsWith('0X')) return s.slice(2);
    return s;
  }

  let previewBox = null;
  function fillPreview() {
    if (!previewBox) return;
    const net = netOf(ui.net);
    const body = wantedBody(net, ui.start);
    const head = net.fixed + body;
    const end = ui.end.trim();
    const dots = Math.max(3, net.len - head.length - end.length);
    previewBox.replaceChildren(
      kicker(T('Так будет выглядеть адрес', 'This is how the address will look')),
      el('div', 'addr', el('span', 'hl', head), el('span', 'dots', '▪'.repeat(dots)), el('span', 'hl', end)),
      el('div', 'meta', el('span', null, net.label),
        ui.count > 1 ? el('span', 'cnt', ui.count + ' ' + T('адресов', 'addresses')) : null));
  }

  function newRequest(main) {
    const net = netOf(ui.net);
    main.append(el('div', 'h1', T('Новая заявка', 'New request')),
      el('div', 'sub', T('Сеть и красивый адрес', 'Network and a nice address')));
    previewBox = el('div', 'well preview');
    main.append(previewBox);
    fillPreview();

    const grid = el('div', 'grid2');
    const sel = el('select', 'input');
    sel.dataset.key = 'net';
    for (const n of S.nets) {
      const o = el('option', null, n.label);
      o.value = n.tag;
      if (n.tag === ui.net) o.selected = true;
      sel.append(o);
    }
    sel.addEventListener('change', () => { ui.net = sel.value; render(); });
    grid.append(el('label', 'field full', el('span', 'label', T('Сеть', 'Network')), sel));
    const start = el('input', 'input');
    start.dataset.key = 'start';
    start.value = ui.start;
    start.spellcheck = false;
    start.placeholder = T('например: Pet', 'e.g. Pet');
    start.addEventListener('input', () => { ui.start = start.value; fillPreview(); });
    grid.append(el('label', 'field', el('span', 'label', T('Начало адреса', 'Address start')),
      el('div', 'affix', el('span', 'pre', net.fixed), start)));
    const end = el('input', 'input');
    end.dataset.key = 'end';
    end.value = ui.end;
    end.spellcheck = false;
    end.placeholder = T('например: 777', 'e.g. 777');
    end.addEventListener('input', () => { ui.end = end.value; fillPreview(); });
    grid.append(el('label', 'field', el('span', 'label', T('Конец адреса · необязательно', 'Address end · optional')), end));
    if (S.create_msg && !S.create_msg.ok) grid.append(el('div', 'full', message(S.create_msg)));
    const num = el('input');
    num.dataset.key = 'count';
    num.value = ui.count;
    const setCount = (v) => { ui.count = Math.max(1, Math.min(100, v | 0 || 1)); num.value = ui.count; fillPreview(); };
    num.addEventListener('input', () => { const v = parseInt(num.value, 10); if (v >= 1) setCount(v); });
    num.addEventListener('blur', () => setCount(parseInt(num.value, 10)));
    const minus = btn('', '−', () => setCount(ui.count - 1));
    const plus = btn('plus', '+', () => setCount(ui.count + 1));
    grid.append(el('div', 'full qty', el('span', 'label', T('Количество адресов', 'Number of addresses')),
      el('div', 'stepper', minus, num, plus),
      el('span', 'hint', T('разных кошельков с этим началом и концом', 'different wallets with this start and end'))));
    const create = btn('primary', S.saving_request ? T('Выберите, куда сохранить файл…', 'Choose where to save the file…') : T('Создать заявку', 'Create request'),
      () => send({ a: 'create', net: ui.net, start: ui.start, end: ui.end, count: ui.count }), S.busy);
    const actions = el('div', 'full actions', create);
    if (S.orders.length) actions.append(btn('btn', T('Отмена', 'Cancel'), () => send({ a: 'cancel_new' }), S.busy));
    grid.append(actions);
    main.append(grid);
  }

  function requestDone(main) {
    const o = S.order;
    main.append(el('div', 'h1', T('Заявка создана', 'Request created')),
      el('div', 'sub', T('Скопируйте сообщение и вставьте его при оформлении заказа на сайте. Секретной части ключа в нём нет.',
        'Copy the message and paste it when placing the order on the site. It does not contain the secret part of the key.')));
    const well = el('div', 'well msgwell',
      el('div', 'head', el('span', null, T('Заявка', 'Request') + ' #' + o.id),
        btn('primary compact', T('Копировать сообщение', 'Copy message'), () => send({ a: 'copy_message' }))),
      el('pre', null, o.message));
    main.append(well);
    if (S.copy_msg) main.append(el('div', 'belowmsg', message(S.copy_msg)));
    const next = btn('btn acc next', T('Дальше — проверка адресов  →', 'Next — verify addresses  →'), () => send({ a: 'step', i: 1 }));
    next.style.marginTop = '20px';
    main.append(next);
  }

  function noOrder(main) {
    main.append(el('div', 'card', el('div', 'h', T('Сначала нужна заявка', 'A request comes first')),
      el('div', 't', T('Создайте заявку на шаге 1 или откройте файл заявки, сохранённый раньше. Если вставите коды продавца, программа сама найдёт их заявку среди сохранённых.',
        'Create a request in step 1 or open a request file saved earlier. If you paste the seller\'s codes, the program finds their request among the saved ones by itself.'))));
  }

  function codesBox(main, key, label, placeholder, action, actionLabel) {
    main.append(el('div', 'label', label));
    const ta = el('textarea', 'input');
    ta.dataset.key = key;
    ta.value = ui[key];
    ta.placeholder = placeholder;
    ta.spellcheck = false;
    ta.rows = 5;
    ta.style.margin = '6px 0 10px';
    ta.addEventListener('input', () => { ui[key] = ta.value; });
    const go = () => send({ a: action, text: ui[key] });
    ta.addEventListener('keydown', (e) => { if (e.key === 'Enter' && e.ctrlKey) { e.preventDefault(); go(); } });
    main.append(ta, btn('primary mid', actionLabel, go));
  }

  function failBlock(main, outcome) {
    main.append(el('div', 'result bad', el('div', 'head', el('span', 'mk', '[✖]'), outcome.summary)));
  }

  function verifyPage(main) {
    main.append(el('div', 'h1', T('Проверка адресов до оплаты', 'Check the addresses before paying')),
      el('div', 'sub', T('Вставьте коды от продавца. Адресов в них нет — программа сама вычислит каждый адрес из вашей половины ключа и половины продавца и проверит, что он подходит под заказ.',
        'Paste the seller\'s codes. They contain no addresses — the program itself computes every address from your key half and the seller\'s half and checks that it matches your order.')));
    if (!S.orders.length) noOrder(main);
    codesBox(main, 'proof', T('Коды подтверждения от продавца (VGPROOF…) — все сразу, в любом виде', 'Confirmation codes from the seller (VGPROOF…) — all at once, in any form'),
      'VGPROOF2:…\nVGPROOF2:…', 'verify', T('Проверить адреса', 'Verify addresses'));
    const v = S.verify;
    if (!v) return;
    if (v.passed && v.found.length) {
      const n = v.found.length;
      const box = el('div', 'result good',
        el('div', 'head', el('span', 'mk', '[✔]'), n === 1 ? T('Адрес настоящий — можно оплачивать', 'The address is genuine — you can pay')
          : T(`Все ${n} адресов настоящие — можно оплачивать`, `All ${n} addresses are genuine — you can pay`)),
        el('div', 'alist-gap'));
      v.found.forEach((a, i) => {
        const row = el('div', 'arow', el('span', 'n', String(i + 1).padStart(2, '0')), el('span', 'ad', addr(a)),
          el('span', 'cp', ui.copiedFound === i ? T('✔ скопировано', '✔ copied') : T('копировать', 'copy')));
        row.addEventListener('click', () => {
          send({ a: 'copy_found', i });
          ui.copiedFound = i;
          setTimeout(() => { if (ui.copiedFound === i) { ui.copiedFound = -1; render(); } }, 1500);
        });
        box.append(row);
      });
      box.append(el('p', null, T('Сверьте эти адреса с адресами в заказе на сайте. После оплаты продавец пришлёт коды VGKEY — с ними вы соберёте кошельки.',
        'Compare these addresses with the ones in your order on the site. After payment the seller sends the VGKEY codes — you assemble the wallets with them.')));
      main.append(box);
      const next = btn('btn acc next', T('Дальше — кошельки  →', 'Next — wallets  →'), () => send({ a: 'step', i: 2 }));
      next.style.marginTop = '16px';
      main.append(next);
    } else {
      failBlock(main, v);
    }
  }

  function walletPage(main) {
    const ws = S.wallets;
    if (!ws.length) {
      main.append(el('div', 'h1', T('Кошельки', 'Wallets')),
        el('div', 'sub', T('После оплаты вставьте половины ключа от продавца — программа соберёт ключи и вычислит адрес каждого.',
          'After paying, paste the seller\'s key halves — the program assembles the keys and computes each address.')));
      if (!S.orders.length) noOrder(main);
      codesBox(main, 'key', T('Половины ключа от продавца (VGKEY…) — все сразу, в любом виде', 'Key halves from the seller (VGKEY…) — all at once, in any form'),
        'VGKEY2:…\nVGKEY2:…', 'assemble', T('Собрать кошельки', 'Assemble wallets'));
      if (S.assemble && !S.assemble.passed) failBlock(main, S.assemble);
      return;
    }
    const n = ws.length;
    const head = el('div', 'whead',
      el('div', null, el('div', 'h1', n === 1 ? T('Ваш кошелёк готов', 'Your wallet is ready') : T(`Ваши кошельки готовы: ${n}`, `Your wallets are ready: ${n}`)),
        el('div', 'sub', S.assemble ? S.assemble.summary : '')),
      el('div', 'row',
        btn('btn acc', S.show_keys ? T('Скрыть ключи', 'Hide keys') : T('Показать ключи', 'Show keys'), () => send({ a: 'toggle_keys' })),
        btn('btn acc', T('Сохранить все в файл…', 'Save all to a file…'), () => send({ a: 'save_wallets' }), S.busy),
        btn('btn', T('Закрыть', 'Close'), () => send({ a: 'close_wallets' }))));
    main.append(head);
    main.append(el('div', 'danger', T('⚠  Ключ даёт полный доступ к кошельку. Никому не показывайте и не отправляйте ключи, в том числе продавцу.',
      '⚠  A key gives full access to its wallet. Never show or send the keys to anyone, including the seller.')));
    if (S.assemble) {
      const fails = S.assemble.checks.filter((c) => c.mark === 'fail');
      if (fails.length) {
        main.append(el('div', 'partial',
          el('div', 'h', T(`Кодов с ошибками: ${fails.length} — эти кошельки не собраны`, `Codes with errors: ${fails.length} — these wallets were not assembled`)),
          el('div', 't', T('Кошельки ниже собраны без ошибок, ими можно пользоваться и сохранить их. Коды с ошибками попросите у продавца заново и вставьте их отдельно — уже собранные кошельки не пропадут из заявки.',
            'The wallets below were assembled without errors — you can use and save them. Ask the seller to resend the codes with errors and paste them separately — the wallets already assembled stay in the request.')),
          checksList(fails)));
      }
      const line = el('div', 'summaryline', (fails.length ? T('Проверок: ', 'Checks: ') : T('Все проверки пройдены: ', 'All checks passed: ')) + S.assemble.checks.length + ' · ');
      const lnk = el('span', 'lnk', ui.showAssembleChecks ? T('скрыть', 'hide') : T('показать', 'show'));
      lnk.addEventListener('click', () => { ui.showAssembleChecks = !ui.showAssembleChecks; render(); });
      line.append(lnk);
      main.append(line);
      if (ui.showAssembleChecks) main.append(checksList(S.assemble.checks, 'inline-checks'));
    }
    const table = el('div', 'table', el('div', 'trow th', el('span', null, '№'),
      el('span', null, T('Адрес / приватный ключ', 'Address / private key').toUpperCase()), el('span', null, '')));
    ws.forEach((w, i) => {
      const key = el('span', 'key', S.show_keys ? w.key : '•'.repeat(64));
      const how = el('details', null, el('summary', null, T('Как получился ключ', 'How the key was made')),
        el('div', 'how',
          el('span', 'v', w.a), el('span', 'op', '+'), el('span', 'v', w.b), el('span', 'op', '='), el('span', 'v last', w.k),
          el('span', 'lbl', T('ваша половина', 'your half')), el('span', null, ''),
          el('span', 'lbl', T('половина продавца', "seller's half")), el('span', null, ''),
          el('span', 'lbl', T('ключ кошелька', 'wallet key'))));
      table.append(el('div', 'trow',
        el('span', 'n', String(i + 1).padStart(2, '0')),
        el('div', 'col', el('span', 'ad', addr(w)), key, how),
        el('div', 'btns',
          btn('btn sm', T('Копировать адрес', 'Copy address'), () => send({ a: 'copy_address', i })),
          btn('btn sm', T('Копировать ключ', 'Copy key'), () => send({ a: 'copy_key', i })))));
    });
    main.append(table);
    if (S.wallet_msg) main.append(el('div', 'belowmsg', message(S.wallet_msg)));
    if (S.assemble) {
      for (const c of S.assemble.checks.filter((c) => c.mark === 'warn')) {
        const note = el('div', 'warnnote', c.text);
        note.style.marginTop = '14px';
        main.append(note);
      }
    }
  }

  function checksList(checks, cls) {
    const box = el('div', cls || null);
    for (const c of checks) {
      box.append(el('div', 'check ' + c.mark, el('span', 'ic', c.mark === 'ok' ? '✔' : c.mark === 'warn' ? '!' : '✖'), el('span', 'tx', c.text)));
    }
    return box;
  }

  function facts(box) {
    const o = S.order;
    if (!o) return;
    box.append(kicker(T('Что заказано', 'What is ordered')));
    const rows = [[T('Сеть', 'Network'), o.net], [T('Начало', 'Start'), o.start], [T('Конец', 'End'), o.end],
      [T('Адресов', 'Addresses'), String(o.count)]];
    for (const [k, v] of rows) box.append(el('div', 'fact', el('span', 'k', k), el('span', 'v', v)));
  }

  function fileBlock(box) {
    const o = S.order;
    if (!o) return;
    const parts = o.parts || [];
    box.append(kicker(T('Файл заявки', 'Request file')));
    box.append(el('div', 'fact violet', el('span', 'k', T('Где лежит', 'Location')), el('span', 'v', o.path)));
    box.append(kicker(T('Что в файле', 'What is in the file')));
    box.append(el('div', 'fact violet secret',
      el('span', 'k', T('Секретная половина ключа', 'Secret key half')),
      el('span', 'v', o.secret_hint || '—'),
      el('span', 'only', T('Знаете только вы', 'Only you know it'))));
    const rows = [
      [T('Публичная половина («отпечаток»)', 'Public half (“fingerprint”)'), parts[1] ? parts[1][0] : '—'],
      [T('Контрольная сумма', 'Checksum'), parts[2] ? parts[2][0] : '—'],
      [T('Номер заявки', 'Request number'), '#' + o.id],
    ];
    for (const [k, v] of rows) box.append(el('div', 'fact violet', el('span', 'k', k), el('span', 'v', v)));
    box.append(el('div', 'warnnote', T('Файл заявки содержит вашу секретную половину ключа. Не удаляйте его до получения кошельков и никому не передавайте.',
      'The request file contains your secret key half. Do not delete it until you receive the wallets and never share it.')));
  }

  function inspectorContent() {
    const box = document.createDocumentFragment();
    const step = S.step;
    if (step === 0) {
      if (!S.order) {
        box.append(kicker(T('Файл заявки', 'Request file')));
        box.append(el('div', 'warnnote', T('Файл заявки хранит вашу секретную половину ключа. Никому его не передавайте и не удаляйте до получения кошельков — без него собрать их невозможно. После нажатия кнопки выберите, куда его сохранить.',
          'The request file holds your secret key half. Never share it and do not delete it until you receive the wallets — they cannot be assembled without it. After pressing the button, choose where to save it.')));
        box.append(kicker(T('Что уходит продавцу', 'What goes to the seller')));
        box.append(el('div', 'rulenote', T('Только код заявки VGREQ1 — «отпечаток» вашей половины. По нему можно искать красивый адрес, но получить ключ нельзя.',
          'Only the VGREQ1 request code — a “fingerprint” of your half. It is enough to search for a nice address, but not to get the key.')));
      } else {
        fileBlock(box);
      }
    } else {
      facts(box);
      const outcome = step === 1 ? S.verify : S.assemble;
      if (outcome) {
        box.append(kicker(T('Что проверила программа', 'What the program checked')));
        box.append(checksList(outcome.checks));
      }
      fileBlock(box);
      if (step === 2) {
        box.append(kicker(T('Что дальше', 'What next')));
        for (const t of [
          T('Импортируйте каждый ключ в кошелёк и убедитесь, что он показывает именно этот адрес.', 'Import each key into a wallet and make sure it shows exactly this address.'),
          T('Для Bitcoin, Litecoin, Dogecoin и Dash выбирайте кошелёк, который умеет нужный тип адреса: например, Phantom и Trust покажут для ключа адрес другого вида (bc1…), а Electrum — Legacy «1…».',
            'For Bitcoin, Litecoin, Dogecoin and Dash use a wallet that supports the right address type: e.g. Phantom and Trust show a different kind of address (bc1…) for the key, Electrum shows Legacy “1…”.'),
          T('Сохраните ключи в надёжном месте (на бумаге или в менеджере паролей).', 'Store the keys safely (on paper or in a password manager).'),
          T('Файл заявки и коды VGKEY после этого лучше удалить: вместе они дают ключи.', 'Afterwards it is best to delete the request file and the VGKEY codes: together they give the keys.'),
        ]) box.append(el('div', 'nextline', '— ' + t));
      }
    }
    return box;
  }

  function renderClosing() {
    let box = document.getElementById('modal');
    if (!S.closing) {
      if (box) box.remove();
      return;
    }
    if (!box) {
      box = el('div', null);
      box.id = 'modal';
      document.body.append(box);
    }
    box.replaceChildren(el('div', 'dialog',
      el('div', 'h', T('Ключи не сохранены', 'The keys are not saved')),
      el('div', 't', T('Если закрыть программу сейчас, собранные кошельки исчезнут из неё. Собрать их снова можно только по кодам VGKEY и файлу заявки.',
        'If you close the program now, the assembled wallets disappear from it. They can be assembled again only from the VGKEY codes and the request file.')),
      el('div', 'row',
        btn('primary compact', T('Сохранить в файл…', 'Save to a file…'), () => send({ a: 'save_wallets' }), S.busy),
        btn('btn danger-btn', T('Закрыть без сохранения', 'Close without saving'), () => send({ a: 'close_confirm' })),
        btn('btn', T('Отмена', 'Cancel'), () => send({ a: 'close_cancel' })))));
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

    for (const b of document.querySelectorAll('#lang button')) b.classList.toggle('on', (b.dataset.ru === '1') === S.ru);
    renderSteps();
    renderOrders();
    renderClosing();
    previewBox = null;
    main.replaceChildren();
    if (S.step === 0) (S.order ? requestDone : newRequest)(main);
    else if (S.step === 1) verifyPage(main);
    else walletPage(main);
    const inline = el('div', 'inline-inspector');
    inline.append(inspectorContent());
    main.append(inline);
    const insp = document.getElementById('inspector');
    insp.replaceChildren(inspectorContent());
    main.scrollTop = scroll;

    const key = [S.step, S.new_mode, S.selected, S.order && S.order.id, S.wallets.length,
      S.verify && S.verify.passed, S.assemble && S.assemble.passed].join('|');
    if (key !== ui.viewKey) {
      ui.viewKey = key;
      [...main.children].forEach((c, i) => { c.classList.add('in'); c.style.animationDelay = Math.min(i, 6) * 45 + 'ms'; });
      [...insp.children].forEach((c, i) => { c.classList.add('in'); c.style.animationDelay = 80 + Math.min(i, 10) * 30 + 'ms'; });
    }

    if (focusKey) {
      const f = document.querySelector(`[data-key="${focusKey}"]`);
      if (f) {
        f.focus();
        try { if (selStart !== null) f.setSelectionRange(selStart, selEnd); } catch (_) {  }
      }
    }
  }

  const B58 = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
  const WINDOW = 4;
  const TICK = 60;
  let scrambleRun = 0;
  let scrambleItems = [];
  const randomChar = (c) => (/\s/.test(c) ? c : B58[(Math.random() * B58.length) | 0]);
  const easeOut = (x) => 1 - Math.pow(1 - x, 3);

  function settle(it) {
    if (it.box.isConnected) it.box.replaceWith(document.createTextNode(it.text));
  }

  function scramble() {
    scrambleItems.forEach(settle);
    const run = ++scrambleRun;
    const nodes = [];
    for (const root of [document.getElementById('steps'), document.getElementById('body')]) {
      const walk = document.createTreeWalker(root, NodeFilter.SHOW_TEXT, {
        acceptNode: (n) => (!n.nodeValue.trim() || n.parentElement.closest('textarea, select, option, input, .scr')
          ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT),
      });
      for (let n = walk.nextNode(); n; n = walk.nextNode()) nodes.push(n);
    }
    scrambleItems = nodes.map((n, i) => {
      const text = n.nodeValue;
      const done = document.createTextNode('');
      const win = el('span', 'scr-win');
      const rest = el('span', 'scr-rest', text);
      const box = el('span', 'scr', done, win, rest);
      n.replaceWith(box);
      return {
        text, box, done, win, rest,
        delay: Math.min(i * 6, 180),
        dur: Math.min(260 + text.length * 12, 720),
        shown: -1, tick: 0,
      };
    });
    const t0 = performance.now();
    const frame = (now) => {
      if (run !== scrambleRun) return;
      let active = false;
      for (const it of scrambleItems) {
        if (!it.box.isConnected || it.shown === Infinity) continue;
        const p = Math.max(0, Math.min(1, (now - t0 - it.delay) / it.dur));
        if (p >= 1) { settle(it); it.shown = Infinity; continue; }
        active = true;
        const len = it.text.length;
        const front = Math.floor(easeOut(p) * (len + WINDOW)) - WINDOW;
        const fixed = Math.max(0, front);
        const end = Math.min(len, front + WINDOW);
        const refresh = now - it.tick >= TICK;
        if (fixed !== it.shown || refresh) {
          it.done.nodeValue = it.text.slice(0, fixed);
          let w = '';
          for (let i = fixed; i < end; i++) w += randomChar(it.text[i]);
          it.win.textContent = w;
          it.rest.textContent = it.text.slice(end);
          it.shown = fixed;
          if (refresh) it.tick = now;
        }
      }
      if (active) requestAnimationFrame(frame);
    };
    requestAnimationFrame(frame);
  }

  window.vb = {
    render(state) {
      const stepChanged = S && S.step !== state.step;
      const langChanged = S && S.ru !== state.ru;
      S = state;
      if (state.wallets.length) ui.key = '';
      document.body.classList.toggle('max', !!state.maximized);
      render();
      if (stepChanged) document.getElementById('main').scrollTop = 0;
      if (langChanged) scramble();
    },
    maximized(max) { document.body.classList.toggle('max', max); },
  };

  for (const id of ['drag', 'drag-brand']) {
    document.getElementById(id).addEventListener('mousedown', (e) => {
      if (e.button !== 0) return;
      if (e.detail === 2) send({ a: 'maximize' });
      else send({ a: 'drag' });
    });
  }
  document.getElementById('w-min').addEventListener('click', () => send({ a: 'minimize' }));
  document.getElementById('w-max').addEventListener('click', () => send({ a: 'maximize' }));
  document.getElementById('w-close').addEventListener('click', () => send({ a: 'close' }));
  for (const b of document.querySelectorAll('#lang button')) {
    b.addEventListener('click', () => send({ a: 'lang', ru: b.dataset.ru === '1' }));
  }
  for (const e of document.querySelectorAll('.edge')) {
    e.addEventListener('mousedown', (ev) => { if (ev.button === 0) { ev.preventDefault(); send({ a: 'resize', dir: e.dataset.dir }); } });
  }
  document.addEventListener('contextmenu', (e) => {
    if (!e.target.closest('input, textarea, .sel-text, pre, .v, .ad, .key')) e.preventDefault();
  });
  document.addEventListener('keydown', (e) => {
    const k = e.key.toLowerCase();
    if (e.key === 'F5' || e.key === 'F3' || e.key === 'F7' || (e.ctrlKey && ['r', 'p', 'f', 'g', 'j', 'u', 's', '+', '-', '=', '0'].includes(k))) e.preventDefault();
  });
  document.addEventListener('dragover', (e) => e.preventDefault());
  document.addEventListener('drop', (e) => e.preventDefault());

  send({ a: 'ready' });
})();
