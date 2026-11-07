mod api;
mod lines;
mod media;
mod net;
mod network_policy;
mod session;

use api::Api;
use tauri::Manager;

fn allowed_frontend_origin(origin: &str) -> bool {
    if matches!(
        origin,
        "tauri://localhost" | "http://tauri.localhost" | "https://tauri.localhost"
    ) {
        return true;
    }
    // 移动端开发可能使用 TAURI_DEV_HOST，发布版不允许这些来源。
    cfg!(debug_assertions)
        && url::Url::parse(origin).is_ok_and(|u| u.scheme() == "http" && u.port() == Some(1420))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Api::new())
        .setup(|app| {
            if let Ok(dir) = app.path().app_data_dir() {
                let _ = std::fs::create_dir_all(&dir);
                // 上次选择的线路（设置 → 线路）在启动时恢复
                net::bind_base(&dir);
                let state: tauri::State<Api> = app.state();
                state.session.bind(dir);
            }
            Ok(())
        })
        .plugin(
            tauri::plugin::Builder::<_, ()>::new("navigation-guard")
                .on_navigation(|_webview, url| {
                    (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
                        || allowed_frontend_origin(&url.origin().ascii_serialization())
                })
                .build(),
        )
        .register_asynchronous_uri_scheme_protocol("hg", |_app, request, responder| {
            let request_origin = request
                .headers()
                .get("Origin")
                .and_then(|v| v.to_str().ok())
                .map(str::to_string);
            if request_origin
                .as_deref()
                .is_some_and(|origin| !allowed_frontend_origin(origin))
            {
                responder.respond(
                    tauri::http::Response::builder()
                        .status(403)
                        .body(b"origin denied".to_vec())
                        .unwrap(),
                );
                return;
            }
            let uri = request.uri().clone();
            let path = uri.path().to_string();
            let query = uri.query().unwrap_or("").to_string();
            // 自定义协议基址统一定义在 media::LOCAL_ORIGIN（Android 是
            // http://hg.localhost），它同时是 m3u8 分片/key 改写用的前缀，
            // 必须与前端 src/api.ts 的 LOCAL 一致。
            tauri::async_runtime::spawn(async move {
                let mut resp = media::handle(&path, &query, media::LOCAL_ORIGIN).await;
                if let Some(origin) =
                    request_origin.and_then(|s| tauri::http::HeaderValue::from_str(&s).ok())
                {
                    resp.headers_mut()
                        .insert("Access-Control-Allow-Origin", origin);
                    resp.headers_mut()
                        .insert("Vary", tauri::http::HeaderValue::from_static("Origin"));
                }
                responder.respond(resp);
            });
        })
        .invoke_handler(tauri::generate_handler![
            api::app_boot,
            api::debug_log,
            api::net_diag,
            api::line_state,
            api::line_set,
            api::line_mirrors,
            api::line_probe,
            api::check_update,
            api::open_url,
            api::api_login,
            api::api_register,
            api::api_logout,
            api::api_me,
            api::api_feed,
            api::api_category,
            api::api_rank,
            api::api_search,
            api::cache_clear,
            api::api_posts,
            api::api_post,
            api::api_post_like,
            api::api_post_collect,
            api::api_post_comments,
            api::api_video,
            api::api_play,
            api::api_comments,
            api::api_comment_send,
            api::api_like,
            api::api_collect,
            api::api_follow,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
