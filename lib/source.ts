// Normalizes ?ref= values and referrer hosts into a short, safe tag.
export function cleanSource(raw: unknown): string | null {
  if (typeof raw !== "string") return null;
  const v = raw.trim().toLowerCase().slice(0, 64);
  return /^[a-z0-9][a-z0-9._-]*$/.test(v) ? v : null;
}
