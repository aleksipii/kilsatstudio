//! Bounded local file operations, never performed on the UI thread.
use motion_project::{AssetKind, ProjectDocument};
use std::{path::PathBuf, sync::mpsc};
pub enum Operation {
    Import {
        source: PathBuf,
        id: String,
        kind: AssetKind,
    },
    Relink {
        id: String,
        path: String,
    },
    Remove {
        id: String,
    },
}
pub struct Task {
    pub project_path: PathBuf,
    pub document: ProjectDocument,
    pub operation: Operation,
}
pub struct Worker {
    tx: mpsc::SyncSender<Task>,
    rx: mpsc::Receiver<Result<ProjectDocument, String>>,
}
impl Worker {
    pub fn new() -> Self {
        let (tx, tasks) = mpsc::sync_channel::<Task>(1);
        let (results, rx) = mpsc::sync_channel(1);
        std::thread::spawn(move || {
            while let Ok(mut task) = tasks.recv() {
                let result = match task.operation {
                    Operation::Import { source, id, kind } => motion_project::import_asset(
                        &task.project_path,
                        &mut task.document,
                        &source,
                        &id,
                        kind,
                    )
                    .map(|_| ()),
                    Operation::Relink { id, path } => motion_project::relink_asset(
                        &task.project_path,
                        &mut task.document,
                        &id,
                        &path,
                    ),
                    Operation::Remove { id } => {
                        motion_project::remove_asset(&mut task.document, &id).map(|_| ())
                    }
                }
                .map(|()| task.document)
                .map_err(|e| e.to_string());
                if results.send(result).is_err() {
                    break;
                }
            }
        });
        Self { tx, rx }
    }
    pub fn start(&self, task: Task) -> Result<(), String> {
        self.tx.try_send(task).map_err(|e| e.to_string())
    }
    pub fn poll(&self) -> Option<Result<ProjectDocument, String>> {
        self.rx.try_recv().ok()
    }
}
