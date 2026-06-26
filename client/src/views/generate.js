import { api, pollJob } from '../api.js';
import { renderJobState, esc } from '../ui.js';

const LAST_KEY = 'fg_generate_last';
const PRESET_KEY = 'fg_generate_preset';

// Feature 1 + 2: prompt → NSFW image, with AI prompt-assist and additional-context /
// negative conditioning (§3.1, §3.2). Settings are sticky (auto-restored) and can be
// saved as reusable named presets so you never have to re-type a prompt.
export function mountGenerate(view) {
  view.innerHTML = `
    <div class="card">
      <h2>Presets</h2>
      <label for="g-preset">Saved settings</label>
      <select id="g-preset"><option value="">— none —</option></select>
      <div class="row">
        <button class="btn secondary" id="g-preset-save">Save</button>
        <button class="btn secondary" id="g-preset-saveas">Save As…</button>
        <button class="btn secondary" id="g-preset-del">Delete</button>
      </div>
      <p class="muted" id="g-preset-note"></p>
    </div>

    <div class="card">
      <h2>Generate image</h2>
      <label for="g-idea">Your idea</label>
      <textarea id="g-idea" placeholder="a short idea — the AI will expand it"></textarea>

      <label for="g-context">Additional context (style, scene, reference notes)</label>
      <textarea id="g-context" placeholder="extra positive details"></textarea>

      <label for="g-negative">Avoid (negative prompt)</label>
      <input id="g-negative" type="text" placeholder="blurry, extra limbs, watermark" />

      <button class="btn secondary" id="g-enhance">✨ Enhance prompt with AI</button>

      <label for="g-positive">Final positive prompt (editable)</label>
      <textarea id="g-positive" placeholder="filled by Enhance, or type your own"></textarea>

      <div class="row">
        <div>
          <label for="g-size">Size</label>
          <select id="g-size">
            <option value="1024">1024 × 1024</option>
            <option value="768" selected>768 × 768</option>
            <option value="832x1216">832 × 1216 (portrait)</option>
            <option value="1216x832">1216 × 832 (landscape)</option>
          </select>
        </div>
        <div>
          <label for="g-steps">Steps</label>
          <input id="g-steps" type="number" value="30" min="1" max="60" />
        </div>
      </div>

      <details class="advanced">
        <summary>Advanced — sampler &amp; VAE</summary>
        <div class="row">
          <div>
            <label for="g-cfg">CFG</label>
            <input id="g-cfg" type="number" value="6" min="1" max="20" step="0.5" />
          </div>
          <div>
            <label for="g-sampler">Sampler</label>
            <select id="g-sampler">
              <option value="dpmpp_2m" selected>dpmpp_2m</option>
              <option value="euler_ancestral">euler_ancestral</option>
              <option value="dpmpp_2m_sde">dpmpp_2m_sde</option>
              <option value="dpmpp_sde">dpmpp_sde</option>
              <option value="euler">euler</option>
            </select>
          </div>
          <div>
            <label for="g-scheduler">Scheduler</label>
            <select id="g-scheduler">
              <option value="karras" selected>karras</option>
              <option value="normal">normal</option>
              <option value="exponential">exponential</option>
              <option value="sgm_uniform">sgm_uniform</option>
            </select>
          </div>
        </div>
        <label for="g-vae">VAE override (optional)</label>
        <input id="g-vae" type="text" placeholder="e.g. sdxl_vae.safetensors — blank uses the checkpoint's VAE" />
      </details>

      <button class="btn" id="g-run">Generate</button>
      <p class="muted" id="g-note"></p>
    </div>
    <div class="card" id="g-result-card" style="display:none">
      <h2>Result</h2>
      <div id="g-result"></div>
    </div>
  `;

  const $ = (id) => view.querySelector(id);
  const note = $('#g-note');
  const presetNote = (m) => ($('#g-preset-note').textContent = m);
  let cancel = null;
  let presets = [];
  let currentPresetId = localStorage.getItem(PRESET_KEY) || '';

  // ---- settings <-> form ----
  const FIELDS = [
    'idea', 'context', 'negative', 'positive',
    'size', 'steps', 'cfg', 'sampler', 'scheduler', 'vae',
  ];
  const collect = () => Object.fromEntries(FIELDS.map((f) => [f, $('#g-' + f).value]));
  const apply = (s) => {
    if (!s || typeof s !== 'object') return;
    FIELDS.forEach((f) => {
      if (s[f] !== undefined && s[f] !== null) $('#g-' + f).value = s[f];
    });
  };
  const persistLast = () => localStorage.setItem(LAST_KEY, JSON.stringify(collect()));

  // Restore the last-used settings so you never start from a blank prompt.
  try {
    const last = JSON.parse(localStorage.getItem(LAST_KEY) || 'null');
    if (last) apply(last);
  } catch {
    /* ignore corrupt cache */
  }

  // Keep the sticky cache fresh as the user edits.
  FIELDS.forEach((f) => {
    const el = $('#g-' + f);
    el.addEventListener('input', persistLast);
    el.addEventListener('change', persistLast);
  });

  // ---- presets ----
  async function refreshPresets(selectId) {
    try {
      presets = await api.listPresets('image');
    } catch {
      presets = [];
    }
    const sel = $('#g-preset');
    sel.innerHTML =
      '<option value="">— none —</option>' +
      presets.map((p) => `<option value="${p.id}">${esc(p.name)}</option>`).join('');
    const want = selectId !== undefined ? selectId : currentPresetId;
    if (want && presets.some((p) => p.id === want)) {
      sel.value = want;
      currentPresetId = want;
    } else {
      currentPresetId = '';
    }
  }

  $('#g-preset').addEventListener('change', () => {
    const id = $('#g-preset').value;
    currentPresetId = id;
    localStorage.setItem(PRESET_KEY, id);
    const p = presets.find((x) => x.id === id);
    if (p) {
      apply(p.data);
      persistLast();
      presetNote(`Loaded “${p.name}”.`);
    } else {
      presetNote('');
    }
  });

  async function saveAs() {
    const name = (window.prompt('Name for these settings:') || '').trim();
    if (!name) return;
    try {
      const p = await api.createPreset(name, collect());
      currentPresetId = p.id;
      localStorage.setItem(PRESET_KEY, p.id);
      await refreshPresets(p.id);
      presetNote(`Saved as “${p.name}”.`);
    } catch (e) {
      presetNote(
        /409/.test(e.message)
          ? 'A preset with that name already exists — pick another name or use Save.'
          : 'Save As failed: ' + e.message
      );
    }
  }

  $('#g-preset-saveas').addEventListener('click', saveAs);

  $('#g-preset-save').addEventListener('click', async () => {
    if (!currentPresetId) return saveAs(); // nothing selected → behaves like Save As
    try {
      const p = await api.updatePreset(currentPresetId, { data: collect() });
      await refreshPresets(p.id);
      presetNote(`Saved “${p.name}”.`);
    } catch (e) {
      presetNote('Save failed: ' + e.message);
    }
  });

  $('#g-preset-del').addEventListener('click', async () => {
    if (!currentPresetId) {
      presetNote('No preset selected.');
      return;
    }
    const p = presets.find((x) => x.id === currentPresetId);
    if (!window.confirm(`Delete preset “${p ? p.name : ''}”?`)) return;
    try {
      await api.deletePreset(currentPresetId);
      currentPresetId = '';
      localStorage.removeItem(PRESET_KEY);
      await refreshPresets('');
      presetNote('Deleted.');
    } catch (e) {
      presetNote('Delete failed: ' + e.message);
    }
  });

  refreshPresets();

  // ---- enhance & generate ----
  $('#g-enhance').addEventListener('click', async () => {
    const idea = $('#g-idea').value.trim();
    if (!idea) {
      note.textContent = 'Enter an idea first.';
      return;
    }
    note.textContent = 'Enhancing…';
    try {
      const r = await api.enhance(idea, $('#g-context').value, $('#g-negative').value);
      $('#g-positive').value = r.positive;
      if (r.negative) $('#g-negative').value = r.negative;
      persistLast();
      note.textContent = r.enhanced
        ? 'Enhanced by local LLM — edit freely.'
        : 'LLM offline — used your text as-is (you can still generate).';
    } catch (e) {
      note.textContent = 'Enhance failed: ' + e.message;
    }
  });

  $('#g-run').addEventListener('click', async () => {
    const positive =
      $('#g-positive').value.trim() ||
      [$('#g-idea').value.trim(), $('#g-context').value.trim()].filter(Boolean).join(', ');
    if (!positive) {
      note.textContent = 'Enter an idea or a prompt first.';
      return;
    }

    let width = 768;
    let height = 768;
    const sz = $('#g-size').value;
    if (sz.includes('x')) [width, height] = sz.split('x').map(Number);
    else width = height = Number(sz);

    const params = {
      positive,
      negative: $('#g-negative').value.trim(),
      width,
      height,
      steps: Number($('#g-steps').value) || 30,
      cfg: Number($('#g-cfg').value) || 6,
      sampler: $('#g-sampler').value,
      scheduler: $('#g-scheduler').value,
    };
    const vae = $('#g-vae').value.trim();
    if (vae) params.vae = vae;

    persistLast();
    note.textContent = 'Submitting…';
    $('#g-run').disabled = true;
    try {
      const job = await api.createJob('image', params);
      $('#g-result-card').style.display = 'block';
      if (cancel) cancel();
      cancel = pollJob(job.id, (j) => {
        renderJobState($('#g-result'), j);
        if (j.status === 'done' || j.status === 'failed' || j.status === 'error') {
          $('#g-run').disabled = false;
          note.textContent = '';
        }
      });
    } catch (e) {
      note.textContent = 'Submit failed: ' + e.message;
      $('#g-run').disabled = false;
    }
  });

  return () => {
    if (cancel) cancel();
  };
}
