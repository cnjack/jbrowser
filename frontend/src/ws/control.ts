export type ControlEvent =
  | { type: 'browser.state'; payload: unknown }
  | { type: 'tab.list'; payload: unknown }
  | { type: 'preview.segment'; payload: ArrayBuffer }
  | { type: 'error'; payload: unknown };

export class ControlSocket {
  private socket: WebSocket | null = null;

  connect(
    token: string,
    onEvent: (event: ControlEvent) => void,
    onConnected?: () => void,
    onDisconnected?: () => void,
  ) {
    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    this.socket = new WebSocket(`${protocol}//${window.location.host}/ws/control`);
    this.socket.binaryType = 'arraybuffer';
    this.socket.onopen = () => {
      this.send({ type: 'auth', payload: { token } });
      onConnected?.();
    };
    this.socket.onmessage = (event) => {
      if (event.data instanceof ArrayBuffer) {
        onEvent({ type: 'preview.segment', payload: event.data });
        return;
      }
      const message = JSON.parse(event.data as string) as { type: string; payload?: unknown };
      if (
        message.type === 'browser.state' ||
        message.type === 'tab.list' ||
        message.type === 'error'
      ) {
        onEvent({ type: message.type, payload: message.payload });
      }
    };
    this.socket.onclose = () => {
      onDisconnected?.();
    };
  }

  subscribe(browserInstanceId: string) {
    this.send({ type: 'browser.subscribe', payload: { browserInstanceId } });
  }

  send(message: unknown) {
    if (this.socket?.readyState === WebSocket.OPEN) {
      this.socket.send(JSON.stringify(message));
    }
  }

  close() {
    this.socket?.close();
    this.socket = null;
  }
}
