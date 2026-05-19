import { ENV } from '../app/env.js';

export function buildShareMessage() {
  const url = document.getElementById('generatedLink').value;
  const name = document.getElementById('linkName').value.trim() || 'there';
  const role = document.getElementById('linkRole').value.trim() || 'the role';
  return {
    subject: `Mock Interview — ${role}`,
    body: `Hi ${name},\n\nHere's your mock interview link for the ${role} position:\n\n${url}\n\nWhat to expect:\n• Click the link, then click "Start Interview" when you're ready\n• You'll have about 20-30 minutes of voice-based questions (technical + behavioral)\n• Use Chrome or Edge in a quiet space with your microphone enabled\n• You'll get a detailed feedback report at the end\n\nGood luck!`,
    url
  };
}

export function shareViaEmail() {
  const m = buildShareMessage();
  const href = `${ENV.EMAIL_SHARE_SCHEME}?subject=${encodeURIComponent(m.subject)}&body=${encodeURIComponent(m.body)}`;
  window.location.href = href;
}

export function shareViaWhatsApp() {
  const m = buildShareMessage();
  // wa.me opens WhatsApp with text pre-filled
  const href = `${ENV.WHATSAPP_SHARE_URL}${encodeURIComponent(m.body)}`;
  window.open(href, '_blank');
}

export function copyShareMessage(event) {
  const m = buildShareMessage();
  navigator.clipboard.writeText(m.body).then(() => {
    const btn = event?.currentTarget;
    if (!btn) return;
    const orig = btn.textContent;
    btn.textContent = '✓ Message copied! Paste into Slack/Teams';
    setTimeout(() => btn.textContent = orig, 3000);
  });
}
