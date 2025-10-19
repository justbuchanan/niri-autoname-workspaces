use niri_ipc::socket::Socket;
use niri_ipc::{Action, Event, Request, Response, Window, Workspace, WorkspaceReferenceArg};
use serde::Deserialize;
use signal_hook::consts::signal::*;
use signal_hook::iterator::Signals;
use std::collections::HashMap;
use std::fs;
use std::process::Command;
use std::thread;

// Config contains icon mappings for programs and is loaded from a toml file.
#[derive(Deserialize, Debug)]
struct Config {
    #[serde(default)]
    matches: Option<HashMap<String, String>>,
    #[serde(default)]
    default: Option<String>,
    #[serde(default)]
    focused_format: Option<String>,
}

const DEFAULT_FOCUSED_FORMAT: &str = "{}";

impl Config {
    fn merge(mut self, other: Config) -> Self {
        if let Some(other_matches) = other.matches {
            self.matches
                .get_or_insert_with(HashMap::new)
                .extend(other_matches);
        }
        self.default = other.default.or(self.default);
        self.focused_format = other.focused_format.or(self.focused_format);
        self
    }

    fn lowercase_keys(mut self) -> Self {
        self.matches = self
            .matches
            .map(|m| m.into_iter().map(|(k, v)| (k.to_lowercase(), v)).collect());
        self
    }
}

const DEFAULT_CONFIG: &str = include_str!("../default_config.toml");

const CONFIG_FILE_PATH: &str = "~/.config/niri/autoname-workspaces.toml";

fn get_workspace_custom_name(workspace: &Workspace) -> Option<String> {
    workspace.name.as_ref().and_then(|n| {
        let name = n
            .find(": ")
            .map(|pos| n[..pos].to_string())
            .unwrap_or_else(|| n.clone());
        // Only consider it a custom name if it contains at least one letter
        if name.chars().any(|c| c.is_alphabetic()) {
            Some(name)
        } else {
            None
        }
    })
}

fn icon_for_window(cfg: &Config, window: &Window) -> String {
    let Some(app_id) = &window.app_id else {
        log::warn!("Window doesn't have an app_id: {:?}", window);
        return cfg.default.clone().unwrap_or_default();
    };

    let app_id_lower = app_id.to_lowercase();

    cfg.matches
        .as_ref()
        .and_then(|m| m.get(&app_id_lower))
        .cloned()
        .or_else(|| {
            log::warn!("No icon configured for app_id='{}'", app_id);
            cfg.default.clone()
        })
        .unwrap_or_default()
}

fn rename_workspaces(cfg: &Config, socket: &mut Socket) -> Result<(), Box<dyn std::error::Error>> {
    let Response::Workspaces(workspaces) = socket.send(Request::Workspaces)?? else {
        return Err("Expected Workspaces response".into());
    };

    // Store workspace info: (custom_name, icons, idx)
    let mut ws_info: HashMap<_, _> = workspaces
        .iter()
        .map(|ws| {
            let custom_name = get_workspace_custom_name(ws);
            (ws.id, (custom_name, String::new(), ws.idx))
        })
        .collect();

    let Response::Windows(mut windows) = socket.send(Request::Windows)?? else {
        return Err("Expected Windows response".into());
    };

    // Sort windows by their position in the scrolling layout
    windows.sort_by_key(|w| w.layout.pos_in_scrolling_layout);

    // Collect icons
    for w in windows
        .iter()
        .filter_map(|w| w.workspace_id.map(|id| (id, w)))
    {
        let mut icon = icon_for_window(cfg, w.1);
        // Apply focused format if this window is focused
        if w.1.is_focused {
            let format = cfg
                .focused_format
                .as_ref()
                .map(String::as_str)
                .unwrap_or(DEFAULT_FOCUSED_FORMAT);
            icon = format.replace("{}", &icon);
        }
        if let Some((_, icons, _)) = ws_info.get_mut(&w.0) {
            icons.push(' ');
            icons.push_str(&icon);
        }
    }

    // Set workspace names
    for (ws_id, (custom_name, icons, idx)) in &ws_info {
        let icons = icons.trim();
        let reference = Some(WorkspaceReferenceArg::Id(*ws_id));

        let action = if icons.is_empty() && custom_name.is_none() {
            Action::UnsetWorkspaceName { reference }
        } else if icons.is_empty() {
            Action::SetWorkspaceName {
                name: custom_name.clone().unwrap(),
                workspace: reference,
            }
        } else {
            let default_name = idx.to_string();
            let name_prefix = custom_name.as_ref().unwrap_or(&default_name);
            Action::SetWorkspaceName {
                name: format!("{}: {}", name_prefix, icons),
                workspace: reference,
            }
        };
        socket.send(Request::Action(action))??;
    }

    Ok(())
}

