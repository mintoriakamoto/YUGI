//! Local provider discovery and shared startup picker presentation.
//! Discovery and selection outcomes stay independent of the picker layout.

use std::io;
use std::process::Stdio;

use crate::hf_catalog;

use futures::future::join_all;
use tokio::io::AsyncBufReadExt as _;

use crate::bottom_pane::picker_option_list;
use crate::bottom_pane::render_menu_surface;
use crate::key_hint;
use crate::key_hint::KeyBinding;
use crate::key_hint::KeyBindingListExt;
use crate::render::Insets;
use crate::render::renderable::FlexRenderable;
use crate::render::renderable::Renderable;
use crate::render::renderable::RenderableExt as _;
use crate::render::renderable::RenderableItem;
use codex_http_client::HttpClient;
use codex_http_client::HttpClientBuilder;
use codex_model_provider_info::DEFAULT_LMSTUDIO_PORT;
use codex_model_provider_info::DEFAULT_OLLAMA_PORT;
use codex_model_provider_info::LMSTUDIO_OSS_PROVIDER_ID;
use codex_model_provider_info::OLLAMA_OSS_PROVIDER_ID;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use crossterm::event::{self};
use crossterm::execute;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::*;
use ratatui::text::Line;
use ratatui::widgets::Clear;
use ratatui::widgets::Paragraph;
use ratatui::widgets::Widget;
use ratatui::widgets::WidgetRef;
use ratatui::widgets::Wrap;
use std::time::Duration;

/// A local AI server discovered during port scanning.
#[derive(Clone)]
pub(crate) struct DiscoveredServer {
    /// Canonical provider ID ("ollama" or "lmstudio").
    pub(crate) provider_id: String,
    /// Port the server is listening on.
    pub(crate) port: u16,
    /// Human-readable label for the picker.
    pub(crate) label: String,
    /// Short description shown below the picker.
    pub(crate) description: String,
    /// Full base URL override (None = use provider default).
    pub(crate) base_url: Option<String>,
}

/// All known local AI servers and their default ports.
struct ServerSpec {
    provider_id: &'static str,
    port: u16,
    label: &'static str,
    description: &'static str,
}

const KNOWN_SERVERS: &[ServerSpec] = &[
    ServerSpec {
        provider_id: OLLAMA_OSS_PROVIDER_ID,
        port: DEFAULT_OLLAMA_PORT,
        label: "Ollama",
        description: "Ollama (port 11434) — default",
    },
    ServerSpec {
        provider_id: LMSTUDIO_OSS_PROVIDER_ID,
        port: DEFAULT_LMSTUDIO_PORT,
        label: "LM Studio",
        description: "LM Studio (port 1234) — OpenAI-compatible",
    },
    ServerSpec {
        provider_id: LMSTUDIO_OSS_PROVIDER_ID,
        port: 8000,
        label: "vLLM / LocalAI (8000)",
        description: "vLLM or compatible server on port 8000",
    },
    ServerSpec {
        provider_id: LMSTUDIO_OSS_PROVIDER_ID,
        port: 8080,
        label: "Strata / LlamaFile (8080)",
        description: "Strata (github.com/Niko1221/Strata) or LlamaFile on port 8080 — runs 177B sharded models on consumer GPUs",
    },
    ServerSpec {
        provider_id: LMSTUDIO_OSS_PROVIDER_ID,
        port: 5001,
        label: "Text-gen-webui (5001)",
        description: "Oobabooga text-generation-webui on port 5001",
    },
    ServerSpec {
        provider_id: OLLAMA_OSS_PROVIDER_ID,
        port: 11435,
        label: "Ollama (11435)",
        description: "Ollama on alternate port 11435",
    },
    ServerSpec {
        provider_id: LMSTUDIO_OSS_PROVIDER_ID,
        port: 12434,
        label: "LM Studio (12434)",
        description: "LM Studio on alternate port 12434",
    },
];

/// An option shown in the interactive picker.
struct SelectOption {
    label: Line<'static>,
    description: String,
    key: KeyCode,
    provider_id: String,
    base_url: Option<String>,
}

// This startup wizard runs before the main TUI runtime keymap is available, so
// it retains the built-in horizontal shortcuts alongside vertical navigation.
const MOVE_LEFT_KEYS: [KeyBinding; 2] = [
    key_hint::plain(KeyCode::Left),
    key_hint::ctrl(KeyCode::Char('h')),
];
const MOVE_RIGHT_KEYS: [KeyBinding; 2] = [
    key_hint::plain(KeyCode::Right),
    key_hint::ctrl(KeyCode::Char('l')),
];

pub struct OssSelectionWidget {
    select_options: Vec<SelectOption>,

    /// Currently selected index.
    selected_option: usize,

    done: bool,
    selection: Option<(String, Option<String>)>, // (provider_id, base_url)
}

impl OssSelectionWidget {
    fn from_discovered(servers: &[DiscoveredServer]) -> Self {
        let digit_keys: &[char] = &['1', '2', '3', '4', '5', '6', '7', '8', '9'];
        let options: Vec<SelectOption> = servers
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let key = if i < digit_keys.len() {
                    KeyCode::Char(digit_keys[i])
                } else {
                    KeyCode::Null
                };
                SelectOption {
                    label: Line::from(vec![
                        Span::raw(s.label.clone()),
                        Span::styled(
                            format!(" (port {})", s.port),
                            Style::default().add_modifier(Modifier::DIM),
                        ),
                    ]),
                    description: s.description.clone(),
                    key,
                    provider_id: s.provider_id.clone(),
                    base_url: s.base_url.clone(),
                }
            })
            .collect();

        Self {
            select_options: options,
            selected_option: 0,
            done: false,
            selection: None,
        }
    }

    /// Consume a key press; returns the completed decision when done.
    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<(String, Option<String>)> {
        if key.kind == KeyEventKind::Press {
            self.handle_select_key(key);
        }
        if self.done {
            self.selection.clone()
        } else {
            None
        }
    }

    fn normalize_keycode(code: KeyCode) -> KeyCode {
        match code {
            KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
            other => other,
        }
    }

    fn handle_select_key(&mut self, key_event: KeyEvent) {
        let n = self.select_options.len();
        match key_event {
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers,
                ..
            } if modifiers.contains(KeyModifiers::CONTROL) => {
                self.send_decision("__CANCELLED__".to_string(), None);
            }
            _ if MOVE_LEFT_KEYS.is_pressed(key_event)
                || matches!(key_event.code, KeyCode::Up | KeyCode::Char('k')) =>
            {
                if n > 0 {
                    self.selected_option = (self.selected_option + n - 1) % n;
                }
            }
            _ if MOVE_RIGHT_KEYS.is_pressed(key_event)
                || matches!(key_event.code, KeyCode::Down | KeyCode::Char('j')) =>
            {
                if n > 0 {
                    self.selected_option = (self.selected_option + 1) % n;
                }
            }
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                if n > 0 {
                    let opt = &self.select_options[self.selected_option];
                    self.send_decision(opt.provider_id.clone(), opt.base_url.clone());
                }
            }
            KeyEvent {
                code: KeyCode::Esc, ..
            } => {
                // Default to first option on Esc.
                if n > 0 {
                    let opt = &self.select_options[0];
                    self.send_decision(opt.provider_id.clone(), opt.base_url.clone());
                }
            }
            KeyEvent { code, .. } => {
                let normalized = Self::normalize_keycode(code);
                if let Some(opt) = self
                    .select_options
                    .iter()
                    .find(|opt| Self::normalize_keycode(opt.key) == normalized)
                {
                    self.send_decision(opt.provider_id.clone(), opt.base_url.clone());
                }
            }
        }
    }

    fn send_decision(&mut self, provider_id: String, base_url: Option<String>) {
        self.selection = Some((provider_id, base_url));
        self.done = true;
    }

    pub fn is_complete(&self) -> bool {
        self.done
    }
}

