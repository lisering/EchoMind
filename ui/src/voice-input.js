/**
 * EchoMind 语音输入模块 — 桌面应用录音 + 静音检测 + OpenAI Whisper API 转写（REQ-RAG-034）。
 *
 * ## 实现方案
 *
 * 桌面应用使用 `navigator.mediaDevices.getUserMedia` + `MediaRecorder` 录制音频，
 * 通过 Web Audio API 的 `AnalyserNode` 实时检测音频电平并可视化，
 * 说话结束后静音 2 秒自动停止录音，
 * 通过 Tauri IPC 发送到 Rust 侧调用 OpenAI 兼容 Whisper API 转写为文本。
 *
 * ## 状态机
 *
 * ```
 * IDLE → REQUESTING_PERMISSION → RECORDING → TRANSCRIBING → DONE → IDLE
 *                                ↑                                    │
 *                                └────── RETRY ←─── (失败时) ─────────┘
 * ```
 *
 * ## 数据流
 *
 * ```
 * 用户点击 #micBtn
 *   → getUserMedia({ audio: true })        // 请求麦克风权限
 *   → MediaRecorder.start()                 // 开始录音
 *   → AnalyserNode 实时检测音频电平 → 可视化
 *   → 用户说话 → 标记 hasSpeechStarted → 显示录音计时器
 *   → 用户停止说话 → 静音 2s → 自动停止录音
 *   → MediaRecorder.stop()
 *   → 收集 audio chunks → Blob → ArrayBuffer
 *   → invoke('transcribe_audio', { audioData, mimeType })
 *   → Rust: POST {base_url}/v1/audio/transcriptions (multipart)
 *   → 返回转写文本 → 填入 #queryInput
 * ```
 */

import { $ } from './utils.js';
import { toast, toastError } from './toast.js';
import { t, getLocale } from './i18n.js';

// ============================================================
// 状态变量
// ============================================================

/** @enum {string} 录音状态 */
const RecordingState = {
  IDLE: 'idle',
  REQUESTING: 'requesting',
  RECORDING: 'recording',
  TRANSCRIBING: 'transcribing',
};

let _state = RecordingState.IDLE;
let _mediaRecorder = null;
let _audioChunks = [];
let _mediaStream = null;
let _autoStopTimer = null;
let _mimeType = 'audio/webm';

// 静音检测相关
let _audioContext = null;
let _analyser = null;
let _silenceCheckTimer = null;
let _hasSpeechStarted = false;
let _silenceStartTime = null;

// 录音计时器
let _recordingTimer = null;
let _recordingStartTime = 0;

// 音频电平可视化
/** @type {Element[]} */
let _levelBars = [];
let _levelAnimTimer = null;

// 常量
const AUTO_STOP_MS = 60000;         // 最大录音时长 60s
const SILENCE_CHECK_INTERVAL = 200;  // 静音检测间隔 ms
const SILENCE_THRESHOLD = 0.01;      // 静音阈值（RMS < 此值视为静音）
const SILENCE_DURATION_MS = 2000;    // 说话后静音多久自动停止
const INITIAL_TIMEOUT_MS = 8000;     // 等待用户开始说话的最长时间
const NUM_LEVEL_BARS = 5;            // 音频电平条数量

// ============================================================
// 初始化
// ============================================================

export function initVoiceInput() {
  const micBtn = $('micBtn');
  if (!micBtn) return;

  if (!navigator.mediaDevices || !navigator.mediaDevices.getUserMedia) {
    micBtn.style.display = 'none';
    return;
  }

  if (typeof MediaRecorder === 'undefined') {
    micBtn.style.display = 'none';
    return;
  }

  const candidates = ['audio/webm', 'audio/ogg', 'audio/mp4'];
  _mimeType = candidates.find((type) => MediaRecorder.isTypeSupported(type)) || 'audio/webm';

  micBtn.addEventListener('click', toggleRecording);
}

// ============================================================
// 录音控制
// ============================================================

async function toggleRecording() {
  if (_state === RecordingState.IDLE) {
    await startRecording();
  } else if (_state === RecordingState.RECORDING) {
    stopRecording();
  }
  // TRANSCRIBING 状态忽略点击
}

