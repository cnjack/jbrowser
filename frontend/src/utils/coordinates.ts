export interface Point {
  x: number;
  y: number;
}

export interface RectLike {
  left: number;
  top: number;
  width: number;
  height: number;
}

export function mapCoordinates(
  clientX: number,
  clientY: number,
  playerRect: RectLike,
  viewportWidth = 1280,
  viewportHeight = 720,
): Point {
  const scaleX = viewportWidth / playerRect.width;
  const scaleY = viewportHeight / playerRect.height;
  return {
    x: Math.round((clientX - playerRect.left) * scaleX),
    y: Math.round((clientY - playerRect.top) * scaleY),
  };
}

