use gtk4 as gtk;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;
use gtk::CssProvider;
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};
use std::cell::RefCell;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

use crate::ipc::*;

// Channel message type for streaming token relay (must be Send)
#[derive(Debug)]
enum StreamMsg {
    Token(String),
    Done(StateSnapshot),
    Err(String),
}

/// GUI action subcommands
#[derive(Debug, Clone)]
pub enum GuiAction {
    Show,
    Hide,
    Toggle,
}

const APP_ID: &str = "com.hbuddenberg.hyprcollab";

// ── IPC Client (sync, blocking) ──────────────────────────────────────

fn socket_path() -> std::path::PathBuf {
    crate::utils::paths::socket_path()
}

fn ipc_call(req: &Request) -> Option<Response> {
    let sock = socket_path();
    let mut stream = UnixStream::connect(&sock).ok()?;
    let json = serde_json::to_string(req).ok()? + "\n";
    stream.write_all(json.as_bytes()).ok()?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    decode_response(&line)
}

// ── Cached State ──────────────────────────────────────────────────────

struct CachedState {
    snapshot: Option<StateSnapshot>,
    working: bool,
    spinner_label: Option<gtk::Label>,
}

impl CachedState {
    fn new() -> Self {
        Self { snapshot: None, working: false, spinner_label: None }
    }

    fn refresh(&mut self) -> bool {
        if let Some(Response::State { data }) = ipc_call(&Request::GetState) {
            self.snapshot = Some(data);
            true
        } else {
            false
        }
    }

    fn get(&self) -> Option<&StateSnapshot> {
        self.snapshot.as_ref()
    }
}

// ── Spinner wheel ──────────────────────────────────────────────────────

const SPINNERS: &[&str] = &[
    "\u{f067}",
    "\u{2811}\u{2809}",
    "\u{28B9}",
    "\u{28B6}",
    "\u{28B7}",
    "\u{2813}",
    "\u{28A7}",
];

fn random_spinner() -> &'static str {
    let i = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() % SPINNERS.len() as u128) as usize;
    SPINNERS[i]
}

// ── GUI Build ─────────────────────────────────────────────────────────

pub fn handle_action(action: GuiAction) {
    match action {
        GuiAction::Show => run_gui(),
        GuiAction::Hide => {}
        GuiAction::Toggle => run_gui(),
    }
}

fn run_gui() {
    let app = gtk::Application::builder()
        .application_id(APP_ID)
        .build();

    app.connect_activate(|app| build_ui(app));
    app.run_with_args(&["--gapplication-service"]);
}

fn build_ui(app: &gtk::Application) {
    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .default_width(920)
        .default_height(620)
        .build();

    // Layer-shell setup — responsive margins based on monitor size
    unsafe {
        window.init_layer_shell();
        LayerShell::set_layer(&window, Layer::Overlay);
        LayerShell::set_keyboard_mode(&window, KeyboardMode::Exclusive);
        LayerShell::set_anchor(&window, Edge::Top, true);
        LayerShell::set_anchor(&window, Edge::Bottom, true);
        LayerShell::set_anchor(&window, Edge::Left, true);
        LayerShell::set_anchor(&window, Edge::Right, true);

        // Get monitor geometry for responsive margins
        let display = gdk::Display::default().unwrap();
        let monitors = display.monitors();
        let monitor = monitors.item(0).unwrap().downcast::<gdk::Monitor>().unwrap();
        let geo = monitor.geometry();
        let w = geo.width();
        let h = geo.height();

        // ~15% horizontal margin (min 40px), ~18% vertical margin (min 40px)
        let h_margin = (w as f32 * 0.15).max(40.0) as i32;
        let v_margin = (h as f32 * 0.10).max(40.0) as i32;

        LayerShell::set_margin(&window, Edge::Top, v_margin);
        LayerShell::set_margin(&window, Edge::Bottom, v_margin);
        LayerShell::set_margin(&window, Edge::Left, h_margin);
        LayerShell::set_margin(&window, Edge::Right, h_margin);
    }

    load_css();

    let state = Rc::new(RefCell::new(CachedState::new()));
    state.borrow_mut().refresh();

    // ── Main layout: Paned (sidebar | chat area) ──
    let main_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let paned = gtk::Paned::new(gtk::Orientation::Horizontal);
    paned.set_position(300);
    paned.set_wide_handle(false);
    paned.set_shrink_start_child(false);
    paned.set_shrink_end_child(false);

    let sidebar = build_sidebar(&state);
    paned.set_start_child(Some(&sidebar));

    let chat_area = build_chat_area(&state);
    paned.set_end_child(Some(&chat_area));

    // ── Global keybindings ──
    let event_controller = gtk::EventControllerKey::new();
    event_controller.connect_key_pressed(move |_, keyval, _, _| {
        let key_name = keyval.name().unwrap_or_default();
        if key_name == "Escape" {
            std::process::exit(0);
        }
        gtk::glib::Propagation::Proceed
    });
    window.add_controller(event_controller);

    main_box.append(&paned);
    window.set_child(Some(&main_box));
    window.show();
}

// ── Sidebar ───────────────────────────────────────────────────────────

