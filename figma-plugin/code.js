/**
 * EchoMind Design System Generator — Figma Plugin v3
 * 修复：字体加载顺序 + 属性访问安全检查
 */

var TOKENS = {
  surface: { 'surface-0': '#0A0A0B', 'surface-1': '#131316', 'surface-2': '#1C1C20', 'surface-3': '#26262C', 'surface-4': '#303036' },
  border: { 'border-subtle': '#1F1F23', 'border-default': '#2A2A2E', 'border-strong': '#3A3A40' },
  text: { 'text-primary': '#F8FAFC', 'text-secondary': '#CBD5E1', 'text-tertiary': '#94A3B8', 'text-quaternary': '#64748B' },
  accent: { 'accent': '#38BDF8', 'accent-hover': '#0EA5E9' },
  semantic: { 'success': '#4ADE80', 'warning': '#FBBF24', 'danger': '#F87171', 'info': '#60A5FA' },
  ink: '#0C1116',
};

var RADIUS = { 'xs': 4, 'sm': 8, 'md': 12, 'lg': 16, 'xl': 20 };

function hexToRgb(hex) {
  if (!hex || typeof hex !== 'string') return { r: 0.5, g: 0.5, b: 0.5 };
  hex = hex.replace('#', '');
  if (hex.length === 3) { hex = hex[0] + hex[0] + hex[1] + hex[1] + hex[2] + hex[2]; }
  return { r: parseInt(hex.substr(0, 2), 16) / 255, g: parseInt(hex.substr(2, 2), 16) / 255, b: parseInt(hex.substr(4, 2), 16) / 255 };
}

function hexToRgba(str) {
  if (!str || typeof str !== 'string') return { r: 0, g: 0, b: 0, a: 0.5 };
  var m = str.match(/rgba?\(([^)]+)\)/);
  if (!m) return { r: 0, g: 0, b: 0, a: 0.5 };
  var p = m[1].split(',');
  return { r: parseFloat(p[0]) / 255, g: parseFloat(p[1]) / 255, b: parseFloat(p[2]) / 255, a: p[3] ? parseFloat(p[3]) : 1 };
}

function safeColor(c) { return (c && typeof c === 'string') ? hexToRgb(c) : { r: 0.5, g: 0.5, b: 0.5 }; }

// 全局字体加载标志
var FONT_LOADED = false;
var FONT_NAME = { family: 'Inter', style: 'Regular' };

function createRect(x, y, w, h, fill, radius) {
  var r = figma.createRectangle();
  r.x = x; r.y = y; r.resize(w, h);
  r.fills = (fill && typeof fill === 'string' && fill !== 'transparent') ? [{ type: 'SOLID', color: safeColor(fill) }] : [];
  if (radius) r.cornerRadius = radius;
  return r;
}

function createText(x, y, text, size, weight, color) {
  var t = figma.createText();
  t.x = x; t.y = y;
  if (FONT_LOADED) { t.fontName = FONT_NAME; }
  t.characters = String(text || '');
  t.fontSize = size || 14;
  t.fills = [{ type: 'SOLID', color: safeColor(color || TOKENS.text['text-primary']) }];
  return t;
}

function createFrame(x, y, w, h, fill, radius, name) {
  var f = figma.createFrame();
  f.x = x; f.y = y; f.resize(w, h);
  f.name = name || 'Frame';
  f.fills = (fill && typeof fill === 'string' && fill !== 'transparent') ? [{ type: 'SOLID', color: safeColor(fill) }] : [];
  if (radius) f.cornerRadius = radius;
  f.layoutMode = 'NONE';
  return f;
}

function setStroke(node, color, weight) {
  node.strokeWeight = weight || 1;
  node.strokes = [{ type: 'SOLID', color: safeColor(color) }];
}

function addText(parent, x, y, text, size, color) {
  var t = createText(x, y, text, size, 400, color);
  parent.appendChild(t);
  return t;
}

function addRect(parent, x, y, w, h, fill, radius) {
  var r = createRect(x, y, w, h, fill, radius);
  parent.appendChild(r);
  return r;
}

function addFrame(parent, x, y, w, h, fill, radius, name) {
  var f = createFrame(x, y, w, h, fill, radius, name);
  parent.appendChild(f);
  return f;
}

// ============================================================
// 页面 1：Design Tokens
// ============================================================

