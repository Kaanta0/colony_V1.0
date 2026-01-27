mod config;
mod scan;

use std::{rc::Rc, sync::Arc};

use anyhow::Result;
use slint::{ModelRc, SharedString, VecModel};

slint::include_modules!();

fn main() -> Result<()> {
    let config = Arc::new(config::Config::load()?);
    let app = AppWindow::new()?;

    let repositories = scan::scan_repositories(&config)?;
    let initial_count = repositories.len();
    app.set_repositories(to_model(repositories));
    app.set_status_message(format!("{initial_count} repo(s) détecté(s)").into());
    app.set_scan_paths(to_model(
        config
            .scan
            .directories
            .iter()
            .map(|path| path.to_string().into())
            .collect::<Vec<_>>(),
    ));

    let app_handle = app.as_weak();
    let config_handle = Arc::clone(&config);
    app.on_rescan(move || {
        let config = Arc::clone(&config_handle);
        let Some(app_handle) = app_handle.upgrade() else {
            return;
        };
        match scan::scan_repositories(&config) {
            Ok(repositories) => {
                let count = repositories.len();
                app_handle.set_repositories(to_model(repositories));
                app_handle.set_status_message(format!("{count} repo(s) détecté(s)").into());
            }
            Err(error) => {
                app_handle.set_status_message(format!("Erreur: {error}").into());
            }
        }
    });

    app.run()?;
    Ok(())
}

fn to_model(items: Vec<SharedString>) -> ModelRc<SharedString> {
    ModelRc::from(Rc::new(VecModel::from(items)))
}
