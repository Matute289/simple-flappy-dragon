import init, { start_game, get_player_y, flap, begin_play, pause_game, resume_game } from './pkg/flappy_dragon.js';

const API = '/api/scores';
let currentPlayerName = '';
let currentMode = 'new'; // 'classic' | 'new'
let dragonLoopActive = false;
let dragonAnimId = null;

const isTouchDevice = 'ontouchstart' in window || navigator.maxTouchPoints > 0;
let gameActive = false;
let gamePaused = false;
let canvasScale = 1.0; // updated by fitCanvas(), used to scale dragon
let isLandscape = window.innerWidth > window.innerHeight; // tracks orientation for smart pause

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
    const cellW  = rect.width  / 83;
    const py = get_player_y();
    const dragonW = Math.round(48 * canvasScale);
    dragon.style.width = dragonW + 'px';
    dragon.style.left = (rect.left + cellW * 3.5 - dragonW / 2) + 'px';
    dragon.style.top  = (rect.top  + py * cellH + cellH / 2 - 7) + 'px';
    dragonAnimId = requestAnimationFrame(loop);
  }
  loop();
}

function stopDragonLoop() {
  dragonLoopActive = false;
  if (dragonAnimId) { cancelAnimationFrame(dragonAnimId); dragonAnimId = null; }
  document.getElementById('dragon-sprite').style.display = 'none';
}

function fitCanvas() {
  const canvas = document.getElementById('canvas');
  if (!canvas.offsetWidth) return;
  const scaleX = window.innerWidth  / canvas.offsetWidth;
  const scaleY = window.innerHeight / canvas.offsetHeight;
  let factor = Math.min(scaleX, scaleY);
  if (!isTouchDevice) factor = Math.min(factor, 1.5); // desktop: max 1.5x upscale
  canvasScale = factor;
  canvas.style.transform = `translate(-50%, -50%) scale(${factor})`;
}

function showTapHint() {
  if (!isTouchDevice) return;
  document.getElementById('tap-hint').style.display = 'block';
}

function dismissTapHint() {
  document.getElementById('tap-hint').style.display = 'none';
}

function showReadyOverlay(classic) {
  fitCanvas(); // canvas now has its correct bracket-lib size (called after start_game)
  const overlay  = document.getElementById('ready-overlay');
  const content  = document.getElementById('ready-content');
  const hint = isTouchDevice ? 'TAP para comenzar' : 'SPACE para comenzar';

  if (classic) {
    overlay.style.background = 'rgba(0,0,0,0.75)';
    const action = isTouchDevice ? '> TAP TO GO <' : '> PRESS SPACE <';
    content.innerHTML =
      `<div class="ready-retro-box">` +
      `▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓<br>` +
      `▓&nbsp;&nbsp;&nbsp;FLAPPY&nbsp;&nbsp;DRAGON&nbsp;&nbsp;&nbsp;▓<br>` +
      `▓&nbsp;&nbsp;&nbsp;&nbsp;${action}&nbsp;&nbsp;&nbsp;&nbsp;▓<br>` +
      `▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓▓` +
      `</div>`;
  } else {
    overlay.style.background = 'transparent';
    content.innerHTML =
      `<div class="ready-title">🐉&nbsp;&nbsp;FLAPPY DRAGON</div>` +
      `<div class="ready-hint">${hint}</div>`;
  }

  overlay.style.display = 'flex';
}

function hideReadyOverlay() {
  document.getElementById('ready-overlay').style.display = 'none';
}

function showPauseBtn() {
  if (isTouchDevice) document.getElementById('pause-btn').style.display = 'flex';
}
function hidePauseBtn() {
  document.getElementById('pause-btn').style.display = 'none';
}

function beginPlay() {
  hideReadyOverlay();
  begin_play();
  gameActive = true;
  if (currentMode !== 'classic') startDragonLoop();
  showTapHint();
  showPauseBtn();
}

function showPauseOverlay(reason) {
  gamePaused = true;
  hidePauseBtn();
  const hintEl = document.getElementById('pause-hint-text');
  hintEl.textContent = isTouchDevice ? 'TAP para continuar' : 'SPACE / ESC para continuar';
  document.getElementById('pause-overlay').style.display = 'flex';
}

