import init, { start_game, get_player_y } from './pkg/flappy_dragon.js';

const API = '/api/scores';
let currentPlayerName = '';
let currentMode = 'new'; // 'classic' | 'new'
let dragonLoopActive = false;
let dragonAnimId = null;

function startDragonLoop() {
  if (dragonLoopActive) return; // already running, prevent duplicate rAF chains
  dragonLoopActive = true;
  const dragon = document.getElementById('dragon-sprite');
  dragon.style.display = 'block';
  const canvas = document.getElementById('canvas');

  function loop() {
    if (!dragonLoopActive) return;
    const rect = canvas.getBoundingClientRect();
    const cellH = rect.height / 50;
    const cellW  = rect.width  / 80;
    const py = get_player_y();
    dragon.style.left = (rect.left + cellW * 0.5 - 20) + 'px';
    dragon.style.top  = (rect.top  + py * cellH + cellH / 2 - 20) + 'px';
    dragonAnimId = requestAnimationFrame(loop);
  }
  loop();
}

function stopDragonLoop() {
  dragonLoopActive = false;
  if (dragonAnimId) { cancelAnimationFrame(dragonAnimId); dragonAnimId = null; }
  document.getElementById('dragon-sprite').style.display = 'none';
}

// ── API ──────────────────────────────────────────────

async function fetchScores() {
  try {
    const res = await fetch(API);
    if (!res.ok) return [];
    return await res.json();
  } catch {
    return [];
  }
}

async function postScore(name, score) {
  try {
    await fetch(API, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name, score }),
    });
  } catch {
    // fire and forget
  }
}

// ── Score table rendering ────────────────────────────

function getPlayerRank(scores, name, score) {
  if (!name) return null;
  const idx = scores.findIndex(e => e.name === name && e.score === score);
  return idx === -1 ? null : idx + 1;
}

const MEDALS = ['🥇', '🥈', '🥉'];

function escHtml(s) {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function renderScoreTable(tableEl, scores, highlightName, highlightScore, limit) {
  const rows = scores.slice(0, limit);
  tableEl.innerHTML =
    `<thead><tr><th>#</th><th>NAME</th><th>PTS</th></tr></thead>` +
    `<tbody>` +
    rows.map((e, i) => {
      const isHL = highlightName && e.name === highlightName && e.score === highlightScore;
      const rank = MEDALS[i] ?? (i + 1);
      return `<tr class="${isHL ? 'highlight' : ''}">
        <td>${isHL ? '→ ' : ''}${rank}</td>
        <td>${escHtml(e.name)}</td>
        <td>${e.score}</td>
      </tr>`;
    }).join('') +
    `</tbody>`;
}

// ── Overlay management ───────────────────────────────

function showMenu() {
  document.getElementById('menu-overlay').style.display = 'flex';
  document.getElementById('gameover-overlay').style.display = 'none';
  document.getElementById('scores-overlay').style.display = 'none';
}

function hideAllOverlays() {
  document.getElementById('menu-overlay').style.display = 'none';
  document.getElementById('gameover-overlay').style.display = 'none';
  document.getElementById('scores-overlay').style.display = 'none';
}

async function showGameOver(score) {
  stopDragonLoop();

  // Fetch first, then decide whether to save
  const scores = await fetchScores();
  const lastScore = scores.length > 0 ? scores[scores.length - 1].score : -1;
  const shouldSave = !!(currentPlayerName && score > 0);

  if (shouldSave) {
    await postScore(currentPlayerName, score);
  }
  const finalScores = shouldSave ? await fetchScores() : scores;

  document.getElementById('go-player-name').textContent =
    currentPlayerName || 'ANONYMOUS';
  document.getElementById('go-score').textContent = `${score} pts`;

  const posEl  = document.getElementById('go-position');
  const anonEl = document.getElementById('go-anon');
  if (currentPlayerName && shouldSave) {
    const rank = getPlayerRank(finalScores, currentPlayerName, score);
    posEl.innerHTML = rank
      ? `Position <span>#${rank}</span>`
      : `Position <span>unranked</span>`;
    posEl.style.display = 'block';
    anonEl.style.display = 'none';
  } else {
    posEl.style.display = 'none';
    anonEl.style.display = 'block';
  }

  renderScoreTable(
    document.getElementById('go-score-table'),
    finalScores,
    currentPlayerName || null,
    (currentPlayerName && shouldSave) ? score : null,
    5
  );

  document.getElementById('gameover-overlay').style.display = 'flex';
  document.getElementById('menu-overlay').style.display   = 'none';
  document.getElementById('scores-overlay').style.display = 'none';
}

async function showScores() {
  const scores = await fetchScores();
  renderScoreTable(
    document.getElementById('scores-table'),
    scores,
    null, null,
    20
  );
  document.getElementById('scores-overlay').style.display = 'flex';
  document.getElementById('menu-overlay').style.display = 'none';
  document.getElementById('gameover-overlay').style.display = 'none';
}

// ── Called by Rust WASM ──────────────────────────────
window.on_game_over = async (score) => { await showGameOver(score); };

// ── Menu background animation ────────────────────────

function initMenuBackground() {
  const bg = document.getElementById('menu-bg');

  const dragon = document.createElement('div');
  dragon.className = 'anim-dragon';
  dragon.textContent = '🐉';
  bg.appendChild(dragon);

  // Three pipe pairs with different heights and timing offsets
  [
    [28, 22, 6.0, 0],
    [35, 18, 6.0, 2],
    [22, 30, 6.0, 4],
  ].forEach(([topH, botH, dur, delay]) => {
    const pair = document.createElement('div');
    pair.className = 'pipe-pair';
    pair.style.cssText = `animation-duration:${dur}s;animation-delay:-${delay}s`;
    const top = document.createElement('div');
    top.className = 'pipe-top';
    top.style.height = `${topH}%`;
    const bot = document.createElement('div');
    bot.className = 'pipe-bottom';
    bot.style.height = `${botH}%`;
    pair.appendChild(top);
    pair.appendChild(bot);
    bg.appendChild(pair);
  });
}

// ── Boot ─────────────────────────────────────────────

document.addEventListener('DOMContentLoaded', async () => {
  initMenuBackground();
  showMenu();

  const classicBtn = document.getElementById('play-classic-btn');
  const newBtn     = document.getElementById('play-new-btn');
  classicBtn.disabled = true;
  newBtn.disabled     = true;

  await init();

  classicBtn.disabled = false;
  newBtn.disabled     = false;

  classicBtn.addEventListener('click', () => {
    currentPlayerName = document.getElementById('name-input').value.trim();
    currentMode = 'classic';
    stopDragonLoop();
    hideAllOverlays();
    start_game(true);
  });

  newBtn.addEventListener('click', () => {
    currentPlayerName = document.getElementById('name-input').value.trim();
    currentMode = 'new';
    hideAllOverlays();
    start_game(false);
    startDragonLoop();
  });

  document.getElementById('scores-btn').addEventListener('click', async () => {
    await showScores();
  });

  document.getElementById('go-play-again-btn').addEventListener('click', () => {
    hideAllOverlays();
    const isClassic = (currentMode === 'classic');
    start_game(isClassic);
    if (!isClassic) startDragonLoop();
  });

  document.getElementById('go-menu-btn').addEventListener('click', () => {
    stopDragonLoop();
    showMenu();
  });

  document.getElementById('scores-back-btn').addEventListener('click', () => {
    showMenu();
  });
});
