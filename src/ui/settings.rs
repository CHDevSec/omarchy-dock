//! Dock settings, reached by right-clicking the launcher button.
//!
//! Two surfaces. The right-click itself opens a short menu — the Omarchy
//! surfaces the dock can raise, plus the handful of actions that are one
//! click and done. Everything with a value attached lives in a proper window
//! behind "Dock settings…", because a popover that has to scroll is a popover
//! that has outgrown being one.
//!
//! Every control writes `config.toml` and lets the file watcher apply the
//! change, so the panel, a hand-edited config, and `omarchy-dockctl` all take
//! exactly one path into the running dock. That costs a file round-trip per
//! change and removes any chance of the panel and the file disagreeing.

use gtk4 as gtk;

use gtk::glib;
use gtk::prelude::*;

use std::cell::RefCell;

use crate::config::{Config, HideMode, Hover, Position, Style};
use crate::ui::menu::MenuAction;

/// The launcher's right-click menu: shell surfaces and one-click actions.
///
/// Deliberately short. Anything with a value to choose lives in the settings
/// window instead — a menu you have to scroll is not a menu.
pub fn build<F>(on_action: F) -> gtk::Popover
where
    F: Fn(MenuAction) + Clone + 'static,
{
    let popover = gtk::Popover::new();
    popover.add_css_class("dock-menu");
    popover.set_autohide(true);

    let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
    list.add_css_class("dock-menu-list");

    // ── Omarchy surfaces ────────────────────────────────────────────────────
    // The launcher already opens the Omarchy menu on a left click; a right
    // click is the natural place to reach the shell's other surfaces directly.
    // These raise the real thing over IPC rather than the dock drawing a
    // lookalike, so they stay correct as Omarchy changes.
    list.append(&heading("Omarchy"));
    for surface in crate::omarchy::Surface::all() {
        let act = on_action.clone();
        let pop = popover.clone();
        list.append(&row(surface.label(), move || {
            act(MenuAction::OpenSurface(surface));
            pop.popdown();
        }));
    }

    // Do Not Disturb belongs to the shell's notification service; the dock
    // only offers the switch. Omitted when the shell cannot be reached, rather
    // than showing a toggle that silently does nothing.
    if let Some(on) = crate::omarchy::dnd_enabled() {
        let pop = popover.clone();
        let dnd = row("Do Not Disturb", move || {
            crate::omarchy::toggle_dnd();
            pop.popdown();
        });
        dnd.set_child(Some(&state_row("Do Not Disturb", if on { "On" } else { "Off" })));
        if on {
            dnd.add_css_class("dock-menu-checked");
        }
        list.append(&dnd);
    }

    list.append(&separator());
    list.append(&heading("Dock"));

    {
        let pop = popover.clone();
        list.append(&row("Settings…", move || {
            pop.popdown();
            open_window();
        }));
    }

    {
        let pop = popover.clone();
        list.append(&row("Add separator", move || {
            add_separator();
            pop.popdown();
        }));
    }

    {
        let pop = popover.clone();
        list.append(&row("Edit config file…", move || {
            let path = crate::config::config_path();
            // Hand off to the desktop's handler rather than guessing an editor.
            let _ = std::process::Command::new("xdg-open").arg(path).spawn();
            pop.popdown();
        }));
    }

    popover.set_child(Some(&list));
    popover
}

/// Open the settings window, or raise the one already open.
///
/// A single instance: opening a second copy of a panel that edits one file on
/// disk would let the two disagree about what that file currently says.
pub fn open_window() {
    thread_local! {
        static OPEN: RefCell<Option<gtk::Window>> = const { RefCell::new(None) };
    }

    OPEN.with(|slot| {
        if let Some(win) = slot.borrow().as_ref() {
            win.present();
            return;
        }

        let win = build_window();
        {
            let slot_clear = || OPEN.with(|s| *s.borrow_mut() = None);
            win.connect_close_request(move |_| {
                slot_clear();
                glib::Propagation::Proceed
            });
        }
        win.present();
        *slot.borrow_mut() = Some(win);
    });
}

