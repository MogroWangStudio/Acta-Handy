//! 应用自更新：检查 GitHub Release → 下载当前平台的更新包 → 退出前把安装任务
//! 交给同一个二进制以 `--handy-apply-update <manifest>` 拉起的一次性安装进程。
//!
//! Release 资产统一为压缩包 `ActaHandy-<版本>-<平台>-<架构>.zip`：
//! - Windows：包内单个 `ActaHandy.exe`，落位时改回用户磁盘上的原文件名；
//! - macOS：包内单个 `ActaHandy-<版本>-<macos-<架构>>.dmg`，挂载后取出
//!   `.app` 整体替换。
//!
//! 安装进程不建窗口、不起 Tauri：它只等主进程退出、换包、启动新版本，然后
//! 退出自己。这样做是因为运行中的可执行文件（Windows）与正在运行的 .app
//! 所在目录都无法由自己替换——必须有一个「不是它」的进程来完成换包。
//!
//! 平台差异：
//! - Windows（便携版单 exe）：把运行中的 exe 改名为 `*.handy-old`（Windows
//!   允许改名运行中的映像），把解压出的新 exe 按原文件名放回原路径并启动；
//!   新版本下次启动时清理 `*.handy-old`。
//! - macOS：解压出 dmg 并挂载，替换 /Applications（或当前所在位置）里的
//!   `.app`，随后清除 quarantine 标记——优先用无密码的 `xattr -r -d`，仍带
//!   标记时弹终端运行 dmg 内自带的「修复启动损坏.command」（sudo 提权），等
//!   标记消失后再启动新版本。

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::cmp::Ordering;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, WebviewWindow};

/// 列出最近的 Release 而不是 /releases/latest：latest 端点永远只返回正式版，
/// 测试通道（beta）拿不到预发布版本。列表端点不含 draft，无需过滤。
const RELEASE_API: &str = "https://api.github.com/repos/MogroWangStudio/Acta-Handy/releases?per_page=30";
/// 测速样本兜底：Release 还没发过、或接口限流时，用仓库里一个固定存在的小
/// 文件量一下两条链路——测速测的是链路，不该被一次 403 拦死。
const PROBE_FALLBACK_URL: &str =
    "https://raw.githubusercontent.com/MogroWangStudio/Acta-Handy/main/package-lock.json";
/// gh-proxy.org 的用法是在原始下载链接前拼一段前缀（见其使用指南）。
const GH_PROXY_PREFIX: &str = "https://gh-proxy.org/";
const USER_AGENT: &str = concat!("ActaHandy-Updater/", env!("CARGO_PKG_VERSION"));
/// 安装进程的启动参数：`--handy-apply-update <manifest.json>`。
pub const APPLY_FLAG: &str = "--handy-apply-update";
const UPDATE_DIR_NAME: &str = "acta-handy-update";
const DOWNLOAD_EVENT: &str = "update://download";
/// macOS 更新包里随 dmg 附带的解锁命令文件（清 quarantine 用）。
#[cfg(target_os = "macos")]
const REPAIR_SCRIPT_NAME: &str = "修复启动损坏.command";
/// 更换 Windows exe 时对杀软/索引器短暂锁文件的容忍重试。
#[cfg(target_os = "windows")]
const LOCK_RETRY_TIMES: usize = 6;
#[cfg(target_os = "windows")]
const LOCK_RETRY_DELAY: Duration = Duration::from_millis(400);
/// 修复脚本需要用户在终端里输入密码，等待标记消失的窗口放宽一些。
#[cfg(target_os = "macos")]
const REPAIR_WAIT: Duration = Duration::from_secs(180);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub prerelease: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub notes: String,
    pub asset_name: String,
    pub size: u64,
    pub url: String,
    /// 是否预发布（测试通道）。前端用它提示「这是 beta 版本」。
    pub prerelease: bool,
}

/// 一条下载链路（GitHub Raw / GitHub-proxy）的测速结果。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RouteProbe {
    pub label: String,
    pub ok: bool,
    pub status: u16,
    pub bytes: u64,
    pub elapsed_ms: u64,
    /// KB/s（按实际读到的字节与总耗时计，含握手）。
    pub kbps: u64,
    /// 失败原因；成功时为空。前端按它区分「读不到数据」与「HTTP 非 2xx」。
    pub error: String,
}

#[derive(Debug, Deserialize)]
struct ReleasePayload {
    tag_name: String,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    body: String,
    #[serde(default)]
    assets: Vec<ReleaseAsset>,
}

#[derive(Debug, Clone, Deserialize)]
struct ReleaseAsset {
    name: String,
    size: u64,
    browser_download_url: String,
}

