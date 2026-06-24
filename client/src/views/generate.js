import { api, pollJob } from '../api.js';
import { renderJobState } from '../ui.js';

// Feature 1 + 2: prompt → NSFW image, with AI prompt-assist and additional-context /
// negative conditioning (§3.1, §3.2).
export function mountGenerate(view) {
  view.innerHTML = `
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
          <input id="g-steps" type="number" value="28" min="1" max="60" />
        </div>
      </div>

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
  let cancel = null;

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
      steps: Number($('#g-steps').value) || 28,
    };

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
