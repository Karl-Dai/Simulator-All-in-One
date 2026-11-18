// Edit-aware C3 auto-correction for IEC 60870-5-104 timing parameters.
//
// `t1` and `k` are the anchors. Editing `t2`/`t3`/`w` clamps only the edited
// field to the dynamic bound; editing `t1`/`k` keeps the anchor and pushes at
// most one neighbor (never cascades). The backend runs an orientation-agnostic
// "t1/k authoritative" normalization that agrees with this on valid output, so
// the form's submissions are a no-op there. Mirrors `iec104sim-core::timing`.

export type TimingFields = {
  t0: number
  t1: number
  t2: number
  t3: number
  k: number
  w: number
}

export type TimingEditField = keyof TimingFields

export type TimingCorrection = { field: string; from: number; to: number }

const TIMING_KEYS: TimingEditField[] = ['t0', 't1', 't2', 't3', 'k', 'w']

export function isTimingField(key: string): key is TimingEditField {
  return (TIMING_KEYS as string[]).includes(key)
}

function clampInt(v: number, lo: number, hi: number): number {
  let n = Number.isFinite(v) ? Math.round(v) : lo
  if (hi < lo) hi = lo
  return Math.min(hi, Math.max(lo, n))
}

/**
 * Correct `f` in place after the user edited `edited`, enforcing
 * `t2 < t1 < t3` and `w ≤ ⌊2k/3⌋`. Returns the list of fields that changed
 * (empty ⇒ the edit was already valid).
 */
export function correctTimingEdit(f: TimingFields, edited: TimingEditField): TimingCorrection[] {
  const before: TimingFields = { ...f }
  switch (edited) {
    case 't0':
      f.t0 = clampInt(f.t0, 1, 255)
      break
    case 't1':
      f.t1 = clampInt(f.t1, 2, 254)
      if (f.t2 >= f.t1) f.t2 = f.t1 - 1
      if (f.t3 <= f.t1) f.t3 = f.t1 + 1
      break
    case 't2':
      f.t2 = clampInt(f.t2, 1, Math.max(1, f.t1 - 1))
      break
    case 't3':
      f.t3 = clampInt(f.t3, Math.min(255, f.t1 + 1), 255)
      break
    case 'k': {
      f.k = clampInt(f.k, 2, 32767)
      const wMax = Math.max(1, Math.floor((2 * f.k) / 3))
      if (f.w > wMax) f.w = wMax
      break
    }
    case 'w': {
      const wMax = Math.max(1, Math.floor((2 * f.k) / 3))
      f.w = clampInt(f.w, 1, wMax)
      break
    }
  }

  const out: TimingCorrection[] = []
  for (const key of TIMING_KEYS) {
    if (before[key] !== f[key]) out.push({ field: key, from: before[key], to: f[key] })
  }
  return out
}

/** Render corrections as a compact symbolic detail, e.g. "t2: 10 → 4". */
export function formatCorrections(corrections: TimingCorrection[]): string {
  return corrections.map((c) => `${c.field}: ${c.from} → ${c.to}`).join(', ')
}
