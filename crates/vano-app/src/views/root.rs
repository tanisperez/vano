//! [`AppView`], the window's root view: the title bar, the current screen (Home, Scanning,
//! Results), the status bar, the toast, and the state that flows between them.
//!
//! The scan runs on `vano-scan`'s own threads. While it runs, one task polls it ten times a
//! second (progress, the folders that finished, the live recommendations) and notifies; when it
//! is done nothing polls any more, so an idle window does no work.

use std::cell::Cell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::*;
use gpui_kit::prelude::*;
use gpui_kit::*;
use vano_core::{NodeId, ROOT, Tree};
use vano_rules::{Context as RulesContext, LiveRules, Recommendation, Status};
use vano_scan::{DiskUsage, Progress, ScanHandle, ScanOptions};

use crate::actions::{ChooseFolder, CloseDetail, OpenSettings, StartScan, ZoomOut};
use crate::state::about::AboutInfo;
use crate::state::debug_open::DebugOpenTarget;
use crate::state::motion::{LIVE_SPRING, Motion, SPRING};
use crate::state::recents::{self, Recent};
use crate::state::results::Results;
use crate::state::settings::{LiveView, Settings, SettingsCategory};
use crate::state::target::{Target, TargetKind};
use crate::state::treemap::TileKey;
use crate::state::update::UpdateState;
use crate::state::{AppState, format, trash};
use crate::theme::PaletteExt;

use super::settings::SettingsSelects;
use super::treemap_view::TileView;

/// How often a running scan is polled.
const POLL_INTERVAL: Duration = Duration::from_millis(100);
/// How long the finished scan stays on screen before the results open by themselves.
const RESULTS_DELAY: Duration = Duration::from_millis(1400);
/// How long the toast stays.
const TOAST_DURATION: Duration = Duration::from_secs(8);

/// The screen the window shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Home,
    Scanning,
    Results,
}

/// How the explorer colors its tiles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorBy {
    #[default]
    Category,
    Cleanup,
}

/// What the right panel of the results shows instead of the list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detail {
    Rec(&'static str),
    Node(NodeId),
}

/// A running (or just finished) scan.
pub struct ScanSession {
    pub target: Target,
    pub handle: Arc<ScanHandle>,
    pub progress: Progress,
    /// Bytes the scan is expected to count, for the percentage: the disk's used space, or the
    /// previous scan of the same folder.
    pub estimate: Option<u64>,
    pub rules: LiveRules,
    pub found: Vec<Recommendation>,
    /// Status of every node a found recommendation covers, for the live tile colors.
    pub found_status: HashMap<NodeId, Status>,
    /// Bumped on every poll, so the live views lay out again.
    pub generation: u64,
    /// The finished tree, once the scan is done and before the results take it.
    pub tree: Option<Arc<Tree>>,
    pub finished: bool,
    _poll: Task<()>,
}

impl ScanSession {
    /// Runs `f` with the live tree, or the finished one.
    pub fn with_tree<R>(&self, f: impl FnOnce(&Tree) -> R) -> R {
        match &self.tree {
            Some(tree) => f(tree),
            None => self.handle.read_tree(f),
        }
    }

    /// The fraction done, when there is an estimate.
    pub fn fraction(&self) -> Option<f32> {
        if self.finished {
            return Some(1.0);
        }
        let estimate = self.estimate.filter(|e| *e > 0)?;
        Some((self.progress.bytes as f64 / estimate as f64).min(0.99) as f32)
    }
}

/// The treemap's animation state, shared by the live view and the explorer.
pub struct TreemapState {
    pub motion: Motion<TileKey, TileView>,
    /// The size of the area the tiles fill, measured while painting.
    pub bounds: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// What the current layout was computed for: area size, focus, data generation.
    pub laid_out: Option<(gpui_kit::Size<Pixels>, NodeId, u64)>,
    /// A zoom waiting for the next layout to animate from.
    pub pending: Option<Transition>,
    pub last_frame: Option<Instant>,
    /// When the running animation started and how many frames it drew, for the debug log.
    pub animation: Option<(Instant, u32)>,
}

