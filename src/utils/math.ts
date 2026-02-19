// ─────────────────────────────────────────────────────────────
// MNMX Math Utilities
// ─────────────────────────────────────────────────────────────

/**
 * Clamp a value between min and max inclusive.
 */
export function clamp(value: number, min: number, max: number): number {
  if (value < min) return min;
  if (value > max) return max;
  return value;
}

/**
 * Normalize a value from [min, max] to [0, 1].
 * Values outside the range are clamped.
 */
export function normalizeToRange(value: number, min: number, max: number): number {
  if (max === min) return 0.5;
  return clamp((value - min) / (max - min), 0, 1);
}

/**
 * Compute a weighted average.
 * values and weights must have the same length.
 */
export function weightedAverage(values: number[], weights: number[]): number {
  if (values.length === 0 || values.length !== weights.length) return 0;
  let sumProduct = 0;
  let sumWeights = 0;
  for (let i = 0; i < values.length; i++) {
    sumProduct += values[i] * weights[i];
    sumWeights += weights[i];
  }
  if (sumWeights === 0) return 0;
  return sumProduct / sumWeights;
}

/**
 * Convert basis points (1 bp = 0.01%) to a decimal fraction.
 * e.g. 50 bps => 0.005
 */
export function basisPointsToDecimal(bps: number): number {
  return bps / 10000;
}

/**
 * Convert a decimal fraction to basis points.
 * e.g. 0.005 => 50 bps
 */
export function decimalToBasisPoints(dec: number): number {
  return Math.round(dec * 10000);
}

/**
 * Safe division that returns a fallback value when dividing by zero.
 */
export function safeDivide(a: number, b: number, fallback: number = 0): number {
  if (b === 0 || !Number.isFinite(b)) return fallback;
  const result = a / b;
  if (!Number.isFinite(result)) return fallback;
  return result;
}

/**
 * Format an amount from its smallest unit representation to human-readable.
 * e.g. formatAmount("1000000", 6) => "1.0"
 */
export function formatAmount(amount: string | bigint, decimals: number): string {
  const str = typeof amount === 'bigint' ? amount.toString() : amount;
  if (decimals === 0) return str;
  const padded = str.padStart(decimals + 1, '0');
  const intPart = padded.slice(0, padded.length - decimals) || '0';
  const fracPart = padded.slice(padded.length - decimals);
  // Trim trailing zeros from fractional part
  const trimmed = fracPart.replace(/0+$/, '');
  if (trimmed.length === 0) return intPart;
  return `${intPart}.${trimmed}`;
}

/**
 * Parse a human-readable amount string into its smallest unit representation.
 * e.g. parseAmount("1.5", 6) => "1500000"
 */
export function parseAmount(amount: string, decimals: number): string {
  const parts = amount.split('.');
  const intPart = parts[0] || '0';
  let fracPart = parts[1] || '';
  if (fracPart.length > decimals) {
    fracPart = fracPart.slice(0, decimals);
  } else {
    fracPart = fracPart.padEnd(decimals, '0');
  }
  const combined = intPart + fracPart;
  // Remove leading zeros but keep at least one digit
  const stripped = combined.replace(/^0+/, '') || '0';
  return stripped;
}

/**
 * Calculate the percentage difference between two values.
 * Returns a signed value: positive if b > a.
 */
export function percentageDifference(a: number, b: number): number {
  if (a === 0 && b === 0) return 0;
  if (a === 0) return b > 0 ? 100 : -100;
  return ((b - a) / Math.abs(a)) * 100;
}

/**
 * Linear interpolation between a and b by factor t (0-1).
 */
export function lerp(a: number, b: number, t: number): number {
  return a + (b - a) * clamp(t, 0, 1);
}

/**
 * Compute the geometric mean of an array of positive numbers.
 */
export function geometricMean(values: number[]): number {
  if (values.length === 0) return 0;
  const filtered = values.filter((v) => v > 0);
  if (filtered.length === 0) return 0;
  const logSum = filtered.reduce((acc, v) => acc + Math.log(v), 0);
  return Math.exp(logSum / filtered.length);
}

/**
 * Sum an array of numbers.
 */
export function sum(values: number[]): number {
  let total = 0;
  for (const v of values) total += v;
  return total;
}

/**
 * Compute the minimum value in an array. Returns Infinity for empty arrays.
 */
export function min(values: number[]): number {
  if (values.length === 0) return Infinity;
  let m = values[0];
  for (let i = 1; i < values.length; i++) {
    if (values[i] < m) m = values[i];
  }
  return m;
}