fn undo_rename_workspaces(socket: &mut Socket) -> Result<(), Box<dyn std::error::Error>> {
    let Response::Workspaces(workspaces) = socket.send(Request::Workspaces)?? else {
        return Err("Expected Workspaces response".into());
    };

    for ws in workspaces {
        let custom_name = get_workspace_custom_name(&ws);
        let reference = Some(WorkspaceReferenceArg::Id(ws.id));

        let action = if let Some(name) = custom_name {
            Action::SetWorkspaceName {
                name,
                workspace: reference,
            }
        } else {
            Action::UnsetWorkspaceName { reference }
        };
        socket.send(Request::Action(action))??;
    }

    Ok(())
}

fn rename_current_workspace(
    socket: &mut Socket,
    cfg: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let Response::Workspaces(workspaces) = socket.send(Request::Workspaces)?? else {
        return Err("Expected Workspaces response".into());
    };

    let current_ws = workspaces
        .iter()
        .find(|ws| ws.is_focused)
        .ok_or("No focused workspace found")?;

    let Response::Windows(windows) = socket.send(Request::Windows)?? else {
        return Err("Expected Windows response".into());
    };

    // Build icons string for current workspace
    let icons: String = windows
        .iter()
        .filter(|w| w.workspace_id == Some(current_ws.id))
        .map(|w| icon_for_window(cfg, w))
        .collect::<Vec<_>>()
        .join(" ");

    // Launch zenity to get user input
    let output = Command::new("zenity")
        .args(&[
            "--entry",
            "--title=Rename Workspace",
            "--text=Enter new workspace name:",
        ])
        .output()?;

    if !output.status.success() {
        return Err("User cancelled or zenity failed".into());
    }

    let name = String::from_utf8(output.stdout)?.trim().to_string();

    // Set workspace name
    let full_name = if icons.is_empty() {
        name
    } else {
        format!("{}: {}", name, icons)
    };

    socket.send(Request::Action(Action::SetWorkspaceName {
        name: full_name,
        workspace: Some(WorkspaceReferenceArg::Id(current_ws.id)),
    }))??;

    Ok(())
}

fn load_config() -> Result<Config, Box<dyn std::error::Error>> {
    let mut config: Config = toml::from_str(DEFAULT_CONFIG)?;
    let expanded_path = shellexpand::tilde(CONFIG_FILE_PATH);

    match fs::read_to_string(expanded_path.as_ref()) {
        Ok(contents) => {
            let user_config = toml::from_str::<Config>(&contents).map_err(|e| {
                format!("Failed to parse user config at {}: {}", CONFIG_FILE_PATH, e)
            })?;
            config = config.merge(user_config);
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            log::warn!("Warning: User config not found at {}", CONFIG_FILE_PATH);
        }
        Err(e) => return Err(e.into()),
    }
    Ok(config.lowercase_keys())
}

fn print_help() {
    print!(
        "\
niri-autoname-workspaces

Automatically names niri workspaces to show icons for open windows.

USAGE:
  niri-autoname-workspaces
  niri-autoname-workspaces [COMMAND]

COMMANDS:
  rename    Interactively rename the current workspace
  --help    Print this help message
  -h        Print this help message

CONFIG:
  Config file: {}
  Configure app_id to icon mappings in TOML format
",
        CONFIG_FILE_PATH
    );
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    // Check for help flag
    if std::env::args().nth(1).as_deref() == Some("--help")
        || std::env::args().nth(1).as_deref() == Some("-h")
    {
        print_help();
        return Ok(());
    }

    let config = load_config()?;

    let mut cmd_socket = Socket::connect()?;

    // Check for "rename" argument
    if std::env::args().nth(1).as_deref() == Some("rename") {
        return rename_current_workspace(&mut cmd_socket, &config);
    }

    let mut subscribe_socket = Socket::connect()?;

    // Set up signal handler to cleanup on exit
    let mut signals = Signals::new(&[SIGINT, SIGTERM, SIGHUP, SIGQUIT])?;
    thread::spawn(move || {
        for sig in signals.forever() {
            println!("Received signal {:?}, cleaning up...", sig);
            if let Ok(mut socket) = Socket::connect() {
                let _ = undo_rename_workspaces(&mut socket);
            }
            std::process::exit(0);
        }
    });

    rename_workspaces(&config, &mut cmd_socket)?;

    let Ok(Response::Handled) = subscribe_socket.send(Request::EventStream)? else {
        return Err("Expected Handled response".into());
    };

    let mut read_event = subscribe_socket.read_events();
    while let Ok(event) = read_event() {
        if matches!(
            event,
            Event::WindowOpenedOrChanged { .. }
                | Event::WindowClosed { .. }
                | Event::WindowLayoutsChanged { .. }
                | Event::WindowFocusChanged { .. }
        ) {
            rename_workspaces(&config, &mut cmd_socket)?;
        }
    }

    // Cleanup on normal exit

    undo_rename_workspaces(&mut Socket::connect()?)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_icons_toml() {
        let config: Config =
            toml::from_str(DEFAULT_CONFIG).expect("Failed to parse default_config.toml");

        // Verify some known entries exist
        let matches = config.matches.as_ref().expect("matches should be present");
        assert!(matches.contains_key("firefox"));
        assert!(matches.contains_key("chromium"));
        assert!(matches.contains_key("alacritty"));

        // Verify default match is set
        assert_eq!(config.default, Some("*".to_string()));
    }
}
