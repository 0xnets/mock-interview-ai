const SCREENS = [
  'setup',
  'welcome',
  'interview',
  'results',
  'loading',
  'login',
  'accept-invite',
  'admin-invite',
  'transcript',
  'report-waiting',
];

export function showScreen(name) {
  SCREENS.forEach(s => {
    const el = document.getElementById('screen-' + s);
    if (el) el.classList.toggle('hidden-screen', s !== name);
  });
}
