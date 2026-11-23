<script setup lang="ts">
import { computed } from "vue";

import IconEye from "./IconEye.vue";
import IconSave from "./IconSave.vue";
import type { FlowNode, ModelInfo } from "../types";
import {
  collectSubtree,
  fitWithin,
  nodeInputLabel,
  nodeInputSize,
  nodeOutputSize,
  nodeSummary,
} from "../lib/flow";

const props = defineProps<{
  node: FlowNode;
  nodes: FlowNode[];
  models: ModelInfo[];
  busy: boolean;
  imageWidth: number;
  imageHeight: number;
  resultTime?: number;
  resultUrl?: string;
}>();

const emit = defineEmits<{
  (event: "update-node", id: string, patch: Partial<FlowNode>): void;
  (event: "add-child", parentId: string): void;
  (event: "remove-node", id: string): void;
  (event: "toggle-visible", id: string): void;
  (event: "toggle-save", id: string): void;
}>();

const canRemove = computed(() => {
  if (props.nodes.length <= 1) return false;
  return props.nodes.length - collectSubtree(props.nodes, props.node.id).size > 0;
});

function defaultModel() {
  return props.models.find((model) => model.available)?.id ?? props.models[0]?.id ?? "qualidade";
}

function modelLabel(id: string) {
  return props.models.find((model) => model.id === id)?.label ?? id;
}

function update(patch: Partial<FlowNode>) {
  emit("update-node", props.node.id, patch);
}

function setMode(mode: FlowNode["mode"]) {
  const patch: Partial<FlowNode> = { mode };
  if (mode === "ai" && !props.node.model) patch.model = defaultModel();
  update(patch);
}

function inputFor() {
  const size = nodeInputSize(props.nodes, props.node, props.imageWidth, props.imageHeight);
  return {
    label: nodeInputLabel(props.nodes, props.node),
    dims: size.width > 0 && size.height > 0 ? `${size.width}×${size.height}` : "—",
  };
}

function isTarget() {
  return !!props.node.targetWidth && !!props.node.targetHeight;
}

function compactSummary() {
  if (props.node.targetWidth && props.node.targetHeight) {
    return `alvo ${props.node.targetWidth}×${props.node.targetHeight}`;
  }
  return `${props.node.scale}x`;
}

function formatMs(ms: number) {
  return ms >= 1000 ? `${(ms / 1000).toFixed(1)} s` : `${ms} ms`;
}

function setTargetMode(target: boolean) {
  if (!target) {
    update({ targetWidth: null, targetHeight: null });
    return;
  }
  const output = nodeOutputSize(props.nodes, props.node, props.imageWidth, props.imageHeight);
  update({ targetWidth: output.width, targetHeight: output.height });
}

function setTargetDimension(dimension: "width" | "height", event: Event) {
  const raw = Number((event.target as HTMLInputElement).value);
  if (!Number.isFinite(raw) || raw < 1) return;
  const value = Math.round(raw);
  const input = nodeInputSize(props.nodes, props.node, props.imageWidth, props.imageHeight);
  const aspect = input.width > 0 && input.height > 0 ? input.height / input.width : 1;

  if (dimension === "width") {
    update({
      targetWidth: value,
      targetHeight: Math.max(1, Math.round(value * aspect)),
    });
  } else {
    update({
      targetWidth: Math.max(1, Math.round(value / aspect)),
      targetHeight: value,
    });
  }
}

function applyTargetPreset(preset: "input" | "2x" | "4k" | "8k") {
  const input = nodeInputSize(props.nodes, props.node, props.imageWidth, props.imageHeight);
  let width = input.width;
  let height = input.height;

  if (preset === "2x") {
    width *= 2;
    height *= 2;
  } else if (preset === "4k" || preset === "8k") {
    const side = preset === "4k" ? 3840 : 7680;
    const fit = fitWithin(input.width, input.height, side, side);
    width = fit.width;
    height = fit.height;
  }

  update({
    targetWidth: Math.max(1, Math.round(width)),
    targetHeight: Math.max(1, Math.round(height)),
  });
}
</script>

