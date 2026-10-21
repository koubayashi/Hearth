// Bridges the Gamepad API onto keyboard events, so every screen only has to
// handle arrows / Enter / Escape and works the same with a controller.

type Key = "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight" | "Enter" | "Escape" | "ContextMenu";

// Standard mapping: https://w3c.github.io/gamepad/#remapping
const BUTTONS: Record<number, Key> = {
  0: "Enter", // A
  1: "Escape", // B
  3: "ContextMenu", // Y
  12: "ArrowUp",
  13: "ArrowDown",
  14: "ArrowLeft",
  15: "ArrowRight",
};

const DEADZONE = 0.5;
const REPEAT_DELAY = 380;
const REPEAT_RATE = 90;

export function startGamepadBridge(): () => void {
  const held = new Map<Key, number>(); // key -> next repeat time
  let raf = 0;

  const press = (key: Key) => {
    const target = (document.activeElement as HTMLElement | null) ?? document.body;
    target.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
    if (key === "Enter" && target instanceof HTMLElement && target.matches("button, a, [role=button]")) {
      target.click();
    }
  };

  const tick = (t: number) => {
    const down = new Set<Key>();
    for (const pad of navigator.getGamepads()) {
      if (!pad) continue;
      pad.buttons.forEach((b, i) => b.pressed && BUTTONS[i] && down.add(BUTTONS[i]));
      const [x = 0, y = 0] = pad.axes;
      if (x < -DEADZONE) down.add("ArrowLeft");
      if (x > DEADZONE) down.add("ArrowRight");
      if (y < -DEADZONE) down.add("ArrowUp");
      if (y > DEADZONE) down.add("ArrowDown");
    }
    for (const key of down) {
      const next = held.get(key);
      if (next === undefined) {
        press(key);
        held.set(key, t + REPEAT_DELAY);
      } else if (t >= next && key.startsWith("Arrow")) {
        press(key);
        held.set(key, t + REPEAT_RATE);
      }
    }
    for (const key of held.keys()) if (!down.has(key)) held.delete(key);
    raf = requestAnimationFrame(tick);
  };

  const onConnect = () => {
    document.documentElement.dataset.input = "gamepad";
    if (!raf) raf = requestAnimationFrame(tick);
  };
  const onPointer = () => {
    document.documentElement.dataset.input = "pointer";
  };

  window.addEventListener("gamepadconnected", onConnect);
  window.addEventListener("pointermove", onPointer, { passive: true });
  if (navigator.getGamepads().some(Boolean)) onConnect();

  return () => {
    cancelAnimationFrame(raf);
    window.removeEventListener("gamepadconnected", onConnect);
    window.removeEventListener("pointermove", onPointer);
  };
}