function createTokenPage() {
  var page = figma.createPage();
  page.name = '🎨 Design Tokens';
  var y = 0, x = 0;

  page.appendChild(createText(0, y, 'EchoMind Design Tokens', 28, 700, TOKENS.text['text-primary']));
  y += 70;

  var groups = [
    { title: 'Surface', items: TOKENS.surface },
    { title: 'Border', items: TOKENS.border },
    { title: 'Text', items: TOKENS.text },
    { title: 'Accent', items: TOKENS.accent },
    { title: 'Semantic', items: TOKENS.semantic },
  ];

  groups.forEach(function (grp) {
    page.appendChild(createText(0, y, grp.title + ' Colors', 18, 600, TOKENS.text['text-secondary']));
    y += 40;
    x = 0;
    Object.keys(grp.items).forEach(function (key) {
      var color = grp.items[key];
      page.appendChild(createFrame(x, y, 120, 60, color, RADIUS.sm, key));
      page.appendChild(createText(x + 8, y + 68, key, 10, 400, TOKENS.text['text-quaternary']));
      page.appendChild(createText(x + 8, y + 82, color, 9, 400, TOKENS.text['text-quaternary']));
      x += 140;
    });
    y += 110;
  });

  // Radius
  page.appendChild(createText(0, y, 'Border Radius', 18, 600, TOKENS.text['text-secondary']));
  y += 40;
  x = 0;
  Object.keys(RADIUS).forEach(function (key) {
    var val = RADIUS[key];
    page.appendChild(createRect(x, y, 50, 50, TOKENS.surface['surface-3'], val));
    page.appendChild(createText(x, y + 58, key + ' ' + val, 10, 400, TOKENS.text['text-quaternary']));
    x += 70;
  });
  y += 90;

  // Typography
  page.appendChild(createText(0, y, 'Typography', 18, 600, TOKENS.text['text-secondary']));
  y += 40;
  var sizes = [28, 20, 16, 14, 13, 11, 10];
  sizes.forEach(function (sz) {
    page.appendChild(createText(0, y, sz + 'px — EchoMind 灵犀设计系统', sz, 400, TOKENS.text['text-primary']));
    y += sz + 20;
  });
}

// ============================================================
// 页面 2：组件库
// ============================================================