impl WidgetRef for &OssSelectionWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let mut column = FlexRenderable::new();
        column.push(/*flex*/ 1, RenderableItem::Borrowed(&""));
        column.push(
            /*flex*/ 0,
            Paragraph::new("Select a local AI server".bold())
                .wrap(Wrap { trim: false })
                .inset(Insets::vh(/*v*/ 0, /*h*/ 2)),
        );
        column.push(
            /*flex*/ 1,
            Paragraph::new("YUGI detected the following running servers.".dim())
                .wrap(Wrap { trim: false })
                .inset(Insets::vh(/*v*/ 0, /*h*/ 2)),
        );
        let labels: Vec<_> = self
            .select_options
            .iter()
            .map(|option| option.label.clone())
            .collect();
        column.push(
            /*flex*/ 0,
            picker_option_list(labels, self.selected_option),
        );
        let desc = self
            .select_options
            .get(self.selected_option)
            .map(|o| o.description.clone())
            .unwrap_or_default();
        column.push(
            /*flex*/ 1,
            Paragraph::new(Span::styled(desc, Style::default().add_modifier(Modifier::DIM)))
                .wrap(Wrap { trim: false })
                .inset(Insets::vh(/*v*/ 0, /*h*/ 2)),
        );
        column.push(/*flex*/ 1, RenderableItem::Borrowed(&""));
        column.push(
            /*flex*/ 0,
            Paragraph::new(Line::from(vec![
                "↑/↓".bold(),
                " choose · ".dim(),
                key_hint::plain(KeyCode::Enter).into(),
                " select · ".dim(),
                key_hint::ctrl(KeyCode::Char('c')).into(),
                " exit".dim(),
            ]))
            .wrap(Wrap { trim: false })
            .inset(Insets::vh(/*v*/ 0, /*h*/ 2)),
        );
        column.push(/*flex*/ 1, RenderableItem::Borrowed(&""));
        let panel = Rect {
            height: column.desired_height(area.width).min(area.height),
            ..area
        };
        render_menu_surface(panel, buf);
        column.render(panel, buf);
    }
}

pub(crate) struct OssProviderSelection {
    pub(crate) provider: String,
    pub(crate) manually_selected: bool,
    /// If Some, caller should set CODEX_OSS_BASE_URL to this value.
    pub(crate) base_url: Option<String>,
}

pub(crate) enum OssProviderDetection {
    AutoSelected(OssProviderSelection),
    /// No single server was auto-selected; present a picker with these options.
    NeedsSelection(Vec<DiscoveredServer>),
}

/// Probe all known local AI server ports in parallel and auto-select if exactly one responds.
pub(crate) async fn detect_oss_provider() -> OssProviderDetection {
    // Bypass proxy for loopback connections; allow invalid inherited CA bundles.
    #[allow(deprecated)]
    let client = HttpClientBuilder::new().build_direct_with_custom_ca_fallback();

    // Probe all ports concurrently.
    let probes: Vec<_> = KNOWN_SERVERS
        .iter()
        .map(|spec| {
            let c = client.clone();
            async move {
                let running = check_port_status(&c, spec.port).await.unwrap_or(false);
                (spec, running)
            }
        })
        .collect();

    let results = join_all(probes).await;

    let mut discovered: Vec<DiscoveredServer> = Vec::new();
    for (spec, running) in results {
        if running {
            let is_default_port = (spec.provider_id == OLLAMA_OSS_PROVIDER_ID
                && spec.port == DEFAULT_OLLAMA_PORT)
                || (spec.provider_id == LMSTUDIO_OSS_PROVIDER_ID
                    && spec.port == DEFAULT_LMSTUDIO_PORT);

            let base_url = if is_default_port {
                None
            } else {
                Some(format!("http://localhost:{}/v1", spec.port))
            };

            discovered.push(DiscoveredServer {
                provider_id: spec.provider_id.to_string(),
                port: spec.port,
                label: spec.label.to_string(),
                description: spec.description.to_string(),
                base_url,
            });
        }
    }

    match discovered.len() {
        1 => {
            let server = discovered.remove(0);
            OssProviderDetection::AutoSelected(OssProviderSelection {
                provider: server.provider_id,
                manually_selected: false,
                base_url: server.base_url,
            })
        }
        _ => {
            if discovered.is_empty() {
                // No servers running — show defaults so user can still pick one to launch.
                discovered.push(DiscoveredServer {
                    provider_id: OLLAMA_OSS_PROVIDER_ID.to_string(),
                    port: DEFAULT_OLLAMA_PORT,
                    label: "Ollama".to_string(),
                    description: "Ollama (port 11434) — YUGI will show model picker on start".to_string(),
                    base_url: None,
                });
                discovered.push(DiscoveredServer {
                    provider_id: LMSTUDIO_OSS_PROVIDER_ID.to_string(),
                    port: DEFAULT_LMSTUDIO_PORT,
                    label: "LM Studio".to_string(),
                    description: "LM Studio (port 1234) — YUGI will show model picker on start".to_string(),
                    base_url: None,
                });
            }
            OssProviderDetection::NeedsSelection(discovered)
        }
    }
}

/// Show the interactive picker and return the user's choice.
pub(crate) async fn select_oss_provider(
    discovered: Vec<DiscoveredServer>,
) -> io::Result<OssProviderSelection> {
    let mut widget = OssSelectionWidget::from_discovered(&discovered);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = (|| {
        terminal.draw(|f| {
            (&widget).render_ref(f.area(), f.buffer_mut());
        })?;
        crate::tui::discard_pending_terminal_input()?;

        loop {
            if let Event::Key(key_event) = event::read()?
                && let Some((provider_id, base_url)) = widget.handle_key_event(key_event)
            {
                break Ok(OssProviderSelection {
                    provider: provider_id,
                    manually_selected: true,
                    base_url,
                });
            }

            terminal.draw(|f| {
                (&widget).render_ref(f.area(), f.buffer_mut());
            })?;
        }
    })();

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    result
}

