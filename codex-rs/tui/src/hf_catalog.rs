//! HuggingFace model-card scanner for YUGI.
//!
//! Fetches model lists from the HuggingFace API, reads each model card README,
//! and extracts benchmark scores (ARC-C, SWE-bench, MMLU, etc.), uncensoredness
//! ratings, ablation method, and tensor info.
//!
//! Results are cached to `~/.config/yugi/model_catalog.json` (TTL: 24 h).
//! The picker falls back to the hardcoded PREFERRED_MODELS when the cache is
//! absent or the network is unreachable.

use std::time::Duration;

use serde::Deserialize;
use serde::Serialize;

const HF_API: &str = "https://huggingface.co/api";
/// Authors whose models we scan (abliterated/uncensored GGUF series).
const CATALOG_AUTHORS: &[&str] = &[
    // — Primary abliteration / uncensoring authors ——————————————————————————
    "DavidAU",
    "SC117",
    "mlabonne",
    "huihui-ai",
    "failspy",
    "Orenguteng",
    "UnfilteredAI",
    // — GGUF quantization specialists ——————————————————————————————————————
    "bartowski",
    "mradermacher",
    "DevQuasar",
    "maddes8cht",
    "QuantFactory",
    "SicariusSicariiStuff",
    "RichardErkhov",
    "casperhansen",
    // — Creative / roleplay fine-tuners (uncensored) ————————————————————————
    "TheDrummer",            // Artemis/Skyfall/Anubis/Valkyrie/Rocinante series
    "Epiculous",             // Violet_Twilight/Effervescence
    "sophosympatheia",       // Midnight-Miqu series
    "ddh0",                  // Qwen3.5-27B Heretic Marvin; DeepSeek uncensored
    "nbeerbower",            // Huihui-Qwen3.5-27B Athanorlite ORPO abliterated
    "grimjim",               // gemma-3 abliterated
    "cognitivecomputations", // dolphin series
    // — Other active abliteration authors ——————————————————————————————————
    "Nitral-AI",
    "PocketDoc",
    "Undi95",
    "MaziyarPanahi",
    "LoneStriker",
    "kalomaze",
    "vihangd",
    "smirki",
    "h2oai",
    // — CRACK / multi-tier abliteration specialists ————————————————————————
    "dealignai",     // CRACK series: Bonsai-27B ternary, MiniCPM-35B, Bolt-74B
    "BoldingBuilds", // in-place abliteration on NVIDIA quantized GGUFs
    // — October 2026 additions ————————————————————————————————————————————
    "0bserverx",   // RVN triple-ARA abliteration (KL:0.0085), 1M+ downloads
    "philbert440", // Qwen3.8-27B cyber-specialized, 507K downloads
    "dima6312",    // Flint-40B abliterix+Optuna, MITRE refusals 0%
    // — HauhauCS family (0/465 refusals, FastMTP, K_P quants) ─────────────────
    "HauhauCS",        // Aggressive/Balanced series: Gemma-4E4B, 9B/12B/26B/27B/35B/122B
    "JonathanColetti", // Qwen3.8-27B-Uncensored heretic (KL:0.0141, 2.4M downloads)
    "LuffyTheFox",     // Genesis noise-repair method (Marchenko-Pastur) on HauhauCS base
];
const CACHE_PATH_SUFFIX: &str = ".config/yugi/model_catalog.json";
/// How many hours before the cache is considered stale.
const CATALOG_TTL_HOURS: u64 = 24;
/// Best-effort Q4_K_M quant tag used when building pull IDs.
const DEFAULT_QUANT: &str = "Q4_K_M";

// ── data model ───────────────────────────────────────────────────────────────

/// Benchmark and ablation metadata extracted from a model card README.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelBenchmarks {
    /// ARC-Challenge (0–100)
    pub arc_c: Option<f32>,
    /// SWE-bench Verified pass@1 (0–100)
    pub swe_bench: Option<f32>,
    /// MMLU (0–100)
    pub mmlu: Option<f32>,
    /// GSM8K (0–100)
    pub gsm8k: Option<f32>,
    /// HumanEval pass@1 (0–100)
    pub humaneval: Option<f32>,
    /// GPQA Diamond (0–100)
    pub gpqa: Option<f32>,
    /// Refusal reduction percentage reported in the card (0–100)
    pub uncensored_pct: Option<f32>,
    /// Short description of the ablation / uncensoring method
    pub ablation_method: String,
    /// Tensor / layer info from the card (e.g. "layers 14–32 modified")
    pub tensors_info: String,
}

