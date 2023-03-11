import { createSignal, onMount } from "solid-js";
import logo from "./assets/logo.svg";
import { invoke } from "@tauri-apps/api/tauri";
import "./App.css";

function App() {
  const [initialized, setInitialized] = createSignal(false);
  const [name, setName] = createSignal("");

  onMount(async () => {
    await invoke("init");
    setInitialized(true);
  });

  async function mouse_move_relative(x: Number, y: Number) {
    if (!initialized) {
      console.log("Not initialized!")
      return;
    }

    // Learn more about Tauri commands at https://tauri.app/v1/guides/features/command
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
