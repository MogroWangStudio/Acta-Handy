// 拖动中的吸附提示：小组件离屏幕边缘或另一个小组件足够近时，返回即将
// 贴合的一侧（卡片朝那侧亮起光效）；停止拖动片刻后归 null。
// 阈值与后端吸附（SNAP_THRESHOLD / SNAP_MARGIN）保持一致，提示即所得。
import { ref, type Ref } from "vue";
import { currentMonitor, getCurrentWindow } from "@tauri-apps/api/window";
import type { WidgetConfig } from "../types/settings";

export type SnapSide = "left" | "right" | "top" | "bottom";

const THRESHOLD = 48;
const MARGIN = 8;

export function useSnapHint(
  self: () => WidgetConfig,
  other: () => WidgetConfig | null,
): Ref<SnapSide | null> {
  const side = ref<SnapSide | null>(null);
  let timer: ReturnType<typeof setTimeout> | null = null;

  async function update(): Promise<void> {
    if (!self().snapToEdge) {
      side.value = null;
      return;
    }
    try {
      const win = getCurrentWindow();
      const [mon, pos, size, sc] = await Promise.all([
        currentMonitor(),
        win.outerPosition(),
        win.outerSize(),
        win.scaleFactor(),
      ]);
      if (!mon) return;
      const x = pos.x / sc;
      const y = pos.y / sc;
      const w = size.width / sc;
      const h = size.height / sc;
      const mx = mon.position.x / sc;
      const my = mon.position.y / sc;
      const mw = mon.size.width / sc;
      const mh = mon.size.height / sc;
      let s: SnapSide | null = null;
      if (Math.abs(x - (mx + MARGIN)) <= THRESHOLD) s = "left";
      else if (Math.abs((mx + mw - MARGIN) - (x + w)) <= THRESHOLD) s = "right";
      if (!s) {
        if (Math.abs(y - (my + MARGIN)) <= THRESHOLD) s = "top";
        else if (Math.abs((my + mh - MARGIN) - (y + h)) <= THRESHOLD) s = "bottom";
      }
      // 互相吸附提示：光效亮在「即将贴合的那条边」——吸到对方右侧时，
      // 卡片的左缘即将贴上，亮左缘。
      if (!s) {
        const cfg = other();
        if (cfg && cfg.x != null && cfg.y != null) {
          const o = { x: cfg.x, y: cfg.y, width: cfg.width, height: cfg.height };
          const vOverlap = y < o.y + o.height + THRESHOLD && y + h > o.y - THRESHOLD;
          const hOverlap = x < o.x + o.width + THRESHOLD && x + w > o.x - THRESHOLD;
          if (vOverlap && Math.abs(o.x + o.width + MARGIN - x) <= THRESHOLD) s = "left";
          else if (vOverlap && Math.abs(x + w - (o.x - MARGIN)) <= THRESHOLD) s = "right";
          else if (hOverlap && Math.abs(o.y + o.height + MARGIN - y) <= THRESHOLD) s = "top";
          else if (hOverlap && Math.abs(y + h - (o.y - MARGIN)) <= THRESHOLD) s = "bottom";
        }
      }
      side.value = s;
    } catch {
      // 浏览器 mock 预览没有窗口 API。
    }
  }

  void (async () => {
    try {
      await getCurrentWindow().onMoved(() => {
        void update();
        if (timer) clearTimeout(timer);
        timer = setTimeout(() => (side.value = null), 180);
      });
    } catch {
      // mock 无窗口事件。
    }
  })();

  return side;
}
