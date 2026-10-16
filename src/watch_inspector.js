(() => {
  const toggle = document.getElementById('inspect-toggle');
  const panel = document.getElementById('inspector');
  const picker = document.getElementById('inspect-entity');
  const filePicker = document.getElementById('inspect-file');
  const details = document.getElementById('inspect-details');
  const relations = document.getElementById('inspect-relations');
  const conflicts = document.getElementById('inspect-conflicts');
  const source = document.getElementById('inspect-source');
  const namespace = 'http://www.w3.org/2000/svg';
  let data, selected, overlay;
  toggle.disabled = true;

  const element = (tag, text, className) => {
    const item = document.createElement(tag);
    if (text !== undefined) item.textContent = text;
    if (className) item.className = className;
    return item;
  };
  const svgElement = (tag, attributes) => {
    const item = document.createElementNS(namespace, tag);
    for (const [key, value] of Object.entries(attributes)) item.setAttribute(key, value);
    return item;
  };
  const number = value => Number(value.toFixed(2));
  const label = location => location.file.split('/').pop() + ':' + location.line + ':' + location.column;
  const relatedAt = (file, line) => data.entities.filter(entity =>
    [entity.declaration, ...entity.relations.map(relation => relation.source)]
      .some(location => location.file === file && location.line <= line && line <= location.end_line));

  function showSource(location) {
    filePicker.value = location.file;
    source.replaceChildren();
    data.files[location.file].split('\n').forEach((text, index) => {
      const line = index + 1;
      const button = element('button', String(line).padStart(3) + '  ' + text);
      button.type = 'button';
      button.dataset.line = line;
      button.classList.toggle('active', location.line <= line && line <= location.end_line);
      button.addEventListener('click', () => {
        const entities = relatedAt(location.file, line);
        const entity = entities.find(entity => entity.id === selected?.id) || entities[0];
        if (entity) select(entity.id, { file: location.file, line, end_line: line, column: 1 });
      });
      source.append(button);
    });
    source.querySelector('.active')?.scrollIntoView({ block: 'nearest' });
  }

  function sourceButton(text, location, action) {
    const button = element('button', text + ' · ' + label(location));
    button.type = 'button';
    button.addEventListener('click', () => {
      if (action) action();
      showSource(location);
    });
    return button;
  }

  function select(id, location) {
    selected = data.entities.find(entity => entity.id === id);
    if (!selected) return;
    picker.value = id;
    try { sessionStorage.setItem(KEY + ':entity', id); } catch (error) { /* storage disabled */ }
    details.replaceChildren();
    details.append(element('p', selected.kind + ' ' + id));
    if (selected.parent) {
      const parent = element('button', 'Parent: ' + selected.parent);
      parent.type = 'button';
      parent.addEventListener('click', () => select(selected.parent));
      details.append(parent);
    }
    if (selected.bounds) {
      const box = selected.bounds;
      details.append(element('p', 'Bounds: x ' + number(box.x) + ', y ' + number(box.y) +
        ' · ' + number(box.width) + ' × ' + number(box.height)));
      details.append(element('p', 'Blue: selected bounds and anchors · amber: related entities'));
      details.append(element('p', 'Anchors: ' + anchors(box).map(([name, x, y]) =>
        name + ' (' + number(x) + ', ' + number(y) + ')').join(' · ')));
    } else {
      details.append(element('p', data.geometry_available
        ? 'This archetype manages the drawing without exposing individual entity bounds.'
        : 'No solved bounds. Follow the source relations and diagnostics below.'));
    }
    details.append(element('pre', selected.reason));
    relations.replaceChildren(sourceButton('Declaration', selected.declaration));
    for (const relation of selected.relations) {
      relations.append(sourceButton(relation.axes + ' · ' + relation.source.text.trim(), relation.source));
    }
    if (!selected.relations.length) relations.append(element('p', 'No explicit layout relation.'));
    conflicts.replaceChildren();
    for (const diagnostic of data.diagnostics) {
      if (diagnostic.entities.length && !diagnostic.entities.includes(id)) continue;
      const button = sourceButton(diagnostic.severity + ': ' + diagnostic.message, diagnostic.source);
      button.classList.add(diagnostic.severity);
      conflicts.append(button);
    }
    if (!conflicts.childElementCount) conflicts.append(element('p', 'No related diagnostics.'));
    showSource(location || selected.declaration);
    draw();
  }

  function anchors(box) {
    return [
      ['left', box.x, box.y + box.height / 2], ['right', box.x + box.width, box.y + box.height / 2],
      ['top', box.x + box.width / 2, box.y], ['bottom', box.x + box.width / 2, box.y + box.height],
      ['center', box.x + box.width / 2, box.y + box.height / 2],
    ];
  }

  function draw() {
    overlay?.remove();
    const svg = canvas.querySelector('svg');
    if (!svg || panel.hidden || !data) return;
    overlay = svgElement('g', { id: 'inspect-overlay', transform: 'translate(' + data.offset.join(',') + ')' });
    svg.append(overlay);
    const boxes = data.entities.filter(entity => entity.bounds).sort((a, b) =>
      b.bounds.width * b.bounds.height - a.bounds.width * a.bounds.height);
    for (const entity of boxes) {
      const box = entity.bounds;
      const hit = svgElement('rect', { x: box.x, y: box.y, width: box.width, height: box.height,
        fill: 'transparent', class: 'inspect-hit', tabindex: 0, role: 'button', 'aria-label': 'Inspect ' + entity.id });
      hit.dataset.entity = entity.id;
      hit.addEventListener('pointerdown', event => event.stopPropagation());
      hit.addEventListener('click', event => { event.stopPropagation(); select(entity.id); });
      hit.addEventListener('keydown', event => {
        if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); select(entity.id); }
      });
      overlay.append(hit);
    }
    if (!selected?.bounds) return;
    const peers = new Set([selected.anchor, selected.parent,
      ...selected.relations.flatMap(relation => relation.peers)].filter(Boolean));
    for (const entity of boxes.filter(entity => entity.id === selected.id || peers.has(entity.id))) {
      const box = entity.bounds;
      const isSelected = entity.id === selected.id;
      overlay.append(svgElement('rect', { x: box.x, y: box.y, width: box.width, height: box.height,
        fill: 'none', stroke: isSelected ? '#0284c7' : '#d97706', 'stroke-width': 2,
        'stroke-dasharray': isSelected ? 'none' : '5 3', 'pointer-events': 'none', 'vector-effect': 'non-scaling-stroke' }));
      if (!isSelected) {
        const from = selected.bounds;
        overlay.append(svgElement('line', { x1: from.x + from.width / 2, y1: from.y + from.height / 2,
          x2: box.x + box.width / 2, y2: box.y + box.height / 2,
          stroke: '#d97706', 'stroke-width': 1, 'stroke-dasharray': '3 3', 'pointer-events': 'none' }));
      }
    }
    const box = selected.bounds;
    for (const [name, x, y] of anchors(box)) {
      const circle = svgElement('circle', { cx: x, cy: y, r: 3, fill: '#0284c7', 'pointer-events': 'none' });
      const title = svgElement('title', {});
      title.textContent = name + ' (' + number(x) + ', ' + number(y) + ')';
      circle.append(title);
      overlay.append(circle);
    }
  }

  function open(value) {
    panel.hidden = !value;
    toggle.setAttribute('aria-expanded', String(value));
    document.body.classList.toggle('inspect-open', value);
    try { sessionStorage.setItem(KEY + ':inspect', String(value)); } catch (error) { /* storage disabled */ }
    draw();
    fit();
  }
  toggle.addEventListener('click', () => open(panel.hidden));
  document.getElementById('inspect-close').addEventListener('click', () => open(false));
  picker.addEventListener('change', () => select(picker.value));
  filePicker.addEventListener('change', () => showSource({ file: filePicker.value, line: 1, end_line: 1, column: 1 }));
  addEventListener('keydown', event => {
    if (event.key === 'Escape' && !panel.hidden) { open(false); toggle.focus(); }
  });

  fetch('/inspect', { cache: 'no-store' }).then(response => response.json()).then(payload => {
    data = payload;
    if (!data.available) {
      if (data.error) { toggle.textContent = 'Inspector: ' + data.error; toggle.title = data.error; }
      else toggle.hidden = true;
      return;
    }
    for (const entity of data.entities) {
      const option = element('option', entity.kind + ' · ' + entity.id);
      option.value = entity.id;
      picker.append(option);
    }
    for (const file of Object.keys(data.files)) {
      const option = element('option', file);
      option.value = file;
      filePicker.append(option);
    }
    toggle.disabled = false;
    let remembered, wasOpen;
    try { remembered = sessionStorage.getItem(KEY + ':entity'); wasOpen = sessionStorage.getItem(KEY + ':inspect') === 'true'; } catch (error) { /* storage disabled */ }
    select(data.entities.some(entity => entity.id === remembered) ? remembered : data.entities[0]?.id);
    if (!data.entities.length) {
      details.append(element('p', 'No entity could be recovered. Select a diagnostic source.'));
      for (const diagnostic of data.diagnostics) conflicts.append(sourceButton(diagnostic.message, diagnostic.source));
    }
    if (wasOpen) open(true);
  }).catch(error => { toggle.textContent = 'Inspector unavailable'; toggle.title = error.message; });
})();
