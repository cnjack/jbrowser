import { describe, expect, it } from 'vitest';
import { mapCoordinates } from './coordinates';

describe('mapCoordinates', () => {
  it('maps player coordinates into viewport coordinates', () => {
    expect(
      mapCoordinates(150, 100, {
        left: 50,
        top: 40,
        width: 640,
        height: 360,
      }),
    ).toEqual({ x: 200, y: 120 });
  });
});