/// 主应用 → 安装进程的任务单。target 在主应用侧就解析好：
/// Windows 为 exe 自身路径，macOS 为应被替换的 .app 包目录。
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateManifest {
    pub version: String,
    pub from_version: String,
    pub payload: PathBuf,
    pub target: PathBuf,
    pub pid: u32,
}

fn update_work_dir() -> PathBuf {
    std::env::temp_dir().join(UPDATE_DIR_NAME)
}

fn log_line(message: &str) {
    let dir = update_work_dir();
    let _ = fs::create_dir_all(&dir);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    if let Ok(mut file) = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("install.log"))
    {
        let _ = writeln!(file, "[{stamp}] {message}");
    }
}

/// 解压更新压缩包，返回包内唯一载荷（ActaHandy.exe / *.dmg）的落盘路径。
/// Release 资产固定「一个 zip 装一个文件」；解出零个条目视为资产异常。
fn extract_payload(zip_path: &Path, work_dir: &Path) -> Result<PathBuf, String> {
    let file = fs::File::open(zip_path).map_err(|error| format!("无法打开更新包：{error}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| format!("更新包损坏：{error}"))?;
    let mut extracted: Option<PathBuf> = None;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| format!("更新包损坏：{error}"))?;
        if entry.is_dir() {
            continue;
        }
        // 条目名可能带路径前缀，只取文件名部分并过一遍安全检查。
        let raw_name = entry.name().to_string();
        let name = Path::new(&raw_name)
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or("更新包内文件名无效")?
            .to_string();
        let destination = work_dir.join(sanitize_asset_name(&name)?);
        let mut output = fs::File::create(&destination).map_err(|error| error.to_string())?;
        std::io::copy(&mut entry, &mut output).map_err(|error| format!("解压失败：{error}"))?;
        output.flush().map_err(|error| error.to_string())?;
        extracted = Some(destination);
    }
    extracted.ok_or_else(|| "更新包里没有找到可安装的文件".into())
}

// ---- 版本比较 ----

/// tag → (major, minor, patch, prerelease 段)。
/// 预发布判定与 CI 发布规则同源：去 v 前缀后含 '-' 即预发布（1.1.0-beta1）。
fn parse_release_tag(tag: &str) -> Option<((u64, u64, u64), Option<String>)> {
    let trimmed = tag.trim().trim_start_matches(['v', 'V']);
    let (core, pre) = match trimmed.split_once('-') {
        Some((core, pre)) => (core, Some(pre.split_once('+').map(|(p, _)| p).unwrap_or(pre).to_string())),
        None => {
            let core = trimmed.split_once('+').map(|(c, _)| c).unwrap_or(trimmed);
            (core, None)
        }
    };
    let mut parts = core.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next().unwrap_or("0").parse().ok()?;
    Some(((major, minor, patch), pre))
}

/// 预发布段内的大小比较：连续数字按数值比（beta10 > beta9），否则按字符串比。
/// semver 规定数字段小于字符串段（1 < alpha）。
fn compare_pre_tokens(a: &str, b: &str) -> Ordering {
    let numeric = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    match (numeric(a), numeric(b)) {
        (true, true) => a.parse::<u64>().unwrap_or(0).cmp(&b.parse::<u64>().unwrap_or(0)),
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => a.cmp(b),
    }
}

/// 把预发布段 token 化（"beta.2" → ["beta", "2"]，"beta2" → ["beta", "2"]）。
fn pre_tokens(pre: &str) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut buffer = String::new();
    let mut last_digit: Option<bool> = None;
    for c in pre.chars() {
        if c == '.' || c == '-' || c == '+' {
            if !buffer.is_empty() {
                tokens.push(std::mem::take(&mut buffer));
            }
            last_digit = None;
            continue;
        }
        let digit = c.is_ascii_digit();
        if last_digit.is_some_and(|d| d != digit) {
            tokens.push(std::mem::take(&mut buffer));
        }
        buffer.push(c);
        last_digit = Some(digit);
    }
    if !buffer.is_empty() {
        tokens.push(buffer);
    }
    tokens
}