function createComponentPage() {
  var page = figma.createPage();
  page.name = '🧩 Components';
  var y = 0, x = 0;

  page.appendChild(createText(0, y, 'EchoMind Components', 28, 700, TOKENS.text['text-primary']));
  y += 60;

  // Buttons
  page.appendChild(createText(0, y, 'Buttons', 16, 600, TOKENS.text['text-secondary']));
  y += 40;

  var btns = [
    { label: '发送', fill: TOKENS.accent['accent'], text: TOKENS.ink },
    { label: '发送 (Hover)', fill: TOKENS.accent['accent-hover'], text: TOKENS.ink },
  ];
  btns.forEach(function (b, i) {
    var btn = createFrame(i * 140, y, 120, 36, b.fill, RADIUS.sm, b.label);
    addText(btn, 40, 8, b.label, 14, b.text);
    page.appendChild(btn);
  });

  var btnSec = createFrame(280, y, 120, 36, 'transparent', RADIUS.sm, 'btn-secondary');
  setStroke(btnSec, TOKENS.border['border-default'], 1);
  addText(btnSec, 40, 8, '取消', 14, TOKENS.text['text-secondary']);
  page.appendChild(btnSec);

  var btnDan = createFrame(420, y, 120, 36, 'transparent', RADIUS.sm, 'btn-danger');
  setStroke(btnDan, TOKENS.semantic.danger, 1);
  addText(btnDan, 35, 8, '■ 停止', 14, TOKENS.semantic.danger);
  page.appendChild(btnDan);

  var btnDis = createFrame(560, y, 120, 36, TOKENS.surface['surface-3'], RADIUS.sm, 'btn-disabled');
  btnDis.opacity = 0.4;
  addText(btnDis, 35, 8, '验证中…', 14, TOKENS.text['text-tertiary']);
  page.appendChild(btnDis);
  y += 70;

  // Inputs
  page.appendChild(createText(0, y, 'Input Fields', 16, 600, TOKENS.text['text-secondary']));
  y += 40;

  var inputs = [
    { label: 'Default', border: TOKENS.border['border-default'], text: '输入问题…', tc: TOKENS.text['text-quaternary'] },
    { label: 'Focus', border: TOKENS.accent['accent'], text: 'EchoMind 支持哪些格式？', tc: TOKENS.text['text-secondary'] },
    { label: 'Error', border: TOKENS.semantic.danger, text: 'API Key 不能为空', tc: TOKENS.semantic.danger },
  ];
  inputs.forEach(function (inp, i) {
    var box = createFrame(i * 320, y, 300, 40, TOKENS.surface['surface-1'], RADIUS.sm, inp.label);
    setStroke(box, inp.border, i === 1 ? 1.5 : 1);
    addText(box, 12, 10, inp.text, 13, inp.tc);
    page.appendChild(box);
  });
  y += 70;

  // Badges
  page.appendChild(createText(0, y, 'Status Badges', 16, 600, TOKENS.text['text-secondary']));
  y += 40;
  var badges = [
    { label: '待索引', color: TOKENS.text['text-tertiary'] },
    { label: '索引中', color: TOKENS.semantic.warning },
    { label: '已索引', color: TOKENS.accent['accent'] },
    { label: '失败', color: TOKENS.semantic.danger },
  ];
  x = 0;
  badges.forEach(function (b) {
    var bg = createFrame(x, y, 80, 22, 'transparent', RADIUS.xs, b.label);
    setStroke(bg, b.color, 1);
    addText(bg, 10, 4, b.label, 11, b.color);
    page.appendChild(bg);
    x += 100;
  });
  y += 50;

  // Toggle
  page.appendChild(createText(0, y, 'Toggle', 16, 600, TOKENS.text['text-secondary']));
  y += 40;
  page.appendChild(createFrame(0, y, 44, 24, TOKENS.accent['accent'], 12, 'toggle-on'));
  page.appendChild(createRect(22, y + 2, 20, 20, '#FFFFFF', 10));
  page.appendChild(createFrame(60, y, 44, 24, TOKENS.surface['surface-4'], 12, 'toggle-off'));
  page.appendChild(createRect(62, y + 2, 20, 20, '#FFFFFF', 10));
  y += 50;

  // Message Bubbles
  page.appendChild(createText(0, y, 'Message Bubbles', 16, 600, TOKENS.text['text-secondary']));
  y += 40;

  var userMsg = createFrame(300, y, 280, 50, TOKENS.accent['accent'], RADIUS.lg, 'msg-user');
  userMsg.opacity = 0.15;
  setStroke(userMsg, TOKENS.accent['accent'], 1);
  addText(userMsg, 16, 14, 'EchoMind 支持哪些格式？', 14, TOKENS.text['text-primary']);
  page.appendChild(userMsg);
  y += 70;

  var aiMsg = createFrame(0, y, 380, 60, TOKENS.surface['surface-2'], RADIUS.lg, 'msg-ai');
  setStroke(aiMsg, TOKENS.border['border-default'], 1);
  addText(aiMsg, 16, 12, 'EchoMind 支持 .md / .txt / .pdf 格式。', 14, TOKENS.text['text-secondary']);
  page.appendChild(aiMsg);
  y += 90;

  // Code Block
  page.appendChild(createText(0, y, 'Code Block', 16, 600, TOKENS.text['text-secondary']));
  y += 40;
  var code = createFrame(0, y, 380, 80, '#0D1117', RADIUS.sm, 'code');
  setStroke(code, TOKENS.border['border-subtle'], 1);
  addText(code, 12, 8, 'RUST', 11, TOKENS.text['text-tertiary']);
  addText(code, 310, 8, '复制', 11, TOKENS.text['text-tertiary']);
  addText(code, 12, 30, 'fn main() { println!("Hi!"); }', 13, '#E2E8F0');
  page.appendChild(code);
}

// ============================================================
// 页面 3：页面布局
// ============================================================

