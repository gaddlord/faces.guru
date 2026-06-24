import { api, getBackend, setBackend } from '../api.js';
import { esc } from '../ui.js';

// Settings: the Mac/backend address (reached over Tailscale) + connection test (§1.2).
export function mountSettings(view) {
  view.innerHTML = `
    <div class="card">
      <h2>Backend</h2>
      <label for="st-url">Mac backend address (Tailscale)</label>
      <input id="st-url" type="text" placeholder="http://100.x.y.z:8080" value="${esc(getBackend())}" />
      <p class="muted">
        On a device, set this to the Mac's Tailscale IP and the backend port (default 8080).
        For local development the default is <code>http://127.0.0.1:8089</code>.
      </p>
      <div class="row">
        <button class="btn secondary" id="st-test">Test connection</button>
        <button class="btn" id="st-save">Save</button>
      </div>
      <p class="muted" id="st-note"></p>
    </div>
    <div class="card">
      <h2>About</h2>
      <p class="muted">
        faces.guru — a private, local-only AI image / face-swap / video tool.
        All generation runs on your Mac; nothing leaves the Tailscale mesh.
      </p>
    </div>
  `;

  const $ = (id) => view.querySelector(id);
  const note = $('#st-note');

  $('#st-save').addEventListener('click', () => {
    setBackend($('#st-url').value);
    $('#st-url').value = getBackend();
    note.textContent = 'Saved.';
    window.dispatchEvent(new Event('fg:backend-changed'));
  });

  $('#st-test').addEventListener('click', async () => {
    // Test against the value currently in the box without persisting it.
    const prev = getBackend();
    setBackend($('#st-url').value);
    note.textContent = 'Testing…';
    try {
      const h = await api.health();
      note.textContent = 'OK — ' + JSON.stringify(h);
    } catch (e) {
      note.textContent = 'Failed: ' + e.message;
    } finally {
      setBackend(prev);
    }
  });

  return null;
}