/// 完整的 Release tag 比较（semver）：数字段逐位比；同版本号时正式版大于
/// 一切预发布；预发布段逐 token 比。用于在 Release 列表里挑出「最新的、
/// 比当前新的」版本——1.1.0-beta1 < 1.1.0-beta2 < 1.1.0 < 1.1.1。
fn compare_release_tags(a: &str, b: &str) -> Ordering {
    let (Some(((a_major, a_minor, a_patch), a_pre)), Some(((b_major, b_minor, b_patch), b_pre))) =
        (parse_release_tag(a), parse_release_tag(b))
    else {
        return Ordering::Equal;
    };
    let core = (a_major, a_minor, a_patch).cmp(&(b_major, b_minor, b_patch));
    if core != Ordering::Equal {
        return core;
    }
    match (a_pre, b_pre) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(a), Some(b)) => {
            let (tokens_a, tokens_b) = (pre_tokens(&a), pre_tokens(&b));
            for (ta, tb) in tokens_a.iter().zip(tokens_b.iter()) {
                let order = compare_pre_tokens(ta, tb);
                if order != Ordering::Equal {
                    return order;
                }
            }
            tokens_a.len().cmp(&tokens_b.len())
        }
    }
}

/// 当前平台对应的资产后缀，与 CI 的压缩包命名同源：
/// `ActaHandy-<版本>-windows-x64.zip` / `-macos-arm64.zip` / `-macos-x64.zip`。
fn asset_suffix() -> &'static str {
    if cfg!(target_os = "windows") {
        "-windows-x64.zip"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "-macos-arm64.zip"
    } else if cfg!(target_os = "macos") {
        "-macos-x64.zip"
    } else {
        ""
    }
}

fn release_asset_filter(name: &str) -> bool {
    let suffix = asset_suffix();
    !suffix.is_empty() && name.ends_with(suffix)
}

fn pick_asset<'a>(assets: &'a [ReleaseAsset], version: &str) -> Option<&'a ReleaseAsset> {
    // 优先取与 tag 版本同名的资产，退而求其次取该平台的任意一个。
    assets
        .iter()
        .find(|asset| release_asset_filter(&asset.name) && asset.name.contains(version))
        .or_else(|| assets.iter().find(|asset| release_asset_filter(&asset.name)))
}

/// 发布说明的预览：取正文前几行压成一句，太长就截断——关于页只有一行的位置。
fn notes_preview(body: &str) -> String {
    let condensed: Vec<&str> = body
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .take(3)
        .collect();
    let text = condensed.join("；");
    if text.chars().count() > 180 {
        format!("{}…", text.chars().take(180).collect::<String>())
    } else {
        text
    }
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(Duration::from_secs(10))
        .build()
        .map_err(|error| error.to_string())
}

/// 更新通道：stable 只收正式版，beta 连预发布一起收。
fn accepts_prerelease(channel: &str) -> bool {
    channel.eq_ignore_ascii_case("beta")
}

/// 下载源：github = 原生直链；proxy = 在下载链接前拼 gh-proxy 前缀。
fn resolve_download_url(raw: &str, source: &str) -> String {
    if source.eq_ignore_ascii_case("proxy") {
        format!("{GH_PROXY_PREFIX}{raw}")
    } else {
        raw.to_string()
    }
}