function createLayoutPage() {
  var page = figma.createPage();
  page.name = '📐 Layouts';
  var y = 0;

  // 1. Wizard
  page.appendChild(createText(0, y, '1. Onboarding Wizard', 16, 600, TOKENS.text['text-secondary']));
  y += 40;
  var wiz = addFrame(page, 0, y, 760, 440, TOKENS.surface['surface-0'], 0, 'Wizard');
  addText(page, 280, y + 30, '灵犀 EchoMind', 24, TOKENS.text['text-primary']);
  addText(page, 230, y + 65, '本地 AI 知识库 · 数据不出域', 12, TOKENS.text['text-quaternary']);

  var pnames = ['DeepSeek', 'OpenAI', 'Ollama'];
  for (var i = 0; i < 3; i++) {
    var act = i === 0;
    var card = addFrame(page, 80 + i * 200, y + 110, 170, 60, TOKENS.surface['surface-2'], RADIUS.lg, pnames[i]);
    setStroke(card, act ? TOKENS.accent['accent'] : TOKENS.border['border-default'], act ? 1.5 : 1);
    addText(card, 12, 10, pnames[i], 13, act ? TOKENS.accent['accent'] : TOKENS.text['text-secondary']);
    addText(card, 12, 30, ['高性价比', 'GPT 系列', '完全离线'][i], 10, TOKENS.text['text-quaternary']);
  }

  addText(page, 80, y + 190, 'API Key', 11, TOKENS.text['text-tertiary']);
  var ki = addFrame(page, 80, y + 208, 600, 38, TOKENS.surface['surface-1'], RADIUS.sm, 'key');
  setStroke(ki, TOKENS.border['border-default'], 1);
  addText(ki, 12, 10, 'sk-...', 13, TOKENS.text['text-quaternary']);

  addText(page, 80, y + 260, 'Base URL', 11, TOKENS.text['text-tertiary']);
  var ui2 = addFrame(page, 80, y + 278, 290, 38, TOKENS.surface['surface-1'], RADIUS.sm, 'url');
  setStroke(ui2, TOKENS.border['border-default'], 1);
  addText(ui2, 12, 10, 'https://api.deepseek.com', 13, TOKENS.text['text-secondary']);

  addText(page, 390, y + 260, 'Model', 11, TOKENS.text['text-tertiary']);
  var mi = addFrame(page, 390, y + 278, 290, 38, TOKENS.surface['surface-1'], RADIUS.sm, 'model');
  setStroke(mi, TOKENS.border['border-default'], 1);
  addText(mi, 12, 10, 'deepseek-chat', 13, TOKENS.text['text-secondary']);

  addText(page, 80, y + 340, '去获取 API Key →', 13, TOKENS.accent['accent']);
  var sb = addFrame(page, 530, y + 335, 150, 38, TOKENS.accent['accent'], RADIUS.lg, 'start');
  addText(sb, 40, 9, '验证并开始', 13, TOKENS.ink);

  y += 480;

  // 2. Main App
  page.appendChild(createText(0, y, '2. Main Application', 16, 600, TOKENS.text['text-secondary']));
  y += 40;
  addFrame(page, 0, y, 960, 460, TOKENS.surface['surface-0'], 0, 'App');

  // Sidebar
  var sb2 = addFrame(page, 0, y, 220, 460, TOKENS.surface['surface-1'], 0, 'Sidebar');
  setStroke(sb2, TOKENS.border['border-subtle'], 1);
  addText(sb2, 16, 16, '灵犀 ·', 15, TOKENS.text['text-primary']);
  addText(sb2, 190, 16, '‹', 16, TOKENS.text['text-quaternary']);

  var tc = addFrame(sb2, 12, 48, 90, 28, TOKENS.surface['surface-3'], RADIUS.sm, 'tab1');
  addText(tc, 20, 6, '💬 会话', 13, TOKENS.text['text-primary']);
  var td = addFrame(sb2, 110, 48, 90, 28, 'transparent', RADIUS.sm, 'tab2');
  addText(td, 14, 6, '📚 知识库', 13, TOKENS.text['text-tertiary']);

  var nc = addFrame(sb2, 12, 88, 196, 32, 'transparent', RADIUS.sm, 'new');
  setStroke(nc, TOKENS.accent['accent'], 1);
  nc.fills = [{ type: 'SOLID', color: { r: 0.22, g: 0.74, b: 0.97, a: 0.1 } }];
  addText(nc, 10, 7, '＋ 新对话', 13, TOKENS.accent['accent']);
  addText(nc, 165, 9, '⌘N', 10, TOKENS.text['text-quaternary']);

  var convs = ['EchoMind 格式？', 'RAG 原理', 'Ollama 配置'];
  for (var ci = 0; ci < convs.length; ci++) {
    var act2 = ci === 0;
    var cv = addFrame(sb2, 12, 130 + ci * 36, 196, 32, 'transparent', RADIUS.sm, convs[ci]);
    if (act2) cv.fills = [{ type: 'SOLID', color: { r: 0.22, g: 0.74, b: 0.97, a: 0.15 } }];
    addText(cv, 8, 8, convs[ci], 12, act2 ? TOKENS.accent['accent'] : TOKENS.text['text-secondary']);
  }

  var st = addFrame(sb2, 0, 420, 220, 40, TOKENS.surface['surface-1'], 0, 'status');
  setStroke(st, TOKENS.border['border-subtle'], 1);
  addText(st, 16, 12, '免费版 · 数据仅本地处理', 11, TOKENS.text['text-quaternary']);
  addText(st, 185, 12, '⚙', 14, TOKENS.text['text-quaternary']);

  // Chat empty state
  addText(page, 520, y + 120, '◈', 44, TOKENS.accent['accent']);
  addText(page, 440, y + 180, '向你的知识库提问', 17, TOKENS.text['text-primary']);
  addText(page, 340, y + 210, '回答仅基于本地数据与你自己配置的模型。', 12, TOKENS.text['text-quaternary']);

  // Input bar
  var ib = addFrame(page, 244, y + 410, 700, 44, TOKENS.surface['surface-1'], RADIUS.lg, 'input');
  setStroke(ib, TOKENS.border['border-default'], 1);
  addText(ib, 16, 12, '＋', 18, TOKENS.text['text-secondary']);
  addText(ib, 48, 13, '输入问题，Enter 发送…', 13, TOKENS.text['text-quaternary']);
  var sd = addFrame(ib, 608, 6, 76, 32, TOKENS.accent['accent'], RADIUS.lg, 'send');
  addText(sd, 18, 6, '发送', 13, TOKENS.ink);

  y += 500;

  // 3. Command Palette
  page.appendChild(createText(0, y, '3. Command Palette', 16, 600, TOKENS.text['text-secondary']));
  y += 40;
  addFrame(page, 0, y, 560, 350, TOKENS.surface['surface-0'], 0, 'overlay').opacity = 0.6;

  var cp = addFrame(page, 130, y + 20, 300, 240, TOKENS.surface['surface-1'], RADIUS.lg, 'Palette');
  setStroke(cp, TOKENS.border['border-strong'], 1);
  cp.effects = [{ type: 'DROP_SHADOW', color: { r: 0, g: 0, b: 0, a: 0.25 }, offset: { x: 0, y: 16 }, radius: 48, visible: true, blendMode: 'NORMAL' }];

  addText(cp, 16, 12, '🔍', 14, TOKENS.text['text-quaternary']);
  addText(cp, 40, 14, '输入命令或搜索…', 13, TOKENS.text['text-quaternary']);
  var ek = addFrame(cp, 230, 8, 28, 20, TOKENS.surface['surface-3'], RADIUS.xs, 'esc');
  addText(ek, 5, 3, 'Esc', 10, TOKENS.text['text-quaternary']);

  var cmds = [
    { g: '导航', items: [['💬', '新建会话', '⌘N'], ['＋', '导入文件', '⌘O'], ['⚙', '打开设置', '⌘,']] },
    { g: '对话', items: [['■', '停止生成', 'Esc'], ['🗑', '清空对话', '⌘⇧⌫']] },
  ];
  var iy = 40;
  cmds.forEach(function (grp) {
    addText(cp, 16, iy, grp.g, 10, TOKENS.text['text-quaternary']);
    iy += 18;
    grp.items.forEach(function (cmd) {
      var sel = grp.g === '导航' && cmd[1] === '新建会话';
      var row = addFrame(cp, 0, iy, 300, 32, 'transparent', 0, cmd[1]);
      if (sel) row.fills = [{ type: 'SOLID', color: { r: 0.22, g: 0.74, b: 0.97, a: 0.1 } }];
      addText(row, 16, 7, cmd[0], 13, TOKENS.text['text-secondary']);
      addText(row, 40, 7, cmd[1], 13, sel ? TOKENS.text['text-primary'] : TOKENS.text['text-secondary']);
      addText(row, 240, 9, cmd[2], 10, TOKENS.text['text-quaternary']);
      iy += 32;
    });
    iy += 6;
  });
  y += 390;

  // 4. Paywall
  page.appendChild(createText(0, y, '4. Paywall', 16, 600, TOKENS.text['text-secondary']));
  y += 40;
  addFrame(page, 0, y, 560, 320, TOKENS.surface['surface-0'], 0, 'pay-overlay').opacity = 0.95;

  var pm = addFrame(page, 130, y + 30, 300, 250, 'transparent', 0, 'Paywall');
  addText(pm, 130, 15, '◈', 32, TOKENS.accent['accent']);
  addText(pm, 80, 60, '升级到 Pro 版', 18, TOKENS.text['text-primary']);
  addText(pm, 20, 92, '配额已满（50/50）', 12, TOKENS.text['text-quaternary']);
  addText(pm, 40, 120, 'License Key', 11, TOKENS.text['text-tertiary']);
  var li = addFrame(pm, 40, 138, 220, 36, TOKENS.surface['surface-1'], RADIUS.sm, 'lic');
  setStroke(li, TOKENS.border['border-default'], 1);
  addText(li, 12, 10, '粘贴 License Key', 13, TOKENS.text['text-quaternary']);

  var lb = addFrame(pm, 40, 188, 100, 34, 'transparent', RADIUS.sm, 'later');
  setStroke(lb, TOKENS.border['border-default'], 1);
  addText(lb, 18, 8, '稍后再说', 13, TOKENS.text['text-secondary']);

  var ab = addFrame(pm, 150, 188, 110, 34, TOKENS.accent['accent'], RADIUS.sm, 'act');
  addText(ab, 24, 8, '激活 Pro', 13, TOKENS.ink);
  y += 360;

  // 5. Settings
  page.appendChild(createText(0, y, '5. Settings', 16, 600, TOKENS.text['text-secondary']));
  y += 40;
  addFrame(page, 0, y, 560, 360, TOKENS.surface['surface-0'], 0, 'set-overlay').opacity = 0.95;

  var sm = addFrame(page, 130, y + 20, 300, 300, 'transparent', 0, 'Settings');
  addText(sm, 120, 10, '设置', 18, TOKENS.text['text-primary']);
  addText(sm, 40, 48, 'LLM 配置', 10, TOKENS.text['text-quaternary']);

  var info = addFrame(sm, 40, 65, 220, 56, TOKENS.surface['surface-1'], RADIUS.sm, 'info');
  setStroke(info, TOKENS.border['border-default'], 1);
  addText(info, 12, 8, '端点', 10, TOKENS.text['text-quaternary']);
  addText(info, 110, 8, 'api.deepseek.com', 10, TOKENS.text['text-secondary']);
  addText(info, 12, 24, '模型', 10, TOKENS.text['text-quaternary']);
  addText(info, 110, 24, 'deepseek-chat', 10, TOKENS.text['text-secondary']);
  addText(info, 12, 40, 'Key', 10, TOKENS.text['text-quaternary']);
  addText(info, 110, 40, 'sk-****5678', 10, TOKENS.text['text-secondary']);

  var eb = addFrame(sm, 40, 132, 220, 34, 'transparent', RADIUS.sm, 'edit');
  setStroke(eb, TOKENS.border['border-default'], 1);
  addText(eb, 60, 8, '修改 LLM 配置', 13, TOKENS.text['text-secondary']);

  var cb = addFrame(sm, 40, 250, 220, 38, TOKENS.accent['accent'], RADIUS.sm, 'done');
  addText(cb, 90, 9, '完成', 13, TOKENS.ink);
}

