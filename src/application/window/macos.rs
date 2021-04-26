// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

//! The `Window` struct and associated types.

use std::{
  cell::RefCell,
  fmt,
  rc::Rc,
  sync::{
    atomic::{AtomicBool, AtomicI32, Ordering},
    mpsc::Sender,
  },
};

use cacao::macos::window::{Window as CacaoWindow, WindowConfig, WindowDelegate, WindowStyle};
use cacao::macos::{App, AppDelegate};
use cacao::webview::{WebView, WebViewConfig, WebViewDelegate};

use super::{CursorIcon, Fullscreen, WindowAttributes, WindowId, WindowRequest};

use winit::{
  dpi::{PhysicalPosition, PhysicalSize, Position},
  window::UserAttentionType,
};

use crate::application::{
  dpi::Size,
  error::{ExternalError, NotSupportedError, OsError},
  event_loop::EventLoopWindowTarget,
  monitor::{MonitorHandle, VideoMode},
};

use crate::application::icon::{BadIcon, Icon};

pub struct AppWindow {
  content: WebView<WebViewInstance>,
}

impl AppDelegate for AppWindow {
  fn did_unhide(&self) {
    println!("unhidden")
  }
  fn did_finish_launching(&self) {
    App::activate();
  }
  fn should_terminate_after_last_window_closed(&self) -> bool {
      true
  }
  fn will_update(&self) {
      
  }
}

impl WindowDelegate for AppWindow {
  const NAME: &'static str = "WindowDelegate";

  fn did_move(&self) {
    println!("moved");
  }

  fn did_load(&mut self, window: CacaoWindow) {
    window.set_content_view(&self.content);
  }
}

#[derive(Default)]
pub struct WebViewInstance;

impl AppWindow {
  pub fn new() -> Self {
    let mut webview_config = WebViewConfig::default();

    // register the protocol in the webview
    webview_config.add_custom_protocol("cacao");

    AppWindow {
      content: WebView::with(webview_config, WebViewInstance::default()),
    }
  }

  pub fn load_url(&self, url: &str) {
    self.content.load_url(url);
  }

  pub fn get_position(&self) -> (i32, i32) {
    (0, 0)
  }

  pub fn get_size(&self) -> (i32, i32) {
    (800, 600)
  }
}

impl WebViewDelegate for WebViewInstance {
  fn on_custom_protocol_request(&self, path: &str) -> Option<Vec<u8>> {
    let requested_asset_path = path.replace("cacao://", "");

    let index_html = r#"
       <!DOCTYPE html>
       <html lang="en">
           <head>
           <meta charset="UTF-8" />
           <meta http-equiv="X-UA-Compatible" content="IE=edge" />
           <meta name="viewport" content="width=device-width, initial-scale=1.0" />
           </head>
           <body>
           <h1>Welcome 🍫</h1>
           <a href="/hello.html">Link</a>
           </body>
       </html>"#;

    let link_html = r#"
       <!DOCTYPE html>
       <html lang="en">
           <head>
           <meta charset="UTF-8" />
           <meta http-equiv="X-UA-Compatible" content="IE=edge" />
           <meta name="viewport" content="width=device-width, initial-scale=1.0" />
           </head>
           <body>
           <h1>Hello!</h1>
           <a href="/index.html">Back home</a>
           </body>
       </html>"#;

    return match requested_asset_path.as_str() {
      "/hello.html" => Some(link_html.as_bytes().into()),
      _ => Some(index_html.as_bytes().into()),
    };
  }
}

/// Represents a window.
///
/// # Example
///
/// ```no_run
/// use winit::{
///     event::{Event, WindowEvent},
///     event_loop::{ControlFlow, EventLoop},
///     window::Window,
/// };
///
/// let mut event_loop = EventLoop::new();
/// let window = Window::new(&event_loop).unwrap();
///
/// event_loop.run(move |event, _, control_flow| {
///     *control_flow = ControlFlow::Wait;
///
///     match event {
///         Event::WindowEvent {
///             event: WindowEvent::CloseRequested,
///             ..
///         } => *control_flow = ControlFlow::Exit,
///         _ => (),
///     }
/// });
/// ```
pub struct Window {
  /// Window id.
  pub(crate) window_id: WindowId,
  /// Gtk application window.
  pub(crate) window: CacaoWindow<AppWindow>,
  /// Window requests sender
  pub(crate) window_requests_tx: Sender<(WindowId, WindowRequest)>,
  scale_factor: Rc<AtomicI32>,
  position: Rc<(AtomicI32, AtomicI32)>,
  size: Rc<(AtomicI32, AtomicI32)>,
  maximized: Rc<AtomicBool>,
  fullscreen: RefCell<Option<Fullscreen>>,
}