fn build_window() -> gtk::Window {
    let cfg = Config::load();

    let win = gtk::Window::builder()
        .title("Dock Settings")
        .default_width(680)
        .default_height(560)
        .resizable(true)
        .build();
    win.add_css_class("dock-settings-window");

    // Esc closes, like every other Omarchy panel.
    {
        let keys = gtk::EventControllerKey::new();
        let w = win.clone();
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gtk::gdk::Key::Escape {
                w.close();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        win.add_controller(keys);
    }

    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::None);
    stack.set_hexpand(true);
    stack.set_vexpand(true);

    // ── placement ───────────────────────────────────────────────────────────
    {
        let (scroll, page) = page("Placement", "Where the dock sits and when it gets out of the way.");

        let g = group(&page, "Position");
        let current = match cfg.dock.position {
            Position::Bottom => 0,
            Position::Top => 1,
            Position::Left => 2,
            Position::Right => 3,
        };
        g.append(&field(
            "Screen edge",
            None,
            &segmented(&["Bottom", "Top", "Left", "Right"], current, |i| {
                edit(move |c| {
                    c.dock.position = match i {
                        1 => Position::Top,
                        2 => Position::Left,
                        3 => Position::Right,
                        _ => Position::Bottom,
                    }
                })
            }),
        ));
        g.append(&toggle(
            "Reserve screen space",
            Some("Windows stop above the dock instead of going under it."),
            cfg.dock.reserve_space,
            |v| edit(move |c| c.dock.reserve_space = v),
        ));

        let g = group(&page, "Auto-hide");
        let current = match cfg.autohide.mode {
            HideMode::Never => 0,
            HideMode::Intelligent => 1,
            HideMode::Always => 2,
        };
        g.append(&field(
            "Hide",
            Some("Intelligent hides only when a window overlaps the dock."),
            &segmented(&["Never", "Intelligent", "Always"], current, |i| {
                edit(move |c| {
                    c.autohide.mode = match i {
                        0 => HideMode::Never,
                        2 => HideMode::Always,
                        _ => HideMode::Intelligent,
                    }
                })
            }),
        ));
        g.append(&toggle(
            "Reveal for attention",
            Some("Show the dock when an app asks for attention."),
            cfg.autohide.reveal_on_attention,
            |v| edit(move |c| c.autohide.reveal_on_attention = v),
        ));

        stack.add_named(&scroll, Some("placement"));
    }

    // ── appearance ──────────────────────────────────────────────────────────
    {
        let (scroll, page) = page("Appearance", "How the dock is drawn. Colours always follow the Omarchy theme.");

        let g = group(&page, "Style");
        g.append(&field(
            "Surface",
            Some("Omarchy uses the shell's own tokens; Glass is translucent and blurred."),
            &segmented(&["Omarchy", "Glass"], if cfg.theme.style == Style::Glass { 1 } else { 0 }, |i| {
                edit(move |c| c.theme.style = if i == 1 { Style::Glass } else { Style::Omarchy })
            }),
        ));
        g.append(&toggle(
            "Monochrome glyphs",
            Some("Draw built-in items as theme-coloured glyphs instead of icons."),
            cfg.items.glyph_ui,
            |v| edit(move |c| c.items.glyph_ui = v),
        ));

        let g = group(&page, "Size");
        g.append(&field(
            "Icon size",
            Some("In pixels."),
            &number(16.0, 128.0, 2.0, 0, cfg.dock.icon_size, |v| edit(move |c| c.dock.icon_size = v)),
        ));
        // 0 means "derive it from the zoom factor", which is the default and
        // keeps magnified icons from overlapping. Any other value pins it.
        g.append(&field(
            "Icon spacing",
            Some("0 = automatic."),
            &number(0.0, 64.0, 1.0, 0, cfg.dock.spacing.unwrap_or(0.0), |v| {
                edit(move |c| c.dock.spacing = if v <= 0.0 { None } else { Some(v) })
            }),
        ));

        stack.add_named(&scroll, Some("appearance"));
    }

    // ── hover ───────────────────────────────────────────────────────────────
    {
        let (scroll, page) = page("Hover", "What happens when the pointer rests on an icon.");

        let g = group(&page, "Effect");
        // Zoom is only meaningful in one of the three modes, so the control
        // that sets it follows the one that selects them.
        let zoom_row = field(
            "Magnification",
            Some("Only used by Magnify."),
            &number(1.05, 2.5, 0.05, 2, cfg.magnify.zoom.max(1.05), |v| edit(move |c| c.magnify.zoom = v)),
        );
        zoom_row.set_sensitive(cfg.magnify.hover == Hover::Scale);
        let current = match cfg.magnify.hover {
            Hover::Fill => 0,
            Hover::Scale => 1,
            Hover::None => 2,
        };
        {
            let zoom_row = zoom_row.clone();
            g.append(&field(
                "On hover",
                Some("Highlight matches the rest of Omarchy."),
                &segmented(&["Highlight", "Magnify", "Nothing"], current, move |i| {
                    let mode = match i {
                        1 => Hover::Scale,
                        2 => Hover::None,
                        _ => Hover::Fill,
                    };
                    zoom_row.set_sensitive(mode == Hover::Scale);
                    edit(move |c| {
                        c.magnify.hover = mode;
                        // `enabled` gates hover reactions as a whole; the mode
                        // says which one. Keep them in step.
                        c.magnify.enabled = mode != Hover::None;
                    })
                }),
            ));
        }
        g.append(&zoom_row);
        g.append(&field(
            "Name delay",
            Some("Milliseconds before the app name appears."),
            &number(0.0, 2000.0, 50.0, 0, cfg.dock.tooltip_delay_ms as f64, |v| {
                edit(move |c| c.dock.tooltip_delay_ms = v as u64)
            }),
        ));

        let g = group(&page, "Window previews");
        let delay_row = field(
            "Preview delay",
            Some("Milliseconds before thumbnails appear."),
            &number(0.0, 3000.0, 50.0, 0, cfg.preview.delay_ms as f64, |v| {
                edit(move |c| c.preview.delay_ms = v as u64)
            }),
        );
        delay_row.set_sensitive(cfg.preview.enabled);
        {
            let row = delay_row.clone();
            g.append(&toggle(
                "Window previews",
                Some("Thumbnails of an app's windows on hover."),
                cfg.preview.enabled,
                move |v| {
                    row.set_sensitive(v);
                    edit(move |c| c.preview.enabled = v)
                },
            ));
        }
        g.append(&delay_row);

        stack.add_named(&scroll, Some("hover"));
    }

    // ── items ───────────────────────────────────────────────────────────────
    {
        let (scroll, page) = page("Items", "What the dock shows besides pinned apps.");

        let g = group(&page, "Apps");
        g.append(&toggle("Running apps", Some("Show apps that are open but not pinned."), cfg.items.show_running, |v| {
            edit(move |c| c.items.show_running = v)
        }));
        g.append(&toggle("Notification badges", None, cfg.items.notification_badges, |v| {
            edit(move |c| c.items.notification_badges = v)
        }));
        g.append(&toggle("Pulse on attention", Some("Animate apps that need attention."), cfg.items.urgent_pulse, |v| {
            edit(move |c| c.items.urgent_pulse = v)
        }));

        let g = group(&page, "Places");
        // One switch per folder: they are independent shortcuts.
        for (i, folder) in cfg.items.folders.iter().enumerate() {
            let name = if folder.name.is_empty() { "Folder".to_string() } else { folder.name.clone() };
            g.append(&toggle(&name, None, folder.enabled, move |v| {
                edit(move |c| {
                    if let Some(f) = c.items.folders.get_mut(i) {
                        f.enabled = v;
                    }
                })
            }));
        }
        g.append(&toggle("Trash", None, cfg.items.show_trash, |v| edit(move |c| c.items.show_trash = v)));
        g.append(&toggle("Removable drives", None, cfg.items.show_drives, |v| {
            edit(move |c| c.items.show_drives = v)
        }));

        let g = group(&page, "System tray");
        // Hosting the tray means claiming a bus name, which happens at startup.
        g.append(&toggle(
            "System tray",
            Some("Takes effect after the dock restarts."),
            cfg.tray.enabled,
            |v| edit(move |c| c.tray.enabled = v),
        ));

        let g = group(&page, "Separators");
        g.append(&action("Add separator", Some("Drag it on the dock to move it."), "Add", add_separator));
        g.append(&action("Remove last separator", None, "Remove", || {
            edit(|c| {
                if let Some(i) =
                    c.items.pinned.iter().rposition(|p| p.trim() == crate::state::SEPARATOR)
                {
                    c.items.pinned.remove(i);
                }
            });
        }));

        stack.add_named(&scroll, Some("items"));
    }

    // ── workspaces ──────────────────────────────────────────────────────────
    {
        let (scroll, page) = page("Workspaces", "Workspace switcher inside the dock.");

        let g = group(&page, "Switcher");
        g.append(&toggle("Workspaces", Some("Show workspace numbers in the dock."), cfg.workspaces.enabled, |v| {
            edit(move |c| c.workspaces.enabled = v)
        }));
        g.append(&toggle("Empty workspaces", None, cfg.workspaces.show_empty, |v| {
            edit(move |c| c.workspaces.show_empty = v)
        }));
        g.append(&toggle("Scratchpad", None, cfg.workspaces.scratchpad, |v| {
            edit(move |c| c.workspaces.scratchpad = v)
        }));
        g.append(&toggle(
            "Hide the bar's workspaces",
            Some("Removes the numbers from the top bar while the dock shows them."),
            cfg.workspaces.hide_bar_workspaces,
            |v| edit(move |c| c.workspaces.hide_bar_workspaces = v),
        ));

        stack.add_named(&scroll, Some("workspaces"));
    }

    // ── sidebar ─────────────────────────────────────────────────────────────
    let sidebar = gtk::Box::new(gtk::Orientation::Vertical, 0);
    sidebar.add_css_class("dock-settings-sidebar");

    let title = gtk::Label::new(Some("Dock"));
    title.add_css_class("dock-settings-title");
    title.set_xalign(0.0);
    sidebar.append(&title);

    let mut first: Option<gtk::ToggleButton> = None;
    for (name, label) in [
        ("placement", "Placement"),
        ("appearance", "Appearance"),
        ("hover", "Hover"),
        ("items", "Items"),
        ("workspaces", "Workspaces"),
    ] {
        let b = gtk::ToggleButton::with_label(label);
        b.add_css_class("dock-settings-nav");
        if let Some(l) = b.child().and_downcast::<gtk::Label>() {
            l.set_xalign(0.0);
        }
        match &first {
            Some(f) => b.set_group(Some(f)),
            None => {
                b.set_active(true);
                first = Some(b.clone());
            }
        }
        let st = stack.clone();
        b.connect_toggled(move |b| {
            if b.is_active() {
                st.set_visible_child_name(name);
            }
        });
        sidebar.append(&b);
    }

    let spacer = gtk::Box::new(gtk::Orientation::Vertical, 0);
    spacer.set_vexpand(true);
    sidebar.append(&spacer);

    let cfg_btn = gtk::Button::with_label("Edit config.toml");
    cfg_btn.add_css_class("dock-settings-nav");
    cfg_btn.add_css_class("dock-settings-footer");
    if let Some(l) = cfg_btn.child().and_downcast::<gtk::Label>() {
        l.set_xalign(0.0);
    }
    cfg_btn.connect_clicked(|_| {
        let path = crate::config::config_path();
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    });
    sidebar.append(&cfg_btn);

    let version = gtk::Label::new(Some(concat!("v", env!("CARGO_PKG_VERSION"))));
    version.add_css_class("dock-settings-version");
    version.set_xalign(0.0);
    sidebar.append(&version);

    let root = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    root.add_css_class("dock-settings");
    root.append(&sidebar);
    root.append(&stack);
    win.set_child(Some(&root));
    win
}