fn build_sidebar(state: &Rc<RefCell<CachedState>>) -> gtk::Box {
    let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sidebar.set_css_classes(&["sidebar"]);

    // ── Title row: ⚙ HYPRCOLLAB + ──
    let title_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    title_row.set_css_classes(&["sidebar-title-row"]);

    let config_btn = gtk::Button::with_label("\u{f013}"); // NF fa-gear
    config_btn.set_css_classes(&["sidebar-icon-btn"]);
    config_btn.set_tooltip_text(Some("Configuration"));

    let logo = gtk::Label::new(Some("HYPRCOLLAB"));
    logo.set_css_classes(&["sidebar-logo"]);
    logo.set_hexpand(true);
    logo.set_xalign(0.0);

    let new_chat_btn = gtk::Button::with_label("\u{f067}"); // NF fa-plus
    new_chat_btn.set_css_classes(&["sidebar-icon-btn", "section-btn"]);
    new_chat_btn.set_tooltip_text(Some("New chat in folder (Ctrl+N)"));

    title_row.append(&config_btn);
    title_row.append(&logo);
    title_row.append(&new_chat_btn);
    sidebar.append(&title_row);

    // Separator
    let sep0 = gtk::Separator::new(gtk::Orientation::Horizontal);
    sep0.set_css_classes(&["sidebar-sep"]);
    sidebar.append(&sep0);

    // ── PROYECTOS section ──
    let proyectos_header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    proyectos_header.set_css_classes(&["section-header-row"]);

    let proyectos_label = gtk::Label::new(Some("PROYECTOS"));
    proyectos_label.set_css_classes(&["section-header"]);
    proyectos_label.set_hexpand(true);
    proyectos_label.set_xalign(0.0);

    let new_folder_btn = gtk::Button::with_label("\u{f65e}"); // NF fa-folder-plus
    new_folder_btn.set_css_classes(&["sidebar-icon-btn", "section-btn"]);
    new_folder_btn.set_tooltip_text(Some("New project folder (Ctrl+Shift+F)"));

    proyectos_header.append(&proyectos_label);
    proyectos_header.append(&new_folder_btn);
    sidebar.append(&proyectos_header);

    // Folders + chats (scrollable, fills remaining space)
    let folders_scrolled = gtk::ScrolledWindow::new();
    folders_scrolled.set_vexpand(true);
    folders_scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);

    let folders_list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    folders_list.set_css_classes(&["folders-list"]);

    let s = state.borrow();
    if let Some(snap) = s.get() {
        for (fi, folder) in snap.folders.iter().enumerate() {
            let is_active_folder = snap.active_folder == Some(fi);

            let folder_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            folder_row.set_css_classes(&["folder-row"]);
            if is_active_folder {
                folder_row.add_css_class("folder-active");
            }
            folder_row.set_focusable(true);

            let icon_label = gtk::Label::new(Some(&folder.icon));
            icon_label.set_css_classes(&["folder-icon"]);

            let name_label = gtk::Label::new(Some(&folder.name));
            name_label.set_css_classes(&["folder-name"]);
            name_label.set_hexpand(true);
            name_label.set_xalign(0.0);

            folder_row.append(&icon_label);
            folder_row.append(&name_label);

            if let Some(ref branch) = folder.git_branch {
                let branch_label = gtk::Label::new(Some(&format!(" {} ", branch)));
                branch_label.set_css_classes(&["git-branch-mini"]);
                folder_row.append(&branch_label);
            }

            folders_list.append(&folder_row);

            // Chat items (only for active folder)
            if is_active_folder {
                for (ci, chat_info) in folder.chats.iter().enumerate() {
                    let is_active_chat = snap.active_chat == Some(ci);

                    let chat_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
                    chat_row.set_css_classes(&["chat-row"]);
                    if is_active_chat {
                        chat_row.add_css_class("chat-active");
                    }
                    chat_row.set_focusable(true);

                    let chat_icon = gtk::Label::new(Some("\u{f075}")); // NF fa-comment
                    chat_icon.set_css_classes(&["chat-icon"]);
                    chat_icon.set_valign(gtk::Align::Start);

                    let text_col = gtk::Box::new(gtk::Orientation::Vertical, 1);
                    text_col.set_hexpand(true);

                    let chat_label = gtk::Label::new(Some(&chat_info.title));
                    chat_label.set_css_classes(&["chat-name"]);
                    chat_label.set_hexpand(true);
                    chat_label.set_xalign(0.0);

                    // Format relative time from unix timestamp
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64;
                    let secs_ago = (now - chat_info.last_active).max(0);
                    let time_str = if secs_ago < 60 {
                        "just now".into()
                    } else if secs_ago < 3600 {
                        format!("{}m ago", secs_ago / 60)
                    } else if secs_ago < 86400 {
                        format!("{}h ago", secs_ago / 3600)
                    } else {
                        format!("{}d ago", secs_ago / 86400)
                    };
                    let date_label = gtk::Label::new(Some(&time_str));
                    date_label.set_css_classes(&["chat-date"]);
                    date_label.set_hexpand(true);
                    date_label.set_xalign(0.0);

                    text_col.append(&chat_label);
                    text_col.append(&date_label);
                    chat_row.append(&chat_icon);
                    chat_row.append(&text_col);
                    folders_list.append(&chat_row);
                }
            }
        }
    }

    folders_scrolled.set_child(Some(&folders_list));
    sidebar.append(&folders_scrolled);

    // ── Separator before AGENTS ──
    let sep1 = gtk::Separator::new(gtk::Orientation::Horizontal);
    sep1.set_css_classes(&["sidebar-sep"]);
    sidebar.append(&sep1);

    // ── AGENTS section (same format as PROYECTOS, pinned at bottom) ──
    let agents_header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    agents_header.set_css_classes(&["section-header-row"]);

    let agents_label = gtk::Label::new(Some("AGENTS"));
    agents_label.set_css_classes(&["section-header"]);
    agents_label.set_hexpand(true);
    agents_label.set_xalign(0.0);

    agents_header.append(&agents_label);
    sidebar.append(&agents_header);

    let agents_scrolled = gtk::ScrolledWindow::new();
    agents_scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    agents_scrolled.set_size_request(-1, 90);
    agents_scrolled.set_vexpand(false);

    let agents_list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    agents_list.set_css_classes(&["agents-list"]);

    if let Some(snap) = s.get() {
        for agent in &snap.agents {
            let is_active = agent.name == snap.active_agent;

            let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
            row.set_css_classes(&["agent-row"]);
            if is_active {
                row.add_css_class("agent-active");
            }
            row.set_focusable(true);

            let dot = if agent.available { "\u{f00c}" } else { "\u{f00d}" };
            let dot_label = gtk::Label::new(Some(dot));
            dot_label.set_css_classes(&["agent-dot"]);

            let name_label = gtk::Label::new(Some(&agent.name));
            name_label.set_css_classes(&["agent-name"]);
            name_label.set_hexpand(true);
            name_label.set_xalign(0.0);

            row.append(&dot_label);
            row.append(&name_label);
            agents_list.append(&row);
        }
    }

    agents_scrolled.set_child(Some(&agents_list));
    sidebar.append(&agents_scrolled);

    sidebar
}

