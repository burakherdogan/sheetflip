const T = window.I18N;
const fill = (key, vars = {}) => Object.entries(vars).reduce((s, [k, v]) => s.replaceAll(`{${k}}`, v), T[key]);
const $ = s => document.querySelector(s);

$('#theme').addEventListener('click', () => {
  const next = document.documentElement.dataset.theme === 'dark' ? 'light' : 'dark';
  document.documentElement.dataset.theme = next;
  localStorage.theme = next;
});

const dialog = $('#confirm');
document.querySelectorAll('form[data-confirm]').forEach(form => {
  form.addEventListener('submit', e => {
    if (form.dataset.ok) return;
    e.preventDefault();
    $('#confirm-text').innerHTML = form.dataset.confirm;
    dialog.showModal();
    dialog.querySelectorAll('button').forEach(b => b.onclick = () => {
      dialog.close();
      if (b.value !== 'yes') return;
      form.dataset.ok = '1';
      form.querySelector('button').disabled = true;
      form.submit();
    });
  });
});

const printerSelect = $('select[data-tested]');
if (printerSelect) {
  const tested = JSON.parse(printerSelect.dataset.tested);
  const update = () => {
    $('#untested').hidden = tested.includes(printerSelect.value);
    $('#test-link').href = '/printer-test?printer=' + encodeURIComponent(printerSelect.value);
  };
  printerSelect.addEventListener('change', update);
  update();
}

