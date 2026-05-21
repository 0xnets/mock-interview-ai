/// Copy an <input>'s value via the async Clipboard API, falling back to
/// `execCommand('copy')` on the selected input when the API is unavailable or
/// fails (non-secure context, older browsers). Returns whether it succeeded.
function copyInputValue(inputEl) {
  if (!inputEl) return false;
  inputEl.focus();
  inputEl.select();
  inputEl.setSelectionRange(0, inputEl.value.length);
  return Boolean(document.execCommand && document.execCommand('copy'));
}

export async function copyToClipboard(inputEl) {
  if (!inputEl || !inputEl.value) return false;
  try {
    if (navigator.clipboard && window.isSecureContext) {
      await navigator.clipboard.writeText(inputEl.value);
      return true;
    }
    return copyInputValue(inputEl);
  } catch {
    return copyInputValue(inputEl);
  }
}
