//! Local provider discovery and shared startup picker presentation.
//! Discovery and selection outcomes stay independent of the picker layout.

use std::io;

use futures::future::join_all;

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
        label: "LlamaFile / LocalAI (8080)",
        description: "LlamaFile or LocalAI on port 8080",
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
                    description: "Ollama (port 11434) — not detected, but you can start it".to_string(),
                    base_url: None,
                });
                discovered.push(DiscoveredServer {
                    provider_id: LMSTUDIO_OSS_PROVIDER_ID.to_string(),
                    port: DEFAULT_LMSTUDIO_PORT,
                    label: "LM Studio".to_string(),
                    description: "LM Studio (port 1234) — not detected, but you can start it".to_string(),
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
