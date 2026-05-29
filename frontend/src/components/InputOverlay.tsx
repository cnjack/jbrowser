import { useEffect, useRef } from 'react';
import { mapCoordinates } from '../utils/coordinates';
import type { BrowserInstance } from '../api/types';

export type InputPayload =
  | { type: 'click'; x: number; y: number; button: 'left' | 'middle' | 'right'; clickCount: number; modifiers: number }
  | { type: 'mousedown'; x: number; y: number; button: 'left' | 'middle' | 'right'; clickCount: number; modifiers: number }
  | { type: 'mouseup'; x: number; y: number; button: 'left' | 'middle' | 'right'; clickCount: number; modifiers: number }
  | { type: 'mousemove'; x: number; y: number; modifiers: number }
  | { type: 'wheel'; x: number; y: number; deltaX: number; deltaY: number; modifiers: number }
  | { type: 'keydown'; key: string; code: string; text: string; modifiers: number; keyCode: number }
  | { type: 'keyup'; key: string; code: string; modifiers: number; keyCode: number };

interface Props {
  browser: BrowserInstance;
  onInput: (payload: InputPayload) => void;
  previewSegment?: ArrayBuffer | null;
}

// Header layout: 1 byte type | 4 bytes stream_id | 8 bytes sequence | 8 bytes timestamp_ms = 21 bytes
const FRAME_HEADER_SIZE = 21;
const FRAME_TYPE_INIT = 0x01;

function stripHeader(buf: ArrayBuffer): { isInit: boolean; payload: ArrayBuffer } {
  const view = new DataView(buf);
  const frameType = view.getUint8(0);
  return {
    isInit: frameType === FRAME_TYPE_INIT,
    payload: buf.slice(FRAME_HEADER_SIZE),
  };
}

const KEY_CODE_MAP: Record<string, number> = {
  Backspace: 8, Tab: 9, Enter: 13, Escape: 27, Space: 32,
  PageUp: 33, PageDown: 34, End: 35, Home: 36,
  ArrowLeft: 37, ArrowUp: 38, ArrowRight: 39, ArrowDown: 40,
  Insert: 45, Delete: 46,
  F1: 112, F2: 113, F3: 114, F4: 115, F5: 116, F6: 117,
  F7: 118, F8: 119, F9: 120, F10: 121, F11: 122, F12: 123,
};

const BTN_MAP: Record<number, 'left' | 'middle' | 'right'> = {
  0: 'left', 1: 'middle', 2: 'right',
};

function getModifiers(e: MouseEvent | KeyboardEvent): number {
  return (e.altKey ? 1 : 0) | (e.ctrlKey ? 2 : 0) | (e.metaKey ? 4 : 0) | (e.shiftKey ? 8 : 0);
}

const PREVENT_DEFAULT_KEYS = new Set([
  'Tab', 'ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown',
  'Backspace', ' ', 'F1', 'F3', 'F5', 'F6',
]);

// Try each codec string in order until one is supported.
const CODEC_CANDIDATES = [
  'video/mp4; codecs="avc1.42E01E"',  // H.264 Constrained Baseline 3.0
  'video/mp4; codecs="avc1.42C01E"',  // H.264 Constrained Baseline 3.0 (alt flags)
  'video/mp4; codecs="avc1.42001E"',  // H.264 Baseline 3.0 (no constraint flags)
  'video/mp4; codecs="avc1.640028"',  // H.264 High 4.0 fallback
];

function pickCodec(): string | null {
  if (typeof MediaSource === 'undefined') return null;
  return CODEC_CANDIDATES.find((c) => MediaSource.isTypeSupported(c)) ?? null;
}

function drainQueue(sb: SourceBuffer, queue: ArrayBuffer[]) {
  if (sb.updating || queue.length === 0) return;
  const next = queue.shift()!;
  try { sb.appendBuffer(next); } catch { /* quota / abort – skip */ }
}

