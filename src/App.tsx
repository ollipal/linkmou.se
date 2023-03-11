import { createSignal, onMount, onCleanup } from "solid-js";
import logo from "./assets/logo.svg";
import { invoke } from "@tauri-apps/api/tauri";
import { emit, listen, UnlistenFn } from '@tauri-apps/api/event'
import "./App.css";

interface SystemEvent {
  name: String,
  x: Number,
  y: Number,
}

function App() {
  const [initialized, setInitialized] = createSignal(false);
  const [name, setName] = createSignal("");
  const [unlisten, setUnlisten] = createSignal<UnlistenFn | undefined>(undefined)

  onMount(async () => {
    await invoke("init");
    setInitialized(true);
    const unlisten_events = await listen('system_event', (event) => {
      const payload = event.payload as SystemEvent;
      console.log(payload.name);
    })
    setUnlisten(() => unlisten_events);
  });

  onCleanup(() => {
    const u = unlisten();
    if (u) { u() }
  });

  async function mouse_move_relative(x: Number, y: Number) {
    if (!initialized) {
      console.log("Not initialized, cannot move mouse")
      return;
    }
    await invoke("mouse_move_relative", { x, y });
  }

  return (
    <div class="container">
      <h1>Welcome to Tauri!</h1>

      <div class="row">
        <a href="https://vitejs.dev" target="_blank">
          <img src="/vite.svg" class="logo vite" alt="Vite logo" />
        </a>
        <a href="https://tauri.app" target="_blank">
          <img src="/tauri.svg" class="logo tauri" alt="Tauri logo" />
        </a>
        <a href="https://solidjs.com" target="_blank">
          <img src={logo} class="logo solid" alt="Solid logo" />
        </a>
      </div>

      <p>Click on the Tauri, Vite, and Solid logos to learn more.</p>

      <div class="row">
        <div>
          <input
            id="greet-input"
            onChange={(e) => setName(e.currentTarget.value)}
            placeholder="Enter a name..."
          />
          <button type="button" onClick={() => mouse_move_relative(1, 1)}>
            Greet
          </button>
        </div>
      </div>

      <p>{name()}</p>
    </div>
  );
}

export default App;