/// One section of the settings window: a title, a one-line description, and
/// the groups below it, scrolling on its own if it outgrows the window.
fn page(title: &str, subtitle: &str) -> (gtk::ScrolledWindow, gtk::Box) {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.add_css_class("dock-settings-page");

    let t = gtk::Label::new(Some(title));
    t.add_css_class("dock-settings-page-title");
    t.set_xalign(0.0);
    page.append(&t);

    let s = gtk::Label::new(Some(subtitle));
    s.add_css_class("dock-settings-page-subtitle");
    s.set_xalign(0.0);
    s.set_wrap(true);
    page.append(&s);

    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_child(Some(&page));
    (scroll, page)
}

/// A titled, bordered block of rows inside a page.
fn group(page: &gtk::Box, title: &str) -> gtk::Box {
    let h = gtk::Label::new(Some(&title.to_uppercase()));
    h.add_css_class("dock-settings-group-title");
    h.set_xalign(0.0);
    page.append(&h);

    let g = gtk::Box::new(gtk::Orientation::Vertical, 0);
    g.add_css_class("dock-settings-group");
    page.append(&g);
    g
}

/// A row: label (with an optional dim hint under it) on the left, the control
/// on the right.
fn field(label: &str, hint: Option<&str>, control: &impl IsA<gtk::Widget>) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 16);
    row.add_css_class("dock-settings-row");

    let text = gtk::Box::new(gtk::Orientation::Vertical, 2);
    text.set_hexpand(true);
    text.set_valign(gtk::Align::Center);
    let l = gtk::Label::new(Some(label));
    l.add_css_class("dock-settings-label");
    l.set_xalign(0.0);
    text.append(&l);
    if let Some(hint) = hint {
        let h = gtk::Label::new(Some(hint));
        h.add_css_class("dock-settings-hint");
        h.set_xalign(0.0);
        h.set_wrap(true);
        text.append(&h);
    }
    row.append(&text);

    control.set_valign(gtk::Align::Center);
    row.append(control);
    row
}