// ── Chat Area (chat panel + info drawer) ─────────────────────────────

fn build_chat_area(state: &Rc<RefCell<CachedState>>) -> gtk::Box {
    let area = gtk::Box::new(gtk::Orientation::Horizontal, 0);

    // Info drawer (right side) — built first so we can wire the toggle button
    let revealer = gtk::Revealer::new();
    revealer.set_css_classes(&["info-revealer"]);
    revealer.set_transition_type(gtk::RevealerTransitionType::SlideLeft);
    revealer.set_transition_duration(200);
    revealer.set_reveal_child(false);

    let info_panel = build_info_panel(state, &revealer);
    revealer.set_child(Some(&info_panel));

    let chat_panel = build_right_panel(state, &revealer);
    area.append(&chat_panel);
    area.append(&revealer);
    area
}

fn build_info_panel(state: &Rc<RefCell<CachedState>>, revealer: &gtk::Revealer) -> gtk::Box {
    let panel = gtk::Box::new(gtk::Orientation::Vertical, 0);
    panel.set_css_classes(&["info-panel"]);
    panel.set_size_request(280, -1);

    // ── Header ──
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    header.set_css_classes(&["info-header"]);

    let title = gtk::Label::new(Some("\u{f013} System Prompt"));
    title.set_css_classes(&["info-title"]);
    title.set_xalign(0.0);
    title.set_hexpand(true);
    header.append(&title);

    // Save button in header — same pattern as close/header buttons which work correctly
    let save_btn_ref = gtk::Button::with_label("\u{f0c7}");
    save_btn_ref.set_css_classes(&["header-btn"]);
    save_btn_ref.set_tooltip_text(Some("Save system prompt"));

    let close_btn = gtk::Button::with_label("\u{f00d}");
    close_btn.set_css_classes(&["header-btn"]);
    close_btn.set_tooltip_text(Some("Close (Ctrl+Shift+P)"));
    let rev_c = revealer.clone();
    close_btn.connect_clicked(move |_| rev_c.set_reveal_child(false));

    header.append(&save_btn_ref);
    header.append(&close_btn);
    panel.append(&header);

    let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
    sep.set_css_classes(&["panel-sep"]);
    panel.append(&sep);

    // ── Hint label ──
    let hint = gtk::Label::new(Some("Instructs the LLM for this chat.\nEmpty = default assistant."));
    hint.set_css_classes(&["info-hint"]);
    hint.set_xalign(0.0);
    hint.set_wrap(true);
    hint.set_margin_start(10);
    hint.set_margin_end(10);
    hint.set_margin_top(8);
    hint.set_margin_bottom(4);
    panel.append(&hint);

    // ── TextView ──
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scrolled.set_css_classes(&["info-scroll"]);
    scrolled.set_kinetic_scrolling(false);
    scrolled.set_overlay_scrolling(false);

    let textview = gtk::TextView::new();
    textview.set_css_classes(&["sysprompt-text"]);
    textview.set_wrap_mode(gtk::WrapMode::Word);
    textview.set_accepts_tab(false);
    textview.set_left_margin(10);
    textview.set_right_margin(10);
    textview.set_top_margin(8);
    textview.set_bottom_margin(8);

    // Pre-fill with current system prompt
    {
        let s = state.borrow();
        if let Some(snap) = s.get() {
            if let Some(ref sp) = snap.system_prompt {
                textview.buffer().set_text(sp);
            }
        }
    }

    scrolled.set_child(Some(&textview));
    panel.append(&scrolled);

    // Wire save action to header save button
    let state_c = state.clone();
    let tv_c = textview.clone();
    save_btn_ref.connect_clicked(move |_| {
        let buf = tv_c.buffer();
        let text = buf.text(&buf.start_iter(), &buf.end_iter(), false).to_string();
        if let Some(resp) = ipc_call(&Request::SetSystemPrompt { content: text }) {
            if let Response::State { data } = resp {
                state_c.borrow_mut().snapshot = Some(data);
            }
        }
    });

    panel
}

