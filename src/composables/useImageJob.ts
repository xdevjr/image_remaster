import { onMounted, onUnmounted, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type {
  ImageInfo,
  RuntimeInfo,
  StartupInfo,
  UpscaleProgress,
  UpscaleResult,
} from "../types";

export function useImageJob() {
  const progress = ref<UpscaleProgress | null>(null);
  let unlisten: UnlistenFn | null = null;

  onMounted(async () => {
    unlisten = await listen<UpscaleProgress>("upscale:progress", (event) => {
      progress.value = event.payload;
    });
  });

  onUnmounted(() => {
    unlisten?.();
    unlisten = null;
  });

  return {
    progress,
    loadImage: (path: string) => invoke<ImageInfo>("load_image", { path }),
    upscale: (path: string, mode: string, scale: number, model: string) =>
      invoke<UpscaleResult>("upscale_image", {
        request: { path, mode, scale, model },
      }),
    saveImage: (sourcePath: string, destPath: string) =>
      invoke<string>("save_image", { sourcePath, destPath }),
    cancel: () => invoke<void>("cancel_job"),
    runtimeInfo: () => invoke<RuntimeInfo>("runtime_info"),
    startupInfo: () => invoke<StartupInfo>("startup_info"),
  };
}