impl Window {
  pub(crate) fn new<T>(
    event_loop_window_target: &EventLoopWindowTarget<T, AppWindow>,
    attributes: WindowAttributes,
  ) -> Result<Self, OsError> {
    let app = &event_loop_window_target.app;

    let mut config = WindowConfig::default();
    config.set_initial_dimensions(100., 100., 800., 600.);

    let mut default_styles = vec![WindowStyle::Closable];

    if attributes.resizable {
      default_styles.push(WindowStyle::Resizable);
    }

    if !attributes.decorations {
      default_styles.push(WindowStyle::Borderless);
    }

    config.set_styles(&default_styles);

    let window = CacaoWindow::with(WindowConfig::default(), AppWindow::new());

    // todo get window id
    let window_id = WindowId(0);

    event_loop_window_target
      .windows
      .borrow_mut()
      .insert(window_id);

    // Set Width/Height & Resizable
    let win_scale_factor = window.backing_scale_factor();
    let (width, height) = attributes
      .inner_size
      .map(|size| size.to_logical::<f64>(win_scale_factor as f64).into())
      .unwrap_or((800, 600));

    /*
    window.set_resizable(attributes.resizable);
    if attributes.resizable {
      window.set_default_size(width, height);
    } else {
      window.set_size_request(width, height);
    }
     */

    let (min_width, min_height): (f64, f64) = attributes
      .min_inner_size
      .map(|size| size.to_logical::<f64>(win_scale_factor as f64).into())
      .unwrap_or_default();

    let (max_width, max_height): (f64, f64) = attributes
      .max_inner_size
      .map(|size| size.to_logical::<f64>(win_scale_factor as f64).into())
      .unwrap_or_default();

    if attributes.min_inner_size.is_some() {
      window.set_minimum_content_size(min_width, min_height);
    }

    if attributes.max_inner_size.is_some() {
      window.set_maximum_content_size(max_width, max_height);
    }

    // TODO: Set Transparent
    //if attributes.transparent {}

    // Rest attributes
    window.set_title(&attributes.title);

    // allow to save window state, will be reopened to same
    // position and size
    window.set_autosave_name("test_wry");

    if attributes.fullscreen.is_some() {
      window.toggle_full_screen();
    }

    // TODO: maximized
    //if attributes.maximized {}

    // todo always on top
    if attributes.always_on_top {}

    // todo update app icon
    //if let Some(icon) = &attributes.window_icon {}

    if attributes.visible {
      window.show();
    };

    let window_requests_tx = event_loop_window_target.window_requests_tx.clone();
    if let Some(delegate) = &window.delegate {
      let w_pos = delegate.get_position();

      let position: Rc<(AtomicI32, AtomicI32)> = Rc::new((w_pos.0.into(), w_pos.1.into()));
      let position_clone = position.clone();

      let w_size = delegate.get_size();
      let size: Rc<(AtomicI32, AtomicI32)> = Rc::new((w_size.0.into(), w_size.1.into()));
      let size_clone = size.clone();
      /*
      window.connect_configure_event(move |_window, event| {
        let (x, y) = event.get_position();
        position_clone.0.store(x, Ordering::Release);
        position_clone.1.store(y, Ordering::Release);

        let (w, h) = event.get_size();
        size_clone.0.store(w as i32, Ordering::Release);
        size_clone.1.store(h as i32, Ordering::Release);

        false
      });
      */

      let w_max = !window.is_miniaturized();
      let maximized: Rc<AtomicBool> = Rc::new(w_max.into());
      let max_clone = maximized.clone();
      /*
      window.connect_window_state_event(move |_window, event| {
        let state = event.get_new_window_state();
        //max_clone.store(state.contains(WindowState::MAXIMIZED), Ordering::Release);

        //Inhibit(false)
      });
      */

      let win_scale_factor = win_scale_factor as i32;
      let scale_factor: Rc<AtomicI32> = Rc::new(win_scale_factor.into());
      let scale_factor_clone = scale_factor.clone();
      /*
      window.connect_property_scale_factor_notify(move |window| {
        scale_factor_clone.store(window.get_scale_factor(), Ordering::Release);
      });
      */

      return Ok(Self {
        window_id,
        window,
        window_requests_tx,
        scale_factor,
        position,
        size,
        maximized,
        fullscreen: RefCell::new(attributes.fullscreen),
      });
    }

    Err(OsError::new(0, "", "Unable to start window"))
  }

  pub fn id(&self) -> WindowId {
    self.window_id
  }

  pub fn scale_factor(&self) -> f64 {
    self.scale_factor.load(Ordering::Acquire) as f64
  }

  pub fn request_redraw(&self) {
    todo!()
  }

  pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
    let (x, y) = &*self.position;
    Ok(PhysicalPosition::new(
      x.load(Ordering::Acquire),
      y.load(Ordering::Acquire),
    ))
  }

  pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
    let (x, y) = &*self.position;
    Ok(PhysicalPosition::new(
      x.load(Ordering::Acquire),
      y.load(Ordering::Acquire),
    ))
  }

  pub fn set_outer_position<P: Into<Position>>(&self, position: P) {
    let (x, y): (i32, i32) = position
      .into()
      .to_physical::<i32>(self.scale_factor())
      .into();

    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::Position((x, y))))
    {
      log::warn!("Fail to send position request: {}", e);
    }
  }

  pub fn inner_size(&self) -> PhysicalSize<u32> {
    let (width, height) = &*self.size;

    PhysicalSize::new(
      width.load(Ordering::Acquire) as u32,
      height.load(Ordering::Acquire) as u32,
    )
  }

  pub fn set_inner_size<S: Into<Size>>(&self, size: S) {
    let (width, height) = size.into().to_logical::<i32>(self.scale_factor()).into();

    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::Size((width, height))))
    {
      log::warn!("Fail to send size request: {}", e);
    }
  }

  pub fn outer_size(&self) -> PhysicalSize<u32> {
    let (width, height) = &*self.size;

    PhysicalSize::new(
      width.load(Ordering::Acquire) as u32,
      height.load(Ordering::Acquire) as u32,
    )
  }

  pub fn set_min_inner_size<S: Into<Size>>(&self, min_size: Option<S>) {
    if let Some(size) = min_size {
      let (min_width, min_height) = size.into().to_logical::<i32>(self.scale_factor()).into();

      if let Err(e) = self.window_requests_tx.send((
        self.window_id,
        WindowRequest::MinSize((min_width, min_height)),
      )) {
        log::warn!("Fail to send min size request: {}", e);
      }
    }
  }
  pub fn set_max_inner_size<S: Into<Size>>(&self, max_size: Option<S>) {
    if let Some(size) = max_size {
      let (max_width, max_height) = size.into().to_logical::<i32>(self.scale_factor()).into();

      if let Err(e) = self.window_requests_tx.send((
        self.window_id,
        WindowRequest::MaxSize((max_width, max_height)),
      )) {
        log::warn!("Fail to send max size request: {}", e);
      }
    }
  }

  pub fn set_title(&self, title: &str) {
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::Title(title.to_string())))
    {
      log::warn!("Fail to send title request: {}", e);
    }
  }

  pub fn set_visible(&self, visible: bool) {
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::Visible(visible)))
    {
      log::warn!("Fail to send visible request: {}", e);
    }
  }

  pub fn set_resizable(&self, resizable: bool) {
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::Resizable(resizable)))
    {
      log::warn!("Fail to send resizable request: {}", e);
    }
  }

  pub fn set_minimized(&self, minimized: bool) {
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::Minimized(minimized)))
    {
      log::warn!("Fail to send minimized request: {}", e);
    }
  }

  pub fn set_maximized(&self, maximized: bool) {
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::Maximized(maximized)))
    {
      log::warn!("Fail to send maximized request: {}", e);
    }
  }

  pub fn is_maximized(&self) -> bool {
    self.maximized.load(Ordering::Acquire)
  }

  pub fn drag_window(&self) {
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::DragWindow))
    {
      log::warn!("Fail to send drag window request: {}", e);
    }
  }

  pub fn set_fullscreen(&self, fullscreen: Option<Fullscreen>) {
    self.fullscreen.replace(fullscreen.clone());
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::Fullscreen(fullscreen)))
    {
      log::warn!("Fail to send fullscreen request: {}", e);
    }
  }

  pub fn fullscreen(&self) -> Option<Fullscreen> {
    self.fullscreen.borrow().clone()
  }

  pub fn set_decorations(&self, decorations: bool) {
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::Decorations(decorations)))
    {
      log::warn!("Fail to send decorations request: {}", e);
    }
  }

  pub fn set_always_on_top(&self, always_on_top: bool) {
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::AlwaysOnTop(always_on_top)))
    {
      log::warn!("Fail to send always on top request: {}", e);
    }
  }

  pub fn set_window_icon(&self, window_icon: Option<Icon>) {
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::WindowIcon(window_icon)))
    {
      log::warn!("Fail to send window icon request: {}", e);
    }
  }

  pub fn set_ime_position<P: Into<Position>>(&self, _position: P) {
    todo!()
  }

  pub fn request_user_attention(&self, request_type: Option<UserAttentionType>) {
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::UserAttention(request_type)))
    {
      log::warn!("Fail to send user attention request: {}", e);
    }
  }

  pub fn set_cursor_icon(&self, cursor: CursorIcon) {
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::CursorIcon(Some(cursor))))
    {
      log::warn!("Fail to send cursor icon request: {}", e);
    }
  }

  pub fn set_cursor_position<P: Into<Position>>(&self, _position: P) -> Result<(), ExternalError> {
    todo!()
  }

  pub fn set_cursor_visible(&self, visible: bool) {
    let cursor = if visible {
      Some(CursorIcon::Default)
    } else {
      None
    };
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::CursorIcon(cursor)))
    {
      log::warn!("Fail to send cursor visibility request: {}", e);
    }
  }

  pub fn current_monitor(&self) -> Option<MonitorHandle> {
    todo!()
  }

  // pub fn available_monitors(&self) -> impl Iterator<Item = MonitorHandle> {
  //   todo!()
  // }

  pub fn primary_monitor(&self) -> Option<MonitorHandle> {
    todo!()
  }
}

// We need GtkWindow to initialize WebView, so we have to keep it in the field.
// It is called on any method.
unsafe impl Send for Window {}
unsafe impl Sync for Window {}