// ---------------------------------------------------------------------------
// Model discovery, picker and auto-install
// ---------------------------------------------------------------------------

/// Curated HuggingFace GGUF models — abliterated/uncensored (Heretic method), high benchmarks.
/// All use the Ollama `hf.co/<author>/<repo>:<quant>` syntax so `ollama pull` fetches them directly.
/// Scores are from model cards: ARC-C via Nightmedia zero-shot instruct, GPQA-Diamond as reported.
/// "Heretic ARA" = Arbitrary-Rank Ablation; refusal drop verified per card.
static PREFERRED_MODELS: &[(&str, &str)] = &[
    // ── 4B class ──────────────────────────────────────────────────────────────
    // Qwen3.5-4B fully abliterated (SicariusSicariiStuff, 2026-03)
    (
        "hf.co/SicariusSicariiStuff/Qwen3.5-4B_Abliterated_GGUF:Q4_K_M",
        "SicariusSicariiStuff  Qwen3.5-4B abliterated  Q4_K_M  ~2.5 GB",
    ),
    // ── 8B class ─────────────────────────────────────────────────────────────
    (
        "hf.co/DavidAU/Qwen3-8B-Hivemind-Instruct-Heretic-Abliterated-Uncensored-NEO-Imatrix-GGUF:Q4_K_M",
        "DavidAU  Qwen3-8B Hivemind Heretic abliterated  Q4_K_M  ~5 GB  ARC-C:0.558 refusals:8/100",
    ),
    (
        "hf.co/mlabonne/Meta-Llama-3.1-8B-Instruct-abliterated-GGUF:Q4_K_M",
        "mlabonne Llama-3.1-8B abliterated  Q4_K_M  ~5 GB  GPQA:0.89 IFEval:73.3  32K downloads",
    ),
    (
        "hf.co/SC117/QwenPaw-Flash-9B-heretic-GGUF:Q4_K_M",
        "SC117    Qwen3-9B QwenPaw-Flash heretic          Q4_K_M  ~6 GB  refusals:3/100",
    ),
    // ── 12B class ────────────────────────────────────────────────────────────
    // gemma-3-12B abliterated (grimjim, multimodal VLM, image+text)
    (
        "hf.co/grimjim/gemma-3-12b-it-abliterated-GGUF:Q4_K_M",
        "grimjim  gemma-3-12B abliterated (VLM)  Q4_K_M  ~7 GB",
    ),
    // Violet_Twilight v0.2 — SLERP merge uncensored creative RP, 8.7K downloads (Epiculous)
    (
        "hf.co/Epiculous/Violet_Twilight-v0.2-GGUF:Q4_K_M",
        "Epiculous  Violet_Twilight v0.2 uncensored RP  Q4_K_M  ~7 GB  8.7K downloads",
    ),
    // ── 14B class ────────────────────────────────────────────────────────────
    (
        "hf.co/bartowski/mlabonne_Qwen3-14B-abliterated-GGUF:Q4_K_M",
        "bartowski/mlabonne  Qwen3-14B abliterated  Q4_K_M  ~9 GB",
    ),
    // ── 16B class (TheDrummer Rocinante-XL) ──────────────────────────────────
    // Rocinante-XL-16B: creative storytelling / RP, 7K downloads, no-refusal by design
    (
        "hf.co/TheDrummer/Rocinante-XL-16B-v1-GGUF:Q4_K_M",
        "TheDrummer  Rocinante-XL-16B v1 uncensored RP  Q4_K_M  ~10 GB  7K downloads",
    ),
    // DavidAU Qwen3-14B Cyber (TheDrummer-class quality)
    (
        "hf.co/DavidAU/Qwen3-14B-Cyber-GGUF:Q4_K_M",
        "DavidAU  Qwen3-14B Cyber uncensored  Q4_K_M  ~9 GB",
    ),
    // ── 27B class (dense, Heretic ARA uncensored) — verified model cards ─────
    // Qwen3.8-27B Turbo: ARC-C 0.735, ARC-E 0.882 (Nightmedia instruct), GPQA-D 89.2 (base),
    //   SWE-bench Pro 61.7 (base); Heretic KL 0.0535, refusals 0/100; hybrid DeltaNet+Attn
    (
        "hf.co/DavidAU/Qwen3.8-27B-TURBO-Fable-Cold-Fusion-735-882-Heretic-Uncensored-NEO-CODER-MAX-MTP-GGUF:Q4_K_M",
        "DavidAU  Qwen3.8-27B TURBO Heretic  Q4_K_M  ~16 GB  ARC-C:0.735 ARC-E:0.882 GPQA:89.2 refusals:11/100 KL:0.0025",
    ),
    // Qwen3.6-27B Fable Fusion: ARC-C 0.711, ARC-E 0.879; Heretic KL 0.0469, refusals 4/100;
    //   MMLU-Redux 93.5, SWE-bench Verified 77.2 (base Qwen3.6-27B); 4M+ total downloads
    (
        "hf.co/DavidAU/Qwen3.6-27B-Fable-Fusion-711-Uncensored-Heretic-NM-DAU-NEO-MAX-MTP-GGUF:Q4_K_M",
        "DavidAU  Qwen3.6-27B Fable-Fusion Heretic  Q4_K_M  ~16 GB  ARC-C:0.711 MMLU:93.5 refusals:4/100",
    ),
    // Qwen3.6-27B NEO-CODE fine-tune: coding-focused, uncensored heretic
    (
        "hf.co/DavidAU/Qwen3.6-27B-Heretic-Uncensored-FINETUNE-NEO-CODE-Di-IMatrix-MAX-GGUF:Q4_K_M",
        "DavidAU  Qwen3.6-27B NEO-CODE Heretic (coder)  Q4_K_M  ~16 GB",
    ),
    // Qwen3.8-27B TWIN-TURBO L: alternate cold-fusion abliteration variant
    (
        "hf.co/DavidAU/Qwen3.8-27B-TWIN-TURBO-Fable-Cold-Fusion-709-L-Uncensored-NM-DAU-NEO-MTP-GGUF:Q4_K_M",
        "DavidAU  Qwen3.8-27B TWIN-TURBO-L Uncensored  Q4_K_M  ~16 GB",
    ),
    // Qwen3.8-27B TWIN-TURBO ULTRA HERETIC: highest-refusal-drop TURBO variant
    (
        "hf.co/DavidAU/Qwen3.8-27B-TWIN-TURBO-Fable-Cold-Fusion-709-ULTRA-HERETIC-Uncensored-NM-DAU-NEO-MTP-GGUF:Q4_K_M",
        "DavidAU  Qwen3.8-27B TWIN-TURBO ULTRA Heretic  Q4_K_M  ~16 GB",
    ),
    // SC117 Qwen3.8-27B uncensored FIT (SC117's abliteration method)
    (
        "hf.co/SC117/Qwen3.8-27B-Uncensored-FIT-GGUF:Q4_K_M",
        "SC117    Qwen3.8-27B Uncensored FIT  Q4_K_M  ~16 GB",
    ),
    // mlabonne gemma-3-27b abliterated (verified, popular, Q4_K_M and Q6_K)
    (
        "hf.co/mlabonne/gemma-3-27b-it-abliterated-GGUF:Q4_K_M",
        "mlabonne gemma-3-27b-it abliterated  Q4_K_M  ~17 GB",
    ),
    (
        "hf.co/mlabonne/gemma-3-27b-it-abliterated-GGUF:Q6_K",
        "mlabonne gemma-3-27b-it abliterated  Q6_K    ~22 GB  (higher quality)",
    ),
    // bartowski re-quant of mlabonne gemma-3-27b (wider quant selection)
    (
        "hf.co/bartowski/mlabonne_gemma-3-27b-it-abliterated-GGUF:Q4_K_M",
        "bartowski/mlabonne gemma-3-27b abliterated  Q4_K_M  ~17 GB",
    ),
    // Qwen3.5-27B Heretic Marvin V1 — uncensored DPO+SFT creative writing anti-rep (ddh0 quant)
    (
        "hf.co/ddh0/Qwen3.5-27B-Heretic-Marvin-V1-GGUF:Q4_K_M",
        "ddh0  Qwen3.5-27B Heretic Marvin V1 (DPO uncensored)  Q4_K_M  ~16 GB",
    ),
    // Huihui-Qwen3.5-27B abliterated Athanorlite ORPO v2 (nbeerbower, multimodal VLM)
    (
        "hf.co/nbeerbower/Huihui-Qwen3.5-27B-abliterated-Athanorlite-ORPO-v2-GGUF:Q4_K_M",
        "nbeerbower  Qwen3.5-27B abliterated Athanorlite ORPO v2 (VLM)  Q4_K_M  ~16 GB",
    ),
    // Effervescence-27B — creative RP uncensored instruct (Epiculous, 2-phase LoRA train)
    (
        "hf.co/Epiculous/Effervescence-27B-GGUF:Q4_K_M",
        "Epiculous  Effervescence-27B uncensored RP  Q4_K_M  ~16 GB",
    ),
    // ── 26B class (SC117 gemma-4-26B) ────────────────────────────────────────
    (
        "hf.co/SC117/gemma-4-26B-A4B-it-qat-heretic-GGUF:Q4_K_M",
        "SC117    gemma-4-26B heretic QAT  Q4_K_M  ~16 GB  refusals:13/100 KL:0.066  28K downloads",
    ),
    // ── 24B class (LFM2.5 MoE uncensored, mradermacher, 2026-10-23) ──────────
    // LFM2.5-24B-A2B-uncensored2: 24B total / 2B active MoE, heretic-tagged uncensored
    (
        "hf.co/mradermacher/LFM2.5-24B-A2B-uncensored2-GGUF:Q4_K_M",
        "mradermacher LFM2.5-24B-A2B uncensored2 (MoE)  Q4_K_M  ~8 GB  [new 2026-10-23]",
    ),
    // LFM2.5-24B-A2B-uncensored2 imatrix variant
    (
        "hf.co/mradermacher/LFM2.5-24B-A2B-uncensored2-i1-GGUF:Q4_K_M",
        "mradermacher LFM2.5-24B-A2B uncensored2 imatrix  Q4_K_M  ~8 GB  [new 2026-10-23]",
    ),
    // ── 31B class ────────────────────────────────────────────────────────────
    // DavidAU gemma-4-31B Mystery Heretic (thinking mode, uncensored)
    (
        "hf.co/DavidAU/gemma-4-31B-it-Mystery-Fine-Tune-HERETIC-UNCENSORED-Thinking-Instruct-GGUF:Q4_K_M",
        "DavidAU  gemma-4-31B Heretic Mystery (thinking)  Q4_K_M  ~19 GB",
    ),
    // TheDrummer Artemis-31B v1.1 — gemma-4-31B base, creative uncensored, 3.7K downloads
    (
        "hf.co/TheDrummer/Artemis-31B-v1.1-GGUF:Q4_K_M",
        "TheDrummer  Artemis-31B v1.1 (gemma-4 base) uncensored  Q4_K_M  ~19 GB  3.7K downloads",
    ),
    // TheDrummer Skyfall-31B v4.2 — Magistral-Small-2509 base, 7K downloads
    (
        "hf.co/TheDrummer/Skyfall-31B-v4.2-GGUF:Q4_K_M",
        "TheDrummer  Skyfall-31B v4.2 (Magistral base) uncensored  Q4_K_M  ~19 GB  7K downloads",
    ),
    // ── 34B MoE class (DavidAU Qwen3-48B-A4B Savant, effective ~34B) ─────────
    (
        "hf.co/DavidAU/Qwen3-48B-A4B-Savant-Commander-Distill-12X-Closed-Open-Heretic-Uncensored-GGUF:Q4_K_M",
        "DavidAU  Qwen3-48B-A4B Savant Heretic (MoE)  Q4_K_M  ~20 GB",
    ),
    // ── 35B class ────────────────────────────────────────────────────────────
    // Sage-A1.5 Heretic Abliterated Uncensored (MoE VLM, mradermacher quant, 2026-10-23)
    (
        "hf.co/mradermacher/Sage-A1.5-Heretic-Abliterated-Uncensored-GGUF:Q4_K_M",
        "mradermacher Sage-A1.5 Heretic Abliterated (MoE VLM)  Q4_K_M  [new 2026-10-23]",
    ),
    (
        "hf.co/mradermacher/Sage-A1.5-Heretic-Abliterated-Uncensored-i1-GGUF:Q4_K_M",
        "mradermacher Sage-A1.5 Heretic Abliterated imatrix (MoE VLM)  Q4_K_M  [new 2026-10-23]",
    ),
    // SC117 Qwen3.6-35B-A3B MoE uncensored heretic — 65K downloads
    (
        "hf.co/SC117/Qwen3.6-35B-A3B-uncensored-heretic-Native-MTP-Preserved-APEX-GGUF:Q4_K_M",
        "SC117    Qwen3.6-35B-A3B Uncensored Heretic (MoE)  Q4_K_M  ~21 GB  65K downloads",
    ),
    // SC117 Ornith-1.0-35B Heretic — 25K downloads (flagship Ornith series)
    (
        "hf.co/SC117/Ornith-1.0-35B-Heretic-MTP-APEX-GGUF:Q4_K_M",
        "SC117    Ornith-1.0-35B Heretic MTP APEX  Q4_K_M  ~21 GB  25K downloads",
    ),
    // SC117 Ornith-1.5-35B-A3B Heretic MoE — 14K downloads (MoE variant)
    (
        "hf.co/SC117/Ornith-1.5-35B-A3B-Heretic-MTP-APEX-GGUF:Q4_K_M",
        "SC117    Ornith-1.5-35B-A3B Heretic (MoE)  Q4_K_M  ~21 GB  14K downloads",
    ),
    (
        "hf.co/SC117/occamy-1.0-abliterated-FIT-GGUF:Q4_K_M",
        "SC117    occamy-1.0 35B abliterated FIT  Q4_K_M  ~21 GB",
    ),
    // ── 49B class (TheDrummer Valkyrie) ──────────────────────────────────────
    // Valkyrie-49B v2.1 — NVIDIA Nemotron-Super-49B base, creative uncensored, 1.1K downloads
    (
        "hf.co/TheDrummer/Valkyrie-49B-v2.1-GGUF:Q4_K_M",
        "TheDrummer  Valkyrie-49B v2.1 (Nemotron-Super base) uncensored  Q4_K_M  ~29 GB  1K downloads",
    ),
    // ── 40B class (expanded from Qwen3.6-27B, 96 layers) ────────────────────
    // Qwen3.6-40B Deckard: ARC-C 0.651 (40B), 2M+ downloads, Claude 4.6 Opus distill,
    //   variable-length reasoning, 1,275 tensors; ~50% more layers than 27B base
    (
        "hf.co/DavidAU/Qwen3.6-40B-Claude-4.6-Opus-Deckard-Heretic-Uncensored-Thinking-NEO-CODE-Di-IMatrix-MAX-GGUF:Q4_K_M",
        "DavidAU  Qwen3.6-40B Deckard Heretic (thinking)  Q4_K_M  ~24 GB  ARC-C:0.651 2M+dl",
    ),
    // Qwen3.6-40B Grand Intelligence Fable Fusion
    (
        "hf.co/DavidAU/Qwen3.6-40B-Grand-Intelligence-Fable-Fusion-Uncensored-Heretic-NM-DAU-NEO-MAX-MTP-GGUF:Q4_K_M",
        "DavidAU  Qwen3.6-40B Grand-Intelligence Heretic  Q4_K_M  ~24 GB",
    ),
    // Qwen3.6-40B Fable-Fusion-6-Core Deckard-Eleanor: 6-core cold fusion merge
    (
        "hf.co/DavidAU/Qwen3.6-40B-Fable-Fusion-6-Core-Deckard-Eleanor-Heretic-Uncensored-NM-DAU-NEO-MAX-MTP-GGUF:Q4_K_M",
        "DavidAU  Qwen3.6-40B 6-Core Deckard-Eleanor Heretic  Q4_K_M  ~24 GB",
    ),
    // ── 70B–72B class ────────────────────────────────────────────────────────
    // Llama-3.3-70B abliterated (huihui-ai base, mradermacher imatrix quant)
    (
        "hf.co/mradermacher/Llama-3.3-70B-Instruct-abliterated-i1-GGUF:Q4_K_M",
        "mradermacher Llama-3.3-70B abliterated imatrix  Q4_K_M  ~43 GB",
    ),
    // Llama-3.1-70B abliterated (mradermacher)
    (
        "hf.co/mradermacher/Llama-3.1-70B-Instruct-abliterated-GGUF:Q4_K_M",
        "mradermacher Llama-3.1-70B abliterated  Q4_K_M  ~43 GB",
    ),
    // Qwen2.5-72B abliterated (huihui-ai, via DevQuasar GGUF pack)
    (
        "hf.co/DevQuasar/huihui-ai.Qwen2.5-72B-Instruct-abliterated-GGUF:Q4_K_M",
        "DevQuasar/huihui-ai  Qwen2.5-72B abliterated  Q4_K_M  ~43 GB",
    ),
    // dolphin-2.9.2-qwen2-72b (uncensored, bartowski quant)
    (
        "hf.co/bartowski/dolphin-2.9.2-qwen2-72b-GGUF:Q4_K_M",
        "bartowski dolphin-2.9.2-qwen2-72b uncensored  Q4_K_M  ~43 GB",
    ),
    // dolphin-2.9-llama3-70b (uncensored, bartowski quant)
    (
        "hf.co/bartowski/dolphin-2.9-llama3-70b-GGUF:Q4_K_M",
        "bartowski dolphin-2.9-llama3-70b uncensored  Q4_K_M  ~43 GB",
    ),
    // TheDrummer Anubis-70B v1.2 — Llama-3.3-70B base, creative uncensored, 2.7K downloads
    (
        "hf.co/TheDrummer/Anubis-70B-v1.2-GGUF:Q4_K_M",
        "TheDrummer  Anubis-70B v1.2 (Llama-3.3 base) uncensored  Q4_K_M  ~43 GB  2.7K downloads",
    ),
    // ── 74B MoE class (NVIDIA Nemotron 3.5 Bolt) ─────────────────────────────
    // 74B total / 9B active, Mamba-2–Transformer hybrid LatentMoE, GPQA-D 86.43
    (
        "hf.co/DavidAU/NVIDIA-Nemotron-3.5-Bolt-Turbo-Brilliance-50x-74B-A9B-24GB-GGUF:Q4_K_M",
        "DavidAU  Nemotron-3.5-Bolt 74B-A9B Turbo (LatentMoE)  Q4_K_M  ~24 GB  GPQA:86.43",
    ),
    // ── 122B MoE class (SC117, Qwen3.5-122B-A10B) ────────────────────────────
    (
        "hf.co/SC117/Qwen3.5-122B-A10B-Uncensored-APEX-Compact-GGUF:Q4_K_M",
        "SC117    Qwen3.5-122B-A10B Uncensored APEX (MoE)  Q4_K_M  ~60 GB",
    ),
    // ── 177B class (GSQ-RCO sharded, requires Strata engine) ─────────────────
    // Qwen3.8 Flash-Next: 125B LM + 51B n-gram embedding + 4B MTP = ~177B
    // GPQA-Diamond 92.93 (IQ3_S measured), SC117 abliteration of GSQ-RCO quant
    // IQ3_S = ~84 GB; needs Strata engine: github.com/Niko1221/Strata (port 8080, 64 GB+ RAM+VRAM)
    (
        "hf.co/DavidAU/Qwen3.8-Flash-Next-Abliterated-Uncensored-Turbo-Brilliance-50x-Reasoning-Instruct-GSQ-RCO-GGUF:IQ3_S",
        "DavidAU  177B Flash-Next Abliterated GSQ-RCO  IQ3_S  ~84 GB  GPQA:92.93  [Strata]",
    ),
    // huihui-ai Flash-Next v2 — updated abliteration (MoE), 2026-10-19
    (
        "hf.co/huihui-ai/Huihui-Qwen3.8-Flash-Next-abliterated-v2-GGUF:Q4_K_M",
        "huihui-ai Qwen3.8-Flash-Next abliterated v2 (MoE)  Q4_K_M  ~84 GB  [new 2026-10-19]",
    ),
    // SC117 Swift-1.5 — newest SC117 abliteration of Flash-Next GSQ-RCO
    (
        "hf.co/SC117/Swift-1.5-Qwen3.8-Flash-Next-GSQ-RCO-abliterated-GGUF:IQ3_S",
        "SC117    177B Swift-1.5 Flash-Next abliterated   IQ3_S  ~84 GB  [Strata]",
    ),
    (
        "hf.co/DavidAU/Qwen3.8-Flash-Next-Turbo-Brilliance-50x-Reasoning-Instruct-GSQ-RCO-GGUF:IQ3_S",
        "DavidAU  177B Flash-Next Turbo GSQ-RCO (base)   IQ3_S  ~84 GB  [Strata]",
    ),
];

