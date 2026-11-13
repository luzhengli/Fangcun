// 方寸原型本地预览器。
// 把 prototype/project/*.dc.html（画布的模板源码）渲染成可交互的页面。
// 只实现画板实际用到的模板语法：{{hole}}、<sc-for>、<sc-if>、onClick/onChange、
// 画板间链接，以及 DCLogic 的 props / state / setState。不依赖画布编辑器的运行时。
(function () {
  'use strict';

  const SRC = '../../prototype/project/';
  const params = new URLSearchParams(location.search);
  const board = params.get('board');

  // ---------- 取值 ----------
  const HOLE = /\{\{\s*([^}]+?)\s*\}\}/g;
  const WHOLE = /^\{\{\s*([^}]+?)\s*\}\}$/;

  function lookup(expr, scope) {
    const e = expr.trim();
    if (e === 'true') return true;
    if (e === 'false') return false;
    if (e === 'null') return null;
    if (/^-?\d+(\.\d+)?$/.test(e)) return Number(e);
    const q = e.match(/^(['"])(.*)\1$/);
    if (q) return q[2];
    let v = scope;
    for (const k of e.split('.')) {
      if (v == null) return undefined;
      v = v[k];
    }
    return v;
  }

  function interpolate(text, scope) {
    return text.replace(HOLE, (_, e) => {
      const v = lookup(e, scope);
      return v == null || v === false ? '' : String(v);
    });
  }

  // ---------- 渲染 ----------
  const BOOL_PROPS = new Set(['checked', 'disabled', 'selected', 'readonly', 'required', 'hidden', 'open']);
  const TEXT_INPUT = /^(text|search|email|url|tel|password|number)$/;

  function eventName(el, attr) {
    const ev = attr.slice(2); // onclick → click
    if (ev === 'change' && (el.tagName === 'TEXTAREA' || (el.tagName === 'INPUT' && TEXT_INPUT.test(el.type || 'text')))) return 'input';
    return ev;
  }

  function renderNodes(nodes, scope, out) {
    for (const n of nodes) renderNode(n, scope, out);
  }

  function renderNode(node, scope, out) {
    if (node.nodeType === Node.TEXT_NODE) {
      const t = node.nodeValue;
      out.appendChild(document.createTextNode(t.indexOf('{{') >= 0 ? interpolate(t, scope) : t));
      return;
    }
    if (node.nodeType !== Node.ELEMENT_NODE) return;

    if (node.tagName === 'TEMPLATE' && node.hasAttribute('data-sc')) {
      const kind = node.getAttribute('data-sc');
      const kids = node.content.childNodes;
      if (kind === 'if') {
        if (lookup(node.getAttribute('value').replace(WHOLE, '$1'), scope)) renderNodes(kids, scope, out);
      } else {
        const list = lookup(node.getAttribute('list').replace(WHOLE, '$1'), scope) || [];
        const as = node.getAttribute('as') || 'item';
        Array.from(list).forEach((item, i) => {
          const s = Object.create(scope);
          s[as] = item;
          s.$index = i;
          renderNodes(kids, s, out);
        });
      }
      return;
    }

    const el = node.cloneNode(false); // 保留命名空间，SVG 图标照常显示
    const deferred = [];
    for (const a of Array.from(node.attributes)) {
      const name = a.name;
      const raw = a.value;
      if (name.startsWith('hint-')) { el.removeAttribute(name); continue; }
      if (raw.indexOf('{{') < 0) continue;
      const whole = raw.match(WHOLE);
      if (!whole) { el.setAttribute(name, interpolate(raw, scope)); continue; }
      const v = lookup(whole[1], scope);
      if (name.length > 2 && name.startsWith('on')) {
        el.removeAttribute(name);
        if (typeof v === 'function') el.addEventListener(eventName(el, name), v);
        continue;
      }
      if (BOOL_PROPS.has(name)) {
        if (v) el.setAttribute(name, ''); else el.removeAttribute(name);
        deferred.push([name, !!v]);
        continue;
      }
      if (name === 'value') {
        el.setAttribute('value', v == null ? '' : String(v));
        deferred.push(['value', v == null ? '' : String(v)]);
        continue;
      }
      if (v == null || v === false) el.removeAttribute(name);
      else el.setAttribute(name, String(v));
    }
    renderNodes(node.childNodes, scope, el);
    for (const [k, v] of deferred) { try { el[k] = v; } catch (e) { /* 只读属性，忽略 */ } }
    out.appendChild(el);
  }

  // ---------- 保留焦点和滚动位置 ----------
  function pathOf(el, root) {
    const p = [];
    while (el && el !== root) {
      p.unshift(Array.prototype.indexOf.call(el.parentNode.childNodes, el));
      el = el.parentNode;
    }
    return el === root ? p : null;
  }
  function nodeAt(path, root) {
    let n = root;
    for (const i of path) { n = n && n.childNodes[i]; }
    return n;
  }

  // ---------- 载入画板 ----------
  async function loadBoard(name) {
    const res = await fetch(SRC + name + '.dc.html', { cache: 'no-store' });
    if (!res.ok) throw new Error('读不到 ' + SRC + name + '.dc.html（' + res.status + '）');
    // <sc-for>/<sc-if> 放在 <tbody> 里会被 HTML 解析器挪走，先换成 <template>。
    const text = (await res.text())
      .replace(/<sc-(for|if)\b/g, '<template data-sc="$1"')
      .replace(/<\/sc-(for|if)>/g, '</template>');
    const doc = new DOMParser().parseFromString(text, 'text/html');

    const title = doc.querySelector('title');
    document.title = (title ? title.textContent : name) + ' · 原型预览';

    const xdc = doc.querySelector('x-dc');
    const helmet = xdc.querySelector('helmet');
    if (helmet) {
      for (const n of Array.from(helmet.children)) document.head.appendChild(document.importNode(n, true));
      helmet.remove();
    }

    const script = doc.querySelector('script[data-dc-script]');
    const decl = JSON.parse(script.getAttribute('data-props') || '{}');
    const props = {};
    for (const k of Object.keys(decl)) if (k.charAt(0) !== '$' && 'default' in decl[k]) props[k] = decl[k].default;
    for (const [k, v] of params) {
      if (k === 'board' || !(k in decl)) continue;
      props[k] = decl[k].editor === 'boolean' ? (v === '1' || v === 'true') : (decl[k].editor === 'int' || decl[k].editor === 'float' ? Number(v) : v);
    }

    const mount = document.getElementById('mount');
    let inst;
    let queued = false;

    class DCLogic {
      constructor() { this.props = props; this.state = undefined; }
      setState(patch) {
        const prev = this.state || {};
        const p = typeof patch === 'function' ? patch(prev, this.props) : patch;
        this.state = Object.assign({}, prev, p);
        schedule();
      }
      forceUpdate() { schedule(); }
    }

    function schedule() {
      if (queued) return;
      queued = true;
      Promise.resolve().then(() => { queued = false; render(); });
    }

    function render() {
      const active = document.activeElement;
      const fpath = active && mount.contains(active) ? pathOf(active, mount) : null;
      const sel = fpath && typeof active.selectionStart === 'number' ? [active.selectionStart, active.selectionEnd] : null;
      const scrolls = [];
      mount.querySelectorAll('*').forEach((el) => {
        if (el.scrollTop || el.scrollLeft) scrolls.push([pathOf(el, mount), el.scrollTop, el.scrollLeft]);
      });

      const vals = inst.renderVals() || {};
      const frag = document.createDocumentFragment();
      renderNodes(xdc.childNodes, vals, frag);
      mount.replaceChildren(frag);

      for (const [p, t, l] of scrolls) { const el = nodeAt(p, mount); if (el && el.nodeType === 1) { el.scrollTop = t; el.scrollLeft = l; } }
      if (fpath) {
        const el = nodeAt(fpath, mount);
        if (el && el.focus) {
          el.focus({ preventScroll: true });
          if (sel && typeof el.setSelectionRange === 'function') { try { el.setSelectionRange(sel[0], sel[1]); } catch (e) { /* 不支持选区的输入框 */ } }
        }
      }
    }

    const Component = new Function('DCLogic', script.textContent + '\nreturn Component;')(DCLogic);
    inst = new Component();
    inst.props = props;
    render();
    if (typeof inst.componentDidMount === 'function') inst.componentDidMount();
  }

  // 画板之间的链接：<a href="Activity.dc.html"> 换成预览器里的地址，保留 dark 等参数。
  document.addEventListener('click', (e) => {
    const a = e.target.closest && e.target.closest('a[href]');
    if (!a) return;
    const m = a.getAttribute('href').match(/^(?:\.\/|\/)?([\w.-]+)\.dc\.html$/);
    if (!m) return;
    e.preventDefault();
    const q = new URLSearchParams(location.search);
    q.set('board', m[1]);
    location.search = q.toString();
  });

  async function showIndex() {
    const res = await fetch(SRC + 'canvas.json', { cache: 'no-store' });
    const canvas = await res.json();
    const list = document.getElementById('boards');
    for (const file of canvas.order) {
      const b = canvas.boards[file] || {};
      const name = file.replace(/\.dc\.html$/, '');
      const li = document.createElement('li');
      li.innerHTML = '<a></a><span></span><a class="alt">深色</a>';
      li.children[0].href = '?board=' + encodeURIComponent(name);
      li.children[0].textContent = b.title || name;
      li.children[1].textContent = file;
      li.children[2].href = '?board=' + encodeURIComponent(name) + '&dark=1';
      list.appendChild(li);
    }
    document.getElementById('canvasTitle').textContent = canvas.title || '原型';
  }

  function fail(err) {
    document.body.classList.add('pv-index');
    const box = document.getElementById('error');
    box.hidden = false;
    box.textContent = String(err && err.message ? err.message : err) +
      (location.protocol === 'file:' ? '\n\n直接双击打开时浏览器不允许读取文件。请在仓库根目录运行 python3 -m http.server 8000，再打开 http://localhost:8000/tools/prototype-preview/' : '');
  }

  if (board) {
    document.body.classList.remove('pv-index');
    loadBoard(board).catch(fail);
  } else {
    showIndex().catch(fail);
  }
})();
