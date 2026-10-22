import { createSignal, onCleanup } from "solid-js";
import { addNumbers, events } from "tauri-specta-custom-plugin";
import logo from "./assets/logo.svg";
import "./App.css";

function randomDigit() {
  return Math.floor(Math.random() * 10);
}

function App() {
  const [latestNumber, setLatestNumber] = createSignal<number | null>(null);
  const [sum, setSum] = createSignal<number | null>(null);
  const [adding, setAdding] = createSignal(false);

  // The plugin exports its events the same way an application exports its own.
  const listener = events.randomNumber.listen(({ payload }) =>
    setLatestNumber(payload),
  );
  onCleanup(() => listener.then((unlisten) => unlisten()));

  async function add() {
    setAdding(true);
    try {
      // `addNumbers` is a command from the plugin rather than the application.
      setSum(await addNumbers(randomDigit(), randomDigit()));
    } finally {
      setAdding(false);
    }
  }

  return (
    <main class="container">
      <h1>Welcome to Tauri + Solid</h1>

      <div class="row">
        <a href="https://vite.dev" target="_blank">
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

      <p>Random number streamed by the plugin: {latestNumber() ?? "…"}</p>

      <div class="row">
        <button onClick={add} disabled={adding()}>
          Add two random digits
        </button>
      </div>
      {sum() !== null && <p>The plugin returned {sum()}</p>}
    </main>
  );
}

export default App;
