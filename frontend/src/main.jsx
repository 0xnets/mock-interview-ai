import { createRoot } from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';
import { App } from './App.jsx';
import './styles/app.css';
import './styles/base.css';
import './styles/components.css';
import './styles/animations.css';
import './styles/screens.css';

// No StrictMode: the interview WebSocket uses a single-use join nonce, and
// StrictMode's double-invoked effects would consume the nonce on the discarded
// first mount.
createRoot(document.getElementById('root')).render(
  <BrowserRouter>
    <App />
  </BrowserRouter>
);
