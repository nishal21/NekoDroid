import './style.css';
import init, {
  VirtualCPU,
  init_emulator,
  execute_cycle,
  get_cycle_count,
  wasm_memory,
  send_touch_event,
  send_key_event,
} from '../pkg/nekodroid.js';

// ── Types ──────────────────────────────────────────────────────────────
type RenderMode = 'noise' | 'gradient' | 'plasma';

// ── Build the UI ───────────────────────────────────────────────────────
const app = document.querySelector<HTMLDivElement>('#app')!;

app.innerHTML = `
  <div class="container">
    <header class="header">
      <div class="logo-glow"></div>
      <h1>🐱 nekodroid</h1>
      <p class="subtitle">Wasm CPU Emulator — Framebuffer Test</p>
    </header>

    <div class="status-panel">
      <div class="status-indicator" id="status-indicator">
        <span class="dot"></span>
        <span id="status-text">Initializing Wasm module...</span>
      </div>
    </div>

    <div class="canvas-wrapper" id="canvas-wrapper"></div>

    <div class="controls">
      <button id="btn-noise" class="active">
        <span class="btn-icon">📺</span>
        Noise
      </button>
      <button id="btn-gradient">
        <span class="btn-icon">🌈</span>
        Gradient
      </button>
      <button id="btn-plasma">
        <span class="btn-icon">🔮</span>
        Plasma
      </button>
      <button id="btn-pause">
        <span class="btn-icon">⏸️</span>
        Pause
      </button>
    </div>

    <div class="output-panel">
      <div class="metrics">
        <div class="metric">
          <span class="metric-label">FPS</span>
          <span class="metric-value" id="fps-display">0</span>
        </div>
        <div class="metric">
          <span class="metric-label">Frames</span>
          <span class="metric-value" id="frame-count">0</span>
        </div>
        <div class="metric">
          <span class="metric-label">CPU Cycles</span>
          <span class="metric-value" id="cycle-count">0</span>
        </div>
        <div class="metric">
          <span class="metric-label">Mode</span>
          <span class="metric-value" id="mode-display">noise</span>
        </div>
      </div>
      <div class="console-log" id="console-log">
        <div class="log-entry log-system">Awaiting Wasm initialization...</div>
      </div>
    </div>

    <footer class="footer">
      <p>Rust → wasm-bindgen → Wasm Memory → TypeScript → Canvas (ImageData)</p>
    </footer>
  </div>
`;

// ── Move the canvas into our wrapper ───────────────────────────────────
const canvas = document.getElementById('screen') as HTMLCanvasElement;
const canvasWrapper = document.getElementById('canvas-wrapper')!;
canvasWrapper.appendChild(canvas);

// ── DOM refs ───────────────────────────────────────────────────────────
const btnNoise = document.getElementById('btn-noise') as HTMLButtonElement;
const btnGradient = document.getElementById('btn-gradient') as HTMLButtonElement;
const btnPlasma = document.getElementById('btn-plasma') as HTMLButtonElement;
const btnPause = document.getElementById('btn-pause') as HTMLButtonElement;
const fpsDisplay = document.getElementById('fps-display')!;
const frameCountEl = document.getElementById('frame-count')!;
const cycleCountEl = document.getElementById('cycle-count')!;
const modeDisplay = document.getElementById('mode-display')!;
const consoleLog = document.getElementById('console-log')!;
const statusText = document.getElementById('status-text')!;
const statusIndicator = document.getElementById('status-indicator')!;

// ── Helpers ────────────────────────────────────────────────────────────
function addLog(message: string, type: 'info' | 'success' | 'system' = 'info') {
  const entry = document.createElement('div');
  entry.className = `log-entry log-${type}`;
  const ts = new Date().toLocaleTimeString('en-US', { hour12: false });
  entry.textContent = `[${ts}] ${message}`;
  consoleLog.appendChild(entry);
  consoleLog.scrollTop = consoleLog.scrollHeight;
  // Keep log size manageable
  while (consoleLog.children.length > 100) {
    consoleLog.removeChild(consoleLog.firstChild!);
  }
}

