//! The in-app updater's glue, as Postino's: the single delayed check at startup, the manual check from
//! the command palette, the restart confirmation and the installation. The decisions and the
//! blocking work live in `state::update`; everything blocking runs on the background executor.
//! Nothing here repeats or polls, so an idle window stays idle.

use gpui_kit::component::notification::Notification;
use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use rust_i18n::t;
use telaio_update::Platform;

use crate::state::update::{
    self, CheckOutcome, INSTALL_COMMAND, InstallOutcome, ManualUpdate, ReadyUpdate, STARTUP_DELAY,
    updater_enabled,
};

use super::root::AppView;

/// The notification text for a newer version that this copy has to get by hand.
fn available_text(manual: ManualUpdate, version: &str) -> String {
    match manual {
        ManualUpdate::PortableZip => t!("update.available_manual", version = version),
        ManualUpdate::Download => t!("update.available_linux", version = version),
        ManualUpdate::InstallScript => t!("update.available_script", version = version),
    }
    .into_owned()
}

impl AppView {
    /// How this copy gets a new version when it downloads nothing.
    pub(crate) fn manual_update(&self) -> ManualUpdate {
        update::manual_update(Platform::current(), self.about_info.install_script)
    }

    /// Copies [`INSTALL_COMMAND`] for a copy installed by the install script, which updates by
    /// running it again.
    pub(crate) fn copy_update_command(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(INSTALL_COMMAND.to_string()));
        window.push_notification(
            Notification::info(t!("update.command_copied", command = INSTALL_COMMAND).into_owned()),
            cx,
        );
    }

    /// Schedules the one automatic check, [`STARTUP_DELAY`] after startup. A no-op unless the
    /// build has an updater and the setting is on. Failures are only logged.
    pub(crate) fn schedule_startup_update_check(&mut self, cx: &mut Context<Self>) {
        let setting = self.state.settings.check_updates;
        if !update::should_check_at_startup(updater_enabled(), setting) {
            return;
        }
        let Some(platform) = Platform::current() else {
            return;
        };
        let portable = self.about_info.portable;
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(STARTUP_DELAY).await;
            // The setting may have been turned off during the delay.
            let start = this
                .update(cx, |view, cx| {
                    let start =
                        view.state.settings.check_updates && view.state.update.begin_check();
                    if start {
                        cx.notify();
                    }
                    start
                })
                .unwrap_or(false);
            if !start {
                return;
            }
            let result = cx
                .background_spawn(async move { update::check_and_download(platform, portable) })
                .await;
            // One repaint per change, for the status bar and an open Settings, "About".
            let _ = this.update(cx, |view, cx| {
                match result {
                    Ok(outcome) => view.state.update.finish_check(&outcome),
                    Err(error) => {
                        view.state.update.fail_check();
                        log::warn!("update check failed: {error}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The command palette's "Check for updates": runs the check whatever the setting says and
    /// reports the result in a notification.
    pub(crate) fn check_for_updates_manually(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(platform) = Platform::current().filter(|_| updater_enabled()) else {
            return;
        };
        // With an update already ready, check anyway: a newer version may have come out since
        // (Postino #300). There is no check flag to raise then, a second click only checks twice.
        let had_ready = self.state.update.ready().is_some();
        if !had_ready && !self.state.update.begin_check() {
            return;
        }
        window.push_notification(Notification::info(t!("update.checking").into_owned()), cx);
        cx.notify();
        let portable = self.about_info.portable;
        let manual = self.manual_update();
        cx.spawn_in(window, async move |this, cx| {
            let result = cx
                .background_spawn(async move { update::check_and_download(platform, portable) })
                .await;
            let _ = this.update_in(cx, |view, window, cx| {
                let notification = match result {
                    Ok(outcome) => {
                        let replaced = update::keep_latest(&mut view.state.update, &outcome);
                        if !had_ready {
                            view.state.update.finish_check(&outcome);
                        }
                        match outcome {
                            CheckOutcome::Ready(ready) if had_ready && !replaced => {
                                Notification::info(
                                    t!("update.already_ready", version = ready.version.as_str())
                                        .into_owned(),
                                )
                            }
                            CheckOutcome::UpToDate => Notification::success(
                                t!("update.up_to_date", version = update::CURRENT_VERSION)
                                    .into_owned(),
                            ),
                            CheckOutcome::Ready(ready) => {
                                let version = ready.version.as_str();
                                let text = if ready.is_manual_download() {
                                    available_text(manual, version)
                                } else {
                                    t!("update.found", version = version).into_owned()
                                };
                                Notification::info(text)
                            }
                        }
                    }
                    Err(error) => {
                        if !had_ready {
                            view.state.update.fail_check();
                        }
                        log::warn!("update check failed: {error}");
                        Notification::error(
                            t!("update.check_failed", error = error.to_string()).into_owned(),
                        )
                    }
                };
                window.push_notification(notification, cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// The "Restart to update" click: installs. Nothing installs without this click.
    pub(crate) fn request_update_restart(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.state.update.ready().is_none() {
            return;
        }
        // Never while files are being moved to the Trash: quitting would cut the move short.
        if self.trash_job_running() {
            return;
        }
        self.install_update(window, cx);
    }

    /// Installs the ready update on the background executor, then quits (the installer or the
    /// swap script takes over) or explains why it could not.
    fn install_update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(platform) = Platform::current() else {
            return;
        };
        let Some(ready) = self.state.update.begin_install() else {
            return;
        };
        let exe = std::env::current_exe().unwrap_or_default();
        let portable = self.about_info.portable;
        cx.spawn_in(window, async move |this, cx| {
            // The ready update may be days old: install the latest version (Postino #300).
            let (ready, outcome) = cx
                .background_spawn(async move {
                    let ready = update::latest_ready(platform, portable, ready);
                    log::info!("installing update {}", ready.version);
                    let outcome = update::start_install(platform, &ready, &exe);
                    (ready, outcome)
                })
                .await;
            let _ = this.update_in(cx, |view, window, cx| {
                view.finish_install(outcome, &ready, window, cx);
            });
        })
        .detach();
    }

    fn finish_install(
        &mut self,
        outcome: InstallOutcome,
        ready: &ReadyUpdate,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match outcome {
            InstallOutcome::Quit => cx.quit(),
            InstallOutcome::Fallback => {
                self.state.update.fail_install();
                log::warn!("update {} cannot be installed in place", ready.version);
                cx.open_url(&ready.notes_url);
                window.push_notification(
                    Notification::warning(t!("update.fallback").into_owned()),
                    cx,
                );
            }
            InstallOutcome::Failed(error) => {
                self.state.update.fail_install();
                log::warn!("installing update {} failed: {error}", ready.version);
                window.push_notification(
                    Notification::error(t!("update.install_failed", error = error).into_owned()),
                    cx,
                );
            }
        }
    }
}
