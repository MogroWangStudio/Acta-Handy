import { createApp } from "vue";
import App from "./App.vue";
import "./styles/tokens.css";
import "./styles/base.css";
import "./styles/widget.css";

if (import.meta.env.VITE_MOCK_TAURI === "1") {
  const { installMock } = await import("./dev/mock-tauri");
  installMock();
}

// 桌面小工具不需要浏览器右键菜单；编辑操作走快捷键。
document.addEventListener("contextmenu", (event) => event.preventDefault());

createApp(App).mount("#app");
