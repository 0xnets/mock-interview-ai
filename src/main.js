import { attachEventHandlers } from './app/events.js';
import { bootstrap } from './app/bootstrap.js';

attachEventHandlers();
window.addEventListener('load', bootstrap);
