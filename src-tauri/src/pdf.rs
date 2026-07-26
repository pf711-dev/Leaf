//! PDF 导出（跨平台）：
//!
#![allow(deprecated)] // cocoa::base::id 等，与 mac_rounded_corners.rs 保持一致
//!
//! - **Windows**：`print_to_pdf(out_path)` 走 WebView2 的 PrintToPdf 静默生成，
//!   `ShouldPrintBackgrounds=true` 根治"彩色背景丢失"。
//! - **macOS**：`print_to_pdf(out_path)` 走 WKWebView 的
//!   `createPDFWithConfiguration:completionHandler:` 静默生成（macOS 11+）。
//!   背景色由注入的 print-color-adjust:exact 保留。
//!
//! 设计要点：
//! - `with_webview` 闭包在主线程执行（Tauri 会 dispatch 过去）。
//! - macOS 用 `oneshot` channel + `block` crate 处理异步 completion handler，
//!   避免在主线程阻塞等待（会导致死锁）。
//! - Windows 结果用 `Arc<Mutex<Option<Result>>>` 跨闭包边界带出。

#[cfg(target_os = "windows")]
use std::path::PathBuf;
#[cfg(target_os = "windows")]
use std::sync::{Arc, Mutex};
#[cfg(any(target_os = "windows", target_os = "macos"))]
use tauri::Manager;

/// 静默导出当前 WebView 为 PDF。
///
/// - Windows：WebView2 PrintToPdf
/// - macOS：WKWebView createPDFWithConfiguration（macOS 11+）
#[tauri::command]
pub async fn print_to_pdf(
    app: tauri::AppHandle,
    out_path: String,
) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        print_to_pdf_windows(&app, out_path).await
    }

    #[cfg(target_os = "macos")]
    {
        print_to_pdf_macos(&app, out_path).await
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = (app, out_path);
        Err("当前平台不支持 PDF 导出".to_string())
    }
}

/// macOS：弹出原生打印面板（NSPrintOperation）。
///
/// 保留此命令用于向后兼容。由于 Tauri 的 runloop 模式限制导致
/// performSelector 调度不可靠，目前此命令仅打印面板路径可能不稳定。
/// 推荐使用 `print_to_pdf` 实现静默 PDF 生成。
#[tauri::command]
pub async fn show_print_dialog(app: tauri::AppHandle) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        show_print_dialog_mac(&app).await
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Err("show_print_dialog 仅支持 macOS；Windows 请用 print_to_pdf".to_string())
    }
}

// ==================== Windows ====================

#[cfg(target_os = "windows")]
async fn print_to_pdf_windows(
    app: &tauri::AppHandle,
    out_path: String,
) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "找不到主窗口".to_string())?;

    // 校验目标路径，提前创建父目录。
    let path = PathBuf::from(&out_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("创建目录失败: {}", e))?;
    }

    // 闭包执行在主线程，结果通过共享容器带出。
    let result: Arc<Mutex<Option<Result<(), String>>>> = Arc::new(Mutex::new(None));
    let result_clone = Arc::clone(&result);

    // with_webview 在主线程同步执行闭包；闭包内阻塞等待 COM 回调完成后才返回。
    window
        .with_webview(move |webview| {
            let r = unsafe { print_via_webview2(&webview, &out_path) };
            *result_clone.lock().unwrap() = Some(r);
        })
        .map_err(|e| format!("访问 WebView 失败: {}", e))?;

    // with_webview 是同步阻塞的，执行到这里时闭包已完成。
    let mut guard = result.lock().unwrap();
    guard
        .take()
        .unwrap_or_else(|| Err("PDF 导出未返回结果".to_string()))
}

