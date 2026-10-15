'use client';

import { useEffect, useRef, useState } from 'react';
import Aurora from './aurora';

// One recording of the real app exploring its example project. While the section is pinned,
// scrolling moves the video forward and back; the steps on the left are its chapters.
// `start` is where each chapter begins in /videos/tour.mp4, in seconds.
const steps = [
  {
    start: 0,
    title: 'Start from a question.',
    text: 'Your agent reads the code. Schematlas keeps what it learns, in your repository.',
  },
  {
    start: 8.669,
    title: 'See the architecture.',
    text: 'Your Mermaid, draw.io and DBML files, laid out cleanly. Every node can cite its code.',
  },
  {
    start: 19.273,
    title: 'Grow a wiki.',
    text: 'Markdown pages with live diagrams and the sources they rely on.',
  },
  {
    start: 28.666,
    title: 'Know when the code moves.',
    text: 'When cited code changes on main, the node says so. Review it, or ask your agent to update it.',
  },
  {
    start: 39.678,
    title: 'Inspect your real schema.',
    text: 'Connect a database and map its live structure, next to your architecture. Only metadata is read.',
  },
];
/** Scroll distance per second of video, in pixels. */
const PACE = 90;
const LENGTH = 55;

export default function AppTour() {
  const section = useRef<HTMLElement>(null);
  const video = useRef<HTMLVideoElement>(null);
  const [active, setActive] = useState(0);
  const [near, setNear] = useState(false);
  const [source, setSource] = useState<string>();
  const [loadError, setLoadError] = useState(false);
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    if (!near) return;
    const controller = new AbortController();
    let objectUrl: string | undefined;
    setLoadError(false);
    // The hosted asset endpoint does not honor Range requests. A complete local
    // Blob gives browsers a seekable resource, including Safari's media pipeline.
    fetch('/videos/tour.mp4', { signal: controller.signal })
      .then(async (response) => {
        if (!response.ok) throw new Error('Video unavailable');
        const blob = await response.blob();
        if (controller.signal.aborted) return;
        objectUrl = URL.createObjectURL(new Blob([blob], { type: 'video/mp4' }));
        setSource(objectUrl);
      })
      .catch(() => {
        if (!controller.signal.aborted) setLoadError(true);
      });
    return () => {
      controller.abort();
      if (objectUrl) URL.revokeObjectURL(objectUrl);
    };
  }, [near, attempt]);
  useEffect(() => {
    const root = section.current;
    if (!root) return;
    // Fetch the video only when the tour is about to be seen.
    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) {
          setNear(true);
          observer.disconnect();
        }
      },
      { rootMargin: '1200px 0px' },
    );
    observer.observe(root);
    return () => observer.disconnect();
  }, []);
  useEffect(() => {
    const root = section.current,
      player = video.current;
    if (!root || !player || !near) return;
    let frame = 0,
      shown = 0;
    const target = () => {
      const rect = root.getBoundingClientRect();
      const span = root.offsetHeight - innerHeight;
      const progress = span > 0 ? Math.min(1, Math.max(0, -rect.top / span)) : 0;
      return progress * (player.duration || LENGTH);
    };
    const tick = () => {
      frame = 0;
      const goal = target();
      // Ease toward the scroll position, and seek only once the last seek has landed.
      shown += (goal - shown) * 0.18;
      if (Math.abs(goal - shown) < 0.01) shown = goal;
      if (!player.seeking && Math.abs(player.currentTime - shown) > 1 / 60)
        player.currentTime = shown;
      let index = 0;
      steps.forEach((step, i) => {
        if (shown >= step.start - 0.05) index = i;
      });
      const end = steps[index + 1]?.start ?? (player.duration || LENGTH);
      root.style.setProperty(
        '--step',
        Math.min(1, (shown - steps[index].start) / (end - steps[index].start)).toFixed(3),
      );
      setActive(index);
      if (Math.abs(goal - shown) > 0.001) frame = requestAnimationFrame(tick);
    };
    const schedule = () => {
      if (!frame) frame = requestAnimationFrame(tick);
    };
    schedule();
    addEventListener('scroll', schedule, { passive: true });
    addEventListener('resize', schedule);
    player.addEventListener('loadedmetadata', schedule);
    player.addEventListener('seeked', schedule);
    return () => {
      cancelAnimationFrame(frame);
      removeEventListener('scroll', schedule);
      removeEventListener('resize', schedule);
      player.removeEventListener('loadedmetadata', schedule);
      player.removeEventListener('seeked', schedule);
    };
  }, [near]);
  const jump = (index: number) => {
    const root = section.current;
    if (!root) return;
    const span = root.offsetHeight - innerHeight;
    const top = root.getBoundingClientRect().top + scrollY;
    const time = steps[index].start + 0.5;
    scrollTo({
      top: top + (span * time) / (video.current?.duration || LENGTH),
      behavior: 'smooth',
    });
  };
  return (
    <section
      id="workspace"
      ref={section}
      className="m-tour"
      style={{ height: `calc(${LENGTH * PACE}px + 100vh)` }}
      aria-label="A tour of the app"
    >
      <div className="m-tour-sticky">
        <div className="m-tour-stage">
          <Aurora palette="spring" />
          <ol className="m-tour-steps" aria-label="Tour steps">
            {steps.map((step, i) => (
              <li
                key={step.title}
                data-state={i === active ? 'active' : i < active ? 'before' : 'after'}
              >
                <button type="button" aria-pressed={i === active} onClick={() => jump(i)}>
                  <span className="m-tour-num">0{i + 1}</span>
                  <span className="m-tour-title">{step.title}</span>
                </button>
                <div className="m-tour-text">
                  <p>{step.text}</p>
                </div>
              </li>
            ))}
          </ol>
          <div className="m-tour-window">
            <video
              ref={video}
              src={source}
              poster="/videos/tour.jpg"
              muted
              playsInline
              preload="auto"
              aria-label="A recording of the app: asking the agent, exploring a diagram, reading the wiki, reviewing a change and connecting a database."
            />
            {near && !source && (
              <div className="m-tour-loading" role="status">
                {loadError ? (
                  <button type="button" onClick={() => setAttempt((value) => value + 1)}>
                    Retry loading the tour
                  </button>
                ) : (
                  'Loading tour…'
                )}
              </div>
            )}
          </div>
        </div>
      </div>
    </section>
  );
}
