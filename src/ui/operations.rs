use super::*;
use spacefinder::actions::{self, RecycleRequest, ShellAction};

#[derive(Clone, Copy)]
pub(super) enum ItemAction {
    Open,
    Parent,
    Reveal,
    Properties,
    CopyName,
    CopyPath,
    CopyParent,
    CopySize,
    Focus,
    Toggle,
    Analyse,
    Recycle,
}

pub(super) enum OperationDone {
    Shell,
    Elevated,
    Recycled { root: PathBuf, restore: PathBuf },
}
pub(super) struct Operation {
    pub label: String,
    pub result: Receiver<Result<OperationDone, String>>,
}
pub(super) struct DeleteDialog {
    request: RecycleRequest,
    name: String,
    typed: String,
    bytes: u64,
    files: u64,
    root: PathBuf,
}

impl SpaceFinder {
    pub(super) fn start_operation(
        &mut self,
        ctx: &egui::Context,
        label: &str,
        task: impl FnOnce() -> Result<OperationDone, String> + Send + 'static,
    ) {
        if self.operation.is_some() {
            return;
        }
        let (tx, rx) = crossbeam_channel::bounded(1);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(task))
                .unwrap_or_else(
                    |_| Err("Windows could not complete the action. Try again.".into()),
                );
            let _ = tx.send(result);
            ctx.request_repaint();
        });
        self.operation = Some(Operation {
            label: label.into(),
            result: rx,
        });
    }

    pub(super) fn poll_operation(&mut self, ctx: &egui::Context) {
        let result = self
            .operation
            .as_ref()
            .and_then(|job| job.result.try_recv().ok());
        if let Some(result) = result {
            self.operation = None;
            match result {
                Ok(OperationDone::Shell) => {}
                Ok(OperationDone::Elevated) => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                Ok(OperationDone::Recycled { root, restore }) => {
                    self.path = platform::display_path(&root);
                    self.start();
                    self.resume_after_scan = Some(restore);
                    self.notice = Some("Item moved to the Recycle Bin. Updating the scan… Free space increases only after the Recycle Bin is emptied.".into());
                }
                Err(error) => self.notice = Some(format!("Action not completed: {error}")),
            }
        }
    }

    pub(super) fn elevate(&mut self, ctx: &egui::Context) {
        if self.elevated || self.operation.is_some() || self.events.is_some() {
            return;
        }
        let path = self
            .scan
            .as_ref()
            .map_or_else(|| PathBuf::from(&self.path), |s| s.root.clone());
        self.start_operation(ctx, "Waiting for Windows authorization…", move || {
            actions::elevate(&path)?;
            Ok(OperationDone::Elevated)
        });
    }

    pub(super) fn item_menu(&self, ui: &mut egui::Ui, index: usize) -> Option<ItemAction> {
        let scan = self.scan.as_ref()?;
        let node = &scan.nodes[index];
        let mut action = None;
        ui.set_min_width(232.0);
        ui.set_max_width(340.0);
        ui.add(egui::Label::new(RichText::new(node.name.to_string_lossy()).strong()).truncate());
        ui.separator();
        let idle =
            self.operation.is_none() && self.events.is_none() && self.delete_dialog.is_none();
        for (label, value) in [
            ("Open", ItemAction::Open),
            ("Show in File Explorer", ItemAction::Reveal),
            ("Open parent folder", ItemAction::Parent),
            ("Windows properties", ItemAction::Properties),
        ] {
            if ui.add_enabled(idle, egui::Button::new(label)).clicked() {
                action = Some(value);
            }
        }
        ui.separator();
        if node.directory {
            if ui.button("Focus this folder in the tree").clicked() {
                action = Some(ItemAction::Focus);
            }
            if ui.button("Expand / collapse").clicked() {
                action = Some(ItemAction::Toggle);
            }
            if ui
                .add_enabled(idle, egui::Button::new("Rescan this folder"))
                .clicked()
            {
                action = Some(ItemAction::Analyse);
            }
            ui.separator();
        }
        ui.menu_button("Copy", |ui| {
            for (label, value) in [
                ("Name", ItemAction::CopyName),
                ("Full path", ItemAction::CopyPath),
                ("Parent folder path", ItemAction::CopyParent),
                ("Size in bytes", ItemAction::CopySize),
            ] {
                if ui.button(label).clicked() {
                    action = Some(value);
                }
            }
        });
        ui.separator();
        let delete = ui.add_enabled(
            idle && index != 0,
            egui::Button::new(
                RichText::new("Move to Recycle Bin…").color(Color32::from_rgb(246, 153, 150)),
            ),
        );
        if index == 0 {
            delete.on_hover_text(
                "The scan root is protected. Scan the parent folder to manage this item.",
            );
        } else if delete.clicked() {
            action = Some(ItemAction::Recycle);
        }
        if action.is_some() {
            ui.close();
        }
        action
    }

    pub(super) fn execute_item(&mut self, ctx: &egui::Context, index: usize, action: ItemAction) {
        let Some(scan) = self.scan.clone() else {
            return;
        };
        let path = scan.path(index);
        match action {
            ItemAction::CopyName => {
                ctx.copy_text(scan.nodes[index].name.to_string_lossy().into_owned())
            }
            ItemAction::CopyPath => ctx.copy_text(platform::display_path(&path)),
            ItemAction::CopyParent => {
                ctx.copy_text(platform::display_path(path.parent().unwrap_or(&path)))
            }
            ItemAction::CopySize => ctx.copy_text(scan.nodes[index].bytes.to_string()),
            ItemAction::Focus => self.focus(index, true),
            ItemAction::Toggle => {
                if self.browser.query.is_empty() && self.browser.extension.is_none() {
                    if !self.browser.expanded.remove(&index) {
                        self.browser.expanded.insert(index);
                    }
                    self.refresh();
                } else {
                    self.notice = Some("Clear the search to collapse branches.".into());
                }
            }
            ItemAction::Analyse => {
                self.path = platform::display_path(&path);
                self.start();
            }
            ItemAction::Recycle => {
                if self.operation.is_some() || self.events.is_some() {
                    return;
                }
                let prepared = (|| {
                    let metadata = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
                    let current = platform::metadata_details(&metadata);
                    let node = &scan.nodes[index];
                    if current.created != node.details.created
                        || (!node.directory
                            && (metadata.len() != node.bytes
                                || current.modified != node.details.modified))
                    {
                        return Err(
                            "The item has changed since scanning. Rescan before deleting.".into(),
                        );
                    }
                    RecycleRequest::prepare(&scan.root, &path)
                })();
                match prepared {
                    Ok(request) => {
                        self.delete_dialog = Some(DeleteDialog {
                            request,
                            name: scan.nodes[index].name.to_string_lossy().into_owned(),
                            typed: String::new(),
                            bytes: scan.nodes[index].bytes,
                            files: scan.nodes[index].files,
                            root: scan.root.clone(),
                        })
                    }
                    Err(error) => self.notice = Some(error),
                }
            }
            ItemAction::Open | ItemAction::Parent | ItemAction::Reveal | ItemAction::Properties => {
                let (action, path) = match action {
                    ItemAction::Open => (ShellAction::Open, path),
                    ItemAction::Parent => (
                        ShellAction::Open,
                        path.parent().unwrap_or(&path).to_path_buf(),
                    ),
                    ItemAction::Reveal => (ShellAction::Reveal, path),
                    _ => (ShellAction::Properties, path),
                };
                self.start_operation(ctx, "Opening in Windows…", move || {
                    actions::shell(action, &path)?;
                    Ok(OperationDone::Shell)
                });
            }
        }
        ctx.request_repaint();
    }

    pub(super) fn confirmation(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.delete_dialog.take() else {
            return;
        };
        let mut confirm = false;
        let mut cancel = false;
        let response = egui::Modal::new(egui::Id::new("recycle-confirmation")).show(ctx, |ui| {
            ui.set_width(480.0);
            ui.heading("Move to Recycle Bin?"); ui.add_space(12.0);
            property(ui, "Selected item", &dialog.name);
            property(ui, "Location", platform::display_path(dialog.request.path()));
            ui.label(format!("{} · {} files in the last scan", human_size(dialog.bytes), count(dialog.files)));
            ui.add_space(8.0);
            ui.label("You can restore the item from the Windows Recycle Bin. Free space may not increase until you empty it.");
            if dialog.request.directory() {
                ui.add_space(12.0);
                ui.colored_label(COLORS[3], "The folder and all its contents will be moved to the Recycle Bin.");
                ui.label("Type the folder name to confirm:");
                ui.add(egui::TextEdit::singleline(&mut dialog.typed).desired_width(f32::INFINITY).hint_text(&dialog.name));
            }
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() { cancel = true; }
                let ready = !dialog.request.directory() || dialog.typed == dialog.name;
                if ui.add_enabled(ready, egui::Button::new(RichText::new("Move to Recycle Bin").color(BG)).fill(Color32::from_rgb(238, 157, 141))).clicked() { confirm = true; }
            });
        });
        if confirm {
            let restore = dialog.request.path().parent().unwrap().to_path_buf();
            self.start_operation(ctx, "Moving to the Recycle Bin…", move || {
                dialog.request.recycle()?;
                Ok(OperationDone::Recycled {
                    root: dialog.root,
                    restore,
                })
            });
        } else if !cancel && !response.should_close() {
            self.delete_dialog = Some(dialog);
        }
    }
}
