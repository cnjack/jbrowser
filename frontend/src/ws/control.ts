export type ControlEvent =
  | { type: 'browser.state'; payload: unknown }
  | { type: 'tab.list'; payload: unknown }
  | { type: 'preview.segment'; payload: ArrayBuffer }
  | { type: 'reset.completed'; payload: unknown }
  | { type: 'reset.failed'; payload: { error: string } }
  | { type: 'error'; payload: unknown };

export class ControlSocket {
  private socket: WebSocket | null = null;
  private authenticated = false;
  private pendingSubscribe: string | null = null;

  connect(
    token: string,
    onEvent: (event: ControlEvent) => void,
    onConnected?: () => void,
    onDisconnected?: () => void,
  ) {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    this.socket = new WebSocket(`${protocol}//${window.location.host}/ws/control`);
    this.socket.binaryType = 'arraybuffer';
    this.authenticated = false;
    this.socket.onopen = () => {
      this.send({ type: 'auth', payload: { token } });
    };
    this.socket.onmessage = (event) => {
      if (event.data instanceof ArrayBuffer) {
        onEvent({ type: 'preview.segment', payload: event.data });
        return;
      }
      const message = JSON.parse(event.data as string) as { type: string; payload?: unknown };
      if (message.type === 'auth.ok') {
        this.authenticated = true;
        onConnected?.();
        // If subscribe was called before auth completed, send it now
        if (this.pendingSubscribe) {
          this.send({ type: 'browser.subscribe', payload: { browserInstanceId: this.pendingSubscribe } });
          this.pendingSubscribe = null;
        }
        return;
      }
      if (message.type === 'auth.error') {
        console.error('WS auth failed:', message.payload);
        onEvent({ type: 'error', payload: message.payload });
        return;
      }
      if (
        message.type === 'browser.state' ||
        message.type === 'tab.list' ||
        message.type === 'reset.completed' ||
        message.type === 'reset.failed' ||
        message.type === 'error'
      ) {
        onEvent({ type: message.type, payload: message.payload } as ControlEvent);
      }
    };
    this.socket.onclose = () => {
      this.authenticated = false;
      onDisconnected?.();
    };
  }

  subscribe(browserInstanceId: string) {
    if (this.authenticated) {
      this.send({ type: 'browser.subscribe', payload: { browserInstanceId } });
    } else {
      // Queue until auth completes
      this.pendingSubscribe = browserInstanceId;
    }
  }

  send(message: unknown) {
    if (this.socket?.readyState === WebSocket.OPEN) {
      this.socket.send(JSON.stringify(message));
    }
  }

  close() {
    this.socket?.close();
    this.socket = null;
    this.authenticated = false;
    this.pendingSubscribe = null;
  }
}
