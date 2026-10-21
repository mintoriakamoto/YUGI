//! Presentation and keyboard navigation checks for the local-provider startup picker.

use super::*;
use crate::test_backend::VT100Backend;
use ratatui::widgets::FrameExt;

fn make_two_servers() -> Vec<DiscoveredServer> {
    vec![
        DiscoveredServer {
            provider_id: LMSTUDIO_OSS_PROVIDER_ID.to_string(),
            port: DEFAULT_LMSTUDIO_PORT,
            label: "LM Studio".to_string(),
            description: "LM Studio (port 1234) — OpenAI-compatible".to_string(),
            base_url: None,
        },
        DiscoveredServer {
            provider_id: OLLAMA_OSS_PROVIDER_ID.to_string(),
            port: DEFAULT_OLLAMA_PORT,
            label: "Ollama".to_string(),
            description: "Ollama (port 11434) — default".to_string(),
            base_url: None,
        },
    ]
}

#[test]
fn provider_picker_snapshots() {
    for (name, width, height, selected) in [
        ("oss_providers_two_servers", 80, 24, 0),
        ("oss_providers_two_servers_narrow", 40, 16, 1),
    ] {
        let servers = make_two_servers();
        let mut widget = OssSelectionWidget::from_discovered(&servers);
        widget.selected_option = selected;
        let mut terminal = Terminal::new(VT100Backend::new(width, height)).expect("terminal");
        terminal
            .draw(|frame| frame.render_widget_ref(&widget, frame.area()))
            .expect("draw");
        insta::assert_snapshot!(name, terminal.backend());
    }
}

#[test]
fn navigation_wraps_without_confirming() {
    for event in [
        KeyEvent::new(KeyCode::Up, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
    ] {
        let servers = make_two_servers();
        let mut widget = OssSelectionWidget::from_discovered(&servers);
        assert_eq!(widget.handle_key_event(event), None);
        assert_eq!((widget.selected_option, widget.is_complete()), (1, false));
        assert_eq!(widget.handle_key_event(event), None);
        assert_eq!((widget.selected_option, widget.is_complete()), (0, false));
    }
}

#[test]
fn number_shortcuts_select_provider() {
    for (key, idx) in [('1', 0usize), ('2', 1usize)] {
        let servers = make_two_servers();
        let mut widget = OssSelectionWidget::from_discovered(&servers);
        let result =
            widget.handle_key_event(KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE));
        assert!(result.is_some(), "key {key} should produce a selection");
        let (provider_id, base_url) = result.unwrap();
        assert_eq!(provider_id, servers[idx].provider_id);
        assert_eq!(base_url, servers[idx].base_url);
        assert!(widget.is_complete());
    }
}
