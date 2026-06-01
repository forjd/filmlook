import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";

const PREVIEW_MAX_EDGE = 1200;

export type FilmOptions = {
  strength: number;
  exposure_stops: number;
  contrast: number;
  shadows: number;
  highlight_rolloff: number;
  grain: number;
  grain_size: number;
  halation: number;
  vignette: number;
  seed: number;
};

export type RecipeSummary = {
  id: string;
  name: string;
  description: string;
  tags: string[];
  monochrome: boolean;
  defaults: FilmOptions;
};

export type PreviewResult = {
  original_data_url: string;
  rendered_data_url: string;
  width: number;
  height: number;
  preview_width: number;
  preview_height: number;
  metadata_summary: string;
  warning: string | null;
};

export type ExportResult = {
  output_path: string;
  warning: string | null;
};

export type ExportRequest = {
  input_path: string;
  output_path: string;
  recipe_id: string;
  options: FilmOptions;
  quality: number;
  auto_orient: boolean;
};

const defaultOptions: FilmOptions = {
  strength: 0.8,
  exposure_stops: 0,
  contrast: 1,
  shadows: 0,
  highlight_rolloff: 1,
  grain: 0.35,
  grain_size: 0.45,
  halation: 0.25,
  vignette: 0.15,
  seed: 1,
};

const fallbackRecipes: RecipeSummary[] = [
  {
    id: "portra-400-35mm",
    name: "Portra 400 35mm",
    description: "Soft contrast, warm skin bias, restrained saturation.",
    tags: ["color", "negative"],
    monochrome: false,
    defaults: defaultOptions,
  },
  {
    id: "kodak-gold-200",
    name: "Kodak Gold 200",
    description: "Warm consumer color with stronger yellows and reds.",
    tags: ["color", "consumer"],
    monochrome: false,
    defaults: { ...defaultOptions, grain: 0.3, halation: 0.18 },
  },
  {
    id: "kodak-ektar-100",
    name: "Kodak Ektar 100",
    description: "Crisp fine-grain color negative with saturated reds and blues.",
    tags: ["color", "negative"],
    monochrome: false,
    defaults: { ...defaultOptions, contrast: 1.08, grain: 0.18 },
  },
  {
    id: "fujifilm-velvia-50",
    name: "Fujifilm Velvia 50",
    description: "Dense slide-film contrast with vivid greens and deep blue skies.",
    tags: ["color", "slide"],
    monochrome: false,
    defaults: { ...defaultOptions, strength: 0.86, contrast: 1.15, grain: 0.12 },
  },
  {
    id: "ilford-hp5-plus-400",
    name: "Ilford HP5 Plus 400",
    description: "High-speed monochrome-inspired contrast and grain.",
    tags: ["monochrome"],
    monochrome: true,
    defaults: { ...defaultOptions, grain: 0.62, grain_size: 0.58 },
  },
  {
    id: "cinestill-800t",
    name: "CineStill 800T",
    description: "Tungsten night color with cool shadows and red halation.",
    tags: ["color", "tungsten"],
    monochrome: false,
    defaults: { ...defaultOptions, grain: 0.58, halation: 0.55 },
  },
];

const fallbackPreview: PreviewResult = {
  original_data_url: "/demo-original.jpg",
  rendered_data_url: "/demo-render.jpg",
  width: 1600,
  height: 1067,
  preview_width: 1400,
  preview_height: 934,
  metadata_summary: "Demo preview; no embedded color profile; assuming sRGB",
  warning: null,
};

export const isTauriRuntime = () =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export async function listRecipes(): Promise<RecipeSummary[]> {
  if (!isTauriRuntime()) {
    return fallbackRecipes;
  }

  return invoke<RecipeSummary[]>("list_recipes");
}

export async function renderPreview(
  inputPath: string | null,
  recipeId: string,
  options: FilmOptions,
): Promise<PreviewResult> {
  if (!isTauriRuntime() || !inputPath) {
    return fallbackPreview;
  }

  return invoke<PreviewResult>("render_preview", {
    inputPath,
    recipeId,
    options,
    maxEdge: PREVIEW_MAX_EDGE,
  });
}

export async function exportImage(request: ExportRequest): Promise<ExportResult> {
  if (!isTauriRuntime()) {
    return {
      output_path: request.output_path,
      warning: "Desktop export is available in the Tauri app.",
    };
  }

  return invoke<ExportResult>("export_image", { request });
}

export async function chooseInputImage(): Promise<string | null> {
  if (!isTauriRuntime()) {
    return null;
  }

  const selected = await open({
    multiple: false,
    directory: false,
    filters: [
      {
        name: "Images",
        extensions: ["jpg", "jpeg", "png", "tif", "tiff", "bmp", "webp", "qoi"],
      },
    ],
  });

  return typeof selected === "string" ? selected : null;
}

export async function chooseOutputImage(defaultName: string): Promise<string | null> {
  if (!isTauriRuntime()) {
    return null;
  }

  return save({
    defaultPath: defaultName,
    filters: [
      {
        name: "JPEG",
        extensions: ["jpg", "jpeg"],
      },
      {
        name: "PNG",
        extensions: ["png"],
      },
    ],
  });
}