pub(crate) enum OssModelDetection {
    /// Exactly one model is installed — use it automatically.
    AutoSelected(String),
    /// Zero or multiple models — show the picker.
    NeedsSelection {
        installed: Vec<String>,
        can_pull: bool,
        provider_id: String,
        #[allow(dead_code)]
        base_url: Option<String>,
    },
}

pub(crate) struct OssModelResult {
    pub(crate) model: String,
    /// True when the model is not yet installed and must be pulled first.
    pub(crate) needs_install: bool,
}

// An option row in the model picker.
struct ModelOption {
    id: String,
    label: Line<'static>,
    key: KeyCode,
    is_installed: bool,
    /// Optional score summary line shown in the detail pane (from HF catalog).
    score_hint: String,
}

pub(crate) struct OssModelWidget {
    server_label: String,
    options: Vec<ModelOption>,
    selected: usize,
    done: bool,
    result: Option<(String, bool)>, // (model_id, needs_install)
}

impl OssModelWidget {
    fn new(server_label: &str, installed: &[String], can_pull: bool) -> Self {
        let digit_keys: &[char] = &['1', '2', '3', '4', '5', '6', '7', '8', '9'];
        let mut options: Vec<ModelOption> = Vec::new();

        for (i, id) in installed.iter().enumerate() {
            let key = if i < digit_keys.len() {
                KeyCode::Char(digit_keys[i])
            } else {
                KeyCode::Null
            };
            options.push(ModelOption {
                id: id.clone(),
                label: Line::from(Span::raw(id.clone())),
                key,
                is_installed: true,
                score_hint: String::new(),
            });
        }

        if can_pull {
            let offset = installed.len();
            // Prefer live catalog (scored + fresher) over the static fallback list.
            let live_catalog = hf_catalog::load_catalog();
            let catalog_pairs: Vec<(String, String, String)> = if let Some(ref entries) = live_catalog {
                entries
                    .iter()
                    .map(|e| (e.pull_id.clone(), e.display.clone(), e.benchmarks.score_summary()))
                    .collect()
            } else {
                PREFERRED_MODELS
                    .iter()
                    .map(|(id, disp)| (id.to_string(), disp.to_string(), String::new()))
                    .collect()
            };

            for (i, (model_id, desc, score_hint)) in catalog_pairs.iter().enumerate() {
                let already = installed.iter().any(|m| m.as_str() == model_id.as_str());
                if already {
                    continue;
                }
                let idx = offset + i;
                let key = if idx < digit_keys.len() {
                    KeyCode::Char(digit_keys[idx])
                } else {
                    KeyCode::Null
                };
                options.push(ModelOption {
                    id: model_id.to_string(),
                    label: Line::from(vec![
                        Span::styled("[install] ", Style::default().fg(ratatui::style::Color::Yellow)),
                        Span::raw(desc.to_string()),
                    ]),
                    key,
                    is_installed: false,
                    score_hint: score_hint.clone(),
                });
            }
        }

        Self {
            server_label: server_label.to_string(),
            options,
            selected: 0,
            done: false,
            result: None,
        }
    }

