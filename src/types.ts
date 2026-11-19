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

export type JobStatus = "pending" | "processing" | "done" | "error" | "cancelled";

export interface JobItem {
  id: string;
  info: ImageInfo;
  result: UpscaleResult | null;
  status: JobStatus;
  error: string | null;
}

export interface BatchProgress {
  current: number;
  total: number;
}
