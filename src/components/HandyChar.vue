<script setup lang="ts">
// Handy（汉迪）：桌面小人，形体直接复用 Acta Handy 的图形 logo（91×165），
// viewBox 向下收脚底为止。眼睛（两枚浅色圆点）循环眨眼，并通过父级注入的
// --eye-x / --eye-y 微微看向光标方向；身体带极轻微的呼吸起伏；拖动时的
// 摇晃由父级注入 --shake（rAF 包络驱动，起摆与站稳都是非线性渐变）。
defineProps<{ width?: number }>();
</script>

<template>
  <div class="handy">
    <svg :width="width ?? 64" viewBox="0 0 91 166" fill="currentColor" aria-hidden="true">
      <g class="handy-figure">
        <g transform="matrix(1,0,0,1,-472.860418,-756.917314)">
          <g transform="matrix(1.410031,0,0,1.410031,-312.48265,373.365759)">
            <path d="M600.857,336.618L577.373,336.618C575.368,336.618 573.483,335.665 572.295,334.051C571.107,332.436 570.757,330.353 571.353,328.439L588.913,272.016L606.865,328.4C607.475,330.317 607.134,332.41 605.947,334.034C604.76,335.658 602.869,336.618 600.857,336.618Z"/>
          </g>
          <g class="handy-eyes">
            <g transform="matrix(0.343585,0,0,0.343585,294.880491,522.736671)">
              <circle class="handy-eye" cx="622.755" cy="911.802" r="9.283"/>
            </g>
            <g transform="matrix(0.343585,0,0,0.343585,313.556419,522.736671)">
              <circle class="handy-eye" cx="622.755" cy="911.802" r="9.283"/>
            </g>
          </g>
          <g transform="matrix(1.410031,0,0,1.410031,-312.48265,373.365759)">
            <path d="M605.581,384.773C604.387,386.987 602.075,388.368 599.56,388.368C593.673,388.377 584.037,388.377 578.15,388.377C575.643,388.376 573.336,387.006 572.137,384.804C571.043,382.804 569.047,381.355 566.652,381.025L561.215,380.277C559.783,380.08 558.511,379.263 557.736,378.043C556.961,376.824 556.761,375.325 557.191,373.945L563.493,353.696C565.421,347.5 571.156,343.278 577.645,343.278L600.766,343.278C607.219,343.278 612.931,347.454 614.889,353.603L621.229,373.515C621.71,375.027 621.488,376.674 620.624,378.004C619.759,379.334 618.344,380.206 616.767,380.38L610.97,381.019C608.639,381.277 606.675,382.769 605.581,384.773Z"/>
          </g>
        </g>
      </g>
    </svg>
  </div>
</template>

<style scoped>
/* 摇摆由父级注入 --shake（角度），挂在 svg 外层，不影响登场动画的 scale。 */
.handy {
  display: block;
  line-height: 0;
  transform: rotate(var(--shake, 0deg));
  transform-origin: 50% 100%;
  will-change: transform;
}

/* 眼睛：与 logo 一致用底色（纸色）透出，深色主题下自动变成深色圆点。 */
.handy-eye {
  fill: var(--paper, #fbfaf6);
  animation: handy-blink 4.8s infinite;
  transform-box: fill-box;
  transform-origin: 50% 50%;
}
@keyframes handy-blink {
  0%, 92.5% { transform: scaleY(1); }
  95.5%, 97% { transform: scaleY(.07); }
  100% { transform: scaleY(1); }
}

/* 眼睛整体微微看向光标：--eye-x / --eye-y 是 viewBox 单位的偏移（±2 上下）。 */
.handy-eyes {
  transform: translate(calc(var(--eye-x, 0) * 1px), calc(var(--eye-y, 0) * 1px));
  transition: transform .3s var(--ease-out);
  will-change: transform;
}

/* 呼吸：从脚底起轻微起伏，让小人看起来是活的。 */
.handy-figure {
  animation: handy-breathe 3.6s ease-in-out infinite;
  transform-box: view-box;
  transform-origin: 50% 99%;
}
@keyframes handy-breathe {
  0%, 100% { transform: scaleY(1); }
  50% { transform: scaleY(1.018); }
}
</style>
