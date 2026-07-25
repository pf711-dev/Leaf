// Unterdrücke Warnings von veralteten Cocoa APIs
#![allow(unexpected_cfgs)]
#![allow(deprecated)]

use tauri::{AppHandle, Runtime, WebviewWindow};

#[cfg(target_os = "macos")]
use cocoa::{
    appkit::{NSWindow, NSWindowStyleMask, NSView, NSWindowTitleVisibility},
    base::id,
    foundation::NSPoint,
};

#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};

/// Configuration for Traffic Lights positioning
pub struct TrafficLightsConfig {
    /// Offset in pixels from default position (positive = right, negative = left)
    pub offset_x: f64,
    /// Offset in pixels from default position (positive = down, negative = up)
    pub offset_y: f64,
}

impl Default for TrafficLightsConfig {
    fn default() -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
        }
    }
}

/// Enables rounded corners for the window (macOS only)
/// Uses only public APIs - App Store compatible
#[tauri::command]
pub fn enable_rounded_corners<R: Runtime>(
    _app: AppHandle<R>,
    window: WebviewWindow<R>,
    offset_x: Option<f64>,
    offset_y: Option<f64>,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let config = TrafficLightsConfig {
            offset_x: offset_x.unwrap_or(0.0),
            offset_y: offset_y.unwrap_or(0.0),
        };

        window
            .with_webview(move |webview| {
                #[cfg(target_os = "macos")]
                unsafe {
                    let ns_window = webview.ns_window() as id;
                    
                    let mut style_mask = ns_window.styleMask();
                    
                    // Add necessary styles for rounded corners
                    style_mask |= NSWindowStyleMask::NSFullSizeContentViewWindowMask;
                    style_mask |= NSWindowStyleMask::NSTitledWindowMask;
                    style_mask |= NSWindowStyleMask::NSClosableWindowMask;
                    style_mask |= NSWindowStyleMask::NSMiniaturizableWindowMask;
                    style_mask |= NSWindowStyleMask::NSResizableWindowMask;
                    
                    ns_window.setStyleMask_(style_mask);
                    ns_window.setTitlebarAppearsTransparent_(cocoa::base::YES);
                    
                    let content_view = ns_window.contentView();
                    content_view.setWantsLayer(cocoa::base::YES);
                    
                    position_traffic_lights(ns_window, config.offset_x, config.offset_y);
                }
            })
            .map_err(|e| e.to_string())?;
        
        Ok(())
    }
    
    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}

/// Enables modern window style with rounded corners and shadow
#[tauri::command]
pub fn enable_modern_window_style<R: Runtime>(
    _app: AppHandle<R>,
    window: WebviewWindow<R>,
    corner_radius: Option<f64>,
    offset_x: Option<f64>,
    offset_y: Option<f64>,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let config = TrafficLightsConfig {
            offset_x: offset_x.unwrap_or(0.0),
            offset_y: offset_y.unwrap_or(0.0),
        };
        let radius = corner_radius.unwrap_or(12.0);

        window
            .with_webview(move |webview| {
                #[cfg(target_os = "macos")]
                unsafe {
                    let ns_window = webview.ns_window() as id;
                    
                    let mut style_mask = ns_window.styleMask();
                    
                    style_mask |= NSWindowStyleMask::NSFullSizeContentViewWindowMask;
                    style_mask |= NSWindowStyleMask::NSTitledWindowMask;
                    style_mask |= NSWindowStyleMask::NSClosableWindowMask;
                    style_mask |= NSWindowStyleMask::NSMiniaturizableWindowMask;
                    style_mask |= NSWindowStyleMask::NSResizableWindowMask;
                    
                    ns_window.setStyleMask_(style_mask);
                    ns_window.setTitlebarAppearsTransparent_(cocoa::base::YES);
                    ns_window.setTitleVisibility_(NSWindowTitleVisibility::NSWindowTitleHidden);
                    ns_window.setHasShadow_(cocoa::base::YES);
                    ns_window.setOpaque_(cocoa::base::NO);
                    
                    let content_view = ns_window.contentView();
                    content_view.setWantsLayer(cocoa::base::YES);

                    let layer: id = msg_send![content_view, layer];
                    if !layer.is_null() {
                        let _: () = msg_send![layer, setCornerRadius: radius];
                        let _: () = msg_send![layer, setMasksToBounds: cocoa::base::YES];
                        // content_view 的 layer 默认有白色背景，圆角边缘会透出白线。
                        // 设为透明，让背景完全由 WebView 内容控制。
                        let transparent: id = msg_send![objc::class!(NSColor), clearColor];
                        let cg_color: id = msg_send![transparent, CGColor];
                        let _: () = msg_send![layer, setBackgroundColor: cg_color];
                    }

                    // 让 WKWebView 不绘制白色背景，避免圆角窗口边缘透出白线。
                    // WKWebView 是 contentView 的子视图（可能有中间容器，递归查找）。
                    set_webview_transparent(content_view);

                    position_traffic_lights(ns_window, config.offset_x, config.offset_y);
                }
            })
            .map_err(|e| e.to_string())?;
        
        Ok(())
    }
    
    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}

/// 运行时调整窗口圆角半径（仅 macOS）。
///
/// 用途：进入「演示模式」时把圆角设为 0，让窗口变成直角，
/// 与直角的 iframe 文档严丝合缝；退出时恢复 12px。
/// 若不调整，圆角窗口的弧线区域铺不满直角文档，四角会透出底层颜色。
#[tauri::command]
pub fn set_corner_radius<R: Runtime>(
    _app: AppHandle<R>,
    window: WebviewWindow<R>,
    radius: Option<f64>,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let r = radius.unwrap_or(12.0);

        window
            .with_webview(move |webview| {
                #[cfg(target_os = "macos")]
                unsafe {
                    let ns_window = webview.ns_window() as id;
                    let content_view = ns_window.contentView();
                    let layer: id = msg_send![content_view, layer];
                    if !layer.is_null() {
                        let _: () = msg_send![layer, setCornerRadius: r];
                    }
                }
            })
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        let _ = radius;
        Ok(())
    }
}