async function startRecording() {
  _setState(RecordingState.REQUESTING);

  try {
    _mediaStream = await navigator.mediaDevices.getUserMedia({
      audio: {
        echoCancellation: true,
        noiseSuppression: true,
        autoGainControl: true,
      },
    });
  } catch (err) {
    _setState(RecordingState.IDLE);
    const name = err?.name || '';
    if (name === 'NotAllowedError' || name === 'SecurityError') {
      toastError(t('voice.permission_denied'));
    } else if (name === 'NotFoundError' || name === 'DevicesNotFoundError') {
      toastError(t('voice.no_microphone'));
    } else {
      toastError(t('voice.start_failed'));
    }
    return;
  }

  // 创建 MediaRecorder
  try {
    _mediaRecorder = new MediaRecorder(_mediaStream, {
      mimeType: _mimeType,
      audioBitsPerSecond: 128000, // 128kbps 足够语音识别
    });
  } catch {
    try {
      _mediaRecorder = new MediaRecorder(_mediaStream);
      _mimeType = _mediaRecorder.mimeType || 'audio/webm';
    } catch {
      _setState(RecordingState.IDLE);
      cleanupStream();
      toastError(t('voice.start_failed'));
      return;
    }
  }

  _audioChunks = [];

  _mediaRecorder.ondataavailable = (event) => {
    if (event.data && event.data.size > 0) {
      _audioChunks.push(event.data);
    }
  };

  _mediaRecorder.onstop = async () => {
    const blob = new Blob(_audioChunks, { type: _mimeType });
    _audioChunks = [];
    cleanupStream();
    stopSilenceDetection();
    stopRecordingTimer();

    if (blob.size < 10) {
      _setState(RecordingState.IDLE);
      toast(t('voice.no_speech_detected'), 'warning');
      return;
    }

    _setState(RecordingState.TRANSCRIBING);

    try {
      const arrayBuffer = await blob.arrayBuffer();
      const audioData = Array.from(new Uint8Array(arrayBuffer));

      const { invoke } = await import('./ipc.js');
      const text = await invoke('transcribe_audio', {
        audioData,
        mimeType: _mimeType,
      });

      const input = $('queryInput');
      if (input && text) {
        // 追加模式：如果输入框已有文本，在末尾追加
        const existing = input.value.trim();
        if (existing) {
          input.value = existing + ' ' + text;
        } else {
          input.value = text;
        }
        input.dispatchEvent(new Event('input', { bubbles: true }));
        input.focus();
        // 移动光标到末尾
        input.setSelectionRange(input.value.length, input.value.length);
      }
    } catch (err) {
      const msg = String(err?.message || err || '');
      console.error('[voice-input] 转写失败:', msg);

      if (msg.includes('VALIDATION')) {
        toastError(t('voice.no_api_config'));
      } else if (msg.includes('空文本') || msg.includes('空语音')) {
        toast(t('voice.no_speech_detected'), 'warning');
      } else if (msg.includes('超时')) {
        toastError(t('voice.timeout'));
      } else if (msg.includes('无法连接')) {
        toastError(t('voice.connection_failed'));
      } else if (msg.includes('404') || msg.includes('Not Found') || msg.includes('not_found')) {
        // STT 端点不存在 — LLM 提供商不支持语音转写
        toastError(t('voice.stt_endpoint_not_found'));
      } else if (msg.includes('401') || msg.includes('Unauthorized') || msg.includes('invalid_api_key')) {
        toastError(t('voice.stt_unauthorized'));
      } else if (msg.includes('400') || msg.includes('Bad Request')) {
        toastError(t('voice.stt_bad_request'));
      } else {
        // 显示截断的实际错误信息，而非笼统的"转写失败"
        const detail = msg.length > 120 ? msg.slice(0, 120) + '…' : msg;
        toastError(`${t('voice.transcribe_failed')}（${detail}）`);
      }
    } finally {
      _setState(RecordingState.IDLE);
    }
  };

  _mediaRecorder.onerror = () => {
    cleanupStream();
    stopSilenceDetection();
    stopRecordingTimer();
    _setState(RecordingState.IDLE);
    toastError(t('voice.start_failed'));
  };

  // 开始录音
  _mediaRecorder.start();
  _hasSpeechStarted = false;
  _silenceStartTime = null;
  _setState(RecordingState.RECORDING);

  // 启动静音检测 + 计时器 + 可视化
  startSilenceDetection();
  startRecordingTimer();
  showRecordingOverlay();

  // 最大录音时长兜底
  clearAutoStopTimer();
  _autoStopTimer = setTimeout(() => {
    stopRecording();
  }, AUTO_STOP_MS);
}