async fn fetch_releases() -> Result<Vec<ReleasePayload>, String> {
    let response = http_client()?
        .get(RELEASE_API)
        .header("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|error| format!("无法连接 GitHub：{error}"))?;
    if !response.status().is_success() {
        return Err(format!("GitHub 返回了 {}", response.status()));
    }
    response.json().await.map_err(|error| error.to_string())
}

/// 查询 GitHub Release 列表，按通道筛选出比当前版本新的最新版本。
/// 通道规则与 CI 发布规则同源：tag 含 '-' 即预发布，stable 通道一律跳过。
pub async fn check_for_update(app: AppHandle, channel: Option<String>) -> Result<Option<UpdateInfo>, String> {
    let beta = accepts_prerelease(channel.as_deref().unwrap_or("stable"));
    let current_tag = app.package_info().version.to_string();
    let releases = fetch_releases().await?;
    // 列表按 GitHub 的「最新优先」排序；逐个过滤通道、比当前新、保留最大者。
    let candidate = releases.iter().find(|release| {
        let is_prerelease = release.prerelease || release.tag_name.trim_start_matches(['v', 'V']).contains('-');
        if !beta && is_prerelease {
            return false;
        }
        compare_release_tags(&release.tag_name, &current_tag) == Ordering::Greater
    });
    let Some(release) = candidate else {
        return Ok(None);
    };
    let version_tag = release.tag_name.trim_start_matches(['v', 'V']).to_string();
    let asset = match pick_asset(&release.assets, &version_tag) {
        Some(asset) => asset.clone(),
        None => return Err(format!("v{version_tag} 没有提供当前平台的安装包")),
    };
    Ok(Some(UpdateInfo {
        prerelease: release.prerelease || version_tag.contains('-'),
        version: version_tag,
        notes: notes_preview(&release.body),
        asset_name: asset.name,
        size: asset.size,
        url: asset.browser_download_url,
    }))
}

/// 流式下载更新包到临时目录，进度经 `update://download` 事件广播。
/// 若本地已有同名且同大小的完整文件（上次「稍后」留下的），直接复用。
pub async fn download_update(
    window: WebviewWindow,
    url: String,
    asset_name: String,
    expected_size: u64,
    source: Option<String>,
) -> Result<String, String> {
    let work_dir = update_work_dir();
    fs::create_dir_all(&work_dir).map_err(|error| error.to_string())?;
    let destination = work_dir.join(sanitize_asset_name(&asset_name)?);
    if destination.is_file() && fs::metadata(&destination).map(|meta| meta.len()).unwrap_or(0) == expected_size {
        return Ok(destination.to_string_lossy().into_owned());
    }

    let target_url = resolve_download_url(&url, source.as_deref().unwrap_or("github"));
    let response = http_client()?
        .get(&target_url)
        .timeout(Duration::from_secs(30 * 60))
        .send()
        .await
        .map_err(|error| format!("下载失败：{error}"))?;
    if !response.status().is_success() {
        return Err(format!("下载失败：服务器返回 {}", response.status()));
    }
    let total = response.content_length().unwrap_or(expected_size);
    let mut stream = response.bytes_stream();
    let mut file = fs::File::create(&destination).map_err(|error| error.to_string())?;
    let mut received: u64 = 0;
    // 进度节流：总量 1% 或 256KB 起步，避免高频 IPC 拖慢下载。
    let mut next_report: u64 = total.div_ceil(100).max(256 * 1024);
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| format!("下载中断：{error}"))?;
        file.write_all(&chunk).map_err(|error| error.to_string())?;
        received += chunk.len() as u64;
        if received >= next_report || received == total {
            next_report = received + total.div_ceil(100).max(256 * 1024);
            let _ = window.emit(
                DOWNLOAD_EVENT,
                json!({ "received": received, "total": total, "percent": if total > 0 { received * 100 / total } else { 0 } }),
            );
        }
    }
    file.flush().map_err(|error| error.to_string())?;
    drop(file);
    if total > 0 && received != total {
        let _ = fs::remove_file(&destination);
        return Err("下载不完整，请重试".into());
    }
    Ok(destination.to_string_lossy().into_owned())
}

/// 检查下载文件名，防路径拼接意外（GitHub 资产名总是安全的，双保险）。
fn sanitize_asset_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains("..")
        || Path::new(trimmed).file_name().and_then(|value| value.to_str()) != Some(trimmed)
    {
        return Err("更新包文件名无效".into());
    }
    Ok(trimmed.to_string())
}

// ---- 测速（验证两条下载链路是否可用、速度几何） ----