fn toggle<F: Fn(bool) + 'static>(label: &str, hint: Option<&str>, on: bool, changed: F) -> gtk::Box {
    let sw = gtk::Switch::new();
    sw.set_active(on);
    sw.connect_state_set(move |_, v| {
        changed(v);
        glib::Propagation::Proceed
    });
    field(label, hint, &sw)
}

/// A row whose control is a single button.
fn action<F: Fn() + 'static>(label: &str, hint: Option<&str>, button: &str, on_click: F) -> gtk::Box {
    let b = gtk::Button::with_label(button);
    b.add_css_class("dock-settings-button");
    b.connect_clicked(move |_| on_click());
    field(label, hint, &b)
}

/// Omarchy's button group: a row of joined buttons, exactly one active. Used
/// instead of a dropdown for short fixed choices, so every option is visible.
fn segmented<F: Fn(u32) + 'static>(options: &[&str], selected: u32, changed: F) -> gtk::Box {
    let bx = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    bx.add_css_class("dock-segmented");
    let changed = std::rc::Rc::new(changed);
    let mut first: Option<gtk::ToggleButton> = None;
    for (i, opt) in options.iter().enumerate() {
        let b = gtk::ToggleButton::with_label(opt);
        b.add_css_class("dock-segment");
        match &first {
            Some(f) => b.set_group(Some(f)),
            None => first = Some(b.clone()),
        }
        if i as u32 == selected {
            b.set_active(true);
        }
        let cb = changed.clone();
        b.connect_toggled(move |b| {
            if b.is_active() {
                cb(i as u32);
            }
        });
        bx.append(&b);
    }
    bx
}

