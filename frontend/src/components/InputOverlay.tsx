import { useEffect, useRef, useState } from 'react';
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
  browserStatus?: string;
  onInput: (payload: InputPayload) => void;
  previewSegment?: ArrayBuffer | null;
  previewEnded?: boolean;
}

// Header layout: 1 byte type | 4 bytes stream_id | 8 bytes sequence | 8 bytes timestamp_ms = 21 bytes
const FRAME_HEADER_SIZE = 21;
const FRAME_TYPE_JPEG = 0x03;

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

export function InputOverlay({ browser, browserStatus, onInput, previewSegment, previewEnded }: Props) {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const wrapRef = useRef<HTMLDivElement>(null);
  const imgPoolRef = useRef<HTMLImageElement[]>([]);
  const [ripples, setRipples] = useState<{ id: number; x: number; y: number }[]>([]);
  const rippleId = useRef(0);
  // Keep latest browserStatus in a ref so event handlers can check it without
  // needing to re-register (which would change the useEffect deps).
  const browserStatusRef = useRef(browserStatus);
  browserStatusRef.current = browserStatus;
  // Virtual cursor: tracks the mapped remote position and shows it on the canvas
  const [virtualCursor, setVirtualCursor] = useState<{ x: number; y: number; visible: boolean }>({ x: 0, y: 0, visible: false });

  // Maintain correct aspect-ratio CSS dimensions so that mapCoordinates stays accurate.
  useEffect(() => {
    const wrap = wrapRef.current;
    const canvas = canvasRef.current;
    if (!wrap || !canvas) return;

    const fit = () => {
      // Use actual canvas pixel dimensions (updated when frames arrive)
      const vw = canvas.width || browser.viewport_width;
      const vh = canvas.height || browser.viewport_height;
      const scale = Math.min(wrap.clientWidth / vw, wrap.clientHeight / vh);
      canvas.style.width  = Math.floor(vw * scale) + 'px';
      canvas.style.height = Math.floor(vh * scale) + 'px';
    };

    fit();
    const ro = new ResizeObserver(fit);
    ro.observe(wrap);
    return () => ro.disconnect();
  }, [browser.viewport_width, browser.viewport_height]);

  // Render incoming JPEG frame to canvas using image pool
  useEffect(() => {
    if (!previewSegment || previewSegment.byteLength <= FRAME_HEADER_SIZE) return;

    const view = new DataView(previewSegment);
    if (view.getUint8(0) !== FRAME_TYPE_JPEG) return;

    const jpeg = previewSegment.slice(FRAME_HEADER_SIZE);
    if (jpeg.byteLength === 0) return;

    const canvas = canvasRef.current;
    const ctx = canvas?.getContext('2d');
    if (!canvas || !ctx) return;

    const pool = imgPoolRef.current;
    const img = pool.pop() || new Image();
    const blob = new Blob([jpeg], { type: 'image/jpeg' });
    const url = URL.createObjectURL(blob);
    img.onload = () => {
      // If the actual frame dimensions differ from the canvas pixel dimensions,
      // update the canvas to match. This keeps coordinate mapping accurate even
      // when Chrome's real viewport doesn't match the assumed dimensions.
      if (canvas.width !== img.naturalWidth || canvas.height !== img.naturalHeight) {
        canvas.width = img.naturalWidth;
        canvas.height = img.naturalHeight;
      }
      ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
      URL.revokeObjectURL(url);
      pool.push(img);
    };
    img.src = url;
  }, [previewSegment]);

  // Mouse + keyboard event listeners
  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const mapCoords = (clientX: number, clientY: number) => {
      const rect = canvas.getBoundingClientRect();
      // Use canvas intrinsic pixel dimensions (which track the actual frame size)
      // instead of browser.viewport_* to stay accurate when Chrome's real viewport
      // differs from the declared dimensions.
      return mapCoordinates(clientX, clientY, rect, canvas.width, canvas.height);
    };

    const handleWheel = (e: WheelEvent) => {
      if (browserStatusRef.current === 'restarting') return;
      e.preventDefault();
      const point = mapCoords(e.clientX, e.clientY);
      onInput({ type: 'wheel', x: point.x, y: point.y, deltaX: e.deltaX, deltaY: e.deltaY, modifiers: getModifiers(e) });
    };

    const handleMouseDown = (e: MouseEvent) => {
      if (browserStatusRef.current === 'restarting') return;
      e.preventDefault();
      canvas.focus();
      // Ripple position relative to canvas-wrap (not canvas), so it stays aligned
      // even when canvas is centered/letterboxed inside the wrap.
      const wrapRect = wrapRef.current!.getBoundingClientRect();
      const cx = e.clientX - wrapRect.left;
      const cy = e.clientY - wrapRect.top;
      const id = ++rippleId.current;
      setRipples((prev) => [...prev, { id, x: cx, y: cy }]);
      setTimeout(() => setRipples((prev) => prev.filter((r) => r.id !== id)), 600);
      const point = mapCoords(e.clientX, e.clientY);
      const button = BTN_MAP[e.button] ?? 'left';
      onInput({ type: 'mousedown', x: point.x, y: point.y, button, clickCount: 1, modifiers: getModifiers(e) });
    };

    const handleMouseUp = (e: MouseEvent) => {
      if (browserStatusRef.current === 'restarting') return;
      const point = mapCoords(e.clientX, e.clientY);
      const button = BTN_MAP[e.button] ?? 'left';
      onInput({ type: 'mouseup', x: point.x, y: point.y, button, clickCount: 1, modifiers: getModifiers(e) });
    };

    const handleMouseMove = (e: MouseEvent) => {
      if (browserStatusRef.current === 'restarting') return;
      const point = mapCoords(e.clientX, e.clientY);
      // Update virtual cursor: convert remote coords back to canvas-local CSS pixels,
      // offset by canvas position within wrap so it stays aligned when letterboxed.
      const rect = canvas.getBoundingClientRect();
      const wrapRect = wrapRef.current!.getBoundingClientRect();
      setVirtualCursor({
        x: (point.x / canvas.width) * rect.width + (rect.left - wrapRect.left),
        y: (point.y / canvas.height) * rect.height + (rect.top - wrapRect.top),
        visible: true,
      });
      onInput({ type: 'mousemove', x: point.x, y: point.y, modifiers: getModifiers(e) });
    };

    const handleMouseLeave = () => {
      setVirtualCursor((c) => ({ ...c, visible: false }));
    };

    const handleDblClick = (e: MouseEvent) => {
      if (browserStatusRef.current === 'restarting') return;
      const point = mapCoords(e.clientX, e.clientY);
      const button = BTN_MAP[e.button] ?? 'left';
      const modifiers = getModifiers(e);
      onInput({ type: 'mousedown', x: point.x, y: point.y, button, clickCount: 2, modifiers });
      onInput({ type: 'mouseup', x: point.x, y: point.y, button, clickCount: 2, modifiers });
    };

    const handleContextMenu = (e: MouseEvent) => { e.preventDefault(); };

    const handleKeyDown = (e: KeyboardEvent) => {
      if (browserStatusRef.current === 'restarting') return;
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
      if (browserStatusRef.current === 'restarting') return;
      const modifiers = getModifiers(e);
      const keyCode = KEY_CODE_MAP[e.code] ?? KEY_CODE_MAP[e.key] ?? 0;
      onInput({ type: 'keyup', key: e.key, code: e.code, modifiers, keyCode });
    };

    canvas.addEventListener('wheel', handleWheel, { passive: false });
    canvas.addEventListener('mousedown', handleMouseDown);
    canvas.addEventListener('mouseup', handleMouseUp);
    canvas.addEventListener('mousemove', handleMouseMove);
    canvas.addEventListener('dblclick', handleDblClick);
    canvas.addEventListener('contextmenu', handleContextMenu);
    canvas.addEventListener('keydown', handleKeyDown);
    canvas.addEventListener('keyup', handleKeyUp);
    canvas.addEventListener('mouseleave', handleMouseLeave);

    return () => {
      canvas.removeEventListener('wheel', handleWheel);
      canvas.removeEventListener('mousedown', handleMouseDown);
      canvas.removeEventListener('mouseup', handleMouseUp);
      canvas.removeEventListener('mousemove', handleMouseMove);
      canvas.removeEventListener('dblclick', handleDblClick);
      canvas.removeEventListener('contextmenu', handleContextMenu);
      canvas.removeEventListener('keydown', handleKeyDown);
      canvas.removeEventListener('keyup', handleKeyUp);
      canvas.removeEventListener('mouseleave', handleMouseLeave);
    };
  }, [browser.viewport_width, browser.viewport_height, onInput]);

  return (
    <div className="canvas-wrap" ref={wrapRef}>
      <canvas
        ref={canvasRef}
        className="browser-canvas"
        width={browser.viewport_width}
        height={browser.viewport_height}
        tabIndex={0}
        onClick={() => canvasRef.current?.focus()}
      />
      {virtualCursor.visible && (
        <svg
          className="virtual-cursor"
          style={{ left: virtualCursor.x, top: virtualCursor.y }}
          width="20"
          height="20"
          viewBox="0 0 24 24"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          <path
            d="M5 3l14 8.5-6.5 1.5-3 6.5z"
            fill="rgba(255,60,60,0.85)"
            stroke="#fff"
            strokeWidth="1.5"
            strokeLinejoin="round"
          />
        </svg>
      )}
      {ripples.map((r) => (
        <span
          key={r.id}
          className="click-ripple"
          style={{ left: r.x, top: r.y }}
        />
      ))}
      {!previewSegment && !previewEnded && (
        <div className="preview-overlay-hint">
          <strong>Waiting for stream…</strong>
          <span>Agent is starting Chrome screencast.</span>
        </div>
      )}
      {previewEnded && (
        <div className="preview-ended-overlay">
          <span>Browser disconnected</span>
        </div>
      )}
    </div>
  );
}