/// GET 目标地址，读到 1 MB 或 8 秒无新数据即停——测速不是下载，样本够算
/// 速度就行，也不给代理与 GitHub 增加整包流量。慢网络（直连 GitHub CDN 常
/// < 50KB/s）读不满 1 MB 是常态：这里没有请求级总超时，只有「块间空闲 8 秒」
/// 的读帽，超时/中途断流时只要已读到样本就照样返回测速结果。
async fn probe_one(label: &str, url: &str) -> RouteProbe {
    const PROBE_BYTES_CAP: u64 = 1024 * 1024;
    const CHUNK_IDLE_CAP: Duration = Duration::from_secs(8);
    let start = Instant::now();
    let mut route = RouteProbe {
        label: label.to_string(),
        ok: false,
        status: 0,
        bytes: 0,
        elapsed_ms: 0,
        kbps: 0,
        error: String::new(),
    };
    let client = match http_client() {
        Ok(client) => client,
        Err(error) => {
            route.error = error;
            return route;
        }
    };
    let response = match client.get(url).send().await {
        Ok(response) => response,
        Err(error) => {
            route.error = format!("无法连接：{error}");
            route.elapsed_ms = start.elapsed().as_millis() as u64;
            return route;
        }
    };
    route.status = response.status().as_u16();
    let mut stream = response.bytes_stream();
    let mut received: u64 = 0;
    let mut stream_error = String::new();
    while received < PROBE_BYTES_CAP {
        match tokio::time::timeout(CHUNK_IDLE_CAP, stream.next()).await {
            Err(_idle) => break, // 块间空闲超过 8 秒：拿已有样本收工
            Ok(None) => break,   // 流自然结束
            Ok(Some(Ok(chunk))) => received += chunk.len() as u64,
            Ok(Some(Err(error))) => {
                stream_error = error.to_string();
                break;
            }
        }
    }
    let elapsed = start.elapsed();
    route.elapsed_ms = elapsed.as_millis() as u64;
    route.bytes = received;
    if received == 0 {
        route.error = if stream_error.is_empty() {
            format!("服务器没有返回数据（HTTP {}）", route.status)
        } else {
            format!("读取中断：{stream_error}")
        };
        return route;
    }
    let seconds = elapsed.as_secs_f64().max(0.001);
    // 部分样本（时间帽/断流截断）同样是有效测速：状态 2xx 且读到了数据。
    route.ok = (200..300).contains(&route.status);
    route.kbps = (received as f64 / 1024.0 / seconds).round() as u64;
    route
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceProbe {
    pub sample: String,
    pub direct: RouteProbe,
    pub proxy: RouteProbe,
}

/// 测速样本：优先取最近 Release 里当前平台资产的下载地址（只测链路，不关心
/// 版本是否比当前新）；接口失败/限流/资产缺失时退回仓库内一份固定存在的文件。
async fn probe_sample_url(channel: Option<String>) -> String {
    let beta = accepts_prerelease(channel.as_deref().unwrap_or("stable"));
    if let Ok(releases) = fetch_releases().await {
        let asset_url = releases
            .iter()
            .filter(|release| beta || !release.prerelease)
            .flat_map(|release| release.assets.iter())
            .find(|asset| release_asset_filter(&asset.name))
            .map(|asset| asset.browser_download_url.clone());
        if let Some(url) = asset_url {
            return url;
        }
    }
    PROBE_FALLBACK_URL.to_string()
}

/// 两条链路各测各的：一路失败（慢网断流、代理失效）不影响另一路的结果。
pub async fn probe_sources(channel: Option<String>) -> Result<SourceProbe, String> {
    let sample = probe_sample_url(channel).await;
    let proxied = format!("{GH_PROXY_PREFIX}{sample}");
    let (direct, proxy) = futures_util::future::join(
        probe_one("GitHub Raw", &sample),
        probe_one("GitHub-Proxy", &proxied),
    )
    .await;
    Ok(SourceProbe { sample, direct, proxy })
}

// ---- 主应用侧入口（lib.rs 以 #[tauri::command] 薄包装后注册） ----

/// 写任务单并以安装模式拉起自身的第二个进程，前端随后退出应用。
pub fn prepare_restart(app: &AppHandle, payload: String, version: String) -> Result<bool, String> {
    let payload_path = PathBuf::from(&payload);
    if !payload_path.is_file() {
        return Err("更新包不存在，请重新下载".into());
    }
    let self_path = std::env::current_exe().map_err(|error| error.to_string())?;
    let target = resolve_update_target(&self_path)?;
    let manifest = UpdateManifest {
        version,
        from_version: app.package_info().version.to_string(),
        payload: payload_path,
        target,
        pid: std::process::id(),
    };
    let work_dir = update_work_dir();
    fs::create_dir_all(&work_dir).map_err(|error| error.to_string())?;
    let manifest_path = work_dir.join("manifest.json");
    fs::write(&manifest_path, serde_json::to_string(&manifest).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())?;
    Command::new(&self_path)
        .arg(APPLY_FLAG)
        .arg(&manifest_path)
        .spawn()
        .map_err(|error| format!("无法启动安装进程：{error}"))?;
    Ok(true)
}

/// 当前运行位置对应的更新目标：
/// - Windows：exe 自身；
/// - macOS：从 /Applications 或 ~/Applications 运行 → 原地替换；从 dmg 等
///   临时位置运行 → 作为全新安装落到 /Applications。
/// 平台差异必须用 #[cfg] 编译期分流：cfg!() 是运行时布尔，两个分支在所有
/// 平台都会被编译，Windows 上会引用不存在的 macOS 专用函数。
fn resolve_update_target(self_path: &Path) -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        return Ok(self_path.to_path_buf());
    }
    #[cfg(target_os = "macos")]
    {
        let bundle =
            running_bundle(self_path).ok_or("无法定位当前应用包，请从「应用程序」文件夹运行更新")?;
        let parent = bundle.parent().map(Path::to_path_buf).unwrap_or_default();
        let home_applications = std::env::var("HOME")
            .map(|home| PathBuf::from(home).join("Applications"))
            .unwrap_or_default();
        if parent == PathBuf::from("/Applications")
            || (!home_applications.as_os_str().is_empty() && parent == home_applications)
        {
            return Ok(bundle);
        }
        return Ok(PathBuf::from("/Applications").join(bundle.file_name().ok_or("应用包名称无效")?));
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        return Err("当前平台不支持应用内自动更新".into());
    }
}

