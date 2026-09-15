/**
 * WCAG relative-luminance contrast maths.
 *
 * Colours may carry alpha (`#rrggbbaa`); every comparison composites them over
 * their backdrop first, exactly like CSS does, so a translucent role such as
 * `border` is measured as it actually renders.
 */

export type Rgba = { r: number; g: number; b: number; a: number };

const HEX_PATTERN = /^#?([0-9a-f]{6}|[0-9a-f]{8})$/i;

export function parseColor(value: string): Rgba | null {
  const match = HEX_PATTERN.exec(value.trim());
  if (!match) return null;

  const hex = match[1];
  return {
    r: Number.parseInt(hex.slice(0, 2), 16),
    g: Number.parseInt(hex.slice(2, 4), 16),
    b: Number.parseInt(hex.slice(4, 6), 16),
    a: hex.length === 8 ? Number.parseInt(hex.slice(6, 8), 16) : 255,
  };
}

export function toHex({ r, g, b, a }: Rgba): string {
  const channel = (value: number) =>
    Math.round(Math.min(255, Math.max(0, value)))
      .toString(16)
      .padStart(2, "0");

  const rgb = `#${channel(r)}${channel(g)}${channel(b)}`;
  return a >= 255 ? rgb : `${rgb}${channel(a)}`;
}

/** Flattens a translucent colour onto an opaque backdrop. */
export function composite(foreground: string, background: string): string {
  const front = parseColor(foreground);
  const back = parseColor(background);
  if (!front || !back) return foreground;

  const alpha = front.a / 255;
  return toHex({
    r: front.r * alpha + back.r * (1 - alpha),
    g: front.g * alpha + back.g * (1 - alpha),
    b: front.b * alpha + back.b * (1 - alpha),
    a: 255,
  });
}

function linearize(channel: number): number {
  const value = channel / 255;
  return value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4;
}

export function luminance(color: string): number {
  const rgba = parseColor(color);
  if (!rgba) return 0;

  return (
    0.2126 * linearize(rgba.r) +
    0.7152 * linearize(rgba.g) +
    0.0722 * linearize(rgba.b)
  );
}

/** WCAG contrast ratio, 1 (identical) to 21 (black on white). */
export function ratio(a: string, b: string): number {
  const [light, dark] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (light + 0.05) / (dark + 0.05);
}

/** Blends towards another colour, keeping the original alpha. */
export function blend(color: string, toward: string, amount: number): string {
  const from = parseColor(color);
  const to = parseColor(toward);
  if (!from || !to) return color;

  return toHex({
    r: from.r + (to.r - from.r) * amount,
    g: from.g + (to.g - from.g) * amount,
    b: from.b + (to.b - from.b) * amount,
    a: from.a,
  });
}

/**
 * Finds the smallest change to `color` that clears `target` under `measure`:
 * it walks towards white and towards black and keeps whichever result is
 * closer to the original, so colours move as little as the target allows.
 */
export function meetContrast(
  color: string,
  target: number,
  measure: (candidate: string) => number,
  steps = 100,
): string {
  if (measure(color) >= target) return color;

  let best: { color: string; ratio: number } = {
    color,
    ratio: measure(color),
  };

  for (const toward of ["#ffffff", "#000000"]) {
    for (let step = 1; step <= steps; step += 1) {
      const candidate = blend(color, toward, step / steps);
      const candidateRatio = measure(candidate);

      if (candidateRatio >= target) return candidate;
      if (candidateRatio > best.ratio)
        best = { color: candidate, ratio: candidateRatio };
    }
  }

  // Nothing clears the target: return the closest we got, so the UI still shows
  // the best available colour rather than silently doing nothing.
  return best.color;
}