impl TreemapState {
    pub fn new() -> Self {
        Self {
            motion: Motion::new(SPRING),
            bounds: Rc::new(Cell::new(None)),
            laid_out: None,
            pending: None,
            last_frame: None,
            animation: None,
        }
    }
}

/// How the next layout of the explorer moves from the previous one.
#[derive(Debug, Clone, Copy)]
pub enum Transition {
    /// Zoomed into the node whose tile was at this rectangle.
    In(vano_core::treemap::Rect),
    /// Zoomed out of `child`: the old tiles shrink into its new tile.
    Out(NodeId),
}

/// The explorer of the results screen.
pub struct Explorer {
    pub focus: NodeId,
    /// Treemap, Rings or Ranking, like the live view of the scan.
    pub view: LiveView,
    /// What the rings or the ranking last showed: focus, generation and view.
    pub views_key: Option<(NodeId, u64, LiveView)>,
    pub color_by: ColorBy,
    pub treemap: TreemapState,
    /// Bumped when the tree changes (items moved to the Trash or put back).
    pub generation: u64,
}

/// A batch of items moved to the Trash, kept for Undo.
#[derive(Debug, Clone)]
pub struct TrashBatch {
    pub nodes: Vec<NodeId>,
    pub paths: Vec<PathBuf>,
    pub size: u64,
    pub count: usize,
    pub at: i64,
}

/// An item of a batch that could not be moved, for the report.
#[derive(Debug, Clone)]
pub struct LeftItem {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub reason: String,
}

/// How a batch that failed in part, or was stopped, ended.
#[derive(Debug, Clone)]
pub struct TrashReport {
    /// Items moved, and their bytes.
    pub moved: usize,
    pub freed: u64,
    /// Items in the batch.
    pub total: usize,
    pub left: Vec<LeftItem>,
    pub undo: Option<TrashBatch>,
}

/// The toast at the bottom of the window.
pub struct Toast {
    pub text: SharedString,
    pub undo: Option<TrashBatch>,
    pub error: bool,
    _hide: Task<()>,
}

/// What the confirmation dialog would move.
#[derive(Debug, Clone)]
pub struct TrashRequest {
    pub rules: Vec<&'static str>,
    pub nodes: Vec<NodeId>,
}

/// The window's root view.
pub struct AppView {
    pub(crate) state: AppState,
    pub(crate) screen: Screen,
    /// The place the next scan reads, picked on the home screen.
    pub(crate) target: Target,
    pub(crate) recents: Vec<Recent>,
    pub(crate) disk: Option<DiskUsage>,
    pub(crate) scan: Option<ScanSession>,
    pub(crate) results: Option<Results>,
    pub(crate) explorer: Explorer,
    pub(crate) detail: Option<Detail>,
    pub(crate) live_view: LiveView,
    pub(crate) live_treemap: TreemapState,
    pub(crate) ranking: Motion<NodeId, super::live_views::RankRow>,
    pub(crate) ring: super::live_views::RingState,
    pub(crate) toast: Option<Toast>,
    pub(crate) trashing: bool,
    /// The batch being moved to the Trash, while it runs.
    pub(crate) trash_progress: Option<Arc<trash::TrashProgress>>,
    pub(crate) settings_category: SettingsCategory,
    pub(crate) settings_selects: Option<SettingsSelects>,
    pub(crate) about_info: AboutInfo,
    pub(crate) rules_ctx: RulesContext,
    pub(crate) home: Option<PathBuf>,
    focus_handle: FocusHandle,
    _results_timer: Option<Task<()>>,
}

