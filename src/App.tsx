import { createSignal, onMount, onCleanup } from "solid-js";
import { invoke } from "@tauri-apps/api/tauri";
import { emit, listen, UnlistenFn } from '@tauri-apps/api/event'
import { appWindow } from '@tauri-apps/api/window'
import "./App.css";

interface MyEvent {
  name: String,
}

function App() {
  const [name, setName] = createSignal("");
  const [unlisten, setUnlisten] = createSignal<UnlistenFn | undefined>(undefined)

  onMount(async () => {
    setName(await invoke("get_random_id"));
    const unlisten_events = await listen('my_event', (event) => {
      console.log(event);
      const payload = event.payload as MyEvent;
      console.log(payload.name);
    })
    setUnlisten(() => unlisten_events);
  });

  onCleanup(() => {
    const u = unlisten();
    if (u) { u() }
  });

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
      <p>{`linkmou.se/${name()}`}</p>
      <button type="button" onClick={() => emit("event-name", { message: 'Tauri is awesome!' }) /* mouse_move_relative(1, 1) */}>
        Greet
      </button>
    </div>
  );
}

export default App;
