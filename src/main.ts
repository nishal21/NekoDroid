import './style.css';
import init, {
  VirtualCPU,
  init_emulator,
  get_cycle_count,
  wasm_memory,
  send_touch_event,
  send_key_event,
  get_cpu_state,
  step_cpu,
  run_batch,
  load_demo_program,
  load_custom_hex,
  load_rom,
  boot_linux_kernel,
  get_vram_ptr,
  get_vram_len,
  get_audio_ctrl,
  get_audio_freq,
} from '../pkg/nekodroid.js';

// ── Web Audio API state ────────────────────────────────────────────────
let audioCtx: AudioContext | null = null;
let oscillator: OscillatorNode | null = null;
let gainNode: GainNode | null = null;
let isAudioInitialized = false;
const WAVEFORMS: OscillatorType[] = ['square', 'sine', 'sawtooth', 'triangle'];

// ── Types ──────────────────────────────────────────────────────────────
type RenderMode = 'noise' | 'gradient' | 'plasma' | 'vram';

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
      <button id="btn-vram">
        <span class="btn-icon">🖥️</span>
        VRAM
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

    <div class="debug-panel" id="debug-panel">
      <div class="debug-header">
        <h2>🔧 ARMv7 CPU Debug</h2>
        <div class="debug-controls">
          <button id="btn-load-demo" class="debug-btn">
            <span class="btn-icon">📦</span> Load Demo
          </button>
          <button id="btn-load-uart" class="debug-btn" style="background: linear-gradient(135deg, #1a5a2a, #2d8a4e); border-color: #3dbb6e;">
            <span class="btn-icon">📟</span> Hello UART
          </button>
          <button id="btn-step" class="debug-btn">
            <span class="btn-icon">⏭️</span> Step
          </button>
          <button id="btn-run10" class="debug-btn">
            <span class="btn-icon">⏩</span> Run 10
          </button>
          <button id="btn-run" class="debug-btn" style="background: linear-gradient(135deg, #065f46, #059669); border-color: #10b981;">
            <span class="btn-icon">▶️</span> Run
          </button>
        </div>
      </div>
      <div class="register-grid" id="register-grid"></div>
      <div class="flags-row" id="flags-row">
        <span class="flag" id="flag-n">N</span>
        <span class="flag" id="flag-z">Z</span>
        <span class="flag" id="flag-c">C</span>
        <span class="flag" id="flag-v">V</span>
        <span class="flag flag-mode" id="flag-t">ARM</span>
      </div>
      <div class="disasm-panel" id="disasm-panel">
        <div class="disasm-header">DISASSEMBLY</div>
        <div class="disasm-lines" id="disasm-lines"></div>
      </div>
      <div class="hex-upload-panel">
        <div class="hex-upload-header">CUSTOM PROGRAM</div>
        <textarea id="hex-input" class="hex-input" rows="4"
          placeholder="Paste ARM hex: e3a00005 e3a0100a e0802001"></textarea>
        <button id="btn-upload-hex" class="debug-btn hex-upload-btn">
          <span class="btn-icon">⬆️</span> Upload to RAM
        </button>
        <div class="rom-upload-header" style="margin-top: 10px; font-family: var(--font-mono); font-size: 0.55rem; color: var(--text-muted); letter-spacing: 0.15em;">LOAD COMPILED ROM (.bin)</div>
        <input type="file" id="rom-file-input" accept=".bin" style="display: none;" />
        <button id="btn-upload-rom" class="debug-btn hex-upload-btn" style="background: linear-gradient(135deg, #4c1d95, #7c3aed); border-color: #a855f7;">
          <span class="btn-icon">💿</span> Select & Load .bin
        </button>
        <div class="linux-upload-header" style="margin-top: 10px; font-family: var(--font-mono); font-size: 0.55rem; color: var(--text-muted); letter-spacing: 0.15em;">BOOT LINUX KERNEL (.zImage / Image)</div>
        <input type="file" id="linux-file-input" accept=".zImage,.bin,Image" style="display: none;" />
        <button id="btn-upload-linux" class="debug-btn hex-upload-btn" style="background: linear-gradient(135deg, #059669, #10b981); border-color: #34d399;">
          <span class="btn-icon">🐧</span> Boot Linux zImage
        </button>
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
const btnVram = document.getElementById('btn-vram') as HTMLButtonElement;
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
    init_emulator(128); // RAM in MB — increase for larger apps (e.g. 512, 1024)

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
    let running = false;  // Continuous CPU execution mode
    const BATCH_SIZE = 200_000; // Instructions per frame (~10ms at 20M ips → 60 FPS)
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
          case 'vram':
            // VRAM mode: no render call needed — CPU writes directly to VRAM
            break;
        }

        // If running continuously, execute a batch of CPU instructions
        // Timer ticks every TIMER_INTERVAL instructions inside the batch
        // to prevent VSYNC-wait loops from burning the entire budget
        if (running) {
          const TIMER_INTERVAL = BATCH_SIZE; // 1 tick per frame → snake frame_skip=4 → 15 moves/sec
          const executed = run_batch(BATCH_SIZE, TIMER_INTERVAL);
          if (executed < BATCH_SIZE) {
            // CPU halted before finishing the batch
            running = false;
            const btnRun = document.getElementById('btn-run')!;
            btnRun.querySelector('.btn-icon')!.textContent = '▶️';
            btnRun.childNodes[1].textContent = ' Run';
            (btnRun as HTMLButtonElement).style.background = 'linear-gradient(135deg, #065f46, #059669)';
            (btnRun as HTMLButtonElement).style.borderColor = '#10b981';
            addLog(`CPU halted after ${get_cycle_count()} cycles`, 'success');
            updateDebugPanel();
          }
        }

        // Process deferred touch release AFTER batch (ensures CPU sees touch for ≥1 full frame)
        if (pendingRelease) {
          send_touch_event(pendingRelease.x, pendingRelease.y, false);
          addLog(`Touch UP at (${pendingRelease.x}, ${pendingRelease.y})`);
          pendingRelease = null;
        }
        // Process deferred key release AFTER batch (ensures CPU sees key for \u22651 full frame)
        if (pendingKeyRelease !== null) {
          send_key_event(pendingKeyRelease, false);
          pendingKeyRelease = null;
        }
        // ── Audio sync: read CPU audio registers → Web Audio API ────
        if (isAudioInitialized && oscillator && audioCtx) {
          const ctrl = get_audio_ctrl();
          const freq = get_audio_freq();

          const enabled = (ctrl & 1) === 1;
          const waveType = (ctrl >> 1) & 3;

          if (enabled) {
            if (audioCtx.state === 'suspended') audioCtx.resume();
            oscillator.type = WAVEFORMS[waveType];
            // Smoothly transition frequency to avoid audio popping
            oscillator.frequency.setTargetAtTime(freq, audioCtx.currentTime, 0.015);
          } else {
            if (audioCtx.state === 'running') audioCtx.suspend();
          }
        }

        frameNumber++;

        // ── Read Wasm memory directly via the pointer ─────────
        const mem = wasm_memory() as WebAssembly.Memory;
        let ptr: number;
        let len: number;

        if (mode === 'vram') {
          // Read from CPU's VRAM (written by ARM programs)
          ptr = get_vram_ptr();
          len = get_vram_len();
        } else {
          // Read from VirtualCPU's demo framebuffer
          ptr = cpu.framebuffer_ptr();
          len = cpu.framebuffer_len();
        }

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
    const modeButtons = [btnNoise, btnGradient, btnPlasma, btnVram];

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
    btnVram.addEventListener('click', () => setMode('vram', btnVram));

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
    let pendingRelease: { x: number; y: number } | null = null;

    // ── Audio initialization (requires user gesture) ───────────────
    function initAudio() {
      if (isAudioInitialized) return;
      audioCtx = new (window.AudioContext || (window as any).webkitAudioContext)();
      gainNode = audioCtx.createGain();
      gainNode.gain.value = 0.1; // Keep volume reasonable
      gainNode.connect(audioCtx.destination);

      oscillator = audioCtx.createOscillator();
      oscillator.connect(gainNode);
      oscillator.start();
      isAudioInitialized = true;
      addLog('🔊 Web Audio initialized', 'success');
    }
    canvas.addEventListener('mousedown', initAudio, { once: true });

    canvas.addEventListener('mousedown', (e) => {
      pendingRelease = null; // cancel any pending release
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
      // Defer release to next frame so CPU sees touch_down=true for ≥1 frame
      pendingRelease = { x, y };
    });

    canvas.addEventListener('mouseleave', () => {
      if (isMouseDown) {
        isMouseDown = false;
        pendingRelease = { x: -1, y: -1 };
      }
    });

    // Keyboard input — listen on document so it works without canvas focus
    const KEY_CODE_MAP: Record<string, number> = {
      ArrowUp: 38, ArrowDown: 40, ArrowLeft: 37, ArrowRight: 39,
      KeyW: 38, KeyS: 40, KeyA: 37, KeyD: 39,
      Space: 32, Enter: 13, Escape: 27,
    };
    let pendingKeyRelease: number | null = null;
    document.addEventListener('keydown', (e) => {
      const code = KEY_CODE_MAP[e.code] ?? e.keyCode;
      if (code >= 37 && code <= 40) e.preventDefault(); // prevent page scroll on arrows
      pendingKeyRelease = null; // cancel any pending release
      send_key_event(code, true);
      addLog(`\u2328\ufe0f Key DOWN: ${e.code} \u2192 ${code}`);
    });
    document.addEventListener('keyup', (e) => {
      const code = KEY_CODE_MAP[e.code] ?? e.keyCode;
      // Defer release to after batch so CPU sees the key for \u22651 full frame
      pendingKeyRelease = code;
    });

    addLog('Input pipeline active: mouse + keyboard → Wasm', 'success');

    // ── Debug panel ─────────────────────────────────────────────────
    const registerGrid = document.getElementById('register-grid')!;
    const regNames = [
      'R0', 'R1', 'R2', 'R3', 'R4', 'R5', 'R6', 'R7',
      'R8', 'R9', 'R10', 'R11', 'R12', 'SP', 'LR', 'PC',
    ];

    // Build register cells
    registerGrid.innerHTML = regNames.map((name, i) =>
      `<div class="reg-cell" id="reg-${i}">
        <span class="reg-name">${name}</span>
        <span class="reg-value" id="reg-val-${i}">00000000</span>
      </div>`
    ).join('');

    const flagN = document.getElementById('flag-n')!;
    const flagZ = document.getElementById('flag-z')!;
    const flagC = document.getElementById('flag-c')!;
    const flagV = document.getElementById('flag-v')!;
    const flagT = document.getElementById('flag-t')!;

    function updateDebugPanel() {
      try {
        const state = JSON.parse(get_cpu_state());
        if (state.error) return;

        // Update register values
        for (let i = 0; i < 16; i++) {
          const el = document.getElementById(`reg-val-${i}`)!;
          const val = state.regs[i] >>> 0; // unsigned
          const hex = val.toString(16).toUpperCase().padStart(8, '0');
          const prevHex = el.textContent;
          el.textContent = hex;
          // Flash changed registers
          if (prevHex !== hex) {
            el.classList.add('reg-changed');
            setTimeout(() => el.classList.remove('reg-changed'), 300);
          }
        }

        // Update flags
        flagN.classList.toggle('flag-set', state.n);
        flagZ.classList.toggle('flag-set', state.z);
        flagC.classList.toggle('flag-set', state.c);
        flagV.classList.toggle('flag-set', state.v);
        flagT.textContent = state.t ? 'THUMB' : 'ARM';
        flagT.classList.toggle('flag-set', state.t);

        // Update disassembly
        const disasmEl = document.getElementById('disasm-lines')!;
        if (state.disasm && state.disasm.length > 0) {
          disasmEl.innerHTML = state.disasm.map((line: string, i: number) =>
            `<div class="disasm-line${i === 0 ? ' disasm-current' : ''}">${line}</div>`
          ).join('');
        }
      } catch (_) { /* ignore parse errors during init */ }
    }

    // Update debug panel at 5 Hz (200ms) to avoid performance impact
    setInterval(updateDebugPanel, 200);
    updateDebugPanel(); // initial render

    // ── Debug buttons ─────────────────────────────────────────────────
    document.getElementById('btn-load-demo')!.addEventListener('click', () => {
      load_demo_program();
      updateDebugPanel();
      addLog('Demo program loaded — click Step to execute', 'success');
    });

    // ── Hello UART Demo ─────────────────────────────────────────────────
    // Hand-assembled ARM program:
    //   0x8000: MOV R1, #0x10000000    ; UART TX base
    //   0x8004: ADD R2, PC, #0x18      ; R2 → string at 0x8020 (PC+4+0x18)
    //   0x8008: LDRB R0, [R2], #1      ; loop: load byte, post-increment
    //   0x800C: CMP R0, #0             ; null terminator?
    //   0x8010: BEQ halt (0x801C)      ; if null → halt
    //   0x8014: STRB R0, [R1]          ; write byte to UART TX
    //   0x8018: B loop (0x8008)         ; next byte
    //   0x801C: B . (halt)             ; infinite loop
    //   0x8020: "Hello World!\n\0"     ; ASCII data (padded to 4-byte boundary)
    const HELLO_UART_HEX = [
      'E3A01201',  // MOV R1, #0x10000000
      'E28F2018',  // ADD R2, PC, #0x18 (points to 0x8020)
      'E4D20001',  // LDRB R0, [R2], #1
      'E3500000',  // CMP R0, #0
      '0A000001',  // BEQ +2 (to 0x801C)
      'E5C10000',  // STRB R0, [R1]
      'EAFFFFFA',  // B -6 (to 0x8008)
      'EAFFFFFE',  // B . (halt)
      '6C6C6548',  // "Hell" (LE)
      '6F57206F',  // "o Wo" (LE)
      '21646C72',  // "rld!" (LE)
      '0000000A',  // "\n\0\0\0" (LE)
    ].join(' ');

    document.getElementById('btn-load-uart')!.addEventListener('click', () => {
      const ok = load_custom_hex(HELLO_UART_HEX);
      if (ok) {
        updateDebugPanel();
        addLog('🐱 Hello UART demo loaded at 0x8000 — press Run 10 to see it print!', 'success');
        addLog('Program writes "Hello World!" to UART TX (0x10000000)', 'info');
      } else {
        addLog('Failed to load UART demo', 'system');
      }
    });

    document.getElementById('btn-step')!.addEventListener('click', () => {
      const ran = step_cpu();
      updateDebugPanel();
      if (ran) {
        addLog('CPU stepped 1 instruction');
      } else {
        addLog('CPU halted — no instruction executed', 'system');
      }
    });

    document.getElementById('btn-run10')!.addEventListener('click', () => {
      let count = 0;
      for (let i = 0; i < 10; i++) {
        if (step_cpu()) count++;
        else break;
      }
      updateDebugPanel();
      addLog(`CPU stepped ${count} instructions`);
    });

    // ── Run/Stop toggle ─────────────────────────────────────────────
    const btnRun = document.getElementById('btn-run')!;
    btnRun.addEventListener('click', () => {
      running = !running;
      if (running) {
        btnRun.querySelector('.btn-icon')!.textContent = '⏹️';
        btnRun.childNodes[1].textContent = ' Stop';
        (btnRun as HTMLButtonElement).style.background = 'linear-gradient(135deg, #7f1d1d, #dc2626)';
        (btnRun as HTMLButtonElement).style.borderColor = '#ef4444';
        addLog(`CPU running (${BATCH_SIZE.toLocaleString()} instructions/frame)...`, 'success');
      } else {
        btnRun.querySelector('.btn-icon')!.textContent = '▶️';
        btnRun.childNodes[1].textContent = ' Run';
        (btnRun as HTMLButtonElement).style.background = 'linear-gradient(135deg, #065f46, #059669)';
        (btnRun as HTMLButtonElement).style.borderColor = '#10b981';
        addLog(`CPU stopped at cycle ${get_cycle_count()}`);
        updateDebugPanel();
      }
    });

    addLog('Debug panel active', 'success');

    // ── Hex upload ──────────────────────────────────────────────────
    document.getElementById('btn-upload-hex')!.addEventListener('click', () => {
      const textarea = document.getElementById('hex-input') as HTMLTextAreaElement;
      const hex = textarea.value.trim();
      if (!hex) {
        addLog('No hex input provided', 'system');
        return;
      }
      const ok = load_custom_hex(hex);
      if (ok) {
        updateDebugPanel();
        addLog('Custom program uploaded — click Step to execute', 'success');
      } else {
        addLog('Failed to parse hex input', 'system');
      }
    });

    // ── ROM file upload ─────────────────────────────────────────────
    const romFileInput = document.getElementById('rom-file-input') as HTMLInputElement;
    document.getElementById('btn-upload-rom')!.addEventListener('click', () => {
      romFileInput.click();
    });

    romFileInput.addEventListener('change', () => {
      const file = romFileInput.files?.[0];
      if (!file) return;
      const reader = new FileReader();
      reader.onload = () => {
        const buffer = reader.result as ArrayBuffer;
        const bytes = new Uint8Array(buffer);
        const ok = load_rom(bytes);
        if (ok) {
          // Auto-switch to VRAM render mode so CPU-drawn pixels are visible
          setMode('vram', btnVram);
          updateDebugPanel();
          addLog(`ROM loaded: ${file.name} (${bytes.length} bytes)`, 'success');
          addLog('Render mode auto-switched to VRAM — CPU can draw to 0x04000000', 'info');
        } else {
          addLog('Failed to load ROM — is the emulator initialized?', 'system');
        }
      };
      reader.readAsArrayBuffer(file);
      // Reset so the same file can be re-selected
      romFileInput.value = '';
    });

    // ── Linux kernel upload ────────────────────────────────────────
    const linuxFileInput = document.getElementById('linux-file-input') as HTMLInputElement;
    document.getElementById('btn-upload-linux')!.addEventListener('click', () => {
      linuxFileInput.click();
    });

    linuxFileInput.addEventListener('change', () => {
      const file = linuxFileInput.files?.[0];
      if (!file) return;
      const reader = new FileReader();
      reader.onload = () => {
        const buffer = reader.result as ArrayBuffer;
        const bytes = new Uint8Array(buffer);
        const ok = boot_linux_kernel(bytes);
        if (ok) {
          setMode('vram', btnVram);
          updateDebugPanel();
          addLog(`🐧 Linux kernel boot prepared: ${file.name} (${bytes.length} bytes)`, 'success');
          addLog('ATAG boot state configured, jumping to kernel at 0x8000', 'info');
          console.log(`🐧 Linux boot sequence ready: ${file.name}, ${bytes.length} bytes`);
        } else {
          addLog('Failed to boot Linux kernel — is the emulator initialized?', 'system');
        }
      };
      reader.readAsArrayBuffer(file);
      linuxFileInput.value = '';
    });

  } catch (err) {
    statusText.textContent = 'Failed to load Wasm module ✗';
    statusIndicator.classList.add('error');
    addLog(`Error: ${err}`, 'system');
    console.error('Wasm init failed:', err);
  }
}

main();