    pub fn handle_key_event(&mut self, key: KeyEvent) -> Option<(String, bool)> {
        if key.kind != KeyEventKind::Press {
            return None;
        }
        let n = self.options.len();
        match key {
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers,
                ..
            } if modifiers.contains(KeyModifiers::CONTROL) => {
                self.result = Some(("__CANCELLED__".to_string(), false));
                self.done = true;
            }
            _ if MOVE_LEFT_KEYS.is_pressed(key)
                || matches!(key.code, KeyCode::Up | KeyCode::Char('k')) =>
            {
                if n > 0 {
                    self.selected = (self.selected + n - 1) % n;
                }
            }
            _ if MOVE_RIGHT_KEYS.is_pressed(key)
                || matches!(key.code, KeyCode::Down | KeyCode::Char('j')) =>
            {
                if n > 0 {
                    self.selected = (self.selected + 1) % n;
                }
            }
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => {
                if n > 0 {
                    let opt = &self.options[self.selected];
                    self.result = Some((opt.id.clone(), !opt.is_installed));
                    self.done = true;
                }
            }
            KeyEvent { code, .. } => {
                let norm = match code {
                    KeyCode::Char(c) => KeyCode::Char(c.to_ascii_lowercase()),
                    other => other,
                };
                if let Some(opt) = self.options.iter().find(|o| {
                    matches!(o.key, KeyCode::Char(c) if KeyCode::Char(c.to_ascii_lowercase()) == norm)
                }) {
                    self.result = Some((opt.id.clone(), !opt.is_installed));
                    self.done = true;
                }
            }
        }
        if self.done {
            self.result.clone()
        } else {
            None
        }
    }
}

