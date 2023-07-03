import { createSignal, onMount, onCleanup } from "solid-js";
import { invoke } from "@tauri-apps/api/tauri";
import { emit, listen, UnlistenFn } from '@tauri-apps/api/event'
import { appWindow } from '@tauri-apps/api/window'
import { writeText } from '@tauri-apps/api/clipboard';

import Pop from "./components/Pop";
import { CONNECTING_SERVER, CONTROLLING_STARTED, CONTROLLING_STOPPED, SERVER_CONNECTED_WAITING_USER, SERVER_DISCONNECTED, USER_CONNECTED, USER_CONNECTING } from "./MessagesToFe";

interface MyEvent {
  name: string,
}

function App() {
  const [name, setName] = createSignal("");
  const [status, setStatus] = createSignal(CONNECTING_SERVER);
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
      {[CONNECTING_SERVER, SERVER_CONNECTED_WAITING_USER, USER_CONNECTING].includes(status()) &&
        <>
          <div style={{
            "margin": "0.5rem",
          }}>
            {"Share this link to give access to you mouse and keyboard"}
          </div>
          <Pop>
            <p onClick={() => {
            writeText(`https://linkmou.se/${name()}`).then(
              () => {
                /* clipboard successfully set */
              },
              () => {
                console.log("Copy fail")
              }
            );
          }}>{`linkmou.se/${name()}`}</p>
          </Pop>
          <button type="button" onClick={async () => {
            await invoke("change_random_id");
            setName(await invoke("get_random_id"));
          }}>
            Change link
          </button>
        </>
      }

      {[USER_CONNECTED, CONTROLLING_STARTED, CONTROLLING_STOPPED].includes(status()) &&
        <>
          <div style={{
            "margin": "0.5rem",
          }}>
            {"User connected!"}
          </div>
          <div style={{
            color: "grey",
            "font-size": "12px",
          }}>
            {status() === CONTROLLING_STARTED
            ? "Controlling"
            : "Not controlling"
            }
          </div>
        </>
      }

      {status() === SERVER_DISCONNECTED &&
        <button type="button" onClick={async () => {
          await invoke("restart_connection");
          setName(await invoke("get_random_id"));
        }}>
          Server disconnected, restart connection
        </button>
      }

      <div style={{
        color: "grey",
        "font-size": "10px",
        "margin-top": "1rem",
      }}>
        {status()}
      </div>
    </div>
  );
}

export default App;
