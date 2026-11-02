mod auth;
mod commands;
mod download;
mod drive;
mod error;
mod fsutil;
mod state;
mod transfer;
mod upload;
mod vault;

#[cfg(test)]
mod testutil;

use tauri::Manager;

pub fn run() {
    drive::init_tls();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let state = state::AppState::new(app.handle())?;
            app.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::accounts_list,
            commands::account_add,
            commands::account_add_from_files,
            commands::account_update,
            commands::account_remove,
            commands::account_export_key,
            commands::vault_reset,
            commands::drive_about,
            commands::drive_shared_drives,
            commands::drive_list,
            commands::drive_create_folder,
            commands::drive_rename,
            commands::drive_move,
            commands::drive_trash,
            commands::drive_restore,
            commands::drive_delete,
            commands::drive_empty_trash,
            commands::drive_permissions,
            commands::drive_share_user,
            commands::drive_share_anyone,
            commands::drive_unshare,
            commands::transfer_upload,
            commands::transfer_download,
            commands::transfer_list,
            commands::transfer_cancel,
            commands::transfer_cancel_all,
            commands::transfer_retry,
            commands::transfer_clear_finished,
        ])
        .run(tauri::generate_context!())
        .expect("không khởi động được ứng dụng Tauri");
}
