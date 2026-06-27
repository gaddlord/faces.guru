import './styles.css';
import { api, initSettings } from './api.js';
import { mountGenerate } from './views/generate.js';
import { mountDescribe } from './views/describe.js';
import { mountSwap } from './views/swap.js';
import { mountVideo } from './views/video.js';
import { mountGallery } from './views/gallery.js';
import { mountSettings } from './views/settings.js';

initSettings();

const TABS = [
  { id: 'generate', label: 'Generate', ic: '✨', mount: mountGenerate },
  { id: 'describe', label: 'Describe', ic: '🔎', mount: mountDescribe },
  { id: 'swap', label: 'Swap', ic: '💞', mount: mountSwap },
  { id: 'video', label: 'Video', ic: '🎬', mount: mountVideo },
  { id: 'gallery', label: 'Gallery', ic: '🖼️', mount: mountGallery },
  { id: 'settings', label: 'Settings', ic: '⚙️', mount: mountSettings },
];

const app = document.getElementById('app');
app.innerHTML = `
  <header class="topbar">
    <h1>faces.guru</h1>
    <span id="conn" class="conn"><span class="dot"></span><span class="txt">checking…</span></span>
  </header>
  <main id="view"></main>
  <nav class="tabbar"></nav>
`;

const viewEl = document.getElementById('view');
const tabbar = app.querySelector('.tabbar');
let cleanup = null;
let current = null;

function navigate(id) {
  const tab = TABS.find((t) => t.id === id) || TABS[0];
  if (typeof cleanup === 'function') cleanup();
  viewEl.innerHTML = '';
  cleanup = tab.mount(viewEl) || null;
  current = tab.id;
  [...tabbar.children].forEach((b) => b.classList.toggle('active', b.dataset.id === tab.id));
  location.hash = '#' + tab.id;
}

TABS.forEach((t) => {
  const b = document.createElement('button');
  b.dataset.id = t.id;
  b.innerHTML = `<span class="ic">${t.ic}</span><span>${t.label}</span>`;
  b.addEventListener('click', () => navigate(t.id));
  tabbar.appendChild(b);
});

// Connection indicator: ping /health periodically.
const conn = document.getElementById('conn');
async function checkConn() {
  try {
    await api.health();
    conn.className = 'conn ok';
    conn.querySelector('.txt').textContent = 'connected';
  } catch {
    conn.className = 'conn bad';
    conn.querySelector('.txt').textContent = 'offline';
  }
}
checkConn();
setInterval(checkConn, 5000);
window.addEventListener('fg:backend-changed', checkConn);

// Allow other views (e.g. Describe → Generate hand-off) to navigate via the hash.
window.addEventListener('hashchange', () => {
  const id = (location.hash || '').replace('#', '');
  if (id && id !== current && TABS.some((t) => t.id === id)) navigate(id);
});

const initial = (location.hash || '').replace('#', '');
navigate(TABS.some((t) => t.id === initial) ? initial : 'generate');