/// 设置红黄绿三个窗口按钮（Traffic Lights）的显隐（仅 macOS）。
///
/// 用途：进入「演示模式」时隐藏它们，避免遮挡文档内容；退出时恢复显示。
/// 实现方式：直接对三个 standardWindowButton 调用 setHidden:。
/// 为避免按钮隐藏后其 superview（NSThemeCloseWidget 容器）仍占据
/// 顶部悬停热区拦截鼠标，对每个按钮的 superview 也一并切换显隐。
#[tauri::command]
pub fn set_traffic_lights_visible<R: Runtime>(
    _app: AppHandle<R>,
    window: WebviewWindow<R>,
    visible: Option<bool>,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let v = visible.unwrap_or(true);

        window
            .with_webview(move |webview| {
                #[cfg(target_os = "macos")]
                unsafe {
                    let ns_window = webview.ns_window() as id;
                    let hidden = if v { 0 } else { 1 }; // YES(1) = 隐藏
                    // 关闭、最小化、最大化三个按钮：按钮本身 + 其直接父视图一起切换显隐。
                    for idx in 0..3u64 {
                        let btn: id = msg_send![ns_window, standardWindowButton: idx];
                        if btn.is_null() {
                            continue;
                        }
                        let _: () = msg_send![btn, setHidden: hidden];
                        let sup: id = msg_send![btn, superview];
                        if !sup.is_null() {
                            let _: () = msg_send![sup, setHidden: hidden];
                        }
                    }
                }
            })
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        let _ = visible;
        Ok(())
    }
}

/// Repositions Traffic Lights only (useful after fullscreen toggle)
#[tauri::command]
pub fn reposition_traffic_lights<R: Runtime>(
    _app: AppHandle<R>,
    window: WebviewWindow<R>,
    offset_x: Option<f64>,
    offset_y: Option<f64>,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let config = TrafficLightsConfig {
            offset_x: offset_x.unwrap_or(0.0),
            offset_y: offset_y.unwrap_or(0.0),
        };

        window
            .with_webview(move |webview| {
                #[cfg(target_os = "macos")]
                unsafe {
                    let ns_window = webview.ns_window() as id;
                    position_traffic_lights(ns_window, config.offset_x, config.offset_y);
                }
            })
            .map_err(|e| e.to_string())?;

        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(())
    }
}

#[cfg(target_os = "macos")]
unsafe fn position_traffic_lights(ns_window: id, offset_x: f64, offset_y: f64) {
    let default_x = 20.0;
    let default_y = 0.0;
    
    let close_button: id = msg_send![ns_window, standardWindowButton: 0];
    let miniaturize_button: id = msg_send![ns_window, standardWindowButton: 1];
    let zoom_button: id = msg_send![ns_window, standardWindowButton: 2];
    
    let new_x = default_x + offset_x;
    let new_y = default_y - offset_y;
    
    if !close_button.is_null() {
        let frame: cocoa::foundation::NSRect = msg_send![close_button, frame];
        let new_frame = cocoa::foundation::NSRect::new(
            NSPoint::new(new_x, new_y),
            frame.size,
        );
        let _: () = msg_send![close_button, setFrame: new_frame];
    }
    
    if !miniaturize_button.is_null() {
        let frame: cocoa::foundation::NSRect = msg_send![miniaturize_button, frame];
        let new_frame = cocoa::foundation::NSRect::new(
            NSPoint::new(new_x + 20.0, new_y),
            frame.size,
        );
        let _: () = msg_send![miniaturize_button, setFrame: new_frame];
    }
    
    if !zoom_button.is_null() {
        let frame: cocoa::foundation::NSRect = msg_send![zoom_button, frame];
        let new_frame = cocoa::foundation::NSRect::new(
            NSPoint::new(new_x + 40.0, new_y),
            frame.size,
        );
        let _: () = msg_send![zoom_button, setFrame: new_frame];
    }
}

/// 递归遍历视图树，找到 WKWebView 并通过 KVC 设置 drawsBackground = NO，
/// 让 WebView 不绘制默认白色背景，避免圆角窗口边缘透出白线。
#[cfg(target_os = "macos")]
unsafe fn set_webview_transparent(view: id) {
    if view.is_null() {
        return;
    }
    // 用 isKindOfClass: 判断是否为 WKWebView（用类名查找类对象）
    let wk_class: id = msg_send![objc::class!(WKWebView), class];
    let is_wk: bool = msg_send![view, isKindOfClass: wk_class];
    if is_wk {
        // KVC: [view setValue:@(NO) forKey:@"drawsBackground"]
        let no_value: id = msg_send![objc::class!(NSNumber), numberWithBool: cocoa::base::NO];
        // [NSString stringWithUTF8String:"drawsBackground"]
        let key: id = msg_send![objc::class!(NSString), stringWithUTF8String: b"drawsBackground\0".as_ptr() as *const i8];
        let _: () = msg_send![view, setValue: no_value forKey: key];
        return;
    }

    // 递归查找子视图
    let subviews: id = msg_send![view, subviews];
    let count: usize = msg_send![subviews, count];
    for i in 0..count {
        let subview: id = msg_send![subviews, objectAtIndex: i];
        set_webview_transparent(subview);
    }
}
