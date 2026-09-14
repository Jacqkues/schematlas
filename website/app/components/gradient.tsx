"use client";

import {
  Component,
  lazy,
  Suspense,
  useEffect,
  useRef,
  useState,
  type ReactNode,
} from "react";
const Shader = lazy(() => import("./shader"));

class GradientBoundary extends Component<
  { children: ReactNode },
  { failed: boolean }
> {
  state = { failed: false };
  static getDerivedStateFromError() {
    return { failed: true };
  }
  render() {
    return this.state.failed ? null : this.props.children;
  }
}

export default function Gradient() {
  const host = useRef<HTMLDivElement>(null);
  const surface = useRef<HTMLDivElement>(null);
  const [visible, setVisible] = useState(false);
  const [reduced, setReduced] = useState(true);
  useEffect(() => {
    const preference = matchMedia("(prefers-reduced-motion: reduce)");
    const updatePreference = () => setReduced(preference.matches);
    updatePreference();
    preference.addEventListener("change", updatePreference);
    let inView = true;
    const updateVisibility = () => setVisible(inView && !document.hidden);
    const observer = new IntersectionObserver(([entry]) => {
      inView = entry.isIntersecting;
      updateVisibility();
    });
    if (host.current) observer.observe(host.current);
    document.addEventListener("visibilitychange", updateVisibility);
    return () => {
      observer.disconnect();
      preference.removeEventListener("change", updatePreference);
      document.removeEventListener("visibilitychange", updateVisibility);
    };
  }, []);
  useEffect(() => {
    const layer = surface.current;
    const hero = host.current?.parentElement;
    if (!layer || !hero || reduced || !visible) return;

    // Keep pointer motion out of React and update only a composited layer.
    let targetX = 0, targetY = 0, x = 0, y = 0;
    let frame = 0, lastTime = 0;
    let rangeX = 60, rangeY = 45;
    const tick = (time: number) => {
      const blend = 1 - Math.exp(-Math.min(time - lastTime, 64) / 180);
      lastTime = time;
      x += (targetX - x) * blend;
      y += (targetY - y) * blend;
      const moving = Math.abs(targetX - x) + Math.abs(targetY - y) > 0.001;
      if (!moving) { x = targetX; y = targetY; }
      layer.style.transform = `translate3d(${x * rangeX}px, ${y * rangeY}px, 0) rotate(${x * 2}deg) scale(1.14)`;
      frame = moving ? requestAnimationFrame(tick) : 0;
    };
    const animate = () => {
      if (!frame) { lastTime = performance.now(); frame = requestAnimationFrame(tick); }
    };
    const move = (event: PointerEvent) => {
      if (event.pointerType !== "mouse" && event.pointerType !== "pen") return;
      const rect = hero.getBoundingClientRect();
      rangeX = Math.min(60, rect.width * 0.04);
      rangeY = Math.min(45, rect.height * 0.04);
      targetX = Math.max(-1, Math.min(1, (event.clientX - rect.left) / rect.width * 2 - 1));
      targetY = Math.max(-1, Math.min(1, (event.clientY - rect.top) / rect.height * 2 - 1));
      animate();
    };
    const reset = () => { targetX = 0; targetY = 0; animate(); };
    hero.addEventListener("pointermove", move, { passive: true });
    hero.addEventListener("pointerleave", reset);
    hero.addEventListener("pointercancel", reset);
    return () => {
      cancelAnimationFrame(frame);
      hero.removeEventListener("pointermove", move);
      hero.removeEventListener("pointerleave", reset);
      hero.removeEventListener("pointercancel", reset);
      layer.style.removeProperty("transform");
    };
  }, [reduced, visible]);

  return (
    <div className="gradient-field" ref={host} aria-hidden="true">
      <div className="shader-surface" ref={surface}>
        {!reduced && visible && (
          <GradientBoundary>
            <Suspense fallback={null}>
              <Shader />
            </Suspense>
          </GradientBoundary>
        )}
      </div>
    </div>
  );
}