impl WidgetRef for &OssModelWidget {
    fn render_ref(&self, area: Rect, buf: &mut Buffer) {
        Clear.render(area, buf);
        let mut column = FlexRenderable::new();
        column.push(1, RenderableItem::Borrowed(&""));

        let installed_count = self.options.iter().filter(|o| o.is_installed).count();
        let title = if installed_count == 0 {
            format!("No models installed on {}", self.server_label)
        } else {
            format!("Select a model — {}", self.server_label)
        };
        column.push(
            0,
            Paragraph::new(title.bold())
                .wrap(Wrap { trim: false })
                .inset(Insets::vh(0, 2)),
        );

        let subtitle = if installed_count == 0 {
            "Choose a model to install:".to_string()
        } else if self.options.len() > installed_count {
            format!("{installed_count} installed  ·  press Enter or a digit to select, or [install] to download")
        } else {
            "↑/↓ to navigate".to_string()
        };
        column.push(
            1,
            Paragraph::new(Span::styled(subtitle, Style::default().add_modifier(Modifier::DIM)))
                .wrap(Wrap { trim: false })
                .inset(Insets::vh(0, 2)),
        );

        let labels: Vec<Line<'static>> = self.options.iter().map(|o| o.label.clone()).collect();
        column.push(0, picker_option_list(labels, self.selected));

        if let Some(opt) = self.options.get(self.selected) {
            let note = if opt.is_installed {
                format!("Model: {}", opt.id)
            } else {
                format!("Will run: ollama pull {}", opt.id)
            };
            column.push(
                1,
                Paragraph::new(Span::styled(note, Style::default().add_modifier(Modifier::DIM)))
                    .wrap(Wrap { trim: false })
                    .inset(Insets::vh(0, 2)),
            );
            if !opt.score_hint.is_empty() {
                column.push(
                    0,
                    Paragraph::new(Span::styled(
                        opt.score_hint.clone(),
                        Style::default().fg(ratatui::style::Color::Cyan),
                    ))
                    .wrap(Wrap { trim: false })
                    .inset(Insets::vh(0, 2)),
                );
            }
            if is_strata_model(&opt.id) {
                column.push(
                    0,
                    Paragraph::new(Line::from(vec![
                        Span::styled(
                            "★ 177B sharded model — use Strata engine: ",
                            Style::default().fg(ratatui::style::Color::Yellow),
                        ),
                        Span::styled(
                            "github.com/Niko1221/Strata  (port 8080, needs 32 GB+ RAM+VRAM)",
                            Style::default()
                                .fg(ratatui::style::Color::Yellow)
                                .add_modifier(Modifier::DIM),
                        ),
                    ]))
                    .wrap(Wrap { trim: false })
                    .inset(Insets::vh(0, 2)),
                );
            }
        }

        column.push(
            0,
            Paragraph::new(Line::from(vec![
                "↑/↓".bold(),
                " choose · ".dim(),
                key_hint::plain(KeyCode::Enter).into(),
                " select · ".dim(),
                key_hint::ctrl(KeyCode::Char('c')).into(),
                " exit".dim(),
            ]))
            .wrap(Wrap { trim: false })
            .inset(Insets::vh(0, 2)),
        );
        column.push(1, RenderableItem::Borrowed(&""));

        let panel = Rect {
            height: column.desired_height(area.width).min(area.height),
            ..area
        };
        render_menu_surface(panel, buf);
        column.render(panel, buf);
    }
}

