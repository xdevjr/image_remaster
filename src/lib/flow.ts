import type { FlowNode } from "../types";

let idCounter = 0;

export function newNodeId() {
  return globalThis.crypto?.randomUUID?.() ?? `node-${Date.now()}-${idCounter++}`;
}

export function createRootNode(model: string): FlowNode {
  return {
    id: newNodeId(),
    name: "1",
    parentId: null,
    mode: "ai",
    scale: 1,
    targetWidth: null,
    targetHeight: null,
    model,
    save: true,
    visible: true,
  };
}

/** Cópia estrutural preservando ids (para editar o fluxo da imagem). */
export function copyNodes(nodes: FlowNode[]): FlowNode[] {
  return nodes.map((node) => ({ ...node }));
}

/** Cópia com ids novos, para uma nova imagem herdar o fluxo modelo. */
export function cloneNodesWithNewIds(nodes: FlowNode[]): FlowNode[] {
  const idMap = new Map<string, string>();
  for (const node of nodes) idMap.set(node.id, newNodeId());
  return nodes.map((node) => ({
    ...node,
    id: idMap.get(node.id) ?? newNodeId(),
    parentId: node.parentId ? (idMap.get(node.parentId) ?? null) : null,
    visible: true,
  }));
}

/** Ordem de exibição/execução: pai antes dos filhos, ramos na ordem de criação. */
export function orderNodes(nodes: FlowNode[]): FlowNode[] {
  const byParent = new Map<string | null, FlowNode[]>();
  for (const node of nodes) {
    const list = byParent.get(node.parentId) ?? [];
    list.push(node);
    byParent.set(node.parentId, list);
  }

  const ordered: FlowNode[] = [];
  const visited = new Set<string>();
  const visit = (parentId: string | null) => {
    for (const node of byParent.get(parentId) ?? []) {
      if (visited.has(node.id)) continue;
      visited.add(node.id);
      ordered.push(node);
      visit(node.id);
    }
  };

  visit(null);
  for (const node of nodes) {
    if (visited.has(node.id)) continue;
    visited.add(node.id);
    ordered.push(node);
    visit(node.id);
  }
  return ordered;
}

export function nodeDepth(nodes: FlowNode[], node: FlowNode): number {
  const byId = new Map(nodes.map((item) => [item.id, item]));
  const seen = new Set<string>();
  let current = node;
  let depth = 0;
  while (current.parentId) {
    if (seen.has(current.id)) break;
    seen.add(current.id);
    const parent = byId.get(current.parentId);
    if (!parent) break;
    current = parent;
    depth += 1;
  }
  return depth;
}

export function nodeInputLabel(nodes: FlowNode[], node: FlowNode): string {
  if (!node.parentId) return "Original";
  const parent = nodes.find((item) => item.id === node.parentId);
  return parent ? `Nó ${parent.name}` : "Original";
}

/** Dimensões de entrada do nó, considerando também ancestrais com resolução alvo. */
export function nodeInputSize(
  nodes: FlowNode[],
  node: FlowNode,
  imageWidth: number,
  imageHeight: number,
) {
  if (!node.parentId) {
    return { width: imageWidth, height: imageHeight };
  }
  const parent = nodes.find((item) => item.id === node.parentId);
  if (!parent) {
    return { width: imageWidth, height: imageHeight };
  }
  return nodeOutputSize(nodes, parent, imageWidth, imageHeight);
}

export function fitWithin(
  inputWidth: number,
  inputHeight: number,
  targetWidth: number,
  targetHeight: number,
) {
  const width = Math.max(1, inputWidth);
  const height = Math.max(1, inputHeight);
  const scale = Math.min(targetWidth / width, targetHeight / height);
  return {
    width: Math.max(1, Math.round(width * scale)),
    height: Math.max(1, Math.round(height * scale)),
  };
}

/** Dimensões da saída do nó (escala ou alvo, considerando a árvore). */
export function nodeOutputSize(
  nodes: FlowNode[],
  node: FlowNode,
  imageWidth: number,
  imageHeight: number,
): { width: number; height: number } {
  const byId = new Map(nodes.map((item) => [item.id, item]));
  const memo = new Map<string, { width: number; height: number }>();
  const visiting = new Set<string>();

  const compute = (current: FlowNode): { width: number; height: number } => {
    const cached = memo.get(current.id);
    if (cached) return cached;
    if (visiting.has(current.id)) {
      return { width: imageWidth, height: imageHeight };
    }
    visiting.add(current.id);

    let input = { width: imageWidth, height: imageHeight };
    if (current.parentId) {
      const parent = byId.get(current.parentId);
      if (parent) input = compute(parent);
    }

    const output =
      current.targetWidth && current.targetHeight
        ? fitWithin(input.width, input.height, current.targetWidth, current.targetHeight)
        : {
            width: Math.max(1, Math.round(input.width * (current.scale || 1))),
            height: Math.max(1, Math.round(input.height * (current.scale || 1))),
          };

    visiting.delete(current.id);
    memo.set(current.id, output);
    return output;
  };

  return compute(node);
}

export function nextNodeName(nodes: FlowNode[], parentId: string | null): string {
  const parent = parentId ? nodes.find((node) => node.id === parentId) : null;
  const prefix = parent ? `${parent.name}.` : "";
  const used = new Set<number>();
  for (const sibling of nodes) {
    if (sibling.parentId !== parentId) continue;
    if (!sibling.name.startsWith(prefix)) continue;
    const suffix = sibling.name.slice(prefix.length);
    if (/^\d+$/.test(suffix)) used.add(Number(suffix));
  }
  let index = 1;
  while (used.has(index)) index += 1;
  return `${prefix}${index}`;
}

