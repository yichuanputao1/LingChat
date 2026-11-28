//! 插件内嵌面板：把插件目录里的静态网页挂到回环地址，供主界面 iframe 加载。
//!
//! 只服务 manifest `[[panels]]` 声明的目录，插件禁用或删除后立即 404；
//! 页面拿不到宿主前端与 Tauri IPC，游戏状态由前端通过 postMessage 推送。

use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    extract::{Path as AxumPath, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use crate::PluginManager;

/// 面板服务的监听端口。进程内只起一次，首次打开面板时延迟启动。
static PORT: std::sync::OnceLock<Mutex<Option<u16>>> = std::sync::OnceLock::new();

const INDEX_FILE: &str = "index.html";
const MAX_PATH_CHARS: usize = 512;

fn port_slot() -> &'static Mutex<Option<u16>> {
    PORT.get_or_init(|| Mutex::new(None))
}

/// 把 URL 里的剩余路径拆成安全的相对路径。
///
/// axum 已经做过百分号解码，这里再挡一次空段、`.`/`..`、反斜杠、盘符与控制字符，
/// 防止越出面板目录。
fn relative_path(rest: &str) -> Result<PathBuf, ()> {
    if rest.len() > MAX_PATH_CHARS || rest.contains('\\') {
        return Err(());
    }
    let mut out = PathBuf::new();
    for part in rest.split('/') {
        if part.is_empty() {
            continue;
        }
        if part == "." || part == ".." || part.contains(':') || part.chars().any(char::is_control) {
            return Err(());
        }
        out.push(part);
    }
    Ok(out)
}

fn not_found(message: String) -> Response {
    (StatusCode::NOT_FOUND, message).into_response()
}

/// 读文件并补上内容类型；所有响应都禁止缓存，方便插件改完页面直接刷新。
async fn file_response(path: PathBuf) -> Response {
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(error) => return not_found(format!("读取面板文件失败：{error}")),
    };
    let mime = mime_guess::from_path(&path).first_or_octet_stream();
    let content_type = HeaderValue::from_str(mime.as_ref())
        .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream"));
    let mut response = Response::new(Body::from(bytes));
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, content_type);
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    // 面板 iframe 是 opaque origin（sandbox 不带 allow-same-origin），页面里
    // fetch 相对路径会带 Origin: null，需要放行才能读自己的静态数据。
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    response
}

/// 静态资源的预检响应：面板可能带自定义请求头读自己的数据文件。
async fn preflight() -> Response {
    let mut response = StatusCode::NO_CONTENT.into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, HEAD, OPTIONS"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("*"),
    );
    response
}

async fn serve_panel(
    app: AppHandle,
    plugin_id: String,
    panel_id: String,
    rest: String,
) -> Response {
    let manager = app.state::<Arc<PluginManager>>().inner().clone();
    let base = match manager.panel_dir(&plugin_id, &panel_id).await {
        Ok(dir) => dir,
        Err(error) => return not_found(error),
    };
    let base = match tokio::fs::canonicalize(&base).await {
        Ok(dir) => dir,
        Err(error) => return not_found(format!("面板目录不可用：{error}")),
    };
    let relative = match relative_path(&rest) {
        Ok(path) => path,
        Err(()) => return not_found("面板路径不合法".to_string()),
    };

    let mut target = if relative.as_os_str().is_empty() {
        base.join(INDEX_FILE)
    } else {
        base.join(&relative)
    };
    if tokio::fs::metadata(&target)
        .await
        .is_ok_and(|meta| meta.is_dir())
    {
        target.push(INDEX_FILE);
    }

    let path = match tokio::fs::canonicalize(&target).await {
        Ok(path) if path.starts_with(&base) => path,
        // 前端路由地址没有扩展名时回落到 index.html；静态资源缺文件直接 404。
        _ if relative.extension().is_none() => base.join(INDEX_FILE),
        _ => return not_found("面板文件不存在".to_string()),
    };
    file_response(path).await
}

async fn serve_root(
    State(app): State<AppHandle>,
    AxumPath((plugin_id, panel_id)): AxumPath<(String, String)>,
) -> Response {
    serve_panel(app, plugin_id, panel_id, String::new()).await
}

async fn serve_route(
    State(app): State<AppHandle>,
    AxumPath((plugin_id, panel_id, rest)): AxumPath<(String, String, String)>,
) -> Response {
    serve_panel(app, plugin_id, panel_id, rest).await
}

/// 启动（或复用）回环静态服务，返回监听端口。
async fn ensure_server(app: &AppHandle) -> Result<u16, String> {
    let mut slot = port_slot().lock().await;
    if let Some(port) = *slot {
        return Ok(port);
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|error| format!("面板服务监听失败：{error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("读取面板服务端口失败：{error}"))?
        .port();
    let router = Router::new()
        .route(
            "/p/{plugin_id}/{panel_id}",
            get(serve_root).options(preflight),
        )
        .route(
            "/p/{plugin_id}/{panel_id}/{*rest}",
            get(serve_route).options(preflight),
        )
        .with_state(app.clone());
    tauri::async_runtime::spawn(async move {
        if let Err(error) = axum::serve(listener, router).await {
            tracing::warn!("插件面板服务退出: {error}");
        }
    });
    *slot = Some(port);
    Ok(port)
}

/// 打开面板前的准备：确认声明与入口文件存在，再返回可嵌入的地址。
pub async fn panel_url(app: &AppHandle, plugin_id: &str, panel_id: &str) -> Result<String, String> {
    let manager = app.state::<Arc<PluginManager>>().inner().clone();
    let dir = manager.panel_dir(plugin_id, panel_id).await?;
    if !tokio::fs::metadata(dir.join(INDEX_FILE))
        .await
        .is_ok_and(|meta| meta.is_file())
    {
        return Err(format!("面板目录缺少 {INDEX_FILE}：{}", dir.display()));
    }
    let port = ensure_server(app).await?;
    // 带上 index.html：面板里的相对路径（./style.css、./data.json）才能正确解析到
    // 同一个目录；catch-all 路由不匹配空路径，光给目录地址会 404。
    Ok(format!(
        "http://127.0.0.1:{port}/p/{plugin_id}/{panel_id}/index.html"
    ))
}
