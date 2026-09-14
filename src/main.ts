import { createApp } from "vue";
import App from "./App.vue";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/widget.css";

if (import.meta.env.VITE_MOCK_TAURI === "1") {
  const { installMock } = await import("./dev/mock-tauri");
  installMock();
}

createApp(App).mount("#app");