export function stopRecording() {
  clearAutoStopTimer();
  stopSilenceDetection();
  stopRecordingTimer();
  hideRecordingOverlay();
  if (_mediaRecorder && _mediaRecorder.state !== 'inactive') {
    try {
      _mediaRecorder.stop();
    } catch { /* ignore */ }
  }
  // 不立即设置 IDLE — onstop 回调会处理状态转换（TRANSCRIBING → IDLE）
  // 仅更新按钮状态（变为非录音态）
  const micBtn = $('micBtn');
  if (micBtn) {
    micBtn.classList.remove('recording');
  }
}

// ============================================================
// 状态管理
// ============================================================

function _setState(newState) {
  _state = newState;
  updateMicButtonState();
  updateRecordingOverlay();
}

// ============================================================
// 静音检测（Web Audio API AnalyserNode）
// ============================================================

function startSilenceDetection() {
  try {
    _audioContext = new (window.AudioContext || window.webkitAudioContext)();
    const source = _audioContext.createMediaStreamSource(_mediaStream);
    _analyser = _audioContext.createAnalyser();
    _analyser.fftSize = 512;
    _analyser.smoothingTimeConstant = 0.5;
    source.connect(_analyser);
  } catch {
    // AudioContext 不可用，静音检测降级：不自动停止，依赖手动点击或 60s 超时
    return;
  }

  const buffer = new Uint8Array(_analyser.fftSize);
  let initialWaitStart = Date.now();

  _silenceCheckTimer = setInterval(() => {
    if (!_analyser || _state !== RecordingState.RECORDING) {
      stopSilenceDetection();
      return;
    }

    _analyser.getByteTimeDomainData(buffer);

    // 计算 RMS（均方根）音量
    let sum = 0;
    for (let i = 0; i < buffer.length; i++) {
      const v = (buffer[i] - 128) / 128;
      sum += v * v;
    }
    const rms = Math.sqrt(sum / buffer.length);

    // 更新音频电平可视化
    updateLevelBars(rms);

    if (rms > SILENCE_THRESHOLD) {
      // 检测到语音
      _hasSpeechStarted = true;
      _silenceStartTime = null;
    } else {
      // 静音
      if (_hasSpeechStarted) {
        // 说话后进入静音
        if (_silenceStartTime === null) {
          _silenceStartTime = Date.now();
        } else if (Date.now() - _silenceStartTime > SILENCE_DURATION_MS) {
          // 静音超过 2s，自动停止
          stopRecording();
        }
      } else {
        // 还没开始说话
        if (Date.now() - initialWaitStart > INITIAL_TIMEOUT_MS) {
          // 等待 8s 仍未说话，自动停止（避免无限录音）
          stopRecording();
        }
      }
    }
  }, SILENCE_CHECK_INTERVAL);
}

function stopSilenceDetection() {
  if (_silenceCheckTimer !== null) {
    clearInterval(_silenceCheckTimer);
    _silenceCheckTimer = null;
  }
  if (_audioContext) {
    try { _audioContext.close(); } catch { /* ignore */ }
    _audioContext = null;
  }
  _analyser = null;
  _hasSpeechStarted = false;
  _silenceStartTime = null;
}

// ============================================================
// 录音计时器
// ============================================================

