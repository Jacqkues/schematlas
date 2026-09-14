"use client";

import { useEffect, useRef, useState } from "react";
import { ArrowUpRight, Play, RotateCcw } from "lucide-react";
import { observeDemo, track } from "@/lib/analytics";

export default function WorkspaceDemo() {
  const container = useRef<HTMLDivElement>(null);
  const iframe = useRef<HTMLIFrameElement>(null);
  const [started, setStarted] = useState(false);
  const [ready, setReady] = useState(false);
  const [failed, setFailed] = useState(false);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    const target = container.current;
    if (!target || !window.IntersectionObserver) return;
    const observer = new IntersectionObserver(([entry]) => {
      if (entry.isIntersecting) {
        setStarted(true);
        observer.disconnect();
      }
    }, { rootMargin: "150px" });
    observer.observe(target);
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    if (!started || ready) return;
    const timeout = window.setTimeout(() => { setFailed(true); track("demo_load_failed"); }, 30_000);
    function receive(event: MessageEvent) {
      if (event.origin !== window.location.origin || event.source !== iframe.current?.contentWindow) return;
      if (event.data?.type === "schematlas:ready") {
        setReady(true);
        setFailed(false);
        track("demo_loaded");
      }
    }
    window.addEventListener("message", receive);
    return () => {
      window.clearTimeout(timeout);
      window.removeEventListener("message", receive);
    };
  }, [started, ready, attempt]);

  useEffect(() => {
    if (!ready || !iframe.current) return;
    return observeDemo(iframe.current);
  }, [ready, attempt]);

  function retry() {
    track("demo_retry");
    setFailed(false);
    setReady(false);
    setStarted(true);
    setAttempt(value => value + 1);
  }

  return (
    <div className="workspace-demo" ref={container}>
      <div className="demo-topline">
        <span><i aria-hidden="true" /> LIVE PLAYGROUND <span className="demo-topline-note">/ Built with the real app</span></span>
        <a href="/demo/index.html" target="_blank" rel="noopener noreferrer">Open full workspace <ArrowUpRight size={14} /></a>
      </div>
      <div className="demo-stage" aria-busy={started && !ready && !failed}>
        {started && <iframe key={attempt} ref={iframe} src="/demo/index.html"
          title="Interactive Schematlas sample workspace"
          className={ready ? "demo-iframe is-ready" : "demo-iframe"}
          tabIndex={ready ? 0 : -1} aria-hidden={!ready}
          allow="camera 'none'; microphone 'none'; geolocation 'none'" />}
        {!ready && <div className="demo-poster">
          <img src="/images/workspace.png" width={2160} height={1221} loading="lazy"
            alt="Sample commerce schema with tables connected by relationships." />
          <div className="demo-loading-card">
            <span className="demo-loading-eyebrow">YOUR FIRST WORKSPACE</span>
            <h3>{failed ? "Let’s try that again." : "Go ahead. Move things around."}</h3>
            <p role="status">{failed ? "The interactive workspace couldn’t load. You can retry or download the app below." : started ? "Opening a sample database and its API…" : "Explore a sample database and its API, right here."}</p>
            {(!started || failed) && <button type="button" onClick={retry}>{failed ? <RotateCcw size={15} /> : <Play size={15} />}{failed ? "Retry workspace" : "Explore the demo"}</button>}
            {started && !failed && <span className="demo-loading-line" aria-hidden="true" />}
          </div>
        </div>}
      </div>
      <p className="demo-hint"><span>Drag a table. Follow a relationship. Get the whole picture.</span><span>Sample data · No setup needed</span></p>
    </div>
  );
}