const input = $('#pdf-input');
if (input) {
  const pdfjs = await import('/pdf.mjs');
  pdfjs.GlobalWorkerOptions.workerSrc = '/pdf.worker.mjs';
  const form = input.form, drop = $('#dropzone'), error = $('#file-error'), sheetsEl = $('#sheets');
  const MAX_SHEETS = 24;
  let url, pdf, thumbs = new Map(), token = 0;

  const showError = msg => { error.textContent = msg; error.hidden = !msg; };

  const thumb = i => {
    if (!thumbs.has(i)) thumbs.set(i, pdf.getPage(i + 1).then(async p => {
      const scale = 240 / p.getViewport({ scale: 1 }).width;
      const vp = p.getViewport({ scale }), c = document.createElement('canvas');
      c.width = vp.width; c.height = vp.height;
      await p.render({ canvasContext: c.getContext('2d'), viewport: vp }).promise;
      return c.toDataURL();
    }));
    return thumbs.get(i);
  };

  const slot = i => {
    const d = document.createElement('div');
    d.className = 'slot' + (i === null ? ' blank' : '');
    if (i !== null) {
      d.innerHTML = `<em>${i + 1}</em>`;
      thumb(i).then(src => { const img = new Image(); img.alt = ''; img.src = src; d.prepend(img); });
    }
    return d;
  };

  const viewer = $('#preview-dialog'), stage = $('#viewer-stage'), large = $('#viewer-paper');
  let previewSides = [], activeSide = 0, zoom = 1, viewerToken = 0;
  const drawViewer = async () => {
    const sheetNumbers = [...new Set(previewSides.map(s => s.sheetNumber))];
    const entries = previewSides.filter(s => s.sheetNumber === sheetNumbers[activeSide]);
    if (!entries.length) return;
    const drawToken = ++viewerToken;
    $('#viewer-title').textContent = fill('js_sheet', { n: sheetNumbers[activeSide] });
    $('#viewer-prev').disabled = activeSide === 0;
    $('#viewer-next').disabled = activeSide === sheetNumbers.length - 1;
    $('#viewer-out').disabled = zoom <= .5;
    $('#viewer-in').disabled = zoom >= 3;
    $('#viewer-zoom').textContent = Math.round(zoom * 100) + '%';
    $('#viewer-error').hidden = true;
    large.replaceChildren();
    const stacked = stage.clientWidth < 600;
    large.style.flexDirection = stacked ? 'column' : 'row';
    try {
      for (const entry of entries) {
        const ratio = entry.columns > entry.rows ? 842 / 595 : 595 / 842;
        const available = (stage.clientWidth - 48) / (stacked ? 1 : entries.length);
        const width = Math.max(140, Math.min(available, (stage.clientHeight - 90) * ratio)) * zoom;
        const face = document.createElement('section');
        face.className = 'viewer-face';
        face.style.width = width + 'px';
        const paper = document.createElement('div');
        paper.className = 'viewer-sheet';
        paper.style.height = width / ratio + 'px';
        paper.style.gridTemplateColumns = `repeat(${entry.columns}, minmax(0, 1fr))`;
        paper.style.gridTemplateRows = `repeat(${entry.rows}, minmax(0, 1fr))`;
        const caption = document.createElement('p');
        caption.className = 'viewer-caption';
        caption.textContent = entry.label + ' · ' + fill('js_page_list', { pages: entry.slots.map(i => i === null ? T.js_blank : i + 1).join(', ') });
        face.append(paper, caption);
        large.append(face);
        const cells = entry.slots.map(i => {
          const cell = document.createElement('div');
          cell.className = 'slot' + (i === null ? ' blank' : '');
          paper.append(cell);
          return cell;
        });
        for (let j = 0; j < entry.slots.length; j++) {
          const i = entry.slots[j];
          if (i === null) continue;
          const page = await pdf.getPage(i + 1);
          if (drawToken !== viewerToken) return;
          const natural = page.getViewport({ scale: 1 });
          const scale = Math.min(width / entry.columns / natural.width, width / ratio / entry.rows / natural.height) * Math.min(devicePixelRatio || 1, 2);
          const viewport = page.getViewport({ scale });
          const canvas = document.createElement('canvas');
          canvas.width = Math.ceil(viewport.width); canvas.height = Math.ceil(viewport.height);
          await page.render({ canvasContext: canvas.getContext('2d'), viewport }).promise;
          if (drawToken !== viewerToken) return;
          cells[j].append(canvas);
        }
      }
    } catch {
      if (drawToken === viewerToken) { $('#viewer-error').textContent = T.js_bad_pdf; $('#viewer-error').hidden = false; }
    }
  };
  const openViewer = index => {
    activeSide = previewSides[index].sheetNumber - 1; zoom = 1;
    viewer.showModal();
    stage.scrollTo(0, 0);
    drawViewer();
  };
  $('#viewer-close').onclick = () => viewer.close();
  viewer.addEventListener('close', () => { viewerToken++; large.replaceChildren(); });
  $('#viewer-prev').onclick = () => { if (activeSide > 0) { activeSide--; drawViewer(); } };
  $('#viewer-next').onclick = () => { if (activeSide + 1 < new Set(previewSides.map(s => s.sheetNumber)).size) { activeSide++; drawViewer(); } };
  $('#viewer-in').onclick = () => { zoom = Math.min(3, zoom + .25); drawViewer(); };
  $('#viewer-out').onclick = () => { zoom = Math.max(.5, zoom - .25); drawViewer(); };
  viewer.addEventListener('keydown', e => {
    if (e.key === 'ArrowLeft' || e.key === 'ArrowRight') {
      e.preventDefault(); $(e.key === 'ArrowLeft' ? '#viewer-prev' : '#viewer-next').click();
    }
  });
  window.addEventListener('resize', () => { if (viewer.open) drawViewer(); });

  const side = (label, slots, columns, rows, sheetNumber) => {
    const s = document.createElement('div'), p = document.createElement('div');
    s.className = 'side';
    p.className = 'paper' + (columns > rows ? ' land' : '');
    p.style.display = 'grid';
    p.style.gridTemplateColumns = `repeat(${columns}, minmax(0, 1fr))`;
    p.style.gridTemplateRows = `repeat(${rows}, minmax(0, 1fr))`;
    const index = previewSides.length;
    const title = fill('js_sheet', { n: sheetNumber }) + ' · ' + label;
    previewSides.push({ slots, columns, rows, title, label, sheetNumber });
    p.tabIndex = 0;
    p.setAttribute('role', 'button');
    p.setAttribute('aria-label', title);
    p.setAttribute('aria-haspopup', 'dialog');
    p.onclick = () => openViewer(index);
    p.onkeydown = e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); openViewer(index); } };
    slots.forEach(i => p.append(slot(i)));
    s.append(p, label);
    return s;
  };

  const render = async () => {
    if (!pdf || !form.nup.checkValidity()) return;
    const my = ++token, mode = form.mode.value;
    const nup = mode === 'booklet' ? 2 : +form.nup.value;
    const q = new URLSearchParams({ n: pdf.numPages, mode, pages: form.pages.value, per_side: nup });
    const data = await (await fetch('/layout?' + q)).json();
    if (my !== token) return;
    if (viewer.open) viewer.close();
    previewSides = [];
    sheetsEl.replaceChildren();
    form.pages.setCustomValidity(data.error || '');
    if (data.error) { $('#sheet-summary').textContent = data.error; return; }
    const per = nup, count = data.front.length / per;
    const copies = Math.max(1, +form.copies.value || 1);
    $('#sheet-summary').textContent = fill('js_summary', { pages: pdf.numPages, sheets: count })
      + (copies > 1 ? fill('js_copies', { copies, total: count * copies }) : '');
    for (let s = 0; s < Math.min(count, MAX_SHEETS); s++) {
      const card = document.createElement('div'), sides = document.createElement('div');
      card.className = 'sheet'; sides.className = 'sides';
      card.innerHTML = `<h4>${fill('js_sheet', { n: s + 1 })}</h4>`;
      const f = data.front.slice(s * per, s * per + per);
      if (mode === 'single') sides.append(side(T.side_single, f, data.columns, data.rows, s + 1));
      else sides.append(side(T.js_front, f, data.columns, data.rows, s + 1), side(T.js_back, data.back.slice(s * per, s * per + per), data.columns, data.rows, s + 1));
      card.append(sides);
      sheetsEl.append(card);
    }
    if (count > MAX_SHEETS) sheetsEl.insertAdjacentHTML('beforeend', `<p class="more">${fill('js_more', { n: count - MAX_SHEETS })}</p>`);
  };

  const load = async () => {
    const file = input.files[0];
    if (!file) return;
    if (!file.name.toLowerCase().endsWith('.pdf')) { showError(T.js_not_pdf); input.value = ''; return; }
    showError('');
    if (url) URL.revokeObjectURL(url);
    url = URL.createObjectURL(file);
    $('#open-pdf').href = url;
    $('#file-name').textContent = file.name;
    $('#file-meta').textContent = file.size < 1048576 ? Math.ceil(file.size / 1024) + ' KB' : (file.size / 1048576).toFixed(1) + ' MB';
    $('#preview-wrap').hidden = false;
    drop.hidden = true;
    try {
      pdf = await pdfjs.getDocument(await file.arrayBuffer()).promise;
    } catch {
      showError(T.js_bad_pdf); return;
    }
    thumbs = new Map();
    render();
  };

  const modeUi = () => {
    const m = form.mode.value;
    $('#edge-box').hidden = m !== 'duplex';
    $('#nup-box').hidden = m === 'booklet';
    $('#booklet-hint').hidden = m !== 'booklet';
  };

  modeUi();
  input.addEventListener('change', load);
  $('#change-file').addEventListener('click', () => input.click());
  form.addEventListener('change', e => { modeUi(); if (![input, form.nup, form.pages, form.copies].includes(e.target)) render(); });
  form.pages.addEventListener('input', render);
  form.copies.addEventListener('input', render);
  form.nup.addEventListener('input', render);
  drop.addEventListener('dragover', e => { e.preventDefault(); drop.classList.add('drag'); });
  drop.addEventListener('dragleave', () => drop.classList.remove('drag'));
  drop.addEventListener('drop', e => {
    e.preventDefault();
    drop.classList.remove('drag');
    if (e.dataTransfer.files.length) { input.files = e.dataTransfer.files; load(); }
  });
}