// ── Right Panel ───────────────────────────────────────────────────────

fn build_right_panel(state: &Rc<RefCell<CachedState>>, revealer: &gtk::Revealer) -> gtk::Box {
    let panel = gtk::Box::new(gtk::Orientation::Vertical, 0);
    panel.set_css_classes(&["chat-panel"]);
    panel.set_hexpand(true);

    let s = state.borrow();
    let snap = s.get();

    // ── Chat header: title + meta + system prompt btn ──
    let header = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    header.set_css_classes(&["chat-header"]);

    let title = gtk::Label::new(Some(
        snap.map(|s| s.active_chat_title.as_str())
            .unwrap_or("No chat selected"),
    ));
    title.set_css_classes(&["chat-title"]);
    title.set_xalign(0.0);
    title.set_hexpand(true);
    header.append(&title);

    let msg_count = snap.map(|s| s.active_messages.len()).unwrap_or(0);
    let tokens = snap.map(|s| s.active_chat_tokens).unwrap_or(0);
    let meta = gtk::Label::new(Some(&format!("{} msgs \u{b7} {} tok", msg_count, tokens)));
    meta.set_css_classes(&["chat-meta"]);
    header.append(&meta);

    let sysprompt_btn = gtk::Button::with_label("\u{f05a}"); // NF fa-info-circle
    sysprompt_btn.set_css_classes(&["header-btn"]);
    sysprompt_btn.set_tooltip_text(Some("System prompt (Ctrl+Shift+P)"));

    // Toggle the info drawer
    let rev_toggle = revealer.clone();
    sysprompt_btn.connect_clicked(move |_| {
        let visible = rev_toggle.reveals_child();
        rev_toggle.set_reveal_child(!visible);
    });
    header.append(&sysprompt_btn);

    panel.append(&header);

    // Separator
    let sep = gtk::Separator::new(gtk::Orientation::Horizontal);
    sep.set_css_classes(&["panel-sep"]);
    panel.append(&sep);

    // ── Messages ──
    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_vexpand(true);
    scrolled.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);

    let msg_list = gtk::Box::new(gtk::Orientation::Vertical, 4);
    msg_list.set_css_classes(&["message-list"]);

    if let Some(snap) = snap {
        for msg in &snap.active_messages {
            let bubble = build_message_bubble(msg);
            msg_list.append(&bubble);
        }
    }

    scrolled.set_child(Some(&msg_list));
    panel.append(&scrolled);

    // Drop immutable borrow before status_bar (needs mutable access)
    drop(s);

    // ── Status bar (agent · model · tokens · spinner, right-aligned) ──
    let status_bar = build_status_bar(state);
    panel.append(&status_bar);

    // ── Input bar ──
    let input_bar = build_input_bar(state, &msg_list, &scrolled);
    panel.append(&input_bar);

    // ── Help line ──
    let help_line = gtk::Label::new(Some(
        " ESC:close \u{2502} Tab:focus \u{2502} j/k:nav \u{2502} Enter:send \u{2502} Ctrl+N:new chat \u{2502} Ctrl+Shift+F:new folder \u{2502} /:commands \u{2502} ?:help",
    ));
    help_line.set_css_classes(&["help-line"]);
    panel.append(&help_line);

    panel
}

fn build_message_bubble(msg: &MessageInfo) -> gtk::Box {
    let is_user = msg.role == "user";
    let is_system = msg.role == "system";

    let outer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    outer.set_css_classes(&["msg-outer"]);

    let bubble = gtk::Box::new(gtk::Orientation::Vertical, 0);
    bubble.set_css_classes(&["msg-bubble"]);

    if is_user {
        bubble.add_css_class("msg-user");
    } else if is_system {
        bubble.add_css_class("msg-system");
    } else {
        bubble.add_css_class("msg-assistant");
    }

    let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    header.set_css_classes(&["msg-header"]);

    let role_icon = if is_user { "\u{f075}" } else if is_system { "\u{f05a}" } else { "\u{f1de}" };
    let role_text = if is_user { "You" } else if is_system { "System" } else { "Agent" };

    let role_label = gtk::Label::new(Some(&format!("{} {}", role_icon, role_text)));
    role_label.set_css_classes(&["msg-role"]);

    let time_label = gtk::Label::new(Some(&format_timestamp(msg.timestamp)));
    time_label.set_css_classes(&["msg-time"]);

    let tokens_label = gtk::Label::new(Some(&format!("{} tok", msg.tokens)));
    tokens_label.set_css_classes(&["msg-tokens"]);

    header.append(&role_label);
    header.set_hexpand(true);
    header.append(&time_label);
    header.append(&tokens_label);
    bubble.append(&header);

    let content = gtk::Label::new(Some(&msg.content));
    content.set_css_classes(&["msg-content"]);
    content.set_wrap(true);
    content.set_wrap_mode(gtk::pango::WrapMode::WordChar);
    content.set_xalign(if is_user { 1.0 } else { 0.0 });
    content.set_selectable(true);
    bubble.append(&content);

    // Alignment: user right, agent left
    if is_user {
        let spacer = gtk::Label::new(None);
        spacer.set_hexpand(true);
        outer.append(&spacer);
        outer.append(&bubble);
    } else {
        outer.append(&bubble);
        let spacer = gtk::Label::new(None);
        spacer.set_hexpand(true);
        outer.append(&spacer);
    }

    outer
}

