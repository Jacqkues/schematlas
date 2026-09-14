"use client";

import { useEffect, useRef, useState, type CSSProperties } from "react";
import { Database, KeyRound, ArrowUpRight, CornerDownRight } from "lucide-react";

type NodeId = "customers" | "orders" | "payments" | "customers-api" | "orders-api";
const nodes: { id: NodeId; name: string; kind: "table" | "api"; x: number; y: number; tone: string; fields: [string, string][] }[] = [
  { id: "customers", name: "customers", kind: "table", x: 25, y: 80, tone: "blue", fields: [["id", "uuid"], ["email", "text"], ["name", "text"]] },
  { id: "orders", name: "orders", kind: "table", x: 335, y: 235, tone: "green", fields: [["id", "uuid"], ["customer_id", "uuid"], ["total", "numeric"]] },
  { id: "payments", name: "payments", kind: "table", x: 25, y: 390, tone: "peach", fields: [["id", "uuid"], ["order_id", "uuid"], ["amount", "numeric"]] },
  { id: "customers-api", name: "/customers/{id}", kind: "api", x: 350, y: 60, tone: "blue", fields: [] },
  { id: "orders-api", name: "/orders/{id}", kind: "api", x: 350, y: 475, tone: "green", fields: [] },
];
const edges: { from: NodeId; to: NodeId; path: string; label?: [number, number, string] }[] = [
  { from: "customers", to: "customers-api", path: "M235 117 H290 V100 H350" },
  { from: "customers", to: "orders", path: "M235 185 H280 V340 H335", label: [290, 270, "1 : N"] },
  { from: "orders", to: "payments", path: "M390 397 V430 H280 V495 H235", label: [289, 470, "1 : N"] },
  { from: "orders", to: "orders-api", path: "M490 397 V435 H466 V475" },
];
const descriptions: Record<NodeId, string> = {
  customers: "One customer. Many orders. One connected picture.",
  orders: "Follow an order from its customer to its payments and API.",
  payments: "Every payment leads back to the order it belongs to.",
  "customers-api": "See the customer behind the endpoint.",
  "orders-api": "Connect an API route to the data it returns.",
};

export default function HeroGraph() {
  const [hovered, setHovered] = useState<NodeId | null>(null);
  const [selected, setSelected] = useState<NodeId | null>(null);
  const [focused, setFocused] = useState<NodeId | null>(null);
  const active = hovered ?? focused ?? selected;
  const scene = useRef<HTMLDivElement>(null);
  const host = useRef<HTMLElement>(null);
  useEffect(() => {
    const root = host.current, layer = scene.current;
    if (!root || !layer) return;
    const preference = matchMedia("(prefers-reduced-motion: reduce)");
    let frame = 0, x = 0, y = 0;
    const render = () => { layer.style.transform = `translate(${x}px, ${y}px)`; frame = 0; };
    const schedule = () => { if (!frame) frame = requestAnimationFrame(render); };
    const reset = () => { x = 0; y = 0; schedule(); };
    const move = (event: PointerEvent) => {
      if (preference.matches || event.pointerType !== "mouse") return;
      const rect = root.getBoundingClientRect();
      x = ((event.clientX - rect.left) / rect.width - 0.5) * 10;
      y = ((event.clientY - rect.top) / rect.height - 0.5) * 10;
      schedule();
    };
    root.addEventListener("pointermove", move, { passive: true });
    root.addEventListener("pointerleave", reset);
    root.addEventListener("pointercancel", reset);
    preference.addEventListener("change", reset);
    return () => {
      cancelAnimationFrame(frame);
      root.removeEventListener("pointermove", move);
      root.removeEventListener("pointerleave", reset);
      root.removeEventListener("pointercancel", reset);
      preference.removeEventListener("change", reset);
    };
  }, []);
  const related = (id: NodeId) => !active || id === active || edges.some(edge =>
    (edge.from === active && edge.to === id) || (edge.to === active && edge.from === id));
  return (
    <figure className="hero-graph" ref={host} aria-label="Sample database and API relationship map">
      <div className="hero-graph-heading"><span className="map-cross">+</span> A SMALL PART OF THE BIG PICTURE <span>01 — 05</span></div>
      <div className="hero-graph-scene" ref={scene} data-active={active ?? "none"}>
        <div className="hero-domain-label">COMMERCE <span>/ sample schema</span></div>
        <svg className="hero-graph-edges" viewBox="0 0 630 580" fill="none" aria-hidden="true">
          {edges.map(edge => <g key={`${edge.from}-${edge.to}`} className={`hero-graph-edge ${active ? edge.from === active || edge.to === active ? "is-active" : "is-muted" : ""}`}>
            <path d={edge.path} />
            {edge.label && <text x={edge.label[0]} y={edge.label[1]}>{edge.label[2]}</text>}
          </g>)}
        </svg>
        {nodes.map((node, index) => <button type="button" key={node.id}
          className={`hero-map-node hero-map-node--${node.kind} tone-${node.tone} ${active === node.id ? "is-active" : ""} ${related(node.id) ? "" : "is-muted"}`}
          style={{ left: `${node.x / 630 * 100}%`, top: `${node.y / 580 * 100}%`, "--node-order": index } as CSSProperties}
          aria-label={`Highlight ${node.kind === "api" ? "GET " : ""}${node.name} relationships`}
          aria-pressed={selected === node.id}
          aria-describedby="hero-graph-description"
          onPointerEnter={event => { if (event.pointerType === "mouse") setHovered(node.id); }}
          onPointerLeave={() => setHovered(null)}
          onFocus={() => setFocused(node.id)} onBlur={() => setFocused(null)}
          onClick={() => setSelected(previous => previous === node.id ? null : node.id)}
          onKeyDown={event => { if (event.key === "Escape") { setSelected(null); setHovered(null); event.currentTarget.blur(); } }}>
          {node.kind === "table" ? <>
            <span className="hero-node-title"><Database size={15} /><strong>{node.name}</strong><span>TABLE</span></span>
            <span className="hero-node-fields">{node.fields.map(([name, type], i) => <span className="hero-node-field" key={name}>
              {i === 0 ? <KeyRound size={11} /> : name.endsWith("_id") ? <CornerDownRight size={11} /> : <span className="field-dot" />}
              <span>{name}</span><small>{type}</small>
            </span>)}</span>
            <span className="hero-node-footer"><span /> public.{node.name}</span>
          </> : <><span className="hero-route-caption">API ENDPOINT <ArrowUpRight size={12} /></span><span className="hero-route-name"><b>GET</b>{node.name}</span></>}
        </button>)}
      </div>
      <figcaption><span id="hero-graph-description" className="hero-graph-hint">{active ? descriptions[active] : "Every connection tells a story. Explore one."}</span><span className="hero-graph-instruction">HOVER / TAP</span></figcaption>
    </figure>
  );
}
