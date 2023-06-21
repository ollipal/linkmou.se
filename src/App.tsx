import { createSignal, onMount, onCleanup } from "solid-js";
import { invoke } from "@tauri-apps/api/tauri";
import { emit, listen, UnlistenFn } from '@tauri-apps/api/event'
import { appWindow } from '@tauri-apps/api/window'
import { writeText } from '@tauri-apps/api/clipboard';
import "./App.css";

interface MyEvent {
  name: string,
}

function App() {
  const [name, setName] = createSignal("");
  const [status, setStatus] = createSignal("CONNECTING SERVER");
  const [unlisten, setUnlisten] = createSignal<UnlistenFn | undefined>(undefined)

  onMount(async () => {
    setName(await invoke("get_random_id"));
    const unlisten_events = await listen('my_event', (event) => {
      console.log(event);
      const payload = event.payload as MyEvent;
      console.log(payload.name);
      setStatus(payload.name);
    })
    await invoke("get_latest_my_event"); // Previous might've been missed
    setUnlisten(() => unlisten_events);
  });

  onCleanup(() => {
    const u = unlisten();
    if (u) { u() }
  });

  return (
    <div class="container">
      <p>{`linkmou.se/${name()}`}</p>
      <button type="button" onClick={() => {
        writeText(`https://linkmou.se/${name()}`).then(
          () => {
            /* clipboard successfully set */
          },
          () => {
            console.log("Copy fail")
          }
        );
      }}>
        Copy link
      </button>
      <button type="button" onClick={async () => {
        await invoke("restart_connection");
        setName(await invoke("get_random_id"));
      }}>
        Restart connection
      </button>
      <button type="button" onClick={async () => {
        await invoke("change_random_id");
        setName(await invoke("get_random_id"));
      }}>
        Change link
      </button>
      <button type="button" onClick={() => emit("event-name", { message: 'Tauri is awesome!' })}>
        Test button
      </button>
      <div>
        {status()}
      </div>
    </div>
  );
}

export default App;