fn build_status_bar(state: &Rc<RefCell<CachedState>>) -> gtk::Box {
    let outer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    outer.set_css_classes(&["status-bar"]);

    let sep = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    sep.set_css_classes(&["status-top-line"]);
    sep.set_size_request(-1, 1);
    outer.append(&sep);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.set_css_classes(&["status-row"]);

    let s = state.borrow();
    let snap = s.get();

    // Folder + git on the left
    if let Some(snap) = snap {
        if let Some(fi) = snap.active_folder {
            if let Some(folder) = snap.folders.get(fi) {
                let folder_label = gtk::Label::new(Some(&folder.name));
                folder_label.set_css_classes(&["status-folder"]);
                row.append(&folder_label);
            }
        }

        if let Some(ref branch) = snap.active_git_branch {
            let branch_label = gtk::Label::new(Some(&format!(" {} ", branch)));
            branch_label.set_css_classes(&["status-branch"]);
            row.append(&branch_label);
        }
    }

    // Spacer to push content right
    let spacer = gtk::Label::new(None);
    spacer.set_hexpand(true);
    row.append(&spacer);

    if let Some(snap) = snap {
        // Separator
        let sep1 = gtk::Label::new(Some("·"));
        sep1.set_css_classes(&["status-sep"]);
        row.append(&sep1);

        let agent_label = gtk::Label::new(Some(&snap.active_agent));
        agent_label.set_css_classes(&["status-agent"]);
        row.append(&agent_label);

        let sep2 = gtk::Label::new(Some("·"));
        sep2.set_css_classes(&["status-sep"]);
        row.append(&sep2);

        let model_label = gtk::Label::new(Some(&snap.active_model));
        model_label.set_css_classes(&["status-model"]);
        row.append(&model_label);

        let sep3 = gtk::Label::new(Some("·"));
        sep3.set_css_classes(&["status-sep"]);
        row.append(&sep3);

        let tokens_label = gtk::Label::new(Some(&format!("{} tok", snap.active_chat_tokens)));
        tokens_label.set_css_classes(&["status-tokens"]);
        row.append(&tokens_label);

        // Spinner label — always created, shown/hidden via state
        let is_working = s.working;
        drop(s);
        let spinner_label = gtk::Label::new(Some(random_spinner()));
        spinner_label.set_css_classes(&["status-spinner"]);
        spinner_label.set_visible(is_working);
        row.append(&spinner_label);

        // Store reference for dynamic toggle
        let mut s2 = state.borrow_mut();
        s2.spinner_label = Some(spinner_label);
    }

    outer.append(&row);
    outer
}