impl AppView {
    pub fn new(
        settings: Settings,
        debug: Option<DebugOpenTarget>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        // The modal backdrop is painted from `has_active_dialog`, which no event of this view
        // reports: re-rendering when the focus leaves for a dialog and comes back keeps it in step.
        cx.on_focus_out(&focus_handle, window, |_, _, _, cx| cx.notify())
            .detach();
        cx.on_focus_in(&focus_handle, window, |_, _, cx| cx.notify())
            .detach();
        cx.on_focus_lost(window, |view, window, cx| {
            if !window.has_active_dialog(cx) {
                view.focus_handle.focus(window, cx);
                cx.defer_in(window, |_, window, _| window.refresh());
            }
        })
        .detach();
        focus_handle.focus(window, cx);

        let target = Target::home().unwrap_or_else(Target::disk);
        let live_view = settings.live_view;
        let mut view = Self {
            state: AppState {
                settings,
                update: UpdateState::default(),
            },
            screen: Screen::Home,
            target,
            recents: recents::load(),
            disk: vano_scan::root_filesystem_usage(&crate::state::target::system_root()),
            scan: None,
            results: None,
            explorer: Explorer {
                focus: ROOT,
                view: live_view,
                views_key: None,
                color_by: ColorBy::Category,
                treemap: TreemapState::new(),
                generation: 0,
            },
            detail: None,
            live_view,
            live_treemap: TreemapState::new(),
            ranking: Motion::new(LIVE_SPRING),
            ring: Default::default(),
            toast: None,
            trashing: false,
            trash_progress: None,
            settings_category: SettingsCategory::default(),
            settings_selects: None,
            about_info: AboutInfo::detect(),
            rules_ctx: RulesContext::current(),
            home: dirs::home_dir(),
            focus_handle,
            _results_timer: None,
        };
        view.live_treemap.motion.set_spring(LIVE_SPRING);
        view.schedule_startup_update_check(cx);
        if let Some(debug) = debug {
            view.apply_debug_open(debug, window, cx);
        }
        view
    }

    fn apply_debug_open(
        &mut self,
        debug: DebugOpenTarget,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match debug {
            DebugOpenTarget::Settings(category) => {
                self.settings_category = category;
                cx.defer_in(window, |view, window, cx| view.open_settings(window, cx));
            }
            DebugOpenTarget::ScanHome => self.start_scan(cx),
            DebugOpenTarget::Scan(path) => {
                self.target = Target::folder(path);
                self.start_scan(cx);
            }
            DebugOpenTarget::Results => {
                if let Some(recent) = self.recents.first().cloned() {
                    self.open_recent(recent, cx);
                }
            }
        }
    }

