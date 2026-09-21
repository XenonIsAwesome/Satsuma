import React from "react";
import ReactDOM from "react-dom/client";

// Two windows share this one entry point (see tauri.conf.json): the normal
// titled "main" window loads plain `index.html` and renders the
// drag-and-drop `App`; the borderless "overlay" window (opened by the
// Windows Shift-hotkey and the Linux file-manager triggers) loads
// `index.html?window=overlay` and renders `OverlayApp` instead. Each is
// loaded dynamically so the other's CSS (in particular App.css's opaque
// `:root` background) never ends up loaded in the overlay window.
const root = ReactDOM.createRoot(document.getElementById("root") as HTMLElement);
const isOverlay = new URLSearchParams(window.location.search).get("window") === "overlay";

const load = isOverlay ? import("./OverlayApp") : import("./App");
void load.then(({ default: Component }) => {
  root.render(
    <React.StrictMode>
      <Component />
    </React.StrictMode>,
  );
});