fn build_input_bar(
    state: &Rc<RefCell<CachedState>>,
    msg_list: &gtk::Box,
    scrolled: &gtk::ScrolledWindow,
) -> gtk::Box {
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    bar.set_css_classes(&["input-bar"]);
    bar.set_vexpand(false);

    let prompt = gtk::Label::new(Some(">"));
    prompt.set_css_classes(&["input-prompt"]);
    bar.append(&prompt);

    let entry = gtk::Entry::new();
    entry.set_css_classes(&["input-entry"]);
    entry.set_placeholder_text(Some("Type a message or /help..."));
    entry.set_hexpand(true);

    let send_btn = gtk::Button::with_label("\u{f044}");
    send_btn.set_css_classes(&["send-btn"]);

    // Clones for closure captures (GTK objects are ref-counted)
    let state_clone = state.clone();
    let msg_list_clone = msg_list.clone();
    let scrolled_clone = scrolled.clone();
    let entry_clone = entry.clone();

    entry.connect_activate(move |entry| {
        let text = entry.text().to_string();
        if text.trim().is_empty() { return; }
        entry.set_text("");
        entry.set_sensitive(false); // block re-entry while streaming

        // ── Add user bubble immediately ───────────────────────────────────
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let user_info = MessageInfo {
            role: "user".into(),
            content: text.clone(),
            timestamp: now,
            tokens: (text.len() / 4) as u32,
        };
        msg_list_clone.append(&build_message_bubble(&user_info));

        // ── Create streaming assistant bubble ─────────────────────────────
        let streaming_label = gtk::Label::new(Some("…"));
        streaming_label.set_css_classes(&["msg-content"]);
        streaming_label.set_wrap(true);
        streaming_label.set_wrap_mode(gtk::pango::WrapMode::WordChar);
        streaming_label.set_xalign(0.0);
        streaming_label.set_selectable(false);

        let streaming_header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        streaming_header.set_css_classes(&["msg-header"]);
        let role_lbl = gtk::Label::new(Some("\u{f1de} Agent"));
        role_lbl.set_css_classes(&["msg-role"]);
        streaming_header.append(&role_lbl);

        let bubble_inner = gtk::Box::new(gtk::Orientation::Vertical, 0);
        bubble_inner.set_css_classes(&["msg-bubble", "msg-assistant"]);
        bubble_inner.append(&streaming_header);
        bubble_inner.append(&streaming_label);

        let bubble_outer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        bubble_outer.set_css_classes(&["msg-outer"]);
        bubble_outer.append(&bubble_inner);
        let spacer = gtk::Label::new(None);
        spacer.set_hexpand(true);
        bubble_outer.append(&spacer);

        msg_list_clone.append(&bubble_outer);

        // ── Show spinner ──────────────────────────────────────────────────
        {
            let mut s = state_clone.borrow_mut();
            s.working = true;
            if let Some(ref lbl) = s.spinner_label {
                lbl.set_visible(true);
            }
        }

        // ── Scroll to bottom ──────────────────────────────────────────────
        scroll_bottom(&scrolled_clone);

        // ── mpsc channel: thread → GTK main thread via polling ───────────
        let (std_tx, std_rx) = std::sync::mpsc::channel::<StreamMsg>();

        // ── Spawn std thread: opens IPC socket, reads streaming responses ─
        let text_for_thread = text.clone();
        std::thread::spawn(move || {
            let sock = crate::utils::paths::socket_path();
            let mut stream = match std::os::unix::net::UnixStream::connect(&sock) {
                Ok(s) => s,
                Err(e) => {
                    let _ = std_tx.send(StreamMsg::Err(e.to_string()));
                    return;
                }
            };

            let req = match serde_json::to_string(&Request::SendMessage { content: text_for_thread }) {
                Ok(j) => j + "\n",
                Err(e) => {
                    let _ = std_tx.send(StreamMsg::Err(e.to_string()));
                    return;
                }
            };

            if stream.write_all(req.as_bytes()).is_err() {
                let _ = std_tx.send(StreamMsg::Err("socket write error".into()));
                return;
            }

            let reader = BufReader::new(stream);
            for line in reader.lines() {
                let line = match line {
                    Ok(l) => l,
                    Err(e) => {
                        let _ = std_tx.send(StreamMsg::Err(e.to_string()));
                        return;
                    }
                };
                match decode_response(&line) {
                    Some(Response::Token { text }) => {
                        if std_tx.send(StreamMsg::Token(text)).is_err() {
                            return;
                        }
                    }
                    Some(Response::State { data }) => {
                        let _ = std_tx.send(StreamMsg::Done(data));
                        return;
                    }
                    Some(Response::Error { msg }) => {
                        let _ = std_tx.send(StreamMsg::Err(msg));
                        return;
                    }
                    _ => {}
                }
            }
        });

        // ── Poll receiver on GTK main thread via 20ms timeout ────────────
        let label_ref = streaming_label.clone();
        let state_ref = state_clone.clone();
        let entry_ref = entry_clone.clone();
        let scrolled_ref = scrolled_clone.clone();
        let mut accumulated = String::new();

        glib::timeout_add_local(std::time::Duration::from_millis(20), move || {
            use std::sync::mpsc::TryRecvError;
            loop {
                match std_rx.try_recv() {
                    Ok(StreamMsg::Token(text)) => {
                        accumulated.push_str(&text);
                        label_ref.set_text(&accumulated);
                        let adj = scrolled_ref.vadjustment();
                        adj.set_value(adj.upper() - adj.page_size());
                    }
                    Ok(StreamMsg::Done(snapshot)) => {
                        label_ref.set_selectable(true);
                        let mut s = state_ref.borrow_mut();
                        s.snapshot = Some(snapshot);
                        s.working = false;
                        if let Some(ref lbl) = s.spinner_label {
                            lbl.set_visible(false);
                        }
                        drop(s);
                        entry_ref.set_sensitive(true);
                        entry_ref.grab_focus();
                        return glib::ControlFlow::Break;
                    }
                    Ok(StreamMsg::Err(e)) => {
                        label_ref.set_text(&format!("Error: {}", e));
                        label_ref.set_selectable(true);
                        let mut s = state_ref.borrow_mut();
                        s.working = false;
                        if let Some(ref lbl) = s.spinner_label {
                            lbl.set_visible(false);
                        }
                        drop(s);
                        entry_ref.set_sensitive(true);
                        entry_ref.grab_focus();
                        return glib::ControlFlow::Break;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        return glib::ControlFlow::Break;
                    }
                }
            }
            glib::ControlFlow::Continue
        });
    });

    // Wire send button to activate entry
    let entry_for_btn = entry.clone();
    send_btn.connect_clicked(move |_| { entry_for_btn.activate(); });

    bar.append(&entry);
    bar.append(&send_btn);

    bar
}

fn scroll_bottom(scrolled: &gtk::ScrolledWindow) {
    let scrolled = scrolled.clone();
    glib::idle_add_local_once(move || {
        let adj = scrolled.vadjustment();
        adj.set_value(adj.upper() - adj.page_size());
    });
}

// ── CSS ────────────────────────────────────────────────────────────────

