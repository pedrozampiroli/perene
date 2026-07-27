import { mount } from "svelte";
import App from "./App.svelte";
import "@xterm/xterm/css/xterm.css";
import "./app.css";

// Mata o menu de contexto nativo do webview (inspecionar/recarregar) — a app tem
// os seus próprios menus. Campos de texto seguem com o menu do sistema.
window.addEventListener(
  "contextmenu",
  (e) => {
    const t = e.target as HTMLElement | null;
    const editable =
      t?.tagName === "INPUT" || t?.tagName === "TEXTAREA" || t?.isContentEditable;
    if (!editable) e.preventDefault();
  },
  { capture: true },
);

const app = mount(App, {
  target: document.getElementById("app")!,
});

export default app;