/// 在 `with_webview` 闭包内（主线程）调用 WebView2 PrintToPdf。
///
/// 使用 webview2-com 的 `PrintToPdfCompletedHandler::wait_for_async_operation`：
/// 它内部泵 Win32 消息循环等待 COM 异步回调完成。
///
/// # Safety
/// 调用 COM 接口的 unsafe 方法，且依赖当前处于主线程（COM STA）。
#[cfg(target_os = "windows")]
unsafe fn print_via_webview2(
    webview: &tauri::webview::PlatformWebview,
    out_path: &str,
) -> Result<(), String> {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2_7, ICoreWebView2PrintSettings,
        ICoreWebView2PrintToPdfCompletedHandler,
    };
    use webview2_com::PrintToPdfCompletedHandler;
    use windows::core::Interface;
    use windows::Win32::Foundation::{BOOL, HRESULT};

    let controller = webview.controller();
    let core = controller
        .CoreWebView2()
        .map_err(|e| format!("获取 CoreWebView2 失败: {}", e))?;

    let core7: ICoreWebView2_7 = core.cast().map_err(|_| {
        "WebView2 版本过低，PrintToPdf 需 ≥ 1.0.1020.30".to_string()
    })?;

    let environment = webview.environment();
    let settings: ICoreWebView2PrintSettings = environment
        .CreatePrintSettings()
        .map_err(|e| format!("创建 PrintSettings 失败: {}", e))?;
    settings
        .SetShouldPrintBackgrounds(true)
        .map_err(|e| format!("设置打印背景失败: {}", e))?;
    let _ = settings.SetShouldPrintHeaderAndFooter(false);

    let path_h = windows::core::HSTRING::from(out_path);

    PrintToPdfCompletedHandler::wait_for_async_operation(
        Box::new(move |handler: ICoreWebView2PrintToPdfCompletedHandler| {
            core7
                .PrintToPdf(&path_h, &settings, &handler)
                .map_err(|e| e.into())
        }),
        Box::new(move |_hr: HRESULT, is_successful: BOOL| {
            if is_successful.as_bool() {
                Ok(())
            } else {
                Err(windows::core::Error::from(HRESULT(-1)))
            }
        }),
    )
    .map_err(|e| format!("PDF 生成失败: {}", e))
}

// ==================== macOS：静默 PDF 生成 ====================

#[cfg(target_os = "macos")]
async fn print_to_pdf_macos(
    app: &tauri::AppHandle,
    out_path: String,
) -> Result<(), String> {
    use tokio::sync::oneshot;

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "找不到主窗口".to_string())?;

    // 提前创建目标目录
    let path = std::path::PathBuf::from(&out_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("创建目录失败: {}", e))?;
    }

    // oneshot channel：completion handler 在主线程调用 tx.send()，
    // 我们在 tokio async 上下文中 rx.await()，避免主线程阻塞。
    let (tx, rx) = oneshot::channel::<Result<Vec<u8>, String>>();

    window
        .with_webview(move |webview| {
            unsafe { start_macos_pdf_capture(&webview, tx) };
        })
        .map_err(|e| format!("访问 WebView 失败: {}", e))?;

    // 在 async 上下文中等待 completion handler 返回数据，
    // 设置 30 秒超时防止永久阻塞。
    let pdf_data = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        rx,
    )
    .await
    .map_err(|_| "PDF 生成超时（30 秒）".to_string())?
    .map_err(|e| format!("channel 关闭: {}", e))??;

    std::fs::write(&out_path, &pdf_data)
        .map_err(|e| format!("写入文件失败: {}", e))?;

    Ok(())
}

/// 在 `with_webview` 闭包内（主线程）发起 WKWebView 的 createPDF 请求。
///
/// 使用 oneshot channel 将结果异步传回，不在主线程阻塞等待。
///
/// # Safety
/// 调用 Objective-C runtime 的 unsafe 消息发送。
#[cfg(target_os = "macos")]
unsafe fn start_macos_pdf_capture(
    webview: &tauri::webview::PlatformWebview,
    tx: tokio::sync::oneshot::Sender<Result<Vec<u8>, String>>,
) {
    use block::ConcreteBlock;
    use cocoa::base::id;
    use objc::{class, msg_send, sel, sel_impl};
    use std::sync::Mutex;

    let wkwebview = webview.inner() as id;
    if wkwebview.is_null() {
        let _ = tx.send(Err("获取 WKWebView 失败".to_string()));
        return;
    }

    // WKPDFConfiguration：全部默认（CGRectNull = 整页）
    let config: id = msg_send![class!(WKPDFConfiguration), new];

    // 用 Mutex<Option> 包装 sender，使闭包为 Fn（而非 FnOnce）。
    // completion handler 仅调用一次，take() 是安全的。
    let tx = Mutex::new(Some(tx));

    // 创建 completion handler block。
    // completion handler 的参数：(NSData * _Nullable, NSError * _Nullable)
    let block = ConcreteBlock::new(move |data: id, error: id| {
        let sender = tx.lock().unwrap().take();
        let Some(sender) = sender else { return };

        if !error.is_null() {
            let desc: id = msg_send![error, localizedDescription];
            let msg = if !desc.is_null() {
                let cstr: *const i8 = msg_send![desc, UTF8String];
                if !cstr.is_null() {
                    std::ffi::CStr::from_ptr(cstr).to_string_lossy().into_owned()
                } else {
                    "未知错误".to_string()
                }
            } else {
                "未知错误".to_string()
            };
            let _ = sender.send(Err(format!("PDF 生成失败: {}", msg)));
        } else if data.is_null() {
            let _ = sender.send(Err("PDF 数据为空".to_string()));
        } else {
            let len: usize = msg_send![data, length];
            let bytes: *const u8 = msg_send![data, bytes];
            let vec = std::slice::from_raw_parts(bytes, len).to_vec();
            let _ = sender.send(Ok(vec));
        }
    });
    let block = block.copy();

    // 发起 createPDF，completion handler 会在主线程被调用
    let _: () = msg_send![wkwebview,
        createPDFWithConfiguration: config
        completionHandler: &*block];
}