/// Returns true for 177B sharded models that benefit from the Strata engine.
fn is_strata_model(id: &str) -> bool {
    id.contains("GSQ-RCO") || id.contains("Flash-Next")
}

/// Fetch the model list from a running server's `/v1/models` endpoint.
async fn fetch_models_from_url(url: &str) -> Vec<String> {
    #[allow(deprecated)]
    let client = HttpClientBuilder::new().build_direct_with_custom_ca_fallback();
    let resp = match client
        .get(url)
        .timeout(Duration::from_secs(4))
        .send()
        .await
    {
        Ok(r) => r,
        Err(_) => return vec![],
    };
    let text = match resp.text().await {
        Ok(t) => t,
        Err(_) => return vec![],
    };
    let v: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(_) => return vec![],
    };
    // OpenAI format: {"data": [{"id": "model-name"}]}
    // Ollama /v1/models: {"data": [{"id": "..."}]}
    // Ollama /api/tags:  {"models": [{"name": "..."}]}
    let mut names: Vec<String> = Vec::new();
    if let Some(arr) = v.get("data").and_then(|d| d.as_array()) {
        for item in arr {
            if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                names.push(id.to_string());
            }
        }
    } else if let Some(arr) = v.get("models").and_then(|d| d.as_array()) {
        for item in arr {
            if let Some(id) = item.get("name").and_then(|v| v.as_str()) {
                names.push(id.to_string());
            }
        }
    }
    names
}

/// Query the selected server and decide how to proceed with model selection.
pub(crate) async fn detect_oss_model(
    provider_id: &str,
    base_url: Option<&str>,
) -> OssModelDetection {
    let models_url = match base_url {
        Some(u) => format!("{u}/models"),
        None if provider_id == OLLAMA_OSS_PROVIDER_ID => {
            format!("http://localhost:{DEFAULT_OLLAMA_PORT}/v1/models")
        }
        None => {
            format!("http://localhost:{DEFAULT_LMSTUDIO_PORT}/v1/models")
        }
    };
    let installed = fetch_models_from_url(&models_url).await;

    // Kick off a background catalog refresh so the next picker open has fresh
    // HuggingFace scores.  Only when cache is absent or stale.
    if hf_catalog::load_catalog().is_none() {
        spawn_catalog_refresh();
    }

    match installed.len() {
        1 => OssModelDetection::AutoSelected(installed.into_iter().next().unwrap()),
        _ => OssModelDetection::NeedsSelection {
            can_pull: provider_id == OLLAMA_OSS_PROVIDER_ID,
            provider_id: provider_id.to_string(),
            base_url: base_url.map(str::to_string),
            installed,
        },
    }
}

/// Spawn a background task to refresh the HuggingFace model catalog and save
/// it to the disk cache.  The result is available on the next picker open.
/// Silently ignores failures — the picker falls back to PREFERRED_MODELS.
pub(crate) fn spawn_catalog_refresh() {
    tokio::spawn(async {
        let entries = hf_catalog::refresh_catalog().await;
        if !entries.is_empty() {
            hf_catalog::save_catalog(&entries);
        }
    });
}

/// Show the interactive model picker and return the chosen model.
pub(crate) async fn select_oss_model(
    server_label: &str,
    installed: Vec<String>,
    can_pull: bool,
) -> io::Result<OssModelResult> {
    let mut widget = OssModelWidget::new(server_label, &installed, can_pull);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = (|| {
        terminal.draw(|f| (&widget).render_ref(f.area(), f.buffer_mut()))?;
        crate::tui::discard_pending_terminal_input()?;
        loop {
            if let Event::Key(key_event) = event::read()?
                && let Some((model_id, needs_install)) = widget.handle_key_event(key_event)
            {
                break Ok(OssModelResult {
                    model: model_id,
                    needs_install,
                });
            }
            terminal.draw(|f| (&widget).render_ref(f.area(), f.buffer_mut()))?;
        }
    })();

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    result
}

