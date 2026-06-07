import init, { start_game } from './pkg/flappy_dragon.js';

const API = '/api/scores';
let currentPlayerName = '';

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
  if (currentPlayerName) {
    await postScore(currentPlayerName, score);
  }
  const scores = await fetchScores();

  document.getElementById('go-player-name').textContent =
    currentPlayerName || 'ANONYMOUS';
  document.getElementById('go-score').textContent = `${score} pts`;

  const posEl  = document.getElementById('go-position');
  const anonEl = document.getElementById('go-anon');
  if (currentPlayerName) {
    const rank = getPlayerRank(scores, currentPlayerName, score);
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
    scores,
    currentPlayerName || null,
    currentPlayerName ? score : null,
    5
  );

  document.getElementById('gameover-overlay').style.display = 'flex';
  document.getElementById('menu-overlay').style.display = 'none';
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

  const playBtn = document.getElementById('play-btn');
  playBtn.disabled = true;
  await init();
  playBtn.disabled = false;

  playBtn.addEventListener('click', () => {
    currentPlayerName = document.getElementById('name-input').value.trim();
    hideAllOverlays();
    start_game();
  });

  document.getElementById('scores-btn').addEventListener('click', async () => {
    await showScores();
  });

  document.getElementById('go-play-again-btn').addEventListener('click', () => {
    hideAllOverlays();
    start_game();
  });

  document.getElementById('go-menu-btn').addEventListener('click', () => {
    showMenu();
  });

  document.getElementById('scores-back-btn').addEventListener('click', () => {
    showMenu();
  });
});