impl ModelBenchmarks {
    /// Composite quality score used for picker sorting.
    /// Weights: SWE-bench (heaviest) > ARC-C > MMLU > HumanEval.
    pub fn composite(&self) -> f32 {
        let swe = self.swe_bench.unwrap_or(0.0) * 3.0;
        let arc = self.arc_c.unwrap_or(0.0) * 2.0;
        let mml = self.mmlu.unwrap_or(0.0) * 1.5;
        let hev = self.humaneval.unwrap_or(0.0);
        swe + arc + mml + hev
    }

    /// One-line summary of scores for display in the picker.
    pub fn score_summary(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        if let Some(v) = self.swe_bench {
            parts.push(format!("SWE {v:.1}"));
        }
        if let Some(v) = self.arc_c {
            parts.push(format!("ARC-C {v:.1}"));
        }
        if let Some(v) = self.mmlu {
            parts.push(format!("MMLU {v:.1}"));
        }
        if let Some(v) = self.uncensored_pct {
            parts.push(format!("{v:.0}% uncensored"));
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!("[{}]", parts.join("  "))
        }
    }
}

/// A single pullable GGUF model discovered from HuggingFace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEntry {
    /// Ollama pull string, e.g. `hf.co/DavidAU/Qwen3-Crack-27B-Instruct-GGUF:Q4_K_M`
    pub pull_id: String,
    /// Human-readable display label (shown in picker)
    pub display: String,
    /// Estimated file size in GB for the selected quant
    pub size_gb: f32,
    /// Extracted benchmark and ablation data
    pub benchmarks: ModelBenchmarks,
    /// Unix seconds when this entry was last fetched
    pub refreshed_at: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct CatalogFile {
    version: u32,
    entries: Vec<CatalogEntry>,
    fetched_at: u64,
}

// ── cache I/O ────────────────────────────────────────────────────────────────

fn cache_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(CACHE_PATH_SUFFIX))
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Load catalog from disk.  Returns `None` if missing or older than 24 h.
pub fn load_catalog() -> Option<Vec<CatalogEntry>> {
    let path = cache_path()?;
    let text = std::fs::read_to_string(&path).ok()?;
    let file: CatalogFile = serde_json::from_str(&text).ok()?;
    if file.version != 1 {
        return None;
    }
    let age_hours = now_unix().saturating_sub(file.fetched_at) / 3600;
    if age_hours > CATALOG_TTL_HOURS {
        return None;
    }
    Some(file.entries)
}

/// Persist catalog to disk (silently ignores I/O errors).
pub fn save_catalog(entries: &[CatalogEntry]) {
    let Some(path) = cache_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let file = CatalogFile {
        version: 1,
        entries: entries.to_vec(),
        fetched_at: now_unix(),
    };
    if let Ok(json) = serde_json::to_string_pretty(&file) {
        let _ = std::fs::write(&path, json);
    }
}

// ── HuggingFace fetching ─────────────────────────────────────────────────────