fn load_css() {
    let provider = CssProvider::new();
    provider.load_from_data(CSS);
    gtk::style_context_add_provider_for_display(
        &gtk::gdk::Display::default().expect("No display"),
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_USER,
    );
}

const CSS: &str = r#"
@define-color bg_primary    #0a0a0a;
@define-color bg_sidebar    #0c0c0c;
@define-color bg_panel      #0a0a0a;
@define-color bg_input      #141414;
@define-color bg_msg_user   #111118;
@define-color bg_msg_agent  #0e0e0e;
@define-color bg_hover      #1a1a1a;
@define-color bg_active     #1a1a1a;

@define-color text_primary  #d4d4d4;
@define-color text_secondary #808080;
@define-color text_tertiary  #5a5a5a;
@define-color text_muted     #5a5a5a;
@define-color text_accent    #569cd6;

@define-color border_color  #262626;
@define-color border_light  #333333;
@define-color border_focus  #569cd6;

@define-color accent_blue   #569cd6;
@define-color accent_teal   #4ec9b0;
@define-color accent_green  #6a9955;
@define-color accent_orange #dcdcaa;
@define-color accent_purple #c586c0;
@define-color accent_red    #f44747;

* {
    font-family: "JetBrainsMono Nerd Font Propo", "JetBrainsMono Nerd Font", "JetBrains Mono", monospace;
    font-size: 13px;
    color: @text_primary;
}

window {
    background-color: @bg_primary;
    border: 1px solid @border_color;
    border-radius: 8px;
}

/* Paned separator */
paned > separator {
    background: @border_color;
    min-width: 1px;
}

.sidebar {
    background-color: @bg_sidebar;
    min-width: 280px;
    padding: 0;
}

.sidebar-title-row {
    padding: 10px 12px;
    border-bottom: 1px solid @border_color;
}

.sidebar-logo {
    font-size: 14px;
    font-weight: bold;
    color: @accent_teal;
    letter-spacing: 2px;
}

.sidebar-icon-btn {
    background: transparent;
    border: 1px solid transparent;
    border-radius: 4px;
    color: @text_secondary;
    padding: 2px 8px;
    font-size: 12px;
    min-width: 28px;
}
.sidebar-icon-btn:hover {
    background: @bg_hover;
    color: @text_primary;
    border-color: @border_color;
}
.sidebar-icon-btn:focus {
    border-color: @border_focus;
    color: @accent_teal;
}

.sidebar-sep {
    background: @border_color;
    min-height: 1px;
    margin: 0;
    padding: 0;
}

.section-header-row {
    padding: 6px 12px 2px;
}

.section-header {
    font-size: 10px;
    font-weight: bold;
    color: @text_tertiary;
    letter-spacing: 1px;
}

.section-btn {
    font-size: 11px;
    padding: 0 2px;
    min-width: 18px;
    background: transparent;
    border: none;
    color: @text_tertiary;
    box-shadow: none;
}
.section-btn:hover {
    color: @text_primary;
    background: transparent;
}
.section-btn:focus {
    color: @accent_teal;
    background: transparent;
}

.folders-list {
    padding: 4px 0;
}

.folder-row {
    padding: 6px 12px;
    border-radius: 3px;
}
.folder-row:hover {
    background: @bg_hover;
}
.folder-row:focus {
    background: @bg_active;
    outline: none;
}
.folder-active {
    background: @bg_active;
    box-shadow: inset 3px 0 0 @accent_teal;
}

.folder-icon {
    font-size: 13px;
    color: @text_muted;
}

.folder-name {
    font-size: 13px;
    font-weight: bold;
    color: @text_primary;
}

.git-branch-mini {
    font-size: 10px;
    color: @accent_green;
    background: alpha(@accent_green, 0.1);
    border: 1px solid alpha(@accent_green, 0.3);
    border-radius: 3px;
    padding: 0 4px;
}

.chat-row {
    padding: 4px 12px 4px 28px;
    border-radius: 3px;
    margin-left: 8px;
}
.chat-row:hover {
    background: @bg_hover;
}
.chat-row:focus {
    background: @bg_active;
    outline: none;
}
.chat-active {
    background: @bg_active;
    box-shadow: inset 4px 0 0 @accent_blue;
}
.chat-active .chat-name {
    color: @accent_blue;
}

.chat-icon {
    font-size: 9px;
    color: @text_muted;
    margin-right: 4px;
}
.chat-active .chat-icon {
    color: @accent_blue;
}
.chat-active .chat-date {
    color: alpha(@accent_blue, 0.5);
}

.chat-name {
    font-size: 12px;
    color: @text_secondary;
}

.chat-date {
    font-size: 10px;
    color: alpha(@text_secondary, 0.5);
}

.agents-list {
    padding: 2px 0;
}

.agent-row {
    padding: 4px 12px;
    border-radius: 3px;
}
.agent-row:hover {
    background: @bg_hover;
}
.agent-row:focus {
    background: @bg_active;
    outline: none;
}
.agent-active {
    background: @bg_active;
}
.agent-active .agent-name {
    color: @accent_blue;
    font-weight: bold;
}

.agent-dot {
    font-size: 9px;
    color: @accent_green;
}

.agent-name {
    font-size: 12px;
    color: @text_secondary;
}

.chat-panel {
    background-color: @bg_panel;
}

