use std::time::Duration;

use gpui::{Context, Div, PromptLevel, Styled, Window, prelude::*, px, rgb};

use crate::{
    app::Workspace,
    controls::*,
    updater::{self, UpdateState},
};

impl Workspace {
    pub fn start_update_checks(&mut self, cx: &mut Context<Self>) {
        // Debug builds do not check automatically or install updates.
        if cfg!(debug_assertions) {
            return;
        }
        self.check_for_updates(cx);
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_secs(6 * 60 * 60))
                    .await;
                if this
                    .update(cx, |this, cx| this.check_for_updates(cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    pub fn check_for_updates(&mut self, cx: &mut Context<Self>) {
        if self.update_state.busy() || matches!(self.update_state, UpdateState::Installed) {
            return;
        }
        self.update_state = UpdateState::Checking;
        cx.notify();
        let task = cx.background_executor().spawn(async { updater::check() });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.update_state = match result {
                    Ok(Some(release)) => UpdateState::Available(release),
                    Ok(None) => UpdateState::Current,
                    Err(error) => UpdateState::Failed(error),
                };
                cx.notify();
            });
        })
        .detach();
    }

    fn install_update(&mut self, cx: &mut Context<Self>) {
        let UpdateState::Available(release) = &self.update_state else {
            return;
        };
        let release = release.clone();
        // Replacement may rename the running executable (Windows) or unlink it
        // (Linux). Preserve the original path before that happens for GPUI's restart.
        match std::env::current_exe() {
            Ok(path) => cx.set_restart_path(path),
            Err(error) => {
                self.update_state = UpdateState::InstallFailed(error.to_string());
                cx.notify();
                return;
            }
        }
        self.update_state = UpdateState::Installing;
        cx.notify();
        let task = cx
            .background_executor()
            .spawn(async move { updater::install(&release) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.update_state = match result {
                    Ok(()) => UpdateState::Installed,
                    Err(error) => UpdateState::InstallFailed(error),
                };
                cx.notify();
            });
        })
        .detach();
    }

    fn restart_for_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let prompt = window.prompt(PromptLevel::Warning, "Restart Bitman?",
            Some("Export any work you want to keep before restarting. Your current image and settings will not be restored."),
            &["Cancel", "Restart"], cx);
        cx.spawn(async move |this, cx| {
            if let Ok(1) = prompt.await {
                let _ = this.update(cx, |_, cx| cx.restart());
            }
        })
        .detach();
    }

    fn update_action(&self, cx: &mut Context<Self>) -> gpui::Stateful<Div> {
        match &self.update_state {
            UpdateState::Available(_) if updater::manual_update_reason().is_none() => {
                primary_button("Install update")
                    .cursor_pointer()
                    .id("install-update")
                    .on_click(cx.listener(|this, _, _, cx| this.install_update(cx)))
            }
            UpdateState::Installed => primary_button("Restart…")
                .cursor_pointer()
                .id("restart-update")
                .on_click(cx.listener(|this, _, window, cx| this.restart_for_update(window, cx))),
            UpdateState::Available(_) => button("Download update ↗")
                .cursor_pointer()
                .id("download-update")
                .on_click(|_, _, cx| cx.open_url(updater::RELEASES_URL)),
            _ => button("Check for updates").id("check-updates").when(
                !self.update_state.busy(),
                |button| {
                    button
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| this.check_for_updates(cx)))
                },
            ),
        }
    }

    pub fn update_controls(&self, cx: &mut Context<Self>) -> Div {
        column()
            .child(muted(self.update_state.message()))
            .child(muted("Release builds check on launch and every six hours. Installation starts only when you choose it."))
            .when_some(updater::manual_update_reason(), |view, reason| view.child(muted(reason)))
            .child(self.update_action(cx))
            .child(button("GitHub releases ↗").cursor_pointer().id("github-releases")
                .on_click(|_, _, cx| cx.open_url(updater::RELEASES_URL)))
    }

    pub fn update_banner(&self, cx: &mut Context<Self>) -> Div {
        row()
            .flex_shrink_0()
            .px(px(18.0))
            .py(px(8.0))
            .bg(rgb(PANEL))
            .child(muted(
                if matches!(self.update_state, UpdateState::InstallFailed(_)) {
                    "Could not install the update. See Help → Updates for details.".into()
                } else {
                    self.update_state.message()
                },
            ))
            .child(self.update_action(cx))
    }
}