#[cfg(target_os = "macos")]
fn running_bundle(self_path: &Path) -> Option<PathBuf> {
    let mut ancestor = self_path.parent()?;
    loop {
        if ancestor.extension().and_then(|value| value.to_str()) == Some("app") {
            return Some(ancestor.to_path_buf());
        }
        ancestor = ancestor.parent()?;
    }
}

// ---- 安装进程 ----

/// 主进程把 `--handy-apply-update` 传进来时返回任务单路径。
pub fn apply_request_from_args() -> Option<PathBuf> {
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        if arg.to_str() == Some(APPLY_FLAG) {
            return args.next().map(PathBuf::from);
        }
    }
    None
}

/// 安装进程的全部工作：等主进程退出 → 换包 → 启动新版本。不建窗口、不起
/// Tauri，失败只写日志（下一次启动时日志仍在，便于排查）。
pub fn run_apply(manifest_path: &Path) -> ! {
    let manifest = fs::read_to_string(manifest_path)
        .ok()
        .and_then(|raw| serde_json::from_str::<UpdateManifest>(&raw).ok());
    let Some(manifest) = manifest else {
        log_line("任务单读取失败，放弃安装");
        std::process::exit(1);
    };
    log_line(&format!(
        "开始安装 {} → {}（payload {}）",
        manifest.from_version,
        manifest.version,
        manifest.payload.display()
    ));
    wait_pid_exit(manifest.pid, Duration::from_secs(30));
    match install(&manifest) {
        Ok(message) => {
            log_line(&format!("安装完成：{message}"));
            let _ = fs::remove_file(manifest_path);
            std::process::exit(0);
        }
        Err(error) => {
            log_line(&format!("安装失败：{error}"));
            std::process::exit(1);
        }
    }
}

/// 平台分流必须用 #[cfg] 编译期进行：cfg!() 是运行时布尔，两个分支在所有平台
/// 都会被编译，Windows 上会引用不存在的 macOS 专用函数。
fn install(manifest: &UpdateManifest) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        return install_windows(manifest);
    }
    #[cfg(target_os = "macos")]
    {
        return install_macos(manifest);
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        let _ = manifest;
        return Err("当前平台不支持应用内自动更新".into());
    }
}

fn wait_pid_exit(pid: u32, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while pid_alive(pid) {
        if Instant::now() >= deadline {
            return;
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

fn pid_alive(pid: u32) -> bool {
    #[cfg(target_os = "windows")]
    {
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH", "/FO", "CSV"])
            .output()
            .map(|output| String::from_utf8_lossy(&output.stdout).starts_with('"'))
            .unwrap_or(false)
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new("ps")
            .args(["-p", &pid.to_string()])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    }
}

fn run_command(program: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(program).args(args).output().map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if stderr.is_empty() { format!("{program} 退出码 {:?}", output.status.code()) } else { stderr })
    }
}

/// rename → 跨卷时回退 copy+delete。
#[cfg(target_os = "windows")]
fn move_file(from: &Path, to: &Path) -> Result<(), String> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(rename_error) => fs::copy(from, to)
            .map(|_| ())
            .and_then(|_| fs::remove_file(from).map_err(|error| error))
            .map_err(|_| rename_error.to_string()),
    }
}

#[cfg(target_os = "macos")]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

/// Windows：解压更新包 → 运行中的 exe 改名为 *.handy-old → 新 exe 以原文件名
/// 放回原路径 → 启动。落点保持用户磁盘上的原文件名：带版本号的文件名会让
/// 快捷方式、开机自启与用户习惯的路径在每次更新后失效。
#[cfg(target_os = "windows")]
fn install_windows(manifest: &UpdateManifest) -> Result<String, String> {
    let work_dir = update_work_dir();
    fs::create_dir_all(&work_dir).map_err(|error| error.to_string())?;
    let payload = extract_payload(&manifest.payload, &work_dir)?;
    let target = &manifest.target;
    let backup = target.with_file_name(format!(
        "{}.handy-old",
        target.file_name().and_then(|name| name.to_str()).unwrap_or("ActaHandy.exe")
    ));
    let _ = fs::remove_file(&backup);
    let mut rename_error = String::from("目标文件被占用");
    for _ in 0..LOCK_RETRY_TIMES {
        match fs::rename(target, &backup) {
            Ok(()) => {
                rename_error.clear();
                break;
            }
            Err(error) => {
                rename_error = error.to_string();
                std::thread::sleep(LOCK_RETRY_DELAY);
            }
        }
    }
    if !rename_error.is_empty() {
        return Err(format!("无法替换应用文件（可能被占用）：{rename_error}"));
    }
    move_file(&payload, target).map_err(|error| format!("无法放置新版本：{error}"))?;
    Command::new(target).spawn().map_err(|error| format!("无法启动新版本：{error}"))?;
    let _ = fs::remove_file(&manifest.payload);
    Ok("更新完成".to_string())
}