/// Pull an Ollama model (`ollama pull <model>`) and stream its progress lines
/// to a minimal full-screen TUI, returning when the pull is complete.
pub(crate) async fn pull_oss_model_with_tui(model: &str) -> io::Result<()> {
    // Spawn `ollama pull <model>`; both stdout and stderr carry progress text.
    let mut child = tokio::process::Command::new("ollama")
        .args(["pull", model])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| io::Error::other(format!("could not run `ollama pull`: {e}")))?;

    let stdout = child.stdout.take().expect("stdout pipe");
    let stderr = child.stderr.take().expect("stderr pipe");

    enable_raw_mode()?;
    let mut out = io::stdout();
    execute!(out, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(out);
    let mut terminal = Terminal::new(backend)?;

    let mut lines: Vec<String> = vec![format!("Pulling {model} …")];
    let mut stdout_reader = tokio::io::BufReader::new(stdout).lines();
    let mut stderr_reader = tokio::io::BufReader::new(stderr).lines();

    let model_owned = model.to_string();
    let result = async {
        loop {
            // Drain any new lines from subprocess output.
            loop {
                match stdout_reader.next_line().await {
                    Ok(Some(line)) => {
                        // Ollama emits JSON progress lines; show the "status" field if present.
                        let display = if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
                            v.get("status")
                                .and_then(|s| s.as_str())
                                .map(str::to_string)
                                .unwrap_or(line)
                        } else {
                            line
                        };
                        lines.push(display);
                    }
                    _ => break,
                }
            }
            loop {
                match stderr_reader.next_line().await {
                    Ok(Some(line)) if !line.is_empty() => lines.push(line),
                    _ => break,
                }
            }

            // Redraw.
            terminal.draw(|f| render_pull_screen(f, &model_owned, &lines))?;

            // Check for Ctrl+C.
            if crossterm::event::poll(Duration::from_millis(100))? {
                if let Event::Key(k) = event::read()? {
                    if k.code == KeyCode::Char('c')
                        && k.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        let _ = child.kill().await;
                        return Err(io::Error::other("cancelled"));
                    }
                }
            }

            // Check if process finished.
            match child.try_wait() {
                Ok(Some(status)) => {
                    lines.push(if status.success() {
                        format!("✓ {model_owned} ready")
                    } else {
                        format!("✗ ollama pull failed (exit {status})")
                    });
                    terminal.draw(|f| render_pull_screen(f, &model_owned, &lines))?;
                    tokio::time::sleep(Duration::from_millis(800)).await;
                    return if status.success() {
                        Ok(())
                    } else {
                        Err(io::Error::other(format!("ollama pull failed: {status}")))
                    };
                }
                Ok(None) => {}
                Err(e) => return Err(e),
            }

            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
    .await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    result
}

fn render_pull_screen(f: &mut ratatui::Frame, model: &str, lines: &[String]) {
    let area = f.area();
    Clear.render(area, f.buffer_mut());
    let mut column = FlexRenderable::new();
    column.push(1, RenderableItem::Borrowed(&""));
    column.push(
        0,
        Paragraph::new(format!("Installing  {model}").bold())
            .wrap(Wrap { trim: false })
            .inset(Insets::vh(0, 2)),
    );
    column.push(1, RenderableItem::Borrowed(&""));

    // Show last N lines of progress output.
    let show: Vec<&str> = lines.iter().rev().take(12).rev().map(String::as_str).collect();
    for line in show {
        let owned = line.to_string();
        column.push(
            0,
            Paragraph::new(Span::styled(owned, Style::default().add_modifier(Modifier::DIM)))
                .wrap(Wrap { trim: false })
                .inset(Insets::vh(0, 2)),
        );
    }
    column.push(1, RenderableItem::Borrowed(&""));
    column.push(
        0,
        Paragraph::new(Line::from(vec![
            key_hint::ctrl(KeyCode::Char('c')).into(),
            " cancel".dim(),
        ]))
        .wrap(Wrap { trim: false })
        .inset(Insets::vh(0, 2)),
    );
    column.push(1, RenderableItem::Borrowed(&""));

    let panel = Rect {
        height: column.desired_height(area.width).min(area.height),
        ..area
    };
    render_menu_surface(panel, f.buffer_mut());
    column.render(panel, f.buffer_mut());
}

async fn check_port_status(client: &HttpClient, port: u16) -> io::Result<bool> {
    let url = format!("http://localhost:{port}");

    match client
        .get(&url)
        .timeout(Duration::from_secs(2))
        .send()
        .await
    {
        Ok(_) => Ok(true), // Any response (even 404) means the server is up.
        Err(_) => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_h_l_move_provider_selection() {
        let servers = vec![
            DiscoveredServer {
                provider_id: LMSTUDIO_OSS_PROVIDER_ID.to_string(),
                port: DEFAULT_LMSTUDIO_PORT,
                label: "LM Studio".to_string(),
                description: "LM Studio".to_string(),
                base_url: None,
            },
            DiscoveredServer {
                provider_id: OLLAMA_OSS_PROVIDER_ID.to_string(),
                port: DEFAULT_OLLAMA_PORT,
                label: "Ollama".to_string(),
                description: "Ollama".to_string(),
                base_url: None,
            },
        ];
        let mut widget = OssSelectionWidget::from_discovered(&servers);

        assert_eq!(widget.selected_option, 0);
        widget.handle_key_event(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL));
        assert_eq!(widget.selected_option, 1);
        widget.handle_key_event(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::CONTROL));
        assert_eq!(widget.selected_option, 0);
    }

    #[tokio::test]
    async fn localhost_probe_succeeds_with_invalid_inherited_ca_bundle() {
        const CHILD_ENV: &str = "CODEX_OSS_SELECTION_INVALID_CA_TEST_CHILD";

        if std::env::var_os(CHILD_ENV).is_none() {
            let temp_dir = tempfile::tempdir().expect("temporary directory should be created");
            let invalid_ca_path = temp_dir.path().join("invalid-ca.pem");
            std::fs::write(&invalid_ca_path, "not a PEM certificate")
                .expect("invalid CA fixture should be written");

            for ca_env in ["CODEX_CA_CERTIFICATE", "SSL_CERT_FILE"] {
                let output = std::process::Command::new(
                    std::env::current_exe().expect("test executable should be available"),
                )
                .arg("--exact")
                .arg("oss_selection::tests::localhost_probe_succeeds_with_invalid_inherited_ca_bundle")
                .arg("--nocapture")
                .env_remove("CODEX_CA_CERTIFICATE")
                .env_remove("SSL_CERT_FILE")
                .env(ca_env, &invalid_ca_path)
                .env(CHILD_ENV, "1")
                .output()
                .expect("isolated CA subprocess should run");

                assert!(
                    output.status.success(),
                    "localhost probe failed with invalid {ca_env}\nstdout:\n{}\nstderr:\n{}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr),
                );
            }
            return;
        }

        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .respond_with(wiremock::ResponseTemplate::new(200))
            .expect(1)
            .mount(&server)
            .await;

        #[allow(deprecated)]
        let client = HttpClientBuilder::new().build_direct_with_custom_ca_fallback();
        assert!(
            check_port_status(&client, server.address().port())
                .await
                .expect("localhost provider probe should complete")
        );
    }
}

#[cfg(test)]
#[path = "oss_selection_tests.rs"]
mod presentation_tests;
