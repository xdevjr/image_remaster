export interface ImageInfo {
  path: string;
  width: number;
  height: number;
  format: string;
  fileSize: number;
}

export interface UpscaleResult {
  outputPath: string;
  width: number;
  height: number;
  inputWidth: number;
  inputHeight: number;
  elapsedMs: number;
  mode: string;
  scale: number;
  model: string;
  backend: string;
}

export interface UpscaleProgress {
  stage: string;
  completed: number;
  total: number;
  percent: number;
  elapsedMs: number;
  step?: number;
  totalSteps?: number;
}

export interface FlowNode {
  id: string;
  name: string;
  parentId: string | null;
  mode: UpscaleMode;
  scale: number;
  /** Quando definidos, substituem a escala por uma resolução alvo. */
  targetWidth: number | null;
  targetHeight: number | null;
  model: string;
  /** Incluído no "Salvar todas → nós marcados". */
  save: boolean;
  /** Visível no comparador (o nó continua sendo processado). */
  visible: boolean;
}

export interface FlowResult extends UpscaleResult {
  nodeId: string;
  parentId: string | null;
  targetWidth: number | null;
  targetHeight: number | null;
  usedModel: boolean;
}

export interface FlowPreset {
  id: string;
  name: string;
  createdAt: number;
  updatedAt: number;
  nodes: FlowNode[];
}

export interface ModelInfo {
  id: string;
  label: string;
  description: string;
  available: boolean;
  sizeBytes: number;
}

export interface GpuInfo {
  active: boolean;
  status: "ready" | "unavailable" | "unknown";
}

export interface RuntimeInfo {
  models: ModelInfo[];
  gpu: GpuInfo;
  defaultModel: string;
}

export interface StartupInfo {
  paths: string[];
  autoRun: boolean;
  model: string | null;
  scale: number | null;
}

export type UpscaleMode = "ai" | "classic";
export type ViewerMode = "side" | "slider";
export type ComparisonLayout = "auto" | "horizontal" | "vertical" | "grid";

export interface ViewerPanel {
  id?: string;
  hideable?: boolean;
  url: string;
  label: string;
  sub?: string;
  width: number;
  height: number;
}

export interface ContextMenuItem {
  id: string;
  label: string;
  danger?: boolean;
  disabled?: boolean;
}

export type JobStatus = "pending" | "processing" | "done" | "error" | "cancelled";

export interface JobItem {
  id: string;
  info: ImageInfo;
  /** Árvore de remasterização desta imagem. */
  nodes: FlowNode[];
  /** Último resultado do fluxo (usado pelo salvar rápido). */
  result: FlowResult | null;
  /** Um resultado por nó processado. */
  results: FlowResult[];
  status: JobStatus;
  error: string | null;
}

export interface BatchProgress {
  current: number;
  total: number;
}