/// 新版本启动后清理上一次更新留下的旧 exe（安装进程退出前无法删除自己的
/// 映像）。扫描同目录下所有 `.handy-old` 结尾的文件——只有 Handy 的更新流程
/// 会制造这种文件，不依赖文件名里的版本号。
#[cfg(target_os = "windows")]
pub fn cleanup_stale_backup() {
    let Ok(self_path) = std::env::current_exe() else { return };
    let Some(dir) = self_path.parent() else { return };
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_backup = path
            .file_name()
            .and_then(|name| name.to_str())
            .map(|name| name.ends_with(".handy-old"))
            .unwrap_or(false);
        if !is_backup || !path.is_file() {
            continue;
        }
        // 安装进程可能仍在收尾，稍等重试；仍失败就留给下次启动。
        for _ in 0..3 {
            match fs::remove_file(&path) {
                Ok(()) => break,
                Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                    std::thread::sleep(Duration::from_millis(500));
                }
                Err(_) => break,
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn cleanup_stale_backup() {}

/// macOS：解压更新包 → 挂载 dmg → 替换 .app → 清 quarantine（必要时弹终端跑
/// dmg 内自带的解锁命令文件）→ 启动。
#[cfg(target_os = "macos")]
fn install_macos(manifest: &UpdateManifest) -> Result<String, String> {
    let work_dir = update_work_dir();
    let mount = work_dir.join("mount");
    let _ = fs::remove_dir_all(&mount);
    fs::create_dir_all(&mount).map_err(|error| error.to_string())?;
    let dmg = extract_payload(&manifest.payload, &work_dir)?;
    let mount_arg = mount.to_string_lossy().into_owned();
    let payload_arg = dmg.to_string_lossy().into_owned();
    run_command(
        "hdiutil",
        &["attach", "-nobrowse", "-readonly", "-noverify", "-mountpoint", mount_arg.as_str(), payload_arg.as_str()],
    )
    .map_err(|error| format!("无法挂载更新包：{error}"))?;

    let source_app = find_mounted_app(&mount).ok_or("更新包里没有找到应用")?;
    // 解锁命令文件必须在卸载前拷出来。
    let repair_script = {
        let candidate = mount.join(REPAIR_SCRIPT_NAME);
        if candidate.is_file() {
            let destination = work_dir.join(REPAIR_SCRIPT_NAME);
            let _ = fs::copy(&candidate, &destination);
            destination.is_file().then_some(destination)
        } else {
            None
        }
    };

    let installed = install_bundle(&source_app, &manifest.target);

    // 卸载尽力而为；失败不影响已完成的安装。
    for _ in 0..3 {
        if run_command("hdiutil", &["detach", mount_arg.as_str()]).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(600));
    }
    if mount.is_dir() {
        let _ = run_command("hdiutil", &["detach", "-force", mount_arg.as_str()]);
    }
    installed?;

    let mut message = "更新完成".to_string();
    if !clear_quarantine(&manifest.target) {
        // 无密码清理失败（例如文件归 root 所有）→ 弹终端运行 dmg 自带的解锁脚本。
        match repair_script {
            Some(script) => {
                let script_arg = script.to_string_lossy().into_owned();
                let _ = run_command("/usr/bin/xattr", &["-d", "com.apple.quarantine", script_arg.as_str()]);
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(&script, fs::Permissions::from_mode(0o755));
                match Command::new("/usr/bin/open").arg(&script).spawn() {
                    Ok(_) => {
                        if !wait_quarantine_cleared(&manifest.target, REPAIR_WAIT) {
                            message = "更新完成，但隔离标记未清除；请稍后手动运行更新包里的修复脚本。".into();
                        }
                    }
                    Err(error) => {
                        message = format!("更新完成，但无法打开修复脚本（{error}），请手动运行。");
                    }
                }
            }
            None => message = "更新完成，但隔离标记未清除；请手动清除后启动。".into(),
        }
    }

    let launch = format!("sleep 1; open {}", shell_quote(&manifest.target.to_string_lossy()));
    Command::new("/bin/sh")
        .arg("-c")
        .arg(&launch)
        .spawn()
        .map_err(|error| format!("无法启动新版本：{error}"))?;
    let _ = fs::remove_file(&manifest.payload);
    Ok(message)
}

#[cfg(target_os = "macos")]
fn find_mounted_app(mount: &Path) -> Option<PathBuf> {
    let mut found = None;
    let entries = fs::read_dir(mount).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.extension().and_then(|value| value.to_str()) == Some("app") {
            found = Some(path);
            break;
        }
    }
    found
}

