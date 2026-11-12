//! Native application menus. Workspace actions are handled by the focused window.
use gpui::{App, KeyBinding, Menu, MenuItem, OsAction, SystemMenuType, actions};

actions!(
    bitman,
    [
        About,
        CheckForUpdates,
        Quit,
        Hide,
        HideOthers,
        ShowAll,
        OpenImage,
        ExportSvg,
        ExportMockup,
        RemoveArtwork,
        CloseWindow,
        Cut,
        Copy,
        Paste,
        SelectAll,
        ShowCompose,
        Undo,
        Redo,
        ShowConvert,
        ShowPrint,
        ShowSimulate,
        ToggleFullScreen,
        Minimize,
        Zoom,
        BringAllToFront,
        ShowHelp,
    ]
);

pub fn init(cx: &mut App) {
    cx.on_action(|_: &Quit, cx| {
        if !crate::updater::install_in_progress() {
            cx.quit();
        }
    });
    cx.on_action(|_: &Hide, cx| cx.hide());
    cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
    cx.on_action(|_: &BringAllToFront, cx| {
        for window in cx.windows() {
            let _ = window.update(cx, |_, window, _| window.activate_window());
        }
        cx.activate(true);
    });

    // Register before building the menus so macOS displays their key equivalents.
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("cmd-alt-h", HideOthers, None),
        KeyBinding::new("cmd-o", OpenImage, None),
        KeyBinding::new("cmd-shift-s", ExportSvg, None),
        KeyBinding::new("cmd-w", CloseWindow, None),
        KeyBinding::new("cmd-x", Cut, None),
        KeyBinding::new("cmd-c", Copy, None),
        KeyBinding::new("cmd-v", Paste, None),
        KeyBinding::new("cmd-a", SelectAll, None),
        KeyBinding::new("cmd-4", ShowCompose, None),
        KeyBinding::new("cmd-z", Undo, Some("Workspace")),
        KeyBinding::new("cmd-shift-z", Redo, Some("Workspace")),
        KeyBinding::new("cmd-1", ShowConvert, None),
        KeyBinding::new("cmd-2", ShowPrint, None),
        KeyBinding::new("cmd-3", ShowSimulate, None),
        KeyBinding::new("cmd-alt-s", ExportMockup, None),
        KeyBinding::new("backspace", RemoveArtwork, Some("Workspace")),
        KeyBinding::new("delete", RemoveArtwork, Some("Workspace")),
        KeyBinding::new("ctrl-cmd-f", ToggleFullScreen, None),
        KeyBinding::new("cmd-m", Minimize, None),
        // GPUI represents shifted punctuation by the resulting character.
        KeyBinding::new("cmd-?", ShowHelp, None),
    ]);
    cx.set_menus(vec![
        Menu {
            name: "Bitman".into(),
            items: vec![
                MenuItem::action("About Bitman", About),
                MenuItem::action("Check for Updates…", CheckForUpdates),
                MenuItem::separator(),
                MenuItem::os_submenu("Services", SystemMenuType::Services),
                MenuItem::separator(),
                MenuItem::action("Hide Bitman", Hide),
                MenuItem::action("Hide Others", HideOthers),
                MenuItem::action("Show All", ShowAll),
                MenuItem::separator(),
                MenuItem::action("Quit Bitman", Quit),
            ],
        },
        Menu {
            name: "File".into(),
            items: vec![
                MenuItem::action("Open Image…", OpenImage),
                MenuItem::action("Export SVG…", ExportSvg),
                MenuItem::action("Save Mock-up JPG…", ExportMockup),
                MenuItem::separator(),
                MenuItem::action("Close Window", CloseWindow),
            ],
        },
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::action("Undo", Undo),
                MenuItem::action("Redo", Redo),
                MenuItem::separator(),
                // Native selectors also work in the Open/Export dialog text fields.
                // Compose fields supply handlers while editing.
                MenuItem::os_action("Cut", Cut, OsAction::Cut),
                MenuItem::os_action("Copy", Copy, OsAction::Copy),
                MenuItem::os_action("Paste", Paste, OsAction::Paste),
                MenuItem::separator(),
                MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
                MenuItem::separator(),
                MenuItem::action("Remove Artwork", RemoveArtwork),
            ],
        },
        Menu {
            name: "View".into(),
            items: vec![
                MenuItem::action("Convert", ShowConvert),
                MenuItem::action("Compose", ShowCompose),
                MenuItem::action("Print", ShowPrint),
                MenuItem::action("Simulate", ShowSimulate),
                MenuItem::separator(),
                MenuItem::action("Toggle Full Screen", ToggleFullScreen),
            ],
        },
        Menu {
            name: "Window".into(),
            items: vec![
                MenuItem::action("Minimize", Minimize),
                MenuItem::action("Zoom", Zoom),
                MenuItem::separator(),
                MenuItem::action("Bring All to Front", BringAllToFront),
            ],
        },
        Menu {
            name: "Help".into(),
            items: vec![MenuItem::action("Bitman Help", ShowHelp)],
        },
    ]);
}