export function InputOverlay({ browser, onInput, previewSegment }: Props) {
  const divRef = useRef<HTMLDivElement>(null);
  const videoRef = useRef<HTMLVideoElement>(null);
  const msRef = useRef<MediaSource | null>(null);
  const sbRef = useRef<SourceBuffer | null>(null);
  // Segments received before MSE is ready are buffered here.
  const queueRef = useRef<ArrayBuffer[]>([]);

  // Initialise MSE once on mount.
  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    const codec = pickCodec();
    if (!codec) return;

    const ms = new MediaSource();
    msRef.current = ms;
    const objectUrl = URL.createObjectURL(ms);
    video.src = objectUrl;

    const onSourceOpen = () => {
      try {
        const sb = ms.addSourceBuffer(codec);
        sbRef.current = sb;
        sb.addEventListener('updateend', () => drainQueue(sb, queueRef.current));
        // Drain anything buffered while MSE was initialising.
        drainQueue(sb, queueRef.current);
      } catch (err) {
        console.error('[MSE] addSourceBuffer failed:', err);
      }
    };

    ms.addEventListener('sourceopen', onSourceOpen);

    // Autoplay: start as soon as the browser has buffered enough.
    const onCanPlay = () => { video.play().catch(() => {}); };
    video.addEventListener('canplay', onCanPlay);

    return () => {
      ms.removeEventListener('sourceopen', onSourceOpen);
      video.removeEventListener('canplay', onCanPlay);
      URL.revokeObjectURL(objectUrl);
      if (ms.readyState === 'open') {
        try { ms.endOfStream(); } catch { /* ignore */ }
      }
    };
  }, []);

  // Append incoming binary segment to MSE (or pre-buffer if not ready yet).
  useEffect(() => {
    if (!previewSegment) return;
    const { payload } = stripHeader(previewSegment);
    if (payload.byteLength === 0) return;

    const sb = sbRef.current;
    const ms = msRef.current;

    if (!sb || ms?.readyState !== 'open') {
      // MSE not ready yet — queue so we don't lose the init segment.
      queueRef.current.push(payload);
      // Keep queue bounded to avoid memory growth.
      if (queueRef.current.length > 60) queueRef.current.shift();
      return;
    }

    if (sb.updating) {
      queueRef.current.push(payload);
    } else {
      try { sb.appendBuffer(payload); } catch { /* quota / abort */ }
    }
  }, [previewSegment]);

  // Mouse + keyboard event listeners (all in one effect)
  useEffect(() => {
    const el = divRef.current;
    if (!el) return;

    const mapCoords = (clientX: number, clientY: number) => {
      const rect = el.getBoundingClientRect();
      return mapCoordinates(clientX, clientY, rect, browser.viewport_width, browser.viewport_height);
    };

    // --- Wheel ---
    const handleWheel = (e: WheelEvent) => {
      e.preventDefault();
      const point = mapCoords(e.clientX, e.clientY);
      const modifiers = getModifiers(e);
      onInput({ type: 'wheel', x: point.x, y: point.y, deltaX: e.deltaX, deltaY: e.deltaY, modifiers });
    };

    // --- Mouse buttons ---
    const handleMouseDown = (e: MouseEvent) => {
      e.preventDefault();
      el.focus();
      const point = mapCoords(e.clientX, e.clientY);
      const button = BTN_MAP[e.button] ?? 'left';
      const modifiers = getModifiers(e);
      onInput({ type: 'mousedown', x: point.x, y: point.y, button, clickCount: 1, modifiers });
    };

    const handleMouseUp = (e: MouseEvent) => {
      const point = mapCoords(e.clientX, e.clientY);
      const button = BTN_MAP[e.button] ?? 'left';
      const modifiers = getModifiers(e);
      onInput({ type: 'mouseup', x: point.x, y: point.y, button, clickCount: 1, modifiers });
    };

    let lastMoveTime = 0;
    const handleMouseMove = (e: MouseEvent) => {
      if (e.buttons === 0) return;
      const now = Date.now();
      if (now - lastMoveTime < 16) return; // ~60fps throttle
      lastMoveTime = now;
      const point = mapCoords(e.clientX, e.clientY);
      const modifiers = getModifiers(e);
      onInput({ type: 'mousemove', x: point.x, y: point.y, modifiers });
    };

    const handleDblClick = (e: MouseEvent) => {
      const point = mapCoords(e.clientX, e.clientY);
      const button = BTN_MAP[e.button] ?? 'left';
      const modifiers = getModifiers(e);
      onInput({ type: 'mousedown', x: point.x, y: point.y, button, clickCount: 2, modifiers });
      onInput({ type: 'mouseup', x: point.x, y: point.y, button, clickCount: 2, modifiers });
    };

    const handleContextMenu = (e: MouseEvent) => { e.preventDefault(); };

    // --- Keyboard ---
    const handleKeyDown = (e: KeyboardEvent) => {
      if (PREVENT_DEFAULT_KEYS.has(e.key)) e.preventDefault();
      const modifiers = getModifiers(e);
      const keyCode = KEY_CODE_MAP[e.code] ?? KEY_CODE_MAP[e.key] ?? 0;
      let text = '';
      if (e.key.length === 1 && !e.ctrlKey && !e.metaKey) text = e.key;
      else if (e.key === 'Enter') text = '\r';
      else if (e.key === 'Tab') text = '\t';
      onInput({ type: 'keydown', key: e.key, code: e.code, text, modifiers, keyCode });
    };

    const handleKeyUp = (e: KeyboardEvent) => {
      const modifiers = getModifiers(e);
      const keyCode = KEY_CODE_MAP[e.code] ?? KEY_CODE_MAP[e.key] ?? 0;
      onInput({ type: 'keyup', key: e.key, code: e.code, modifiers, keyCode });
    };

    el.addEventListener('wheel', handleWheel, { passive: false });
    el.addEventListener('mousedown', handleMouseDown);
    el.addEventListener('mouseup', handleMouseUp);
    el.addEventListener('mousemove', handleMouseMove);
    el.addEventListener('dblclick', handleDblClick);
    el.addEventListener('contextmenu', handleContextMenu);
    el.addEventListener('keydown', handleKeyDown);
    el.addEventListener('keyup', handleKeyUp);

    return () => {
      el.removeEventListener('wheel', handleWheel);
      el.removeEventListener('mousedown', handleMouseDown);
      el.removeEventListener('mouseup', handleMouseUp);
      el.removeEventListener('mousemove', handleMouseMove);
      el.removeEventListener('dblclick', handleDblClick);
      el.removeEventListener('contextmenu', handleContextMenu);
      el.removeEventListener('keydown', handleKeyDown);
      el.removeEventListener('keyup', handleKeyUp);
    };
  }, [browser.viewport_width, browser.viewport_height, onInput]);

  const msSupported = typeof MediaSource !== 'undefined';

  return (
    <div ref={divRef} className="preview-surface" tabIndex={0}>
      {msSupported ? (
        <video
          ref={videoRef}
          className="preview-video"
          muted
          playsInline
          style={{ width: '100%', height: '100%', objectFit: 'contain', display: 'block' }}
        />
      ) : (
        <div>
          <strong>Preview unavailable</strong>
          <span>MediaSource API not supported in this browser.</span>
        </div>
      )}
      {msSupported && !previewSegment && (
        <div className="preview-overlay-hint">
          <strong>Waiting for video stream…</strong>
          <span>Agent is starting Chrome and encoding the display.</span>
        </div>
      )}
    </div>
  );
}