// ==================== macOS：打印面板（保留） ====================

#[cfg(target_os = "macos")]
async fn show_print_dialog_mac(app: &tauri::AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "找不到主窗口".to_string())?;

    let app_handle = app.clone();
    window
        .with_webview(move |webview| {
            let r = unsafe { start_print_operation(&webview) };
            if let Err(msg) = r {
                let _ = tauri::Emitter::emit(
                    &app_handle,
                    "pdf-export-error",
                    msg,
                );
            }
        })
        .map_err(|e| format!("访问 WebView 失败: {}", e))?;

    Ok(())
}

/// 创建 NSPrintOperation 并通过 `runOperationModalForWindow:`
/// 以异步 sheet 形式在主窗口上显示打印面板。
/// 此方法立即返回（不创建嵌套 runloop），打印面板在后续 runloop
/// 迭代中显示，用户可在面板中选择 "PDF → 存储为 PDF" 导出。
///
/// # Safety
/// 调用 Objective-C runtime 的 unsafe 消息发送。
#[cfg(target_os = "macos")]
unsafe fn start_print_operation(
    webview: &tauri::webview::PlatformWebview,
) -> Result<(), String> {
    use cocoa::base::id;
    use objc::{class, msg_send, sel, sel_impl};

    let wkwebview = webview.inner() as id;
    if wkwebview.is_null() {
        return Err("获取 WKWebView 失败".to_string());
    }

    let print_info: id = msg_send![class!(NSPrintInfo), sharedPrintInfo];
    if print_info.is_null() {
        return Err("获取 NSPrintInfo 失败".to_string());
    }

    let _: () = msg_send![print_info, setHorizontalPagination: 1i64];
    let _: () = msg_send![print_info, setVerticalPagination: 1i64];

    let print_op: id = msg_send![wkwebview, printOperationWithPrintInfo: print_info];
    if print_op.is_null() {
        return Err("创建 NSPrintOperation 失败".to_string());
    }

    let panel: id = msg_send![print_op, printPanel];
    let _: () = msg_send![panel, setOptions: 0x40 | 0x4 | 0x100];

    let ns_window = webview.ns_window() as id;
    if ns_window.is_null() {
        return Err("获取 NSWindow 失败".to_string());
    }

    // 使用 runOperationModalForWindow:delegate:didRunSelector:contextInfo:。
    // 这是异步方法（立即返回），以 sheet 形式在主窗口上显示打印面板，
    // 不会创建嵌套 runloop，因此不会触发之前同步 runModalForWindow: 的崩溃。
    // delegate/didRunSelector/contextInfo 全为 NULL 表示不需要完成回调。
    let nil_delegate: id = std::ptr::null_mut();
    let null_sel: objc::runtime::Sel = unsafe { std::mem::zeroed() };
    let _: () = msg_send![print_op,
        runOperationModalForWindow: ns_window
        delegate: nil_delegate
        didRunSelector: null_sel
        contextInfo: std::ptr::null_mut::<std::ffi::c_void>()];

    Ok(())
}
