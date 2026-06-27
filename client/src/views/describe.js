import { api } from '../api.js';
import { previewFile } from '../ui.js';

const ASPECTS = [
  { key: 'face', label: 'Face' },
  { key: 'body', label: 'Body' },
  { key: 'posture', label: 'Posture' },
  { key: 'clothing', label: 'Clothing' },
  { key: 'environment', label: 'Environment' },
];

// Image → prompt: upload an image, pick aspects, and a vision model writes a detailed
// diffusion prompt you can send straight to the Generate screen.
export function mountDescribe(view) {
  ensureStyles();
  view.innerHTML = `
    <div class="card">
      <h2>Describe image → prompt</h2>
      <p class="muted">Upload an image; a vision model writes a detailed prompt for the aspects you pick.</p>
      <label class="file-pick">
        Tap to pick an image
        <input id="d-image" type="file" accept="image/*" />
      </label>
      <img id="d-prev" class="preview" />

      <label>What to describe</label>
      <div id="d-aspects" class="aspects">
        ${ASPECTS.map(
          (a) =>
            `<label class="chk"><input type="checkbox" value="${a.key}" checked /> ${a.label}</label>`
        ).join('')}
      </div>

      <button class="btn" id="d-run">Describe</button>
      <p class="muted" id="d-note"></p>

      <div id="d-out-wrap" style="display:none">
        <label for="d-out">Generated prompt (editable)</label>
        <textarea id="d-out" style="min-height:140px"></textarea>
        <button class="btn" id="d-send">Send to Generate →</button>
      </div>
    </div>
  `;

  const $ = (s) => view.querySelector(s);
  const note = $('#d-note');
  let uploadedId = null;

  $('#d-image').addEventListener('change', (e) => {
    previewFile(e.target.files[0], $('#d-prev'));
    uploadedId = null; // re-upload the new image on next Describe
    $('#d-out-wrap').style.display = 'none';
  });

  $('#d-run').addEventListener('click', async () => {
    const file = $('#d-image').files[0];
    if (!file) {
      note.textContent = 'Pick an image first.';
      return;
    }
    const aspects = [...view.querySelectorAll('#d-aspects input:checked')].map((c) => c.value);
    if (!aspects.length) {
      note.textContent = 'Select at least one aspect.';
      return;
    }

    $('#d-run').disabled = true;
    note.textContent = 'Uploading…';
    try {
      if (!uploadedId) {
        const up = await api.uploadMedia(file);
        uploadedId = up.id;
      }
      note.textContent = 'Analyzing image… (vision models are slower)';
      const r = await api.describeImage(uploadedId, aspects);
      $('#d-out').value = r.prompt || '';
      $('#d-out-wrap').style.display = 'block';
      note.textContent = r.described
        ? 'Done — edit if needed, then send to Generate.'
        : 'Vision model offline — placeholder text (load a vision model and set FG_VISION_MODEL).';
    } catch (e) {
      note.textContent = 'Describe failed: ' + e.message;
    } finally {
      $('#d-run').disabled = false;
    }
  });

  $('#d-send').addEventListener('click', () => {
    const text = $('#d-out').value.trim();
    if (!text) return;
    // Hand off to the Generate screen, which picks this up on mount.
    localStorage.setItem('fg_pending_positive', text);
    location.hash = '#generate';
  });

  return null;
}

function ensureStyles() {
  if (document.getElementById('describe-styles')) return;
  const s = document.createElement('style');
  s.id = 'describe-styles';
  s.textContent =
    '.aspects{display:flex;flex-wrap:wrap;gap:10px;margin-bottom:6px}' +
    '.aspects .chk{display:flex;align-items:center;gap:6px;background:var(--surface-2);' +
    'border:1px solid var(--border);border-radius:999px;padding:8px 12px;font-size:14px;' +
    'color:var(--text);margin:0;cursor:pointer}' +
    '.aspects .chk input{width:auto;margin:0}';
  document.head.appendChild(s);
}
