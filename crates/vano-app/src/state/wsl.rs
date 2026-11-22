//! Deleting inside a WSL 2 distribution, as the confirmation's batch: `vano-wsl delete` reports
//! its progress and how each item ended, and this turns that into the same [`Outcome`] the
//! Trash and the local permanent deletion give.

use std::sync::atomic::Ordering;

use vano_system::wsl::DeleteLine;

use super::trash::{Outcome, TrashError, TrashProgress};

/// Deletes the Linux `paths` for good inside `distro`. A blocking call, for the background
/// executor. `progress.cancel` stops before the next file.
pub fn delete(distro: &str, paths: &[String], progress: &TrashProgress) -> Outcome {
    let Some(agent) = vano_system::wsl::agent() else {
        let error = TrashError::Os(format!("{} is missing", vano_system::wsl::AGENT));
        return Outcome {
            failed: (0..paths.len())
                .map(|index| (index, error.clone()))
                .collect(),
            ..Outcome::default()
        };
    };
    let mut outcome = Outcome::default();
    let ran = vano_system::wsl::delete(distro, &agent, paths, &progress.cancel, |line| {
        apply(&mut outcome, progress, line);
    });
    if let Err(error) = ran {
        let reported: Vec<usize> = outcome
            .moved
            .iter()
            .chain(outcome.failed.iter().map(|(index, _)| index))
            .copied()
            .collect();
        for index in (0..paths.len()).filter(|index| !reported.contains(index)) {
            outcome.failed.push((index, TrashError::Os(error.clone())));
        }
    }
    outcome.cancelled = progress.cancel.load(Ordering::Relaxed);
    outcome
}

/// Adds one line of `vano-wsl delete` to the outcome and the progress.
fn apply(outcome: &mut Outcome, progress: &TrashProgress, line: DeleteLine) {
    match line {
        DeleteLine::Progress(files, bytes) => {
            progress.files.store(files, Ordering::Relaxed);
            progress.bytes.store(bytes, Ordering::Relaxed);
        }
        DeleteLine::Gone(index) => {
            outcome.moved.push(index);
            progress.done.fetch_add(1, Ordering::Relaxed);
        }
        DeleteLine::Failed(index, reason) => {
            let error = if reason == "other file system" {
                TrashError::OtherFileSystem
            } else {
                TrashError::Os(reason)
            };
            outcome.failed.push((index, error));
            progress.done.fetch_add(1, Ordering::Relaxed);
        }
        DeleteLine::Stopped(index) => {
            outcome.partial.push(index);
            outcome.cancelled = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::trash::DeleteMode;

    #[test]
    fn the_lines_become_an_outcome() {
        let progress = TrashProgress::new(DeleteMode::Permanent, 3, 10, 100);
        let mut outcome = Outcome::default();
        for line in [
            DeleteLine::Progress(4, 40),
            DeleteLine::Gone(0),
            DeleteLine::Failed(1, "Permission denied".to_string()),
            DeleteLine::Failed(2, "other file system".to_string()),
        ] {
            apply(&mut outcome, &progress, line);
        }
        assert_eq!(outcome.moved, vec![0]);
        assert_eq!(
            outcome.failed,
            vec![
                (1, TrashError::Os("Permission denied".to_string())),
                (2, TrashError::OtherFileSystem)
            ]
        );
        assert_eq!(progress.files_done(), 4);
        assert_eq!(progress.done.load(Ordering::Relaxed), 3);
    }

    #[test]
    fn a_stopped_item_cancels_the_batch() {
        let progress = TrashProgress::new(DeleteMode::Permanent, 1, 10, 100);
        let mut outcome = Outcome::default();
        apply(&mut outcome, &progress, DeleteLine::Stopped(0));
        assert!(outcome.cancelled);
        assert_eq!(outcome.partial, vec![0]);
    }
}
