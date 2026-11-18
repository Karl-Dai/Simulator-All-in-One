//! Native close waits for the suite frontend to finish its registered writes.
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager, RunEvent, Runtime, State, WindowEvent};

/// The frontend arms the guard only after its event listener is installed.
/// A failed initial page can therefore still be closed normally.
#[derive(Default)]
pub struct CloseGuard {
    armed: AtomicBool,
    requested: AtomicBool,
    approved: AtomicBool,
}
#[derive(Debug, PartialEq, Eq)]
enum CloseDecision {
    Allow,
    Prevent { notify: bool },
}
impl CloseGuard {
    fn enable(&self) {
        self.armed.store(true, Ordering::Release);
    }
    fn request(&self) -> CloseDecision {
        if !self.armed.load(Ordering::Acquire) || self.approved.load(Ordering::Acquire) {
            return CloseDecision::Allow;
        }
        let notify = !self.requested.swap(true, Ordering::AcqRel);
        CloseDecision::Prevent { notify }
    }
    fn confirm(&self) -> Result<(), String> {
        if !self.armed.load(Ordering::Acquire)
            || !self.requested.load(Ordering::Acquire)
            || self
                .approved
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return Err("No configuration close request is pending".into());
        }
        self.requested.store(false, Ordering::Release);
        Ok(())
    }
}

#[tauri::command]
pub async fn simlab_enable_configuration_close(guard: State<'_, CloseGuard>) -> Result<(), String> {
    guard.enable();
    Ok(())
}

#[tauri::command]
pub async fn simlab_confirm_configuration_close(
    app: AppHandle,
    guard: State<'_, CloseGuard>,
) -> Result<(), String> {
    guard.confirm()?;
    // exit(0) emits ExitRequested too; approval lets that event proceed.
    app.exit(0);
    Ok(())
}

fn request_flush<R: Runtime>(app: &AppHandle<R>) {
    if let Err(error) = app.emit("saio-before-close", ()) {
        // Permit another native close request to retry a failed delivery.
        app.state::<CloseGuard>()
            .requested
            .store(false, Ordering::Release);
        log::error!("configuration flush request could not reach the frontend: {error}");
    }
}

pub fn protect<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    builder
        .manage(CloseGuard::default())
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if let CloseDecision::Prevent { notify } = window.state::<CloseGuard>().request() {
                    api.prevent_close();
                    if notify {
                        request_flush(window.app_handle());
                    }
                }
            }
        })
}

pub fn on_run_event<R: Runtime>(app: &AppHandle<R>, event: RunEvent) {
    if let RunEvent::ExitRequested { api, .. } = event {
        if let CloseDecision::Prevent { notify } = app.state::<CloseGuard>().request() {
            api.prevent_exit();
            if notify {
                request_flush(app);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_page_without_listener_can_close_or_exit() {
        let guard = CloseGuard::default();
        assert_eq!(guard.request(), CloseDecision::Allow);
        assert!(guard.confirm().is_err());
    }

    #[test]
    fn confirmation_without_an_active_request_cannot_exit() {
        let guard = CloseGuard::default();
        guard.enable();
        assert!(guard.confirm().is_err());
        assert!(!guard.approved.load(Ordering::Acquire));
    }

    #[test]
    fn armed_window_and_exit_requests_coalesce_until_confirmation() {
        let guard = CloseGuard::default();
        guard.enable();
        assert_eq!(guard.request(), CloseDecision::Prevent { notify: true });
        assert_eq!(guard.request(), CloseDecision::Prevent { notify: false });
        // A failed frontend flush does not acknowledge the pending request.
        assert_eq!(guard.request(), CloseDecision::Prevent { notify: false });
        guard.confirm().unwrap();
        assert_eq!(guard.request(), CloseDecision::Allow);
        assert!(guard.confirm().is_err());
    }

    #[test]
    fn enabling_again_does_not_revoke_an_approved_exit() {
        let guard = CloseGuard::default();
        guard.enable();
        guard.request();
        guard.confirm().unwrap();
        guard.enable();
        assert_eq!(guard.request(), CloseDecision::Allow);
    }
}
