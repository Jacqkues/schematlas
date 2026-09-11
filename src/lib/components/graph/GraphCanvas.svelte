<script lang="ts">
  import { tick, untrack, onDestroy } from 'svelte';
  import { fly } from 'svelte/transition';
  import {
    SvelteFlow,
    Background,
    Controls,
    MiniMap,
    Panel,
    type Node,
    type Edge,
    useSvelteFlow,
  } from '@xyflow/svelte';
  import GroupNode from './GroupNode.svelte';
  import RelationEdge from './RelationEdge.svelte';
  import { relationships, neighborhood } from '$lib/services/relationships';
  import EntityNode from './EntityNode.svelte';
  import {
    createPositionResolver,
    graphBounds,
    NODE_WIDTH,
    MAX_FIELDS,
    nodeHeight,
    matchingIds,
    filterGraph,
    groupBounds,
    translateGroup,
  } from '$lib/services/layout';
  import { arrangeInWorker, type LayoutJob } from '$lib/services/arrange';
  import { motion } from '$lib/motion';
  import type { Entity, Source, Position } from '$lib/types';
  import { LayoutGrid, Scan, Map as MapIcon, SearchX } from '@lucide/svelte';
  let {
    source,
    query,
    namespaces = [],
    related = true,
    focusNodeIds = [],
    onselect,
    onsave,
  }: {
    source: Source;
    query: string;
    namespaces?: string[];
    related?: boolean;
    focusNodeIds?: string[];
    onselect: (entity: Entity | null) => void;
    onsave: (positions: Record<string, Position>) => void;
  } = $props();
  let displayedNodes = $state.raw<Node[]>([]);
  let drag: {
    id: string;
    origin: Position;
    positions: Record<string, Position>;
    members: string[];
  } | null = null;
  let edges = $state.raw<Edge[]>([]);
  let minimap = $state(false);
  let initialized = $state(false);
  let selectedId = $state<string | null>(null);
  let arranging = $state(false);
  let layoutError = $state('');
  let job: LayoutJob | null = null;
  let fitted = false;
  let dragFrame = 0;
  let pendingDrag: { node: Node | null; nodes: Node[] } | null = null;
  onDestroy(() => {
    job?.cancel();
    cancelAnimationFrame(dragFrame);
  });
  const resolvePositions = createPositionResolver();
  const flow = useSvelteFlow();
  const edgeTypes = { relation: RelationEdge };
  const nodeTypes = { entity: EntityNode, canvasGroup: GroupNode };
  let filteredGraph = $derived(filterGraph(source.graph, namespaces, related));
  let visible = $derived(matchingIds(filteredGraph, query));
  $effect(() => {
    const positions = resolvePositions(source.graph, source.positions);
    const previous = new Map(untrack(() => displayedNodes).map((node) => [node.id, node]));
    const incoming = new Map<string, Set<string>>();
    const outgoing = new Map<string, Set<string>>();
    for (const relation of source.graph.relations) {
      if (relation.sourceField) {
        const fields = outgoing.get(relation.source) ?? new Set<string>();
        fields.add(relation.sourceField);
        outgoing.set(relation.source, fields);
      }
      if (relation.targetField) {
        const fields = incoming.get(relation.target) ?? new Set<string>();
        fields.add(relation.targetField);
        incoming.set(relation.target, fields);
      }
    }
    drag = null;
    const entities: Node[] = source.graph.entities.map((entity) => {
      const foreignFields = outgoing.get(entity.id) ?? new Set<string>();
      const incomingFields = incoming.get(entity.id) ?? new Set<string>();
      // Reuse node data while the entity and its own ports are unchanged, so cards do not re-render on every save.
      const signature = JSON.stringify([entity, [...foreignFields], [...incomingFields]]);
      const old = previous.get(entity.id);
      return {
        id: entity.id,
        type: 'entity',
        position: positions[entity.id],
        selected: old?.selected ?? false,
        initialWidth: NODE_WIDTH,
        initialHeight: nodeHeight(entity.fields.length),
        measured: old?.measured,
        data:
          old?.data.signature === signature
            ? old.data
            : {
                signature,
                entity,
                foreignFields,
                incomingFields,
                oninspect: () => onselect(entity),
              },
        deletable: false,
        zIndex: 1,
      };
    });
    displayedNodes = untrack(() => withOverlays(entities));
    const entityById = new Map(source.graph.entities.map((entity) => [entity.id, entity]));
    const fieldVisible = (e: Entity | undefined, name: string | null) =>
      !!name && !!e?.fields.slice(0, MAX_FIELDS).some((f) => f.name === name);
    edges = relationships(source.graph, source.kind === 'database').map((r) => ({
      id: r.id,
      source: r.source,
      target: r.target,
      sourceHandle: fieldVisible(entityById.get(r.source), r.sourceField)
        ? `out-${r.sourceField}`
        : 'entity-out',
      targetHandle: fieldVisible(entityById.get(r.target), r.targetField)
        ? `in-${r.targetField}`
        : 'entity-in',
      type: 'relation',
      data: { ...r },
      ariaLabel: r.description,
      deletable: false,
      // Highlight changes the stroke (CSS classes), never its layer above table cards.
      zIndex: 0,
    }));
  });
  $effect(() => {
    const relatedIds = neighborhood(source.graph, selectedId);
    void visible;
    const active = selectedId;
    untrack(() => {
      displayedNodes = withOverlays(
        displayedNodes
          .filter((n) => n.type === 'entity')
          .map((n) => {
            const className = !active
              ? ''
              : relatedIds.has(n.id)
                ? 'entity-related'
                : 'entity-dimmed';
            return n.class === className ? n : { ...n, class: className };
          }),
      );
    });
  });
  function withOverlays(entities: Node[]): Node[] {
    const previous = new Map(
      displayedNodes.filter((n) => n.type === 'canvasGroup').map((n) => [n.id, n]),
    );
    const overlays: Node[] = groupBounds(
      source.groups ?? [],
      { ...source.graph, entities: source.graph.entities.filter((e) => visible.has(e.id)) },
      Object.fromEntries(entities.map((n) => [n.id, n.position])),
    ).map((group) => ({
      id: group.id,
      type: 'canvasGroup',
      position: { x: group.x, y: group.y },
      data: {
        name: group.name,
        count: group.count,
        color: group.color,
        onmove: (delta: Position) => nudgeGroup(group.groupId, delta),
      },
      width: group.width,
      height: group.height,
      style: `width:${group.width}px;height:${group.height}px`,
      class: 'pointer-events-none',
      draggable: true,
      dragHandle: '.group-drag-handle',
      selectable: false,
      focusable: false,
      connectable: false,
      deletable: false,
      zIndex: -1,
    }));
    return [
      ...overlays.map((n) => {
        const old = previous.get(n.id);
        return old &&
          old.position.x === n.position.x &&
          old.position.y === n.position.y &&
          old.width === n.width &&
          old.height === n.height &&
          old.data.name === n.data.name &&
          old.data.color === n.data.color &&
          old.data.count === n.data.count
          ? old
          : n;
      }),
      ...entities.map((n) => {
        const hidden = !visible.has(n.id);
        return n.hidden === hidden ? n : { ...n, hidden };
      }),
    ];
  }
  function currentPositions() {
    return Object.fromEntries(
      displayedNodes.filter((n) => n.type === 'entity').map((n) => [n.id, n.position]),
    );
  }
  function applyPositions(positions: Record<string, Position>) {
    displayedNodes = withOverlays(
      displayedNodes
        .filter((n) => n.type === 'entity')
        .map((n) => {
          const position = positions[n.id] ?? n.position;
          return n.position.x === position.x && n.position.y === position.y
            ? n
            : { ...n, position };
        }),
    );
  }
  function startDrag(node: Node | null) {
    if (node?.type !== 'canvasGroup') return;
    const group = source.groups?.find((g) => `canvas-group:${g.id}` === node.id);
    if (group) {
      const positions = currentPositions();
      const bounds = groupBounds(
        [group],
        { ...source.graph, entities: source.graph.entities.filter((e) => visible.has(e.id)) },
        positions,
      )[0];
      if (bounds)
        drag = {
          id: node.id,
          origin: { x: bounds.x, y: bounds.y },
          positions,
          members: group.nodeIds,
        };
    }
  }
  function scheduleDrag(node: Node | null, nodes: Node[]) {
    pendingDrag = { node, nodes };
    if (!dragFrame)
      dragFrame = requestAnimationFrame(() => {
        dragFrame = 0;
        if (pendingDrag) moveDrag(pendingDrag.node, pendingDrag.nodes);
        pendingDrag = null;
      });
  }
  function moveDrag(node: Node | null, movingNodes: Node[]) {
    if (drag && node && drag.id === node.id) {
      applyPositions(
        translateGroup(drag.positions, drag.members, {
          x: node.position.x - drag.origin.x,
          y: node.position.y - drag.origin.y,
        }),
      );
    } else {
      applyPositions({
        ...currentPositions(),
        ...Object.fromEntries(
          movingNodes.filter((n) => n.type === 'entity').map((n) => [n.id, n.position]),
        ),
      });
    }
  }
  function nudgeGroup(id: string, delta: Position) {
    const group = source.groups?.find((g) => g.id === id);
    if (!group) return;
    applyPositions(translateGroup(currentPositions(), group.nodeIds, delta));
    persist();
  }
  let displayedEdges = $derived(
    edges.map((e) => {
      const active = !!selectedId && (e.source === selectedId || e.target === selectedId);
      return {
        ...e,
        hidden: !visible.has(e.source) || !visible.has(e.target),
        class: active ? 'edge-active' : selectedId ? 'edge-muted' : '',
        data: { ...e.data, showLabels: active },
      };
    }),
  );
  async function fitGraph(duration = 200, all = false) {
    await tick();
    const chosen = new Set(all ? [] : focusNodeIds.filter((id) => visible.has(id)));
    const bounds = graphBounds(source.graph, currentPositions(), chosen.size ? chosen : visible);
    if (bounds) await flow.fitBounds(bounds, { duration, padding: 0.15 });
  }
  // The first fit is instant; later filter changes glide to the new bounds.
  $effect(() => {
    void query;
    void namespaces;
    void related;
    void focusNodeIds;
    if (!initialized) return;
    let cancelled = false;
    void tick().then(() => {
      if (cancelled) return;
      untrack(() => void fitGraph(fitted ? 200 : 0));
      fitted = true;
    });
    return () => {
      cancelled = true;
    };
  });
  function persist() {
    const positions = currentPositions();
    if (Object.keys(positions).length) onsave(positions);
  }
  async function arrange() {
    if (arranging) return;
    arranging = true;
    layoutError = '';
    const graph = source.graph;
    const groups = source.groups ?? [];
    const before = JSON.stringify(currentPositions());
    const groupSnapshot = JSON.stringify(groups);
    try {
      job = arrangeInWorker(graph, groups);
      const positions = await job.result;
      // Discard a result if the agent changed the canvas while layout was running.
      if (
        source.graph !== graph ||
        JSON.stringify(source.groups ?? []) !== groupSnapshot ||
        JSON.stringify(currentPositions()) !== before
      ) {
        layoutError = 'Canvas changed. Run layout again.';
        return;
      }
      applyPositions(positions);
      persist();
      void fitGraph(250, true);
    } catch (e) {
      layoutError = e instanceof Error ? e.message : 'Layout worker could not start.';
    } finally {
      job = null;
      arranging = false;
    }
  }