/// 替换 .app：用户身份的 rm+cp 通常足够（拖拽安装的应用归用户所有）；
/// 权限不足时走 osascript 管理员授权（系统原生密码框）。
#[cfg(target_os = "macos")]
fn install_bundle(source_app: &Path, target: &Path) -> Result<(), String> {
    let source = source_app.to_string_lossy().into_owned();
    let destination = target.to_string_lossy().into_owned();
    let remove = run_command("/bin/rm", &["-rf", &destination]);
    let copy = run_command("/bin/cp", &["-R", &source, &destination]);
    match (remove, copy) {
        (Ok(()), Ok(())) if target.is_dir() => Ok(()),
        _ => {
            let script = format!("rm -rf {destination} && cp -R {source} {destination}");
            let apple_script = format!("do shell script \"{}\" with administrator privileges", script);
            let output = Command::new("osascript")
                .args(["-e", &apple_script])
                .output()
                .map_err(|error| error.to_string())?;
            if output.status.success() && target.is_dir() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                Err(if stderr.is_empty() { "安装新版本失败".into() } else { stderr })
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn quarantine_present(target: &Path) -> bool {
    let target_arg = target.to_string_lossy().into_owned();
    Command::new("/usr/bin/xattr")
        .args(["-p", "com.apple.quarantine", target_arg.as_str()])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// 清除 quarantine。先静默 `xattr -r -d`（拖拽安装的应用归用户所有，无需
/// 密码）；返回是否已彻底清除。
#[cfg(target_os = "macos")]
fn clear_quarantine(target: &Path) -> bool {
    let target_arg = target.to_string_lossy().into_owned();
    let _ = run_command("/usr/bin/xattr", &["-r", "-d", "com.apple.quarantine", target_arg.as_str()]);
    !quarantine_present(target)
}

#[cfg(target_os = "macos")]
fn wait_quarantine_cleared(target: &Path, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while quarantine_present(target) {
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(1000));
    }
    true
}

/// 关于页显示的应用信息：版本号与平台来自编译产物，不在前端硬编码。
pub fn app_info(app: &AppHandle) -> AppInfo {
    let version = app.package_info().version.to_string();
    AppInfo { prerelease: version.contains('-'), version }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tags() {
        assert_eq!(parse_release_tag("v1.1.0"), Some(((1, 1, 0), None)));
        assert_eq!(parse_release_tag("1.10.0"), Some(((1, 10, 0), None)));
        assert_eq!(parse_release_tag("v1.1.0-beta1"), Some(((1, 1, 0), Some("beta1".into()))));
        assert_eq!(parse_release_tag("release-42"), None);
    }

    #[test]
    fn orders_prerelease_tags() {
        use compare_release_tags as cmp;
        // beta 递进与「beta < 正式」的 semver 次序；beta10 按数值大于 beta9。
        assert_eq!(cmp("v1.1.0-beta1", "1.1.0-beta2"), Ordering::Less);
        assert_eq!(cmp("1.1.0-beta9", "1.1.0-beta10"), Ordering::Less);
        assert_eq!(cmp("1.1.0-beta1", "1.1.0"), Ordering::Less);
        assert_eq!(cmp("1.1.0", "1.0.9"), Ordering::Greater);
        assert_eq!(cmp("v1.1.0", "1.1.0"), Ordering::Equal);
        // 跨版本号：1.1.0-beta1 依然大于 1.0.0。
        assert_eq!(cmp("1.1.0-beta1", "1.0.0"), Ordering::Greater);
    }

    #[test]
    fn sanitizes_names() {
        assert!(sanitize_asset_name("ActaHandy-1.1.0-beta1-macos-arm64.zip").is_ok());
        assert!(sanitize_asset_name("../evil.zip").is_err());
        assert!(sanitize_asset_name("").is_err());
        assert!(sanitize_asset_name("a/b.zip").is_err());
    }

    #[test]
    fn prefixes_proxy_only_when_chosen() {
        let raw = "https://github.com/x/y/releases/download/v1/z.zip";
        assert_eq!(resolve_download_url(raw, "github"), raw);
        assert_eq!(resolve_download_url(raw, "proxy"), format!("{GH_PROXY_PREFIX}{raw}"));
        assert_eq!(resolve_download_url(raw, "PROXY"), format!("{GH_PROXY_PREFIX}{raw}"));
    }
}
