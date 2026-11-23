import { ref } from "vue";

import { newNodeId } from "../lib/flow";
import type { FlowNode, FlowPreset } from "../types";

const STORAGE_KEY = "image-remaster:flow-presets:v1";

function isValidNode(value: unknown): value is FlowNode {
  if (!value || typeof value !== "object") return false;
  const node = value as Partial<FlowNode>;
  return (
    typeof node.id === "string" &&
    typeof node.name === "string" &&
    (node.parentId === null || typeof node.parentId === "string") &&
    (node.mode === "ai" || node.mode === "classic") &&
    typeof node.scale === "number" &&
    typeof node.model === "string"
  );
}

function loadPresets(): FlowPreset[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return [];
    const parsed: unknown = JSON.parse(raw);
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter((entry): entry is FlowPreset => {
        if (!entry || typeof entry !== "object") return false;
        const preset = entry as Partial<FlowPreset>;
        return (
          typeof preset.id === "string" &&
          typeof preset.name === "string" &&
          Array.isArray(preset.nodes) &&
          preset.nodes.every(isValidNode)
        );
      })
      .map((preset) => ({
        ...preset,
        createdAt: preset.createdAt ?? Date.now(),
        updatedAt: preset.updatedAt ?? preset.createdAt ?? Date.now(),
        nodes: preset.nodes.map((node) => {
          const hasTarget =
            typeof node.targetWidth === "number" && typeof node.targetHeight === "number";
          return {
            ...node,
            targetWidth: hasTarget ? node.targetWidth : null,
            targetHeight: hasTarget ? node.targetHeight : null,
            save: node.save !== false,
            visible: true,
          };
        }),
      }));
  } catch {
    return [];
  }
}

export function useFlowPresets() {
  const presets = ref<FlowPreset[]>(loadPresets());

  function persist() {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(presets.value));
    } catch {
      /* armazenamento indisponível: presets ficam só nesta sessão */
    }
  }

  function savePreset(name: string, nodes: FlowNode[]) {
    const trimmed = name.trim();
    if (!trimmed) return;
    const snapshot = nodes.map((node) => ({ ...node }));
    const existing = presets.value.find(
      (preset) => preset.name.toLowerCase() === trimmed.toLowerCase(),
    );

    if (existing) {
      existing.nodes = snapshot;
      existing.updatedAt = Date.now();
    } else {
      presets.value.unshift({
        id: newNodeId(),
        name: trimmed,
        createdAt: Date.now(),
        updatedAt: Date.now(),
        nodes: snapshot,
      });
    }

    presets.value = [...presets.value];
    persist();
  }

  function removePreset(id: string) {
    presets.value = presets.value.filter((preset) => preset.id !== id);
    persist();
  }

  return { presets, savePreset, removePreset };
}