</script>

<div class="relative min-w-0 flex-1 bg-canvas" aria-label="Interactive schema graph">
  <SvelteFlow
    bind:nodes={displayedNodes}
    edges={displayedEdges}
    {nodeTypes}
    {edgeTypes}
    colorMode="dark"
    class="[--xy-background-color:var(--color-canvas)]"
    oninit={() => {
      initialized = true;
      if (
        source.graph.entities.some(
          (e) =>
            !Number.isFinite(source.positions[e.id]?.x) ||
            !Number.isFinite(source.positions[e.id]?.y),
        )
      )
        void tick().then(arrange);
    }}
    minZoom={0.01}
    maxZoom={1.8}
    nodesConnectable={false}
    elevateEdgesOnSelect={false}
    deleteKey={null}
    onlyRenderVisibleElements
    onselectionchange={({ nodes }) => {
      selectedId = nodes.find((n) => n.type === 'entity')?.id ?? null;
    }}
    onpaneclick={() => {
      selectedId = null;
      onselect(null);
    }}
    onnodedragstart={({ targetNode }) => startDrag(targetNode)}
    onnodedrag={({ targetNode, nodes }) => scheduleDrag(targetNode, nodes)}
    onnodedragstop={({ targetNode, nodes }) => {
      cancelAnimationFrame(dragFrame);
      dragFrame = 0;
      pendingDrag = null;
      moveDrag(targetNode, nodes);
      drag = null;
      persist();
    }}
    onselectiondragstop={persist}
  >
    <Background patternColor="#272d33" gap={22} size={1} /><Controls
      showLock={false}
      showFitView={false}
      class="overflow-hidden rounded-[7px] border border-line-soft shadow-[0_2px_5px_#0002] [&_button]:size-[29px]"
      buttonBgColor="var(--color-surface)"
      buttonBgColorHover="var(--color-surface-3)"
      buttonColor="var(--color-soft)"
      buttonColorHover="var(--color-ink)"
      buttonBorderColor="var(--color-line-soft)"
    />
    {#if minimap}<MiniMap
        pannable
        zoomable
        class="bottom-1 max-h-[95px] max-w-[145px] overflow-hidden rounded-md border border-line-soft"
        bgColor="var(--color-surface)"
        nodeColor={source.kind === 'openapi' ? '#526d5e' : '#475460'}
        maskColor="rgba(6,8,10,0.85)"
      />{/if}
    <Panel position="top-right"
      ><div
        class="flex items-center gap-1 rounded-[7px] border border-line-soft bg-surface-3 p-[3px] text-[#a6b0b9] shadow-[0_3px_12px_#0004]"
      >
        <button
          class="icon-btn text-inherit hover:bg-surface-4"
          aria-label="Auto layout"
          title="Arrange tables by domain and relationships"
          disabled={arranging}
          onclick={arrange}><LayoutGrid size={17} /></button
        ><button
          class="icon-btn text-inherit hover:bg-surface-4"
          aria-label="Fit graph to screen"
          onclick={() => fitGraph(250, true)}><Scan size={17} /></button
        ><span class="mx-0.5 h-[15px] w-px bg-line"></span><button
          class="icon-btn text-inherit hover:bg-surface-4 aria-pressed:bg-surface-4"
          aria-label="Toggle minimap"
          aria-pressed={minimap}
          onclick={() => (minimap = !minimap)}><MapIcon size={17} /></button
        >
      </div></Panel
    >
  </SvelteFlow>
  {#if arranging || layoutError}<div
      class="absolute bottom-5 left-1/2 -translate-x-1/2 rounded-lg border border-accent-line bg-accent-soft px-4 py-2.5 text-xs text-accent-text"
      role="status"
      transition:fly={motion.toast()}
    >
      {arranging ? 'Arranging domains…' : layoutError}
    </div>{/if}
  {#if !visible.size}<div
      class="pointer-events-none absolute top-1/2 left-1/2 min-w-[270px] -translate-x-1/2 -translate-y-1/2 rounded-xl bg-[#17191cee] p-[25px] text-center text-[#bdbfc2]"
      transition:fly={motion.popover()}
    >
      <SearchX size={30} />
      <h3 class="mt-[15px] mb-2 font-medium">
        {query ? 'No matching nodes' : 'No visible schema objects'}
      </h3>
      <p class="text-[11px]">
        {query
          ? 'Try a table, endpoint, model, or column name.'
          : 'This source contains no objects visible to this connection.'}
      </p>
    </div>{/if}
</div>