/// A square numeric field with − / + steppers.
fn number<F: Fn(f64) + 'static>(min: f64, max: f64, step: f64, digits: u32, value: f64, changed: F) -> gtk::SpinButton {
    let s = gtk::SpinButton::with_range(min, max, step);
    s.set_digits(digits);
    s.set_value(value);
    s.add_css_class("dock-settings-number");
    s.connect_value_changed(move |s| changed(s.value()));
    s
}

/// Insert a divider somewhere it will actually be visible.
fn add_separator() {
    edit(|c| {
        let sep = crate::state::SEPARATOR;
        // Appending puts it exactly where the automatic divider already goes,
        // so the two collapse and nothing appears to happen. Insert it
        // mid-list instead, where it is visible and can be dragged into place.
        let mut at = c.items.pinned.len() / 2;
        // Never land next to an existing separator: adjacent dividers collapse
        // when rendered, so the click would look like a no-op and invite the
        // user to click again, piling up dead entries in the config.
        let is_sep =
            |i: usize| c.items.pinned.get(i).is_some_and(|p: &String| p.trim() == sep);
        while at < c.items.pinned.len() && (is_sep(at) || (at > 0 && is_sep(at - 1))) {
            at += 1;
        }
        if at > 0 && is_sep(at - 1) {
            return;
        }
        c.items.pinned.insert(at.min(c.items.pinned.len()), sep.into());
    });
}

/// Load, mutate, and save the config. The watcher applies it.
fn edit<F: FnOnce(&mut Config)>(f: F) {
    if let Err(e) = Config::edit(f) {
        tracing::error!(error = %e, "cannot save settings");
    }
}

fn row<F: Fn() + 'static>(label: &str, on_click: F) -> gtk::Button {
    let b = gtk::Button::with_label(label);
    b.add_css_class("dock-menu-item");
    b.set_has_frame(false);
    if let Some(l) = b.child().and_downcast::<gtk::Label>() {
        l.set_xalign(0.0);
    }
    b.connect_clicked(move |_| on_click());
    b
}

/// A menu row's content with a current-state label on the right.
fn state_row(label: &str, state: &str) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let l = gtk::Label::new(Some(label));
    l.set_xalign(0.0);
    l.set_hexpand(true);
    b.append(&l);
    let st = gtk::Label::new(Some(state));
    st.add_css_class("dock-menu-workspace");
    b.append(&st);
    b
}

fn heading(text: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.add_css_class("dock-menu-heading");
    l.set_xalign(0.0);
    l
}

fn separator() -> gtk::Separator {
    let s = gtk::Separator::new(gtk::Orientation::Horizontal);
    s.add_css_class("dock-menu-sep");
    s
}
