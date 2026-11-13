// 方寸原型本地预览器。
// 把 prototype/project/*.dc.html（画布的模板源码）渲染成可交互的页面。
// 实现的语法：{{hole}}、<sc-for>、<sc-if>、onClick/onChange 等事件、<dc-import> 子组件、
// 画板间链接，以及 DCLogic 的 props / state / setState。不依赖画布编辑器的运行时。
// 遇到不认识的标签、残留的 {{ 或脚本报错时，页面顶部挂警告条，并写进 window.__dcPreview.issues，
// check.js 靠它判断预览器要不要补。
(function () {
  'use strict';

  const SRC = '../../prototype/project/';
  const params = new URLSearchParams(location.search);
  const board = params.get('board');
  const report = { ready: false, issues: [] };
  window.__dcPreview = report;
  const issue = (msg) => { if (report.issues.indexOf(msg) < 0) report.issues.push(msg); };

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
    if (!/^[\w$]+(\.[\w$]+)*$/.test(e)) { issue('不支持的表达式：{{ ' + e + ' }}'); return undefined; }
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

  function attrValue(raw, scope) {
    if (raw.indexOf('{{') < 0) return raw;
    const whole = raw.match(WHOLE);
    return whole ? lookup(whole[1], scope) : interpolate(raw, scope);
  }

  // ---------- 组件定义 ----------
  const defs = {}; // 名称 → { xdc, Component, decl }
  const instances = {}; // 实例键 → 组件实例，跨渲染保留 state
  let counters = {};
  let queued = false;
  let renderAll = () => {};

  class DCLogic {
    constructor() { this.props = {}; this.state = undefined; }
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
    Promise.resolve().then(() => { queued = false; renderAll(); });
  }

  async function loadDef(name) {
    if (defs[name]) return defs[name];
    const res = await fetch(SRC + name + '.dc.html', { cache: 'no-store' });
    if (!res.ok) throw new Error('读不到 ' + SRC + name + '.dc.html（' + res.status + '）');
    // <sc-for>/<sc-if> 放在 <tbody> 里会被 HTML 解析器挪走，先换成 <template>。
    const text = (await res.text())
      .replace(/<sc-(for|if)\b/g, '<template data-sc="$1"')
      .replace(/<\/sc-(for|if)>/g, '</template>');
    const doc = new DOMParser().parseFromString(text, 'text/html');
    const xdc = doc.querySelector('x-dc');
    if (!xdc) throw new Error(name + '.dc.html 里没有 <x-dc>');

    const helmet = xdc.querySelector('helmet');
    if (helmet) {
      for (const n of Array.from(helmet.children)) document.head.appendChild(document.importNode(n, true));
      helmet.remove();
    }
    for (const s of doc.head.querySelectorAll('script[src], link[rel=stylesheet]')) {
      const src = s.getAttribute('src') || s.getAttribute('href');
      if (!/(^|\/)support\.js$/.test(src)) issue('画板 ' + name + ' 引用了预览器没有加载的文件：' + src);
    }

    const script = doc.querySelector('script[data-dc-script]');
    const decl = script ? JSON.parse(script.getAttribute('data-props') || '{}') : {};
    let Component = class extends DCLogic { renderVals() { return {}; } };
    if (script) Component = new Function('DCLogic', script.textContent + '\nreturn Component;')(DCLogic);

    const def = { name, xdc, Component, decl, title: (doc.querySelector('title') || {}).textContent };
    defs[name] = def;
    // 子组件一并预载
    const kids = new Set(Array.from(xdc.querySelectorAll('dc-import[name]')).map((n) => n.getAttribute('name')));
    for (const t of xdc.querySelectorAll('template')) {
      for (const n of t.content.querySelectorAll('dc-import[name]')) kids.add(n.getAttribute('name'));
    }
    for (const k of kids) await loadDef(k);
    return def;
  }

  function defaultsOf(decl) {
    const props = {};
    for (const k of Object.keys(decl)) if (k.charAt(0) !== '$' && decl[k] && 'default' in decl[k]) props[k] = decl[k].default;
    return props;
  }

  function renderDef(def, key, props, out) {
    let inst = instances[key];
    if (!inst) { inst = new def.Component(); instances[key] = inst; inst.props = props; inst.__new = true; }
    inst.props = props;
    let vals = {};
    try { vals = inst.renderVals() || {}; } catch (e) { issue(def.name + ' 的 renderVals 出错：' + e.message); }
    renderNodes(def.xdc.childNodes, vals, out);
  }

  // ---------- 渲染 ----------
  const BOOL_PROPS = new Set(['checked', 'disabled', 'selected', 'readonly', 'required', 'hidden', 'open']);
  const TEXT_INPUT = /^(text|search|email|url|tel|password|number)$/;
  const camel = (s) => s.replace(/-([a-z])/g, (_, c) => c.toUpperCase());

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
    const tag = node.tagName;

    if (tag === 'TEMPLATE' && node.hasAttribute('data-sc')) {
      const kind = node.getAttribute('data-sc');
      const kids = node.content.childNodes;
      if (kind === 'if') {
        if (attrValue(node.getAttribute('value') || '', scope)) renderNodes(kids, scope, out);
      } else {
        const list = attrValue(node.getAttribute('list') || '', scope) || [];
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

    if (tag === 'DC-IMPORT') {
      const name = node.getAttribute('name');
      const def = defs[name];
      if (!def) { issue('找不到子组件 ' + name + '.dc.html'); return; }
      const props = defaultsOf(def.decl);
      for (const a of Array.from(node.attributes)) {
        if (a.name === 'name' || a.name.startsWith('hint-')) continue;
        props[camel(a.name)] = attrValue(a.value, scope);
      }
      counters[name] = (counters[name] || 0) + 1;
      const box = document.createElement('div');
      box.style.display = 'contents';
      box.setAttribute('data-dc-import', name);
      renderDef(def, name + '#' + counters[name], props, box);
      out.appendChild(box);
      return;
    }

    if (tag.indexOf('-') > 0 && tag !== 'X-DC') issue('预览器不认识 <' + tag.toLowerCase() + '>，按普通标签显示');

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

  let issuesDismissed = 0;
  function showIssues() {
    let bar = document.getElementById('pv-issues');
    if (!report.issues.length || issuesDismissed === report.issues.length) { if (bar) bar.remove(); return; }
    if (!bar) {
      bar = document.createElement('div');
      bar.id = 'pv-issues';
      bar.setAttribute('role', 'alert');
      bar.innerHTML = '<span></span><button type="button" aria-label="关闭提示">×</button>';
      bar.querySelector('button').addEventListener('click', () => { issuesDismissed = report.issues.length; bar.remove(); });
      document.body.appendChild(bar);
    }
    bar.firstChild.textContent = '预览可能和网页版不一致，以源文件为准：' + report.issues.join('；');
  }

  // ---------- 载入画板 ----------
  async function loadBoard(name) {
    const def = await loadDef(name);
    document.title = (def.title || name) + ' · 原型预览';
    const props = defaultsOf(def.decl);
    for (const [k, v] of params) {
      if (k === 'board' || !(k in def.decl)) continue;
      const ed = def.decl[k].editor;
      props[k] = ed === 'boolean' ? (v === '1' || v === 'true') : (ed === 'int' || ed === 'float' || ed === 'range' ? Number(v) : v);
    }

    const mount = document.getElementById('mount');
    renderAll = function () {
      const active = document.activeElement;
      const fpath = active && mount.contains(active) ? pathOf(active, mount) : null;
      const sel = fpath && typeof active.selectionStart === 'number' ? [active.selectionStart, active.selectionEnd] : null;
      const scrolls = [];
      mount.querySelectorAll('*').forEach((el) => {
        if (el.scrollTop || el.scrollLeft) scrolls.push([pathOf(el, mount), el.scrollTop, el.scrollLeft]);
      });

      counters = {};
      const frag = document.createDocumentFragment();
      renderDef(def, name + '#root', props, frag);
      mount.replaceChildren(frag);

      for (const [p, t, l] of scrolls) { const el = nodeAt(p, mount); if (el && el.nodeType === 1) { el.scrollTop = t; el.scrollLeft = l; } }
      if (fpath) {
        const el = nodeAt(fpath, mount);
        if (el && el.focus) {
          el.focus({ preventScroll: true });
          if (sel && typeof el.setSelectionRange === 'function') { try { el.setSelectionRange(sel[0], sel[1]); } catch (e) { /* 不支持选区的输入框 */ } }
        }
      }
      if (/\{\{/.test(mount.innerHTML)) issue('页面里还有没替换的 {{…}}');
      for (const k of Object.keys(instances)) {
        const inst = instances[k];
        if (inst.__new) { inst.__new = false; if (typeof inst.componentDidMount === 'function') inst.componentDidMount(); }
      }
      showIssues();
    };
    renderAll();
    report.ready = true;
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

  window.addEventListener('error', (e) => { issue('脚本出错：' + e.message); showIssues(); });

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
    report.ready = true;
  }

  function fail(err) {
    issue(String(err && err.message ? err.message : err));
    document.body.classList.add('pv-index');
    const box = document.getElementById('error');
    box.hidden = false;
    box.textContent = report.issues.join('\n') +
      (location.protocol === 'file:' ? '\n\n直接双击打开时浏览器不允许读取文件。请在仓库根目录运行 python3 -m http.server 8000，再打开 http://localhost:8000/tools/prototype-preview/' : '');
    report.ready = true;
  }

  if (board) {
    document.body.classList.remove('pv-index');
    loadBoard(board).catch(fail);
  } else {
    showIndex().catch(fail);
  }
})();
