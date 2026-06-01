import {
  Aperture,
  Download,
  FileImage,
  FolderOpen,
  Image as ImageIcon,
  RotateCcw,
  SlidersHorizontal,
  Star,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  type FilmOptions,
  type PreviewResult,
  type RecipeSummary,
  chooseInputImage,
  chooseOutputImage,
  exportImage,
  isTauriRuntime,
  listRecipes,
  renderPreview,
} from "./api/filmlook";

type ControlSpec = {
  key: keyof FilmOptions;
  label: string;
  min: number;
  max: number;
  step: number;
  display?: (value: number) => string;
};

const controls: ControlSpec[] = [
  { key: "strength", label: "Strength", min: 0, max: 1, step: 0.01 },
  { key: "exposure_stops", label: "Exposure", min: -3, max: 3, step: 0.05, display: signedStops },
  { key: "contrast", label: "Contrast", min: 0.25, max: 2.5, step: 0.01 },
  { key: "shadows", label: "Shadows", min: -1, max: 1, step: 0.01 },
  { key: "highlight_rolloff", label: "Highlight rolloff", min: 0, max: 2, step: 0.01 },
  { key: "grain", label: "Grain", min: 0, max: 1, step: 0.01 },
  { key: "grain_size", label: "Grain size", min: 0, max: 1, step: 0.01 },
  { key: "halation", label: "Halation", min: 0, max: 1, step: 0.01 },
  { key: "vignette", label: "Vignette", min: 0, max: 1, step: 0.01 },
];

function signedStops(value: number) {
  const formatted = `${Math.abs(value).toFixed(2)} EV`;
  return value > 0 ? `+${formatted}` : value < 0 ? `-${formatted}` : "0.00 EV";
}

function numberValue(value: number) {
  return Number.isInteger(value) ? value.toString() : value.toFixed(2);
}

