/**
 * EchoMind 语音输入模块 — SpeechRecognition 麦克风转写（REQ-RAG-034）。
 *
 * 基于 Web Speech API（SpeechRecognition / webkitSpeechRecognition），
 * 浏览器原生支持，零新增依赖。语言跟随当前 UI 语言（zh-CN / en-US）。
 * 不支持 Web Speech API 的 WebView 自动隐藏入口。
 *
 * 职责：
 * 1. 检测 Web Speech API 支持并初始化 recognition 实例
 * 2. 麦克风按钮点击：启动/停止语音识别
 * 3. 实时转写结果填入 #queryInput
 * 4. 录音中脉冲动画状态管理
 * 5. 错误处理（权限拒绝等）
 */

import { $ } from './utils.js';
import { toast, toastError } from './toast.js';
import { t, getLocale } from './i18n.js';

/** 当前 SpeechRecognition 实例（null 表示未初始化或不可用）。 */
let _recognition = null;

/** 是否正在录音。 */
let _isRecording = false;

/**
 * 获取当前语音识别语言代码。
 * 跟随 UI 语言：zh-CN → zh-CN，其他 → en-US。
 * @returns {string} BCP 47 语言标签
 */
function getRecognitionLang() {
  const locale = getLocale();
  return locale === 'zh-CN' ? 'zh-CN' : 'en-US';
}

/**
 * 初始化语音输入：检测 Web Speech API 支持，绑定麦克风按钮事件。
 * 在 main.js boot() 中调用。
 */
export function initVoiceInput() {
  const micBtn = $('micBtn');
  if (!micBtn) return;

  // 检测 Web Speech API 支持
  const SpeechRecognitionClass = window.SpeechRecognition || window.webkitSpeechRecognition;
  if (!SpeechRecognitionClass) {
    // 不支持：隐藏麦克风按钮（优雅降级，AC-4）
    micBtn.style.display = 'none';
    return;
  }

  // 创建 recognition 实例
  try {
    _recognition = new SpeechRecognitionClass();
  } catch (e) {
    micBtn.style.display = 'none';
    return;
  }

  _recognition.lang = getRecognitionLang();
  _recognition.continuous = false;
  _recognition.interimResults = true;
  _recognition.maxAlternatives = 1;

  _recognition.onresult = (event) => {
    const input = $('queryInput');
    if (!input) return;
    let transcript = '';
    for (let i = event.resultIndex; i < event.results.length; i++) {
      const result = event.results[i];
      if (result && result[0]) {
        transcript += result[0].transcript;
      }
    }
    if (transcript) {
      input.value = transcript;
      // 触发 input 事件，让 slash command / doc mention 检测生效
      input.dispatchEvent(new Event('input', { bubbles: true }));
    }
  };

  _recognition.onerror = (event) => {
    const error = event.error || '';
    if (error === 'not-allowed' || error === 'service-not-allowed') {
      toastError(t('voice.permission_denied'));
    } else if (error === 'no-speech') {
      // 无语音输入，静默处理
    } else if (error === 'audio-capture') {
      toastError(t('voice.permission_denied'));
    } else {
      toast(t('voice.not_supported'), 'warning');
    }
  };

  _recognition.onend = () => {
    _isRecording = false;
    updateMicButtonState();
  };

  _recognition.onstart = () => {
    _isRecording = true;
    // 录音开始时更新语言（用户可能切换了 UI 语言）
    _recognition.lang = getRecognitionLang();
    updateMicButtonState();
  };

  // 绑定点击事件
  micBtn.addEventListener('click', toggleRecording);
}

/**
 * 切换录音状态：未录音 → 启动，录音中 → 停止。
 */
function toggleRecording() {
  if (!_recognition) return;
  if (_isRecording) {
    try {
      _recognition.stop();
    } catch (_) {
      // 忽略 stop 异常
    }
    _isRecording = false;
    updateMicButtonState();
  } else {
    try {
      _recognition.start();
    } catch (e) {
      // start 可能抛 InvalidStateError（已 started），静默忽略
    }
  }
}

/**
 * 更新麦克风按钮的视觉状态（录音中 / 空闲）。
 */
function updateMicButtonState() {
  const micBtn = $('micBtn');
  if (!micBtn) return;
  if (_isRecording) {
    micBtn.classList.add('recording');
    micBtn.title = t('voice.stop_recording');
    micBtn.setAttribute('aria-label', t('voice.stop_recording'));
  } else {
    micBtn.classList.remove('recording');
    micBtn.title = t('voice.start_recording');
    micBtn.setAttribute('aria-label', t('voice.start_recording'));
  }
}

/**
 * 停止录音（供外部调用，如发送消息时自动停止）。
 */
export function stopRecording() {
  if (_isRecording && _recognition) {
    try {
      _recognition.stop();
    } catch (_) {
      // 忽略
    }
    _isRecording = false;
    updateMicButtonState();
  }
}

/**
 * 停止所有 TTS 朗读（供新对话/页面卸载时调用）。
 * 委托给 tts 模块的 cleanup 函数。
 */
export function stopAllTTS() {
  if (typeof window.speechSynthesis !== 'undefined' && window.speechSynthesis) {
    try {
      window.speechSynthesis.cancel();
    } catch (_) {
      // 忽略
    }
  }
  // 重置所有 TTS 按钮状态
  document.querySelectorAll('.tts-btn.speaking').forEach((btn) => {
    btn.classList.remove('speaking');
    btn.title = t('voice.listen');
    btn.setAttribute('aria-label', t('voice.listen'));
    const svg = btn.querySelector('svg');
    if (svg) {
      svg.innerHTML = '<path d="M11 5L6 9H2v6h4l5 4V5z"/><path d="M15.54 8.46a5 5 0 0 1 0 7.07"/><path d="M19.07 4.93a10 10 0 0 1 0 14.14"/>';
    }
  });
}

