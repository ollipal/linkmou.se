import { createSignal, onMount, onCleanup } from "solid-js";
import { invoke } from "@tauri-apps/api/tauri";
import { listen, UnlistenFn } from '@tauri-apps/api/event'
import { appWindow } from '@tauri-apps/api/window'
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
      <div data-tauri-drag-region class="titlebar">
        {/* <div class="titlebar-button" id="titlebar-minimize">
          <img
            src="https://api.iconify.design/mdi:window-minimize.svg"
            alt="minimize"
          />
        </div> */}
        {/* <div class="titlebar-button" id="titlebar-maximize">
          <img
            src="https://api.iconify.design/mdi:window-maximize.svg"
            alt="maximize"
          />
        </div> */}
        <div
          class="titlebar-button"
          id="titlebar-close"
          onClick={() => appWindow.close()}
        >  
          <img src="https://api.iconify.design/mdi:close.svg" alt="close" />
        </div>
      </div>

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
