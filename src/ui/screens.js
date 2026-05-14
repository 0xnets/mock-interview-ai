export function showScreen(name) {
  ['setup', 'welcome', 'interview', 'results', 'loading'].forEach(s => {
    document.getElementById('screen-' + s).classList.toggle('hidden-screen', s !== name);
  });
}

// =============== SPEECH RECOGNITION ===============
