/**
 * EchoMind Tailwind CSS 配置 — 预构建模式。
 *
 * 此文件从 ui/index.html 内联配置提取，用于 `npx tailwindcss` CLI 预构建。
 * 预构建 CSS 替代 vendor/tailwindcss.js (441KB JIT 运行时)，
 * 仅包含源码中实际使用的工具类（~50-80KB），体积减少 ~80%。
 *
 * 构建命令：
 *   npx tailwindcss -i ui/styles/tailwind-input.css -o ui/vendor/tailwind-prebuilt.css --minify
 *
 * safelist 说明：
 *   zClass() 运行时生成 z-[55]/z-[60]/z-[65]/z-[70]/z-[75]/z-[80]/z-[90]/z-[95]/z-[99999]
 *   这些任意值类名无法被 Tailwind CLI 静态扫描发现，需显式 safelist。
 */

/** @type {import('tailwindcss').Config} */
export default {
  content: [
    './ui/src/**/*.js',
    './ui/index.html',
    './ui/styles/**/*.css',
  ],
  safelist: [
    // zClass() 动态生成的 z-index 任意值类
    'z-[55]', 'z-[60]', 'z-[65]', 'z-[70]', 'z-[75]',
    'z-[80]', 'z-[90]', 'z-[95]', 'z-[99999]',
  ],
  theme: {
    extend: {
      /* ============================================================
       * Color Tokens — 映射 CSS 变量
       * ============================================================ */
      colors: {
        'surface-0': 'var(--surface-0)',
        'surface-1': 'var(--surface-1)',
        'surface-2': 'var(--surface-2)',
        'surface-3': 'var(--surface-3)',
        'surface-4': 'var(--surface-4)',
        'border-subtle': 'var(--border-subtle)',
        'border-default': 'var(--border-default)',
        'border-strong': 'var(--border-strong)',
        'text-primary': 'var(--text-primary)',
        'text-secondary': 'var(--text-secondary)',
        'text-tertiary': 'var(--text-tertiary)',
        'text-quaternary': 'var(--text-quaternary)',
        'ink': 'var(--ink)',
        'accent': '#38BDF8',
        'accent-hover': '#0EA5E9',
        'success': 'var(--success)',
        'warning': 'var(--warning)',
        'danger': 'var(--danger)',
        'info': 'var(--info)',
        'msg-user-bg': 'var(--msg-user-bg)',
        'msg-user-border': 'var(--msg-user-border)',
        'msg-assistant-bg': 'var(--msg-assistant-bg)',
        'msg-assistant-border': 'var(--msg-assistant-border)',
      },
      /* ============================================================
       * Spacing Tokens — 映射 CSS 变量
       * ============================================================ */
      spacing: {
        '0': 'var(--space-0)',
        '1': 'var(--space-1)',
        '2': 'var(--space-2)',
        '3': 'var(--space-3)',
        '4': 'var(--space-4)',
        '5': 'var(--space-5)',
        '6': 'var(--space-6)',
        '8': 'var(--space-8)',
        '10': 'var(--space-10)',
        '12': 'var(--space-12)',
      },
      /* ============================================================
       * Typography Tokens
       * ============================================================ */
      fontSize: {
        'xs': 'var(--text-xs)',
        'sm': 'var(--text-sm)',
        'base': 'var(--text-base)',
        'lg': 'var(--text-lg)',
      },
      lineHeight: {
        'tight': 'var(--leading-tight)',
        'normal': 'var(--leading-normal)',
      },
      fontFamily: {
        sans: ['-apple-system', '"SF Pro SC"', '"PingFang SC"', '"Segoe UI"', 'sans-serif'],
        mono: ['"SF Mono"', '"JetBrains Mono"', '"Fira Code"', 'monospace'],
      },
      /* ============================================================
       * Border Radius Tokens
       * ============================================================ */
      borderRadius: {
        'msg': 'var(--msg-radius)',
        'xs': '4px', 'sm': '8px', 'md': '12px', 'lg': '16px', 'xl': '20px', '2xl': '24px',
      },
      /* ============================================================
       * Transition Tokens
       * ============================================================ */
      transitionDuration: {
        'micro': 'var(--duration-micro)',
        'fast': 'var(--duration-fast)',
        'normal': 'var(--duration-normal)',
      },
      backgroundImage: {
        'dropdown-arrow': "url(\"data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='8' viewBox='0 0 12 8' fill='none'%3E%3Cpath d='M1 1L6 6L11 1' stroke='%2394A3B8' stroke-width='1.5' stroke-linecap='round' stroke-linejoin='round'/%3E%3C/svg%3E\")",
      },
      /* ============================================================
       * Keyframes — 动画注册
       * ============================================================ */
      keyframes: {
        fadeIn:     { '0%': { opacity: '0', transform: 'translateY(6px)' }, '100%': { opacity: '1', transform: 'none' } },
        scaleIn:    { '0%': { opacity: '0', transform: 'scale(0.96)' }, '100%': { opacity: '1', transform: 'scale(1)' } },
        slideUp:    { '0%': { opacity: '0', transform: 'translateY(100%)' }, '100%': { opacity: '1', transform: 'translateY(0)' } },
        messageIn:  { '0%': { opacity: '0', transform: 'translateY(12px) scale(0.98)' }, '100%': { opacity: '1', transform: 'none' } },
        spin:       { '0%': { transform: 'rotate(0deg)' }, '100%': { transform: 'rotate(360deg)' } },
        caretBlink: { '0%,50%': { opacity: '1' }, '51%,100%': { opacity: '0' } },
        panelIn:    { '0%': { opacity: '0', transform: 'translateY(4px)' }, '100%': { opacity: '1', transform: 'translateY(0)' } },
        editFadeIn: { '0%': { opacity: '0' }, '100%': { opacity: '1' } },
        citeFlash:  { '0%,100%': { background: 'rgba(var(--accent-rgb),0.3)' }, '50%': { background: 'rgba(var(--accent-rgb),0.5)' } },
      },
      /* ============================================================
       * Animation — animate-* 工具类
       * ============================================================ */
      animation: {
        'fade-in':     'fadeIn 0.25s ease-out',
        'scale-in':    'scaleIn 0.15s ease-out',
        'slide-up':    'slideUp 0.2s cubic-bezier(0.34,1.56,0.64,1)',
        'message-in':  'messageIn 0.4s cubic-bezier(0.34,1.56,0.64,1)',
        'spin':        'spin 0.8s linear infinite',
        'caret-blink': 'caretBlink 600ms infinite step-start',
        'panel-in':    'panelIn 0.15s ease-out',
        'edit-fade-in':'editFadeIn 0.2s ease-out',
        'cite-flash':  'citeFlash 0.6s ease',
        'followup-in': 'panelIn 0.3s ease-out',
      },
      boxShadow: {
        'float': '0 4px 12px rgba(0,0,0,0.15)',
        'pop': '0 8px 24px rgba(0,0,0,0.2)',
        'modal': '0 16px 48px rgba(0,0,0,0.25)',
        'glow': '0 0 0 3px rgba(56,189,248,0.25)',
      },
    },
  },
  plugins: [],
};