/**
 * 获取 TTS 语速设置（从 localStorage 读取）。
 * @returns {number} 语速（0.5~2.0，默认 1.0）
 */
export function getVoiceRate() {
  try {
    const rate = parseFloat(localStorage.getItem('voice.rate') || '1.0');
    if (isNaN(rate) || rate < 0.5 || rate > 2.0) return 1.0;
    return rate;
  } catch (_) {
    return 1.0;
  }
}

/**
 * 为指定 AI 消息块创建 TTS 朗读按钮。
 * 在 chat-render.js showMsgActions() 中调用。
 *
 * @param {HTMLElement} blockEl - AI 消息块根元素
 * @param {string} rawMarkdown - 原始 Markdown 文本
 * @returns {HTMLElement|null} 朗读按钮元素（不支持 TTS 时返回 null）
 */
export function createTtsButton(blockEl, rawMarkdown) {
  // 检测 speechSynthesis 支持
  if (typeof window.speechSynthesis === 'undefined' || !window.speechSynthesis) {
    return null;
  }

  const btn = document.createElement('button');
  btn.className = 'tts-btn msg-action-btn p-1.5 rounded-md text-text-secondary hover:text-accent hover:bg-accent/10 transition-colors duration-fast';
  btn.title = t('voice.listen');
  btn.setAttribute('aria-label', t('voice.listen'));
  btn.innerHTML = '<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 5L6 9H2v6h4l5 4V5z"/><path d="M15.54 8.46a5 5 0 0 1 0 7.07"/><path d="M19.07 4.93a10 10 0 0 1 0 14.14"/></svg>';

  btn.onclick = () => {
    if (btn.classList.contains('speaking')) {
      // 朗读中：停止
      if (window.speechSynthesis) {
        try {
          window.speechSynthesis.cancel();
        } catch (_) {
          // 忽略
        }
      }
      btn.classList.remove('speaking');
      btn.title = t('voice.listen');
      btn.setAttribute('aria-label', t('voice.listen'));
      const svg = btn.querySelector('svg');
      if (svg) {
        svg.innerHTML = '<path d="M11 5L6 9H2v6h4l5 4V5z"/><path d="M15.54 8.46a5 5 0 0 1 0 7.07"/><path d="M19.07 4.93a10 10 0 0 1 0 14.14"/>';
      }
    } else {
      // 开始朗读
      const mdEl = blockEl.querySelector('.md');
      let plainText;
      if (mdEl) {
        const clone = mdEl.cloneNode(true);
        clone.querySelectorAll('.code-header, .copy-btn, .code-lang').forEach((el) => el.remove());
        plainText = clone.textContent || '';
      } else {
        plainText = rawMarkdown;
      }
      plainText = plainText.trim();
      if (!plainText) return;

      // 停止其他正在进行的朗读
      if (window.speechSynthesis) {
        try {
          window.speechSynthesis.cancel();
        } catch (_) {
          // 忽略
        }
      }
      // 重置其他按钮状态
      document.querySelectorAll('.tts-btn.speaking').forEach((b) => {
        b.classList.remove('speaking');
        b.title = t('voice.listen');
        b.setAttribute('aria-label', t('voice.listen'));
        const svgEl = b.querySelector('svg');
        if (svgEl) {
          svgEl.innerHTML = '<path d="M11 5L6 9H2v6h4l5 4V5z"/><path d="M15.54 8.46a5 5 0 0 1 0 7.07"/><path d="M19.07 4.93a10 10 0 0 1 0 14.14"/>';
        }
      });

      const utterance = new SpeechSynthesisUtterance(plainText);
      utterance.lang = getLocale() === 'zh-CN' ? 'zh-CN' : 'en-US';
      utterance.rate = getVoiceRate();

      utterance.onend = () => {
        btn.classList.remove('speaking');
        btn.title = t('voice.listen');
        btn.setAttribute('aria-label', t('voice.listen'));
        const svg = btn.querySelector('svg');
        if (svg) {
          svg.innerHTML = '<path d="M11 5L6 9H2v6h4l5 4V5z"/><path d="M15.54 8.46a5 5 0 0 1 0 7.07"/><path d="M19.07 4.93a10 10 0 0 1 0 14.14"/>';
        }
      };

      utterance.onerror = () => {
        btn.classList.remove('speaking');
        btn.title = t('voice.listen');
        btn.setAttribute('aria-label', t('voice.listen'));
        const svg = btn.querySelector('svg');
        if (svg) {
          svg.innerHTML = '<path d="M11 5L6 9H2v6h4l5 4V5z"/><path d="M15.54 8.46a5 5 0 0 1 0 7.07"/><path d="M19.07 4.93a10 10 0 0 1 0 14.14"/>';
        }
      };

      if (window.speechSynthesis) {
        window.speechSynthesis.speak(utterance);
      }

      btn.classList.add('speaking');
      btn.title = t('voice.stop_listening');
      btn.setAttribute('aria-label', t('voice.stop_listening'));
      const svg = btn.querySelector('svg');
      if (svg) {
        svg.innerHTML = '<rect x="6" y="6" width="12" height="12" rx="2"/>';
      }
    }
  };

  return btn;
}