function resumeFromPause() {
  resume_game();
  document.getElementById('pause-overlay').style.display = 'none';
  gamePaused = false;
  if (gameActive) showPauseBtn();
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
  document.getElementById('win-overlay').style.display = 'none';
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

async function showWin(score) {
  stopDragonLoop();

  document.getElementById('win-player-name').textContent =
    currentPlayerName || 'ANONYMOUS';
  document.getElementById('win-score-display').textContent = `${score} pts`;

  if (currentPlayerName && score > 0) {
    await postScore(currentPlayerName, score);
  }

  document.getElementById('win-overlay').style.display    = 'flex';
  document.getElementById('menu-overlay').style.display   = 'none';
  document.getElementById('gameover-overlay').style.display = 'none';
  document.getElementById('scores-overlay').style.display = 'none';
}

// ── Called by Rust WASM ──────────────────────────────
window.on_game_over = async (score) => {
  gameActive = false;
  gamePaused = false;
  dismissTapHint();
  hideReadyOverlay();
  hidePauseBtn();
  document.getElementById('pause-overlay').style.display = 'none';
  await showGameOver(score);
};
window.on_game_win = async (score) => {
  gameActive = false;
  gamePaused = false;
  dismissTapHint();
  hideReadyOverlay();
  hidePauseBtn();
  document.getElementById('pause-overlay').style.display = 'none';
  await showWin(score);
};

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
  // fitCanvas() intentionally NOT called here — canvas has default 300×150 size before
  // start_game() runs. fitCanvas() is called inside showReadyOverlay() instead.

  classicBtn.disabled = false;
  newBtn.disabled     = false;

  classicBtn.addEventListener('click', () => {
    currentPlayerName = document.getElementById('name-input').value.trim();
    currentMode = 'classic';
    stopDragonLoop();
    hideAllOverlays();
    start_game(true);
    showReadyOverlay(true);
  });

  newBtn.addEventListener('click', () => {
    currentPlayerName = document.getElementById('name-input').value.trim();
    currentMode = 'new';
    stopDragonLoop();
    hideAllOverlays();
    start_game(false);
    showReadyOverlay(false);
  });

  document.getElementById('scores-btn').addEventListener('click', async () => {
    await showScores();
  });

  document.getElementById('pause-btn').addEventListener('click', () => {
    if (gameActive && !gamePaused) {
      pause_game();
      showPauseOverlay('manual');
    }
  });

  document.getElementById('go-play-again-btn').addEventListener('click', () => {
    stopDragonLoop();
    hideAllOverlays();
    const isClassic = (currentMode === 'classic');
    start_game(isClassic);
    showReadyOverlay(isClassic);
  });

  document.getElementById('go-menu-btn').addEventListener('click', () => {
    stopDragonLoop();
    hidePauseBtn();
    showMenu();
  });

  document.getElementById('scores-back-btn').addEventListener('click', () => {
    showMenu();
  });

  document.getElementById('win-play-again-btn').addEventListener('click', () => {
    stopDragonLoop();
    document.getElementById('win-overlay').style.display = 'none';
    hideAllOverlays();
    const isClassic = (currentMode === 'classic');
    start_game(isClassic);
    showReadyOverlay(isClassic);
  });

  document.getElementById('win-menu-btn').addEventListener('click', () => {
    document.getElementById('win-overlay').style.display = 'none';
    stopDragonLoop();
    showMenu();
  });

  // ── Responsive / input event listeners ──────────────

  window.addEventListener('resize', () => {
    const nowLandscape = window.innerWidth > window.innerHeight;
    fitCanvas();
    if (isTouchDevice && gameActive && !gamePaused && nowLandscape !== isLandscape) {
      pause_game();
      showPauseOverlay('rotation');
    }
    isLandscape = nowLandscape;
  });

  document.addEventListener('visibilitychange', () => {
    if (document.hidden && gameActive && !gamePaused) {
      pause_game();
      showPauseOverlay('background');
    }
  });

  document.addEventListener('keydown', (e) => {
    if (e.key === ' ') {
      const readyOverlay = document.getElementById('ready-overlay');
      if (readyOverlay.style.display !== 'none') {
        // Rust's wait() only watches BEGIN_REQUESTED (not keyboard), so JS must call begin_play()
        beginPlay();
        return;
      }
      if (gamePaused) {
        resumeFromPause();
        return;
      }
    }
    if (e.key === 'Escape' && (gameActive || gamePaused)) {
      if (gamePaused) { resumeFromPause(); }
      else            { pause_game(); showPauseOverlay('manual'); }
    }
  });

  document.addEventListener('touchstart', (e) => {
    if (gamePaused) { e.preventDefault(); resumeFromPause(); return; }
    const readyOverlay = document.getElementById('ready-overlay');
    if (readyOverlay.style.display !== 'none') {
      e.preventDefault();
      beginPlay();
      return;
    }
    if (!gameActive) return;
    if (e.target.closest('#pause-btn')) return; // let the button's click handler fire
    e.preventDefault();
    flap();
    dismissTapHint();
  }, { passive: false });
});