/// Fetch and parse a fresh catalog from the HuggingFace API.
///
/// Returns entries sorted best-first.  Never panics; returns an empty vec on
/// network or parse failure.
pub async fn refresh_catalog() -> Vec<CatalogEntry> {
    #[allow(deprecated)]
    let client = codex_http_client::HttpClientBuilder::new()
        .build_with_transport_default_proxy_and_custom_ca_fallback();

    let mut entries: Vec<CatalogEntry> = Vec::new();

    for author in CATALOG_AUTHORS {
        let url = format!("{HF_API}/models?author={author}&filter=gguf&sort=downloads&limit=100");
        let text = match client
            .get(&url)
            .timeout(Duration::from_secs(20))
            .send()
            .await
        {
            Ok(r) => match r.text().await {
                Ok(t) => t,
                Err(_) => continue,
            },
            Err(_) => continue,
        };

        let models: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let Some(arr) = models.as_array() else {
            continue;
        };

        for item in arr {
            let Some(model_id) = item.get("modelId").and_then(|v| v.as_str()) else {
                continue;
            };

            if !is_interesting_model(model_id) {
                continue;
            }

            // Fetch the README (the model card lives there)
            let readme_url = format!("https://huggingface.co/{model_id}/raw/main/README.md");
            let readme = match client
                .get(&readme_url)
                .timeout(Duration::from_secs(15))
                .send()
                .await
            {
                Ok(r) => r.text().await.unwrap_or_default(),
                Err(_) => String::new(),
            };

            let benchmarks = parse_benchmarks(&readme);
            let (size_gb, quant) = pick_quant_and_size(model_id, &readme);
            let pull_id = format!("hf.co/{}:{}", model_id, quant);
            let display = build_display(model_id, &benchmarks, size_gb, &quant);

            entries.push(CatalogEntry {
                pull_id,
                display,
                size_gb,
                benchmarks,
                refreshed_at: now_unix(),
            });
        }
    }

    // Sort best-first: composite score descending, then larger models first
    entries.sort_by(|a, b| {
        let sa = a.benchmarks.composite() + (a.size_gb * 0.1);
        let sb = b.benchmarks.composite() + (b.size_gb * 0.1);
        sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
    });

    entries
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn is_interesting_model(id: &str) -> bool {
    let low = id.to_lowercase();
    // Must be GGUF
    if !low.contains("gguf") {
        return false;
    }
    // Must be any notable size (8B and up) or a special large series
    let has_param = low.contains("8b")
        || low.contains("9b")
        || low.contains("12b")
        || low.contains("14b")
        || low.contains("26b")
        || low.contains("27b")
        || low.contains("30b")
        || low.contains("31b")
        || low.contains("32b")
        || low.contains("35b")
        || low.contains("40b")
        || low.contains("48b")
        || low.contains("70b")
        || low.contains("72b")
        || low.contains("122b")
        || low.contains("123b")
        || low.contains("177b")
        || low.contains("235b")
        || low.contains("flash-next")
        || low.contains("gsq-rco");
    if !has_param {
        return false;
    }
    // Must be abliterated / uncensored / cyber / crack / drowzee / heretic / dolphin series
    low.contains("ablit")
        || low.contains("uncensor")
        || low.contains("cyber")
        || low.contains("crack")
        || low.contains("drowzee")
        || low.contains("drowzy")
        || low.contains("heretic")
        || low.contains("flash-next")
        || low.contains("gsq-rco")
        || low.contains("dolphin")
        || low.contains("lexi")
        || low.contains("neural")
}

/// Parse benchmark scores and ablation metadata from a raw model card README.
pub fn parse_benchmarks(readme: &str) -> ModelBenchmarks {
    let mut bm = ModelBenchmarks::default();

    for line in readme.lines() {
        let low = line.to_lowercase();

        // ── benchmark scores ────────────────────────────────────────────────
        // ARC-Challenge
        if (low.contains("arc-c")
            || low.contains("arc_c")
            || low.contains("arc-challenge")
            || low.contains("arc_challenge"))
            && !low.contains("arc-e")
            && bm.arc_c.is_none()
        {
            bm.arc_c = extract_score(line);
        }

        // SWE-bench
        if (low.contains("swe-bench") || low.contains("swebench") || low.contains("swe_bench"))
            && bm.swe_bench.is_none()
        {
            bm.swe_bench = extract_score(line);
        }

        // MMLU (skip MMLU-Pro)
        if low.contains("mmlu") && !low.contains("pro") && bm.mmlu.is_none() {
            bm.mmlu = extract_score(line);
        }

        // GSM8K
        if (low.contains("gsm8k") || low.contains("gsm-8k")) && bm.gsm8k.is_none() {
            bm.gsm8k = extract_score(line);
        }

        // HumanEval
        if (low.contains("humaneval") || low.contains("human_eval") || low.contains("human eval"))
            && bm.humaneval.is_none()
        {
            bm.humaneval = extract_score(line);
        }

        // GPQA Diamond
        if low.contains("gpqa") && bm.gpqa.is_none() {
            bm.gpqa = extract_score(line);
        }

        // ── uncensoredness / refusal percentage ─────────────────────────────
        if (low.contains("refusal") || low.contains("uncensor"))
            && (low.contains("reduction") || low.contains("reduced") || low.contains('%'))
            && bm.uncensored_pct.is_none()
        {
            bm.uncensored_pct = extract_percentage(line);
        }

        // ── ablation method ─────────────────────────────────────────────────
        if bm.ablation_method.is_empty() {
            if low.contains("orpo") {
                bm.ablation_method = "ORPO fine-tuning".to_string();
            } else if low.contains("abliterat") && low.contains("lora") {
                bm.ablation_method = "LoRA abliteration".to_string();
            } else if low.contains("abliterat") {
                bm.ablation_method = "Abliteration".to_string();
            } else if low.contains("direct weight") || low.contains("weight surgery") {
                bm.ablation_method = "Direct weight surgery".to_string();
            }
        }

        // ── tensor / layer info ──────────────────────────────────────────────
        if bm.tensors_info.is_empty()
            && (low.contains("tensor") || low.contains("layer"))
            && (low.contains("remov")
                || low.contains("modif")
                || low.contains("surgic")
                || low.contains("ablat"))
        {
            let trimmed = line.trim();
            if trimmed.len() < 200 {
                bm.tensors_info = trimmed.to_string();
            }
        }
    }

    bm
}

/// Extract the first float in (0, 100] from a line (skips years, counts, etc.)
fn extract_score(line: &str) -> Option<f32> {
    for token in line.split(['|', ':', '\t', ' ', ',', '(', ')']) {
        let t = token.trim().trim_end_matches('%');
        if let Ok(v) = t.parse::<f32>() {
            if v > 0.0 && v <= 100.0 {
                return Some(v);
            }
        }
    }
    None
}

/// Extract the first "X%" percentage value from a line.
fn extract_percentage(line: &str) -> Option<f32> {
    let bytes = line.as_bytes();
    let pct_pos = line.find('%')?;
    // Walk backwards from '%' to find the start of the number
    let before = &line[..pct_pos];
    let start = before
        .rfind(|c: char| !c.is_ascii_digit() && c != '.')
        .map(|i| i + 1)
        .unwrap_or(0);
    before[start..]
        .parse::<f32>()
        .ok()
        .filter(|_| bytes[pct_pos] == b'%')
}

/// Choose the best quant to recommend and estimate its disk size.
fn pick_quant_and_size(model_id: &str, readme: &str) -> (f32, String) {
    let low = readme.to_lowercase();
    let id_low = model_id.to_lowercase();

    // IQ3_S for 177B sharded (Strata) models
    if id_low.contains("gsq-rco") || id_low.contains("flash-next") || id_low.contains("177b") {
        return (84.0, "IQ3_S".to_string());
    }

    // Q4_K_M is the default; use Q5_K_M if the card explicitly recommends it
    let quant = if low.contains("q5_k_m") && !low.contains("q4_k_m") {
        "Q5_K_M"
    } else {
        DEFAULT_QUANT
    };

    // Rough GB estimate by parameter count
    let size_gb: f32 = if id_low.contains("235b") {
        140.0
    } else if id_low.contains("123b") {
        75.0
    } else if id_low.contains("72b") || id_low.contains("70b") {
        43.0
    } else if id_low.contains("40b") {
        24.0
    } else if id_low.contains("35b") {
        21.0
    } else if id_low.contains("32b") {
        20.0
    } else if id_low.contains("27b") {
        16.0
    } else if id_low.contains("14b") || id_low.contains("13b") {
        10.0
    } else if id_low.contains("12b") {
        8.0
    } else if id_low.contains("9b") {
        6.0
    } else if id_low.contains("8b") || id_low.contains("7b") {
        5.0
    } else {
        5.0
    };

    (size_gb, quant.to_string())
}

/// Build a one-line display label for the picker.
fn build_display(model_id: &str, bm: &ModelBenchmarks, size_gb: f32, quant: &str) -> String {
    // Strip author prefix from the display name
    let short = model_id.split('/').last().unwrap_or(model_id);
    // Remove trailing "-GGUF" suffix
    let short = short.trim_end_matches("-GGUF");
    let score_str = bm.score_summary();
    let scores = if score_str.is_empty() {
        String::new()
    } else {
        format!("  {score_str}")
    };
    format!("{short}  {quant}  {size_gb:.0} GB{scores}")
}

// ── public helpers ────────────────────────────────────────────────────────────

/// Convert catalog entries to the `(&str, &str)` pairs used by the static
/// `PREFERRED_MODELS` fallback — useful when merging catalog results at runtime.
///
/// Returns owned strings; callers box them into `'static`-lifetime slices as
/// needed or use the `CatalogEntry` type directly in the model picker.
#[allow(dead_code)]
pub fn catalog_entries_to_display(entries: &[CatalogEntry]) -> Vec<(String, String)> {
    entries
        .iter()
        .map(|e| (e.pull_id.clone(), e.display.clone()))
        .collect()
}