<template>
  <div class="flow-graph-node" :class="{ hidden: !node.visible }">
    <div class="flow-graph-main">
      <span class="flow-graph-thumb">
        <img v-if="resultUrl" :src="resultUrl" alt="" />
        <span v-else class="flow-graph-thumb-empty">{{ compactSummary() }}</span>
      </span>
      <span class="flow-graph-text">
        <span class="flow-graph-name">Nó {{ node.name }}</span>
        <span class="flow-graph-summary">{{ nodeSummary(node, modelLabel) }}</span>
        <span class="flow-graph-input">← {{ inputFor().label }} · {{ inputFor().dims }}</span>
        <span v-if="resultTime !== undefined" class="flow-graph-time">
          {{ formatMs(resultTime) }}
        </span>
      </span>
      <span class="flow-graph-actions">
        <button
          class="graph-action"
          :class="{ on: node.save }"
          :title="
            node.save ? 'Salvar: incluído no Salvar todas' : 'Salvar: incluir no Salvar todas'
          "
          @click.stop="emit('toggle-save', node.id)"
        >
          <IconSave :active="node.save" :size="13" />
        </button>
        <button
          class="graph-action"
          :class="{ off: !node.visible }"
          :title="node.visible ? 'Ocultar do comparador' : 'Mostrar no comparador'"
          @click.stop="emit('toggle-visible', node.id)"
        >
          <IconEye :off="!node.visible" :size="13" />
        </button>
      </span>
    </div>

    <div class="flow-controls">
      <div class="pipeline-row">
        <span class="group-label">Motor</span>
        <div class="segmented">
          <button
            :class="['segmented-item', { active: node.mode === 'ai' }]"
            :disabled="busy"
            @click="setMode('ai')"
          >
            IA
          </button>
          <button
            :class="['segmented-item', { active: node.mode === 'classic' }]"
            :disabled="busy"
            @click="setMode('classic')"
          >
            Rápido
          </button>
        </div>
      </div>

      <div class="pipeline-row">
        <span class="group-label">Saída</span>
        <div class="segmented">
          <button
            :class="['segmented-item', { active: !isTarget() }]"
            :disabled="busy"
            @click="setTargetMode(false)"
          >
            Escala
          </button>
          <button
            :class="['segmented-item', { active: isTarget() }]"
            :disabled="busy"
            title="Definir uma resolução alvo (proporção mantida)"
            @click="setTargetMode(true)"
          >
            Alvo
          </button>
        </div>
      </div>

      <div v-if="!isTarget()" class="pipeline-row">
        <span class="group-label">Escala</span>
        <div class="segmented">
          <button
            v-for="value in [1, 2, 4]"
            :key="value"
            :class="['segmented-item', { active: node.scale === value }]"
            :disabled="busy"
            @click="update({ scale: value })"
          >
            {{ value }}x
          </button>
        </div>
      </div>

      <template v-else>
        <div class="pipeline-row flow-target-row">
          <span class="group-label">Alvo</span>
          <div class="flow-target-fields">
            <input
              type="number"
              min="1"
              :value="node.targetWidth ?? ''"
              :disabled="busy"
              title="Largura alvo"
              @input="setTargetDimension('width', $event)"
            />
            <span>×</span>
            <input
              type="number"
              min="1"
              :value="node.targetHeight ?? ''"
              :disabled="busy"
              title="Altura alvo"
              @input="setTargetDimension('height', $event)"
            />
          </div>
        </div>
        <div class="flow-target-presets flow-target-presets-compact">
          <button class="btn btn-mini" :disabled="busy" @click="applyTargetPreset('input')">
            Entrada
          </button>
          <button class="btn btn-mini" :disabled="busy" @click="applyTargetPreset('2x')">
            2×
          </button>
          <button class="btn btn-mini" :disabled="busy" @click="applyTargetPreset('4k')">
            4K
          </button>
          <button class="btn btn-mini" :disabled="busy" @click="applyTargetPreset('8k')">
            8K
          </button>
        </div>
      </template>

      <div v-if="node.mode === 'ai'" class="pipeline-row">
        <span class="group-label">Modelo</span>
        <div class="segmented pipeline-models">
          <button
            v-for="option in models"
            :key="option.id"
            :class="['segmented-item', { active: node.model === option.id }]"
            :disabled="busy || !option.available"
            :title="option.description"
            @click="update({ model: option.id })"
          >
            {{ option.label }}
          </button>
        </div>
      </div>
    </div>

    <div class="flow-graph-footer">
      <button class="flow-graph-add" :disabled="busy" @click.stop="emit('add-child', node.id)">
        + Ramo
      </button>
      <button
        class="graph-remove"
        :disabled="busy || !canRemove"
        title="Remover nó e todos os seus descendentes"
        @click.stop="emit('remove-node', node.id)"
      >
        ×
      </button>
    </div>
  </div>
</template>