export function descendantCount(nodes: FlowNode[], nodeId: string): number {
  let count = 0;
  let frontier = [nodeId];
  while (frontier.length > 0) {
    const next: string[] = [];
    for (const parent of frontier) {
      for (const child of nodes) {
        if (child.parentId === parent) {
          count += 1;
          next.push(child.id);
        }
      }
    }
    frontier = next;
  }
  return count;
}

/** Ids do nó e de todos os seus descendentes. */
export function collectSubtree(nodes: FlowNode[], rootId: string): Set<string> {
  const removed = new Set<string>([rootId]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const node of nodes) {
      if (node.parentId && removed.has(node.parentId) && !removed.has(node.id)) {
        removed.add(node.id);
        changed = true;
      }
    }
  }
  return removed;
}

export function nodeSummary(node: FlowNode, modelLabel: (id: string) => string): string {
  const engine = node.mode === "classic" ? "Rápido" : modelLabel(node.model);
  if (node.targetWidth && node.targetHeight) {
    return `alvo ${node.targetWidth}×${node.targetHeight} · ${engine}`;
  }
  return `${node.scale}x ${engine}`;
}

/* ---------- Layout do editor gráfico ---------- */

export const FLOW_NODE_WIDTH = 260;
export const FLOW_NODE_HEIGHT = 210;
export const FLOW_ORIGINAL_WIDTH = 200;
export const FLOW_ORIGINAL_HEIGHT = 150;
const FLOW_GAP_X = 28;
const FLOW_GAP_Y = 48;
const FLOW_ORIGINAL_Y = 20;
const FLOW_PADDING = 24;

export interface FlowLayoutNode {
  node: FlowNode;
  x: number;
  y: number;
}

export interface FlowLayoutEdge {
  from: string | null;
  to: string;
  x1: number;
  y1: number;
  x2: number;
  y2: number;
}

export interface FlowLayout {
  width: number;
  height: number;
  original: { x: number; y: number };
  nodes: FlowLayoutNode[];
  edges: FlowLayoutEdge[];
}

/** Layout de árvore: folhas em colunas, pais centralizados sobre os filhos. */
export function layoutFlowTree(nodes: FlowNode[]): FlowLayout {
  const childrenMap = new Map<string | null, FlowNode[]>();
  for (const node of nodes) {
    const list = childrenMap.get(node.parentId) ?? [];
    list.push(node);
    childrenMap.set(node.parentId, list);
  }

  const roots = childrenMap.get(null) ?? [];
  const xById = new Map<string, number>();
  const yById = new Map<string, number>();
  let cursor = 0;

  const visit = (node: FlowNode, depth: number) => {
    yById.set(
      node.id,
      FLOW_ORIGINAL_Y + FLOW_ORIGINAL_HEIGHT + FLOW_GAP_Y + (depth - 1) * (FLOW_NODE_HEIGHT + FLOW_GAP_Y),
    );
    const children = childrenMap.get(node.id) ?? [];
    if (children.length === 0) {
      xById.set(node.id, cursor);
      cursor += FLOW_NODE_WIDTH + FLOW_GAP_X;
    } else {
      for (const child of children) visit(child, depth + 1);
      const first = xById.get(children[0].id) ?? 0;
      const last = xById.get(children[children.length - 1].id) ?? first;
      xById.set(node.id, (first + last) / 2);
    }
  };

  for (const root of roots) visit(root, 1);

  const rootXs = roots.map((root) => xById.get(root.id) ?? 0);
  const rootsMin = rootXs.length > 0 ? Math.min(...rootXs) : 0;
  const rootsMax = rootXs.length > 0 ? Math.max(...rootXs) : 0;
  const originalX =
    roots.length > 0 ? (rootsMin + rootsMax + FLOW_NODE_WIDTH) / 2 - FLOW_ORIGINAL_WIDTH / 2 : 0;

  const minX = Math.min(originalX, ...Array.from(xById.values()), 0);
  const shift = -minX + FLOW_PADDING;

  let maxX = originalX + FLOW_ORIGINAL_WIDTH;
  let maxY = FLOW_ORIGINAL_Y + FLOW_ORIGINAL_HEIGHT;
  for (const node of nodes) {
    maxX = Math.max(maxX, (xById.get(node.id) ?? 0) + FLOW_NODE_WIDTH);
    maxY = Math.max(maxY, (yById.get(node.id) ?? 0) + FLOW_NODE_HEIGHT);
  }

  const layoutNodes: FlowLayoutNode[] = nodes.map((node) => ({
    node,
    x: (xById.get(node.id) ?? 0) + shift,
    y: yById.get(node.id) ?? FLOW_ORIGINAL_Y + FLOW_ORIGINAL_HEIGHT + FLOW_GAP_Y,
  }));
  const byId = new Map(layoutNodes.map((entry) => [entry.node.id, entry]));
  const original = { x: originalX + shift, y: FLOW_ORIGINAL_Y };

  const edges: FlowLayoutEdge[] = layoutNodes.map((entry) => {
    const parent = entry.node.parentId ? byId.get(entry.node.parentId) : null;
    return {
      from: entry.node.parentId,
      to: entry.node.id,
      x1: parent ? parent.x + FLOW_NODE_WIDTH / 2 : original.x + FLOW_ORIGINAL_WIDTH / 2,
      y1: parent ? parent.y + FLOW_NODE_HEIGHT : original.y + FLOW_ORIGINAL_HEIGHT,
      x2: entry.x + FLOW_NODE_WIDTH / 2,
      y2: entry.y,
    };
  });

  return {
    width: maxX + shift + FLOW_PADDING,
    height: maxY + FLOW_PADDING,
    original,
    nodes: layoutNodes,
    edges,
  };
}
