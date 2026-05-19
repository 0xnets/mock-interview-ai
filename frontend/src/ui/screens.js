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

const AUTH_NAV_BUTTONS = {
  settings: 'settingsBtn',
  invites: 'adminInvitesBtn',
  logout: 'logoutBtn',
};

function screenAuthNavKey(name) {
  if (name === 'setup') return 'settings';
  if (name === 'admin-invite') return 'invites';
  return null;
}

export function setActiveAuthNavButton(activeKey) {
  Object.entries(AUTH_NAV_BUTTONS).forEach(([key, id]) => {
    const el = document.getElementById(id);
    if (!el) return;
    const active = key === activeKey;
    el.classList.toggle('is-active', active);
    el.setAttribute('aria-pressed', active ? 'true' : 'false');
  });
}

export function showScreen(name) {
  SCREENS.forEach(s => {
    const el = document.getElementById('screen-' + s);
    if (el) el.classList.toggle('hidden-screen', s !== name);
  });
  setActiveAuthNavButton(screenAuthNavKey(name));
}