function startRecordingTimer() {
  _recordingStartTime = Date.now();
  _recordingTimer = setInterval(() => {
    const elapsed = Math.floor((Date.now() - _recordingStartTime) / 1000);
    const mins = Math.floor(elapsed / 60);
    const secs = elapsed % 60;
    const display = `${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;

    const timerEl = document.getElementById('recordingTimer');
    if (timerEl) {
      timerEl.textContent = display;
    }

    // 接近最大时长时变色警告
    if (elapsed >= 50) {
      const overlay = document.getElementById('recordingOverlay');
      if (overlay) overlay.classList.add('recording-warning');
    }
  }, 1000);
}

function stopRecordingTimer() {
  if (_recordingTimer !== null) {
    clearInterval(_recordingTimer);
    _recordingTimer = null;
  }
}

// ============================================================
// 录音遮罩 + 音频电平可视化
// ============================================================

function showRecordingOverlay() {
  let overlay = document.getElementById('recordingOverlay');
  if (!overlay) return; // E2E mock 环境可能没有

  overlay.classList.remove('hidden');
  overlay.classList.remove('recording-warning');

  // 初始化电平条
  _levelBars = Array.from(overlay.querySelectorAll('.level-bar'));
}

function hideRecordingOverlay() {
  const overlay = document.getElementById('recordingOverlay');
  if (overlay) {
    overlay.classList.add('hidden');
    overlay.classList.remove('recording-warning');
  }
  // 重置电平条
  _levelBars = [];
}

function updateRecordingOverlay() {
  const overlay = document.getElementById('recordingOverlay');
  if (!overlay) return;

  if (_state === RecordingState.RECORDING) {
    overlay.classList.remove('hidden');
  } else if (_state === RecordingState.TRANSCRIBING) {
    overlay.classList.add('hidden');
  }
}

function updateLevelBars(rms) {
  if (!_levelBars || _levelBars.length === 0) return;

  // 将 RMS (0~0.5 典型范围) 映射到 0~NUM_LEVEL_BARS
  const normalized = Math.min(rms / 0.15, 1.0);
  const activeBars = Math.round(normalized * NUM_LEVEL_BARS);

  _levelBars.forEach((bar, i) => {
    if (i < activeBars) {
      bar.classList.add('active');
    } else {
      bar.classList.remove('active');
    }
  });
}

// ============================================================
// 辅助函数
// ============================================================

function cleanupStream() {
  if (_mediaStream) {
    _mediaStream.getTracks().forEach((track) => track.stop());
    _mediaStream = null;
  }
  _mediaRecorder = null;
}

function clearAutoStopTimer() {
  if (_autoStopTimer !== null) {
    clearTimeout(_autoStopTimer);
    _autoStopTimer = null;
  }
}

function updateMicButtonState() {
  const micBtn = $('micBtn');
  if (!micBtn) return;

  if (_state === RecordingState.RECORDING) {
    micBtn.classList.add('recording');
    micBtn.classList.remove('transcribing');
    micBtn.title = t('voice.stop_recording');
    micBtn.setAttribute('aria-label', t('voice.stop_recording'));
  } else if (_state === RecordingState.TRANSCRIBING) {
    micBtn.classList.remove('recording');
    micBtn.classList.add('transcribing');
    micBtn.title = t('voice.transcribing');
    micBtn.setAttribute('aria-label', t('voice.transcribing'));
  } else if (_state === RecordingState.REQUESTING) {
    micBtn.classList.add('transcribing');
    micBtn.title = t('voice.requesting_permission');
    micBtn.setAttribute('aria-label', t('voice.requesting_permission'));
  } else {
    micBtn.classList.remove('recording', 'transcribing');
    micBtn.title = t('voice.start_recording');
    micBtn.setAttribute('aria-label', t('voice.start_recording'));
  }
}

// ============================================================
// TTS 朗读（语音合成）— 与录音无关，保持原有实现
// ============================================================

export function stopAllTTS() {
  if (typeof window.speechSynthesis !== 'undefined' && window.speechSynthesis) {
    try { window.speechSynthesis.cancel(); } catch { /* ignore */ }
  }
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

export function getVoiceRate() {
  try {
    const rate = parseFloat(localStorage.getItem('voice.rate') || '1.0');
    if (isNaN(rate) || rate < 0.5 || rate > 2.0) return 1.0;
    return rate;
  } catch {
    return 1.0;
  }
}

export function createTtsButton(blockEl, rawMarkdown) {
  if (typeof window.speechSynthesis === 'undefined' || !window.speechSynthesis) {
    return null;
  }

  const btn = document.createElement('button');
  btn.className = 'tts-btn msg-action-btn';
  btn.title = t('voice.listen');
  btn.setAttribute('aria-label', t('voice.listen'));
  btn.innerHTML = '<svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M11 5L6 9H2v6h4l5 4V5z"/><path d="M15.54 8.46a5 5 0 0 1 0 7.07"/><path d="M19.07 4.93a10 10 0 0 1 0 14.14"/></svg>';

  btn.onclick = () => {
    if (btn.classList.contains('speaking')) {
      if (window.speechSynthesis) {
        try { window.speechSynthesis.cancel(); } catch { /* ignore */ }
      }
      btn.classList.remove('speaking');
      btn.title = t('voice.listen');
      btn.setAttribute('aria-label', t('voice.listen'));
      const svg = btn.querySelector('svg');
      if (svg) {
        svg.innerHTML = '<path d="M11 5L6 9H2v6h4l5 4V5z"/><path d="M15.54 8.46a5 5 0 0 1 0 7.07"/><path d="M19.07 4.93a10 10 0 0 1 0 14.14"/>';
      }
    } else {
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

      if (window.speechSynthesis) {
        try { window.speechSynthesis.cancel(); } catch { /* ignore */ }
      }
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
      utterance.lang = getLocale() === 'zh-CN' ? 'zh-CN' : getLocale() === 'ja' ? 'ja-JP' : 'en-US';
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