// ============================================================
// 主入口
// ============================================================

figma.showUI(__html__, { width: 360, height: 400 });

figma.ui.onmessage = function (msg) {
  if (msg.type === 'generate') {
    figma.notify('正在生成…', { timeout: 2000 });

    // 尝试加载字体，失败也继续生成
    figma.loadFontAsync({ family: 'Inter', style: 'Regular' }).then(function () {
      FONT_LOADED = true;
    }).catch(function () {
      // 字体加载失败，继续用默认字体
      figma.notify('Inter 字体不可用，使用默认字体', { timeout: 3000 });
    }).then(function () {
      try {
        createTokenPage();
        createComponentPage();
        createLayoutPage();
        // 切换到 Design Tokens 页面（倒数第三个 = 第一个创建的页面）
        var allPages = figma.root.children;
        if (allPages.length >= 4) {
          figma.currentPage = allPages[allPages.length - 3];
        }
        figma.notify('✅ 设计系统已生成！', { timeout: 5000 });
        figma.ui.postMessage({ type: 'done', pages: 3 });
      } catch (err) {
        figma.notify('❌ ' + (err.message || err), { timeout: 5000 });
        figma.ui.postMessage({ type: 'error', message: String(err.message || err) });
      }
    });
  }

  if (msg.type === 'cancel') {
    figma.closePlugin();
  }
};
