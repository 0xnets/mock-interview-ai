export function isValidEmail(value) {
  const email = value.trim();
  if (!email || email.length > 254 || /\s/.test(email)) return false;
  const parts = email.split('@');
  if (parts.length !== 2) return false;
  const [local, domain] = parts;
  if (!local || local.length > 64 || !domain || domain.length > 253) return false;
  if (!/^[!-9=?A-Z^-~]+$/i.test(local)) return false;
  const labels = domain.split('.');
  if (labels.length < 2) return false;
  return labels.every(label => (
    label.length > 0
    && label.length <= 63
    && /^[A-Za-z0-9-]+$/.test(label)
    && !label.startsWith('-')
    && !label.endsWith('-')
  ));
}