// ── Main ───────────────────────────────────────────────────────────────
async function main() {
  try {
    // Initialize Wasm
    await init();
    init_emulator();

    // Create VirtualCPU with framebuffer
    const cpu = new VirtualCPU();
    const ctx = canvas.getContext('2d')!;
    const screenWidth = cpu.width();
    const screenHeight = cpu.height();

    statusText.textContent = `Wasm loaded ✓ — Framebuffer ${screenWidth}×${screenHeight}`;
    statusIndicator.classList.add('online');
    addLog(`VirtualCPU created: ${screenWidth}×${screenHeight} RGBA framebuffer (${cpu.framebuffer_len()} bytes)`, 'success');

    // ── Render state ──────────────────────────────────────────────
    let mode: RenderMode = 'noise';
    let paused = false;
    let frameNumber = 0;
    let lastTime = performance.now();
    let fpsFrames = 0;
    let fpsAccum = 0;

    // ── Render loop ───────────────────────────────────────────────
    function renderFrame(now: number) {
      if (!paused) {
        // Compute FPS
        const dt = now - lastTime;
        lastTime = now;
        fpsAccum += dt;
        fpsFrames++;
        if (fpsAccum >= 500) {
          const fps = Math.round((fpsFrames / fpsAccum) * 1000);
          fpsDisplay.textContent = fps.toString();
          fpsFrames = 0;
          fpsAccum = 0;
        }

        // Tell the Wasm side to render into its framebuffer
        switch (mode) {
          case 'noise':
            cpu.render_noise();
            break;
          case 'gradient':
            cpu.render_gradient(frameNumber);
            break;
          case 'plasma':
            cpu.render_plasma(frameNumber * 0.03);
            break;
        }

        // Execute a CPU cycle per frame
        execute_cycle();
        frameNumber++;

        // ── Read Wasm memory directly via the pointer ─────────
        const ptr = cpu.framebuffer_ptr();
        const len = cpu.framebuffer_len();
        const mem = wasm_memory() as WebAssembly.Memory;
        const wasmMemory = new Uint8ClampedArray(
          mem.buffer,
          ptr,
          len
        );

        // Create ImageData from the Wasm framebuffer and draw to canvas
        const imageData = new ImageData(wasmMemory, screenWidth, screenHeight);
        ctx.putImageData(imageData, 0, 0);

        // Update metrics (throttled to avoid layout thrashing)
        if (frameNumber % 10 === 0) {
          frameCountEl.textContent = frameNumber.toString();
          cycleCountEl.textContent = get_cycle_count().toString();
        }
      }

      requestAnimationFrame(renderFrame);
    }

    requestAnimationFrame(renderFrame);
    addLog('Render loop started at 60 FPS target', 'success');

    // ── Mode buttons ──────────────────────────────────────────────
    const modeButtons = [btnNoise, btnGradient, btnPlasma];

    function setMode(newMode: RenderMode, btn: HTMLButtonElement) {
      mode = newMode;
      modeDisplay.textContent = newMode;
      modeButtons.forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
      addLog(`Render mode → ${newMode}`);
    }

    btnNoise.addEventListener('click', () => setMode('noise', btnNoise));
    btnGradient.addEventListener('click', () => setMode('gradient', btnGradient));
    btnPlasma.addEventListener('click', () => setMode('plasma', btnPlasma));

    btnPause.addEventListener('click', () => {
      paused = !paused;
      btnPause.querySelector('.btn-icon')!.textContent = paused ? '▶️' : '⏸️';
      btnPause.childNodes[1].textContent = paused ? ' Resume' : ' Pause';
      addLog(paused ? 'Rendering paused' : 'Rendering resumed');
    });

    // ── Input event pipeline ──────────────────────────────────────────
    // Translate browser CSS pixel coordinates → framebuffer pixel coordinates
    function canvasCoords(e: MouseEvent): [number, number] {
      const rect = canvas.getBoundingClientRect();
      const scaleX = screenWidth / rect.width;
      const scaleY = screenHeight / rect.height;
      const x = Math.floor((e.clientX - rect.left) * scaleX);
      const y = Math.floor((e.clientY - rect.top) * scaleY);
      return [x, y];
    }

    let isMouseDown = false;

    canvas.addEventListener('mousedown', (e) => {
      isMouseDown = true;
      const [x, y] = canvasCoords(e);
      send_touch_event(x, y, true);
      addLog(`Touch DOWN at (${x}, ${y})`);
    });

    canvas.addEventListener('mousemove', (e) => {
      if (!isMouseDown) return;
      const [x, y] = canvasCoords(e);
      send_touch_event(x, y, true);
    });

    canvas.addEventListener('mouseup', (e) => {
      isMouseDown = false;
      const [x, y] = canvasCoords(e);
      send_touch_event(x, y, false);
      addLog(`Touch UP at (${x}, ${y})`);
    });

    canvas.addEventListener('mouseleave', () => {
      if (isMouseDown) {
        isMouseDown = false;
        send_touch_event(-1, -1, false);
        addLog('Touch cancelled (cursor left canvas)');
      }
    });

    // Keyboard input — canvas needs to be focusable
    canvas.setAttribute('tabindex', '0');
    canvas.addEventListener('keydown', (e) => {
      e.preventDefault();
      send_key_event(e.keyCode);
      addLog(`Key pressed: ${e.key} (code=${e.keyCode})`);
    });

    addLog('Input pipeline active: mouse + keyboard → Wasm', 'success');

  } catch (err) {
    statusText.textContent = 'Failed to load Wasm module ✗';
    statusIndicator.classList.add('error');
    addLog(`Error: ${err}`, 'system');
    console.error('Wasm init failed:', err);
  }
}

main();