    /// Strings cached outside `render`: none so far, the views rebuild every text each frame.
    pub(crate) fn relocalize(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {}

    /// Whether files are being moved to the Trash or put back right now.
    pub(crate) fn trash_job_running(&self) -> bool {
        self.trashing
    }

    /// Picks what the next scan reads.
    pub(crate) fn set_target(&mut self, target: Target, cx: &mut Context<Self>) {
        self.target = target;
        cx.notify();
    }

    /// Opens the system folder picker and selects the folder chosen.
    pub(crate) fn choose_folder(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let _ = view.update(cx, |view, cx| {
                view.set_target(Target::folder(path), cx);
            });
        })
        .detach();
    }

    /// Starts scanning [`Self::target`].
    pub(crate) fn start_scan(&mut self, cx: &mut Context<Self>) {
        if self.trashing {
            return;
        }
        let target = self.target.clone();
        log::info!("scan started: {}", target.path.display());
        let handle = Arc::new(vano_scan::start(ScanOptions::new(target.path.clone())));
        let estimate = match target.kind {
            TargetKind::Disk => self.disk.map(|usage| usage.used()),
            _ => self
                .recents
                .iter()
                .find(|recent| recent.target.path == target.path)
                .map(|recent| recent.size),
        };
        let poll = cx.spawn(async move |view, cx| {
            loop {
                cx.background_executor().timer(POLL_INTERVAL).await;
                let done = view
                    .update(cx, |view, cx| view.poll_scan(cx))
                    .unwrap_or(true);
                if done {
                    break;
                }
            }
        });
        self.scan = Some(ScanSession {
            target,
            handle,
            progress: Progress {
                files: 0,
                dirs: 0,
                bytes: 0,
                errors: 0,
                elapsed: Duration::ZERO,
                current: None,
                finished: false,
                cancelled: false,
            },
            estimate,
            rules: LiveRules::new(self.rules_ctx.clone()),
            found: Vec::new(),
            found_status: HashMap::new(),
            generation: 0,
            tree: None,
            finished: false,
            _poll: poll,
        });
        self.live_treemap.motion.clear();
        self.live_treemap.laid_out = None;
        self.ranking.clear();
        self.ring = Default::default();
        self.live_view = self.state.settings.live_view;
        self.detail = None;
        self.screen = Screen::Scanning;
        self._results_timer = None;
        cx.notify();
    }

    /// One poll of the running scan. Returns whether polling should stop.
    fn poll_scan(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(session) = self.scan.as_mut() else {
            return true;
        };
        session.progress = session.handle.progress();
        let completed = session.handle.drain_completed();
        if !completed.is_empty() {
            let rules = &mut session.rules;
            session
                .handle
                .read_tree(|tree| rules.on_completed(tree, &completed));
            let found: Vec<Recommendation> = session
                .rules
                .found()
                .into_iter()
                .filter(|rec| rec.size >= crate::state::results::MIN_REC_SIZE)
                .collect();
            if found.len() != session.found.len() {
                session.found_status = found
                    .iter()
                    .flat_map(|rec| rec.nodes.iter().map(move |node| (*node, rec.status)))
                    .collect();
                session.found = found;
            }
        }
        session.generation += 1;
        let finished = session.handle.is_finished();
        if finished {
            self.finish_scan(cx);
        }
        cx.notify();
        finished
    }

    /// The scan is done: keep its tree, evaluate it in the background, save it, and show the
    /// results a moment later.
    fn finish_scan(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.scan.as_mut() else {
            return;
        };
        if session.progress.cancelled {
            log::info!("scan cancelled");
            self.scan = None;
            self.screen = Screen::Home;
            return;
        }
        let Some(tree) = session.handle.take_tree() else {
            return;
        };
        session.finished = true;
        let tree = Arc::new(tree);
        session.tree = Some(tree.clone());
        let progress = session.progress.clone();
        log::info!(
            "scan finished: {} files, {} dirs, {}, {} errors in {:.2} s",
            progress.files,
            progress.dirs,
            vano_core::size::format_plain(progress.bytes),
            progress.errors,
            progress.elapsed.as_secs_f64(),
        );
        let target = session.target.clone();
        let ctx = self.rules_ctx.clone();
        let started = Instant::now();
        let evaluate = cx.background_executor().spawn({
            let tree = tree.clone();
            async move {
                let categories = vano_rules::categorize(&tree, &ctx);
                let recs = vano_rules::evaluate(&tree, &ctx);
                (categories, recs)
            }
        });
        let task = cx.spawn(async move |view, cx| {
            let (categories, recs) = evaluate.await;
            log::debug!("rules evaluated in {:?}", started.elapsed());
            let delay = RESULTS_DELAY.saturating_sub(started.elapsed());
            cx.background_executor().timer(delay).await;
            let _ = view.update(cx, |view, cx| {
                view.show_scan_results(target, categories, recs, progress, cx);
            });
        });
        self._results_timer = Some(task);
        let _ = tree;
    }

    /// Builds the results of the finished scan and switches to them.
    fn show_scan_results(
        &mut self,
        target: Target,
        categories: Vec<vano_core::Category>,
        recs: Vec<Recommendation>,
        progress: Progress,
        cx: &mut Context<Self>,
    ) {
        let Some(session) = self.scan.take() else {
            return;
        };
        let Some(tree) = session.tree else {
            return;
        };
        let tree = Arc::try_unwrap(tree).unwrap_or_else(|shared| (*shared).clone());
        let scanned_at = format::now();
        let seconds = progress.elapsed.as_secs_f64();
        let recent = Recent {
            target: target.clone(),
            size: tree.node(ROOT).size,
            files: u64::from(tree.node(ROOT).files),
            scanned_at,
            seconds,
            cache: recents::cache_name(&target),
        };
        // Saving the tree runs in the background: a few tens of milliseconds for millions of
        // nodes, but never on the UI thread.
        if let Some(path) = recent.cache_path() {
            let bytes = vano_core::persist::encode(&tree);
            cx.background_executor()
                .spawn(async move {
                    if let Some(parent) = path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Err(error) = std::fs::write(&path, bytes) {
                        log::warn!("could not save the scan to {}: {error}", path.display());
                    }
                })
                .detach();
        }
        self.recents = recents::record(recent);
        self.results = Some(Results::with(target, tree, categories, recs, seconds));
        self.open_results_screen(cx);
    }

    fn open_results_screen(&mut self, cx: &mut Context<Self>) {
        self.explorer.focus = ROOT;
        // The results keep the view the scan was showing.
        self.explorer.view = self.live_view;
        self.explorer.views_key = None;
        self.explorer.generation += 1;
        self.explorer.treemap.laid_out = None;
        self.explorer.treemap.pending = None;
        // The explorer starts from where the live treemap left its tiles, so the results grow
        // out of the scan instead of popping in.
        self.explorer.treemap.motion =
            std::mem::replace(&mut self.live_treemap.motion, Motion::new(LIVE_SPRING));
        self.explorer.treemap.motion.set_spring(SPRING);
        // Same area as the live treemap, so the first frame of the results is already laid out
        // for it, with the results' data.
        if self.explorer.treemap.bounds.get().is_none() {
            self.explorer
                .treemap
                .bounds
                .set(self.live_treemap.bounds.get());
        }
        self.detail = None;
        self.screen = Screen::Results;
        self.disk =
            vano_scan::root_filesystem_usage(&crate::state::target::system_root()).or(self.disk);
        cx.notify();
    }

    /// Opens a remembered scan from its saved tree, or scans again when the tree is gone.
    pub(crate) fn open_recent(&mut self, recent: Recent, cx: &mut Context<Self>) {
        let Some(path) = recent.cache_path() else {
            self.target = recent.target;
            self.start_scan(cx);
            return;
        };
        let ctx = self.rules_ctx.clone();
        let load = cx.background_executor().spawn(async move {
            let tree = vano_scan::cache::load(&path).ok()?;
            let categories = vano_rules::categorize(&tree, &ctx);
            let recs = vano_rules::evaluate(&tree, &ctx);
            Some((tree, categories, recs))
        });
        cx.spawn(async move |view, cx| {
            let loaded = load.await;
            let _ = view.update(cx, |view, cx| match loaded {
                Some((tree, categories, recs)) => {
                    view.target = recent.target.clone();
                    view.results = Some(Results::with(
                        recent.target,
                        tree,
                        categories,
                        recs,
                        recent.seconds,
                    ));
                    view.explorer.treemap.motion.clear();
                    view.open_results_screen(cx);
                }
                None => {
                    view.target = recent.target;
                    view.start_scan(cx);
                }
            });
        })
        .detach();
    }

    /// Stops the running scan and goes back home.
    pub(crate) fn cancel_scan(&mut self, cx: &mut Context<Self>) {
        if let Some(session) = self.scan.take() {
            session.handle.cancel();
        }
        self._results_timer = None;
        self.screen = Screen::Home;
        cx.notify();
    }

    /// The finished scan's "View results" button: no need to wait for the delay.
    pub(crate) fn view_results_now(&mut self, cx: &mut Context<Self>) {
        let ready = self.scan.as_ref().is_some_and(|session| session.finished);
        if !ready {
            return;
        }
        // The evaluation task shows the results when it is done; it only waits for the delay
        // when it is faster than it, so nothing is lost by switching right away when it ended.
        if let Some(task) = self._results_timer.take() {
            cx.spawn(async move |_, _| task.await).detach();
        }
    }

    pub(crate) fn go_home(&mut self, cx: &mut Context<Self>) {
        if self.screen == Screen::Scanning {
            self.cancel_scan(cx);
            return;
        }
        self.screen = Screen::Home;
        self.detail = None;
        cx.notify();
    }

    /// Shows a short message at the bottom of the window.
    pub(crate) fn show_toast(
        &mut self,
        text: impl Into<SharedString>,
        undo: Option<TrashBatch>,
        error: bool,
        cx: &mut Context<Self>,
    ) {
        let hide = cx.spawn(async move |view, cx| {
            cx.background_executor().timer(TOAST_DURATION).await;
            let _ = view.update(cx, |view, cx| {
                view.toast = None;
                cx.notify();
            });
        });
        self.toast = Some(Toast {
            text: text.into(),
            undo,
            error,
            _hide: hide,
        });
        cx.notify();
    }

    pub(crate) fn close_toast(&mut self, cx: &mut Context<Self>) {
        self.toast = None;
        cx.notify();
    }

    /// Moves what `request` names to the Trash, in the background.
    pub(crate) fn trash(
        &mut self,
        request: TrashRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(results) = self.results.as_ref() else {
            return;
        };
        if self.trashing {
            return;
        }
        let mut nodes: Vec<NodeId> = request
            .rules
            .iter()
            .filter_map(|rule| results.rec(rule))
            .flat_map(|rec| rec.nodes.iter().copied())
            .chain(request.nodes.iter().copied())
            .filter(|node| *node != ROOT && !results.tree.is_removed(*node))
            .collect();
        nodes.sort_unstable();
        nodes.dedup();
        // A node inside another one of the batch goes with its ancestor.
        let all = nodes.clone();
        nodes.retain(|node| {
            !all.iter()
                .any(|other| other != node && results.tree.is_within(*node, *other))
        });
        if nodes.is_empty() {
            return;
        }
        let paths: Vec<PathBuf> = nodes.iter().map(|node| results.tree.path(*node)).collect();
        let sizes: Vec<u64> = nodes
            .iter()
            .map(|node| results.tree.node(*node).size)
            .collect();
        let size: u64 = sizes.iter().sum();
        let count = request.rules.len() + request.nodes.len();
        let progress = Arc::new(trash::TrashProgress::new(paths.len()));
        self.trashing = true;
        self.trash_progress = Some(progress.clone());
        cx.notify();
        let at = trash::now();
        let batch = TrashBatch {
            nodes,
            paths: paths.clone(),
            size,
            count,
            at,
        };
        log::info!("moving {} items ({} bytes) to the Trash", paths.len(), size);
        let shared = progress.clone();
        let job = cx.background_executor().spawn(async move {
            let outcome = trash::move_each(&paths, &shared.cancel, |index| {
                shared.done.fetch_add(1, Ordering::Relaxed);
                shared.bytes.fetch_add(sizes[index], Ordering::Relaxed);
            });
            shared.finished.store(true, Ordering::Relaxed);
            outcome
        });
        cx.spawn_in(window, async move |view, cx| {
            // Repaint the status bar's count while the job runs, a few times a second.
            while !progress.finished.load(Ordering::Relaxed) {
                cx.background_executor().timer(POLL_INTERVAL).await;
                if view.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
            let outcome = job.await;
            let _ = view.update_in(cx, |view, window, cx| {
                view.finish_trash(batch, outcome, window, cx);
            });
        })
        .detach();
    }

    /// Stops the batch being moved before its next item.
    pub(crate) fn cancel_trash(&mut self, cx: &mut Context<Self>) {
        if let Some(progress) = &self.trash_progress {
            progress.cancel.store(true, Ordering::Relaxed);
            cx.notify();
        }
    }

    fn finish_trash(
        &mut self,
        batch: TrashBatch,
        outcome: trash::Outcome,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.trashing = false;
        self.trash_progress = None;
        let home = self.home.clone();
        let Some(results) = self.results.as_mut() else {
            return;
        };
        for (index, error) in &outcome.failed {
            log::warn!(
                "could not move {} to the Trash: {error:?}",
                batch.paths[*index].display()
            );
        }
        let left: Vec<LeftItem> = outcome
            .failed
            .iter()
            .map(|(index, error)| LeftItem {
                name: results.tree.display_name(batch.nodes[*index]),
                path: format::short_path(&batch.paths[*index], home.as_deref()),
                size: results.tree.node(batch.nodes[*index]).size,
                reason: match error {
                    trash::TrashError::Refused => rust_i18n::t!("report.refused").into_owned(),
                    trash::TrashError::Os(text) => text.clone(),
                },
            })
            .collect();
        let total = batch.nodes.len();
        // Only what left its place: everything else stays in the tree.
        let complete = outcome.moved.len() == batch.nodes.len();
        let moved = if complete {
            batch
        } else {
            let nodes: Vec<NodeId> = outcome
                .moved
                .iter()
                .map(|index| batch.nodes[*index])
                .collect();
            TrashBatch {
                size: nodes.iter().map(|node| results.tree.node(*node).size).sum(),
                count: nodes.len(),
                paths: outcome
                    .moved
                    .iter()
                    .map(|index| batch.paths[*index].clone())
                    .collect(),
                nodes,
                at: batch.at,
            }
        };
        if !moved.nodes.is_empty() {
            results.mark_trashed(&moved.nodes);
            self.explorer.generation += 1;
            if let Some(Detail::Node(node)) = self.detail
                && results.tree.is_removed(node)
            {
                self.detail = None;
            }
            if let Some(Detail::Rec(rule)) = self.detail
                && results.removed.contains(rule)
            {
                self.detail = None;
            }
            // Zoomed into something that is gone: back to the nearest folder still there.
            while self.explorer.focus != ROOT && results.tree.is_removed(self.explorer.focus) {
                self.explorer.focus = results.tree.node(self.explorer.focus).parent;
            }
            // Read again rather than added: the Trash is on the same disk, so the space only
            // comes back when the user empties it.
            results.fs = vano_scan::root_filesystem_usage(&results.target.path);
            self.disk = vano_scan::root_filesystem_usage(&crate::state::target::system_root());
        }
        let (count, items, freed) = (moved.count, moved.nodes.len(), moved.size);
        let undo = (trash::can_restore() && items > 0).then_some(moved);
        if left.is_empty() && !outcome.cancelled {
            let text = format!(
                "{} \u{b7} {}",
                format::counted("toast.moved", count as u64),
                rust_i18n::t!("toast.freed", size = format::size(freed)),
            );
            self.show_toast(text, undo, false, cx);
        } else {
            // Something needs explaining: what moved, what stayed and why.
            let report = TrashReport {
                moved: items,
                freed,
                total,
                left,
                undo,
            };
            self.open_trash_report(report, window, cx);
        }
        cx.notify();
    }

    /// The toast's Undo: puts the last batch back from the Trash.
    pub(crate) fn undo_trash(&mut self, cx: &mut Context<Self>) {
        let Some(batch) = self.toast.take().and_then(|toast| toast.undo) else {
            return;
        };
        self.restore(batch, cx);
    }

    /// Puts `batch` back from the Trash, in the background.
    pub(crate) fn restore(&mut self, batch: TrashBatch, cx: &mut Context<Self>) {
        if self.trashing {
            return;
        }
        self.trashing = true;
        cx.notify();
        let paths = batch.paths.clone();
        let since = batch.at;
        let job = cx
            .background_executor()
            .spawn(async move { trash::restore(&paths, since) });
        cx.spawn(async move |view, cx| {
            let outcome = job.await;
            let _ = view.update(cx, |view, cx| {
                view.trashing = false;
                match outcome {
                    Ok(()) => {
                        if let Some(results) = view.results.as_mut() {
                            results.mark_restored(&batch.nodes);
                        }
                        if let Some(results) = view.results.as_mut() {
                            results.fs = vano_scan::root_filesystem_usage(&results.target.path);
                        }
                        view.disk =
                            vano_scan::root_filesystem_usage(&crate::state::target::system_root());
                        view.explorer.generation += 1;
                        let text = format::counted("toast.restored", batch.count as u64);
                        view.show_toast(text, None, false, cx);
                    }
                    Err(error) => {
                        log::warn!("could not restore from the Trash: {error}");
                        let text =
                            rust_i18n::t!("toast.restore_failed", error = error).into_owned();
                        view.show_toast(text, None, true, cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn on_open_settings(&mut self, _: &OpenSettings, window: &mut Window, cx: &mut Context<Self>) {
        self.open_settings(window, cx);
    }

    fn on_start_scan(&mut self, _: &StartScan, _window: &mut Window, cx: &mut Context<Self>) {
        if self.screen == Screen::Home {
            self.start_scan(cx);
        }
    }

    fn on_choose_folder(&mut self, _: &ChooseFolder, _: &mut Window, cx: &mut Context<Self>) {
        if self.screen == Screen::Home {
            self.choose_folder(cx);
        }
    }

    fn on_zoom_out(&mut self, _: &ZoomOut, _window: &mut Window, cx: &mut Context<Self>) {
        if self.screen == Screen::Results {
            self.zoom_out(cx);
        }
    }

    fn on_close_detail(&mut self, _: &CloseDetail, _window: &mut Window, cx: &mut Context<Self>) {
        if self.detail.take().is_some() {
            cx.notify();
        } else if self.screen == Screen::Results && self.explorer.focus != ROOT {
            self.zoom_out(cx);
        }
    }

    /// Advances `treemap`'s springs to now and asks for another frame while they move.
    pub(crate) fn tick(treemap: &mut TreemapState, window: &mut Window, reduce_motion: bool) {
        let now = Instant::now();
        let dt = treemap
            .last_frame
            .map(|last| now.duration_since(last).as_secs_f32())
            .unwrap_or(1.0 / 60.0);
        if reduce_motion {
            treemap.motion.snap();
            treemap.last_frame = None;
            return;
        }
        if treemap.motion.step(dt) {
            treemap.last_frame = Some(now);
            let (start, frames) = treemap.animation.get_or_insert((now, 0));
            *frames += 1;
            let _ = start;
            window.request_animation_frame();
        } else {
            treemap.last_frame = None;
            if let Some((start, frames)) = treemap.animation.take() {
                let elapsed = now.duration_since(start);
                log::debug!(
                    "treemap animation: {frames} frames in {:.0} ms ({:.0} fps)",
                    elapsed.as_secs_f64() * 1000.0,
                    f64::from(frames) / elapsed.as_secs_f64().max(0.001),
                );
            }
        }
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        log::trace!("render");
        let palette = cx.palette();
        let weak = cx.weak_entity();
        let title_bar = self.render_title_bar(weak, window, cx);
        let body = match self.screen {
            Screen::Home => self.render_home(window, cx),
            Screen::Scanning => self.render_scanning(window, cx),
            Screen::Results => self.render_results(window, cx),
        };
        let status_bar = self.render_status_bar(cx);
        let toast = self.render_toast(cx);

        v_flex()
            .size_full()
            .bg(palette.bg)
            .text_color(palette.fg)
            .font_family(cx.theme().font_family.clone())
            .key_context("Vano")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::on_open_settings))
            .on_action(cx.listener(Self::on_start_scan))
            .on_action(cx.listener(Self::on_choose_folder))
            .on_action(cx.listener(Self::on_zoom_out))
            .on_action(cx.listener(Self::on_close_detail))
            .child(title_bar)
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .relative()
                    .child(body)
                    .children(toast),
            )
            .child(status_bar)
            .when(window.has_active_dialog(cx), |this| {
                this.relative()
                    .child(div().absolute().inset_0().bg(palette.scrim))
            })
    }
}