/* ── Info / System-Prompt drawer ── */
.info-panel {
    background-color: @bg_sidebar;
    border-left: 1px solid @border_color;
}

.info-header {
    padding: 10px 14px;
    border-bottom: 1px solid @border_color;
}

.info-title {
    font-size: 13px;
    font-weight: bold;
    color: @accent_teal;
}

.info-hint {
    font-size: 11px;
    color: @text_tertiary;
}

.info-scroll {
    background-color: @bg_input;
    margin: 0 10px 4px 10px;
    border-radius: 4px;
    border: 1px solid @border_color;
}

/* Viewport inside ScrolledWindow must also be dark */
.info-scroll > viewport {
    background-color: @bg_input;
}

.info-scroll > viewport > textview {
    background-color: @bg_input;
}

.sysprompt-text {
    background-color: @bg_input;
    color: @text_primary;
    font-size: 12px;
}

.sysprompt-text text {
    background-color: @bg_input;
    color: @text_primary;
}

/* Revealer doesn't inherit parent background — explicit hex value */
.info-revealer {
    background-color: #0c0c0c;
}

.chat-header {
    padding: 10px 14px;
    border-bottom: 1px solid @border_color;
}

.chat-title {
    font-size: 14px;
    font-weight: bold;
    color: @text_primary;
}

.chat-meta {
    font-size: 11px;
    color: @text_tertiary;
}

.header-btn {
    background: transparent;
    border: 1px solid transparent;
    border-radius: 4px;
    color: @text_secondary;
    padding: 2px 8px;
    font-size: 12px;
    min-width: 28px;
}
.header-btn:hover {
    background: @bg_hover;
    color: @text_primary;
    border-color: @border_color;
}
.header-btn:focus {
    border-color: @border_focus;
    color: @accent_teal;
}

.panel-sep {
    background: @border_color;
    min-height: 1px;
}

.message-list {
    padding: 8px 14px;
}

.msg-outer {
    padding: 4px 0;
}

.msg-bubble {
    padding: 6px 10px;
    border-radius: 4px;
}

.msg-user {
    background: @bg_msg_user;
    border-left: 2px solid @accent_teal;
}
.msg-assistant {
    background: @bg_msg_agent;
    border-right: 2px solid @accent_blue;
}
.msg-system {
    background: alpha(@accent_orange, 0.05);
    border-left: 2px solid @accent_orange;
}

.msg-user .msg-role { color: @accent_teal; }
.msg-assistant .msg-role { color: @accent_blue; }
.msg-system .msg-role { color: @accent_orange; }

.msg-header {
    margin-bottom: 2px;
}

.msg-role {
    font-size: 11px;
    font-weight: bold;
}

.msg-time {
    font-size: 10px;
    color: @text_tertiary;
}
.msg-tokens {
    font-size: 10px;
    color: @text_tertiary;
}

.msg-content {
    font-size: 13px;
    color: @text_primary;
    line-height: 1.5;
}

.status-bar {
    background-color: @bg_sidebar;
}
.status-top-line {
    background: @border_color;
    min-height: 1px;
}
.status-row {
    padding: 4px 12px;
}
.status-model {
    font-size: 10px;
    color: @accent_purple;
}
.status-folder {
    font-size: 10px;
    color: @text_secondary;
}
.status-branch {
    font-size: 10px;
    color: @accent_green;
}
.status-sep {
    font-size: 10px;
    color: @text_tertiary;
}
.status-agent {
    font-size: 10px;
    color: @accent_teal;
}
.status-tokens {
    font-size: 10px;
    color: @text_tertiary;
}
.status-spinner {
    font-size: 12px;
    color: @accent_teal;
    animation: spin 1s linear infinite;
}

.input-bar {
    background: @bg_input;
    border-top: 1px solid @border_color;
    padding: 8px 12px;
}

.input-prompt {
    color: @accent_teal;
    font-weight: bold;
    font-size: 14px;
    margin-right: 4px;
}

.input-entry {
    background: @bg_primary;
    border: 1px solid @border_color;
    border-radius: 4px;
    padding: 6px 10px;
    color: @text_primary;
    font-size: 13px;
}
.input-entry:focus {
    border-color: @border_focus;
    box-shadow: 0 0 4px alpha(@accent_blue, 0.3);
}
.input-entry placeholder {
    color: @text_tertiary;
}

.send-btn {
    background: transparent;
    border: 1px solid @border_color;
    border-radius: 4px;
    color: @accent_teal;
    padding: 4px 12px;
    min-width: 36px;
}
.send-btn:hover {
    background: alpha(@accent_teal, 0.1);
    border-color: @accent_teal;
}

.help-line {
    font-size: 10px;
    color: @text_tertiary;
    padding: 4px 12px;
    background: @bg_sidebar;
    border-top: 1px solid alpha(@border_color, 0.5);
}
"#;

// ── Helpers ────────────────────────────────────────────────────────────

fn format_timestamp(ts: i64) -> String {
    let secs = ts as u64;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let diff = now.saturating_sub(secs);
    if diff < 60 {
        "now".into()
    } else if diff < 3600 {
        format!("{}m", diff / 60)
    } else if diff < 86400 {
        format!("{}h", diff / 3600)
    } else {
        format!("{}d", diff / 86400)
    }
}