function App() {
  const [recipes, setRecipes] = useState<RecipeSummary[]>([]);
  const [selectedRecipeId, setSelectedRecipeId] = useState("portra-400-35mm");
  const [options, setOptions] = useState<FilmOptions | null>(null);
  const [quality, setQuality] = useState(92);
  const [inputPath, setInputPath] = useState<string | null>(null);
  const [preview, setPreview] = useState<PreviewResult | null>(null);
  const [split, setSplit] = useState(52);
  const [status, setStatus] = useState("Ready");
  const [isBusy, setIsBusy] = useState(false);
  const previewRequestId = useRef(0);

  const selectedRecipe = useMemo(
    () => recipes.find((recipe) => recipe.id === selectedRecipeId) ?? recipes[0],
    [recipes, selectedRecipeId],
  );

  useEffect(() => {
    let cancelled = false;

    listRecipes()
      .then((items) => {
        if (cancelled) {
          return;
        }
        setRecipes(items);
        const initial = items.find((recipe) => recipe.id === selectedRecipeId) ?? items[0];
        if (initial) {
          setSelectedRecipeId(initial.id);
          setOptions(initial.defaults);
        }
      })
      .catch((error) => setStatus(`Recipe load failed: ${String(error)}`));

    return () => {
      cancelled = true;
    };
  }, []);

  const refreshPreview = useCallback(
    async (path: string | null, recipeId: string, nextOptions: FilmOptions) => {
      const requestId = previewRequestId.current + 1;
      previewRequestId.current = requestId;
      setIsBusy(true);
      setStatus(path ? "Rendering preview" : "Preview sample");

      try {
        const result = await renderPreview(path, recipeId, nextOptions);
        if (requestId !== previewRequestId.current) {
          return;
        }
        setPreview(result);
        setStatus(result.warning ?? (path ? "Preview ready" : "Preview sample"));
      } catch (error) {
        if (requestId !== previewRequestId.current) {
          return;
        }
        setStatus(`Preview failed: ${String(error)}`);
      } finally {
        if (requestId === previewRequestId.current) {
          setIsBusy(false);
        }
      }
    },
    [],
  );

  useEffect(() => {
    if (!options || !selectedRecipe) {
      return;
    }

    const timer = window.setTimeout(() => {
      void refreshPreview(inputPath, selectedRecipe.id, options);
    }, inputPath ? 250 : 0);

    return () => window.clearTimeout(timer);
  }, [inputPath, options, refreshPreview, selectedRecipe]);

  const updateRecipe = (recipeId: string) => {
    const recipe = recipes.find((item) => item.id === recipeId);
    if (!recipe) {
      return;
    }

    setSelectedRecipeId(recipe.id);
    setOptions(recipe.defaults);
  };

  const updateOption = (key: keyof FilmOptions, value: number) => {
    setOptions((current) => (current ? { ...current, [key]: value } : current));
  };

  const openImage = async () => {
    const selected = await chooseInputImage();
    if (!selected) {
      setStatus(isTauriRuntime() ? "Ready" : "Running in browser preview");
      return;
    }

    setInputPath(selected);
    setStatus("Image loaded");
  };

  const exportCurrent = async () => {
    if (!options || !selectedRecipe || !inputPath) {
      setStatus(isTauriRuntime() ? "Open an image before export" : "Desktop export is available in Tauri");
      return;
    }

    const outputPath = await chooseOutputImage(defaultOutputName(inputPath, selectedRecipe.id));
    if (!outputPath) {
      setStatus("Ready");
      return;
    }

    setIsBusy(true);
    setStatus("Exporting image");
    try {
      const result = await exportImage({
        input_path: inputPath,
        output_path: outputPath,
        recipe_id: selectedRecipe.id,
        options,
        quality,
        auto_orient: true,
      });
      setStatus(result.warning ?? `Wrote ${result.output_path}`);
    } catch (error) {
      setStatus(`Export failed: ${String(error)}`);
    } finally {
      setIsBusy(false);
    }
  };

  const resetOptions = () => {
    if (selectedRecipe) {
      setOptions(selectedRecipe.defaults);
      setQuality(92);
    }
  };

  return (
    <main className="app-shell">
      <header className="topbar">
        <div className="brand">
          <Aperture size={22} strokeWidth={1.9} />
          <span>filmlook</span>
        </div>
        <div className="toolbar">
          <button className="tool-button" type="button" onClick={openImage}>
            <FolderOpen size={17} />
            <span>Open</span>
          </button>
          <button className="tool-button primary" type="button" onClick={exportCurrent} disabled={isBusy}>
            <Download size={17} />
            <span>Export</span>
          </button>
        </div>
      </header>

      <section className="workspace">
        <aside className="recipe-panel" aria-label="Recipes">
          <div className="panel-title">
            <FileImage size={17} />
            <span>Recipes</span>
          </div>
          <div className="recipe-list">
            {recipes.map((recipe) => (
              <button
                className={recipe.id === selectedRecipeId ? "recipe-row selected" : "recipe-row"}
                key={recipe.id}
                type="button"
                onClick={() => updateRecipe(recipe.id)}
              >
                <img alt="" className="recipe-thumb" src={recipeThumb(recipe.id)} />
                <span className="recipe-copy">
                  <span className="recipe-name">{recipe.name}</span>
                  <span className="recipe-id">{recipe.id}</span>
                </span>
                <Star className="recipe-star" size={17} />
              </button>
            ))}
          </div>
        </aside>

        <section className="viewer-panel" aria-label="Image preview">
          <div className="viewer-head">
            <div className="viewer-title">
              <ImageIcon size={17} />
              <span>{selectedRecipe?.name ?? "Portra 400 35mm"}</span>
            </div>
            <span className="viewer-meta">
              {preview ? `${preview.width} x ${preview.height}` : "No image"}
            </span>
          </div>

          <div className="compare-stage">
            {preview ? (
              <div className="compare-frame" style={{ "--split": `${split}%` } as React.CSSProperties}>
                <img className="compare-image original" src={preview.original_data_url} alt="Original preview" />
                <img className="compare-image rendered" src={preview.rendered_data_url} alt="Film render preview" />
                <span className="compare-label before">Before</span>
                <span className="compare-label after">After</span>
                <div className="split-handle" aria-hidden="true">
                  <span />
                </div>
                <input
                  aria-label="Before after split"
                  className="split-range"
                  max={95}
                  min={5}
                  onChange={(event) => setSplit(Number(event.target.value))}
                  type="range"
                  value={split}
                />
              </div>
            ) : (
              <div className="empty-frame">
                <ImageIcon size={34} />
                <span>No image loaded</span>
              </div>
            )}
          </div>
        </section>

        <aside className="inspector-panel" aria-label="Adjustments">
          <div className="panel-title inspector-title">
            <SlidersHorizontal size={17} />
            <span>Adjustments</span>
            <button className="icon-button" type="button" onClick={resetOptions} aria-label="Reset options">
              <RotateCcw size={15} />
            </button>
          </div>

          {selectedRecipe && (
            <section className="recipe-detail">
              <h2>{selectedRecipe.name}</h2>
              <p>{selectedRecipe.description}</p>
            </section>
          )}

          {options && (
            <section className="control-stack">
              {controls.map((control) => (
                <label className="control-row" key={control.key}>
                  <span className="control-label">{control.label}</span>
                  <span className="control-value">
                    {control.display
                      ? control.display(Number(options[control.key]))
                      : numberValue(Number(options[control.key]))}
                  </span>
                  <input
                    max={control.max}
                    min={control.min}
                    onChange={(event) => updateOption(control.key, Number(event.target.value))}
                    step={control.step}
                    type="range"
                    value={Number(options[control.key])}
                  />
                </label>
              ))}

              <label className="number-row">
                <span>Seed</span>
                <input
                  min={0}
                  onChange={(event) => updateOption("seed", Number(event.target.value))}
                  step={1}
                  type="number"
                  value={options.seed}
                />
              </label>
              <label className="number-row">
                <span>Quality</span>
                <input
                  max={100}
                  min={1}
                  onChange={(event) => setQuality(Number(event.target.value))}
                  step={1}
                  type="number"
                  value={quality}
                />
              </label>
            </section>
          )}
        </aside>
      </section>

      <footer className="statusbar">
        <span>{preview?.metadata_summary ?? "No metadata"}</span>
        <span>{isBusy ? "Working" : status}</span>
      </footer>
    </main>
  );
}

function defaultOutputName(inputPath: string, recipeId: string) {
  const fileName = inputPath.split(/[\\/]/).pop() ?? "filmlook-output.jpg";
  const dot = fileName.lastIndexOf(".");
  const stem = dot > 0 ? fileName.slice(0, dot) : fileName;
  return `${stem}-${recipeId}.jpg`;
}

function recipeThumb(recipeId: string) {
  const thumbs: Record<string, string> = {
    "portra-400-35mm": "/thumb-portra.jpg",
    "kodak-gold-200": "/thumb-gold.jpg",
    "kodak-ektar-100": "/thumb-ektar.jpg",
    "fujifilm-velvia-50": "/thumb-velvia.jpg",
    "ilford-hp5-plus-400": "/thumb-hp5.jpg",
    "cinestill-800t": "/thumb-cinestill.jpg",
  };

  return thumbs[recipeId] ?? "/demo-original.jpg";
}

export default App;
