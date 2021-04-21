// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

use std::{
  cell::RefCell,
  fmt,
  sync::{
    atomic::{AtomicBool, AtomicI32, Ordering},
    mpsc::Sender,
  },
};

use gdk::{Cursor, CursorType, EventMask, WindowEdge, WindowExt};
use gtk::{prelude::*, ApplicationWindow};
use winit::{
  dpi::{PhysicalPosition, PhysicalSize, Position},
  window::{CursorIcon, UserAttentionType},
};

use super::{
  dpi::Size,
  error::{ExternalError, NotSupportedError, OsError},
  event_loop::EventLoopWindowTarget,
  monitor::{MonitorHandle, VideoMode},
};

pub use super::icon::{BadIcon, Icon};

/// Identifier of a window. Unique for each window.
///
/// Can be obtained with `window.id()`.
///
/// Whenever you receive an event specific to a window, this event contains a `WindowId` which you
/// can then compare to the ids of your windows.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WindowId(pub(crate) u32);

impl WindowId {
  /// Returns a dummy `WindowId`, useful for unit testing. The only guarantee made about the return
  /// value of this function is that it will always be equal to itself and to future values returned
  /// by this function.  No other guarantees are made. This may be equal to a real `WindowId`.
  ///
  /// **Passing this into a winit function will result in undefined behavior.**
  pub unsafe fn dummy() -> Self {
    WindowId(0)
  }
}

/// Attributes to use when creating a window.
#[derive(Debug, Clone)]
pub struct WindowAttributes {
  /// The dimensions of the window. If this is `None`, some platform-specific dimensions will be
  /// used.
  ///
  /// The default is `None`.
  pub inner_size: Option<Size>,

  /// The minimum dimensions a window can be, If this is `None`, the window will have no minimum dimensions (aside from reserved).
  ///
  /// The default is `None`.
  pub min_inner_size: Option<Size>,

  /// The maximum dimensions a window can be, If this is `None`, the maximum will have no maximum or will be set to the primary monitor's dimensions by the platform.
  ///
  /// The default is `None`.
  pub max_inner_size: Option<Size>,

  /// Whether the window is resizable or not.
  ///
  /// The default is `true`.
  pub resizable: bool,

  /// Whether the window should be set as fullscreen upon creation.
  ///
  /// The default is `None`.
  pub fullscreen: Option<Fullscreen>,

  /// The title of the window in the title bar.
  ///
  /// The default is `"winit window"`.
  pub title: String,

  /// Whether the window should be maximized upon creation.
  ///
  /// The default is `false`.
  pub maximized: bool,

  /// Whether the window should be immediately visible upon creation.
  ///
  /// The default is `true`.
  pub visible: bool,

  /// Whether the the window should be transparent. If this is true, writing colors
  /// with alpha values different than `1.0` will produce a transparent window.
  ///
  /// The default is `false`.
  pub transparent: bool,

  /// Whether the window should have borders and bars.
  ///
  /// The default is `true`.
  pub decorations: bool,

  /// Whether the window should always be on top of other windows.
  ///
  /// The default is `false`.
  pub always_on_top: bool,

  /// The window icon.
  ///
  /// The default is `None`.
  pub window_icon: Option<Icon>,
}

impl Default for WindowAttributes {
  #[inline]
  fn default() -> WindowAttributes {
    WindowAttributes {
      inner_size: None,
      min_inner_size: None,
      max_inner_size: None,
      resizable: true,
      title: "winit window".to_owned(),
      maximized: false,
      fullscreen: None,
      visible: true,
      transparent: false,
      decorations: true,
      always_on_top: false,
      window_icon: None,
    }
  }
}

/// Object that allows you to build windows.
#[derive(Clone, Default)]
pub struct WindowBuilder {
  /// The attributes to use to create the window.
  pub window: WindowAttributes,
}

impl fmt::Debug for WindowBuilder {
  fn fmt(&self, fmtr: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmtr
      .debug_struct("WindowBuilder")
      .field("window", &self.window)
      .finish()
  }
}

impl WindowBuilder {
  /// Initializes a new `WindowBuilder` with default values.
  #[inline]
  pub fn new() -> Self {
    Default::default()
  }

  /// Requests the window to be of specific dimensions.
  ///
  /// See [`Window::set_inner_size`] for details.
  ///
  /// [`Window::set_inner_size`]: crate::window::Window::set_inner_size
  #[inline]
  pub fn with_inner_size<S: Into<Size>>(mut self, size: S) -> Self {
    self.window.inner_size = Some(size.into());
    self
  }

  /// Sets a minimum dimension size for the window.
  ///
  /// See [`Window::set_min_inner_size`] for details.
  ///
  /// [`Window::set_min_inner_size`]: crate::window::Window::set_min_inner_size
  #[inline]
  pub fn with_min_inner_size<S: Into<Size>>(mut self, min_size: S) -> Self {
    self.window.min_inner_size = Some(min_size.into());
    self
  }

  /// Sets a maximum dimension size for the window.
  ///
  /// See [`Window::set_max_inner_size`] for details.
  ///
  /// [`Window::set_max_inner_size`]: crate::window::Window::set_max_inner_size
  #[inline]
  pub fn with_max_inner_size<S: Into<Size>>(mut self, max_size: S) -> Self {
    self.window.max_inner_size = Some(max_size.into());
    self
  }

  /// Sets whether the window is resizable or not.
  ///
  /// See [`Window::set_resizable`] for details.
  ///
  /// [`Window::set_resizable`]: crate::window::Window::set_resizable
  #[inline]
  pub fn with_resizable(mut self, resizable: bool) -> Self {
    self.window.resizable = resizable;
    self
  }

  /// Requests a specific title for the window.
  ///
  /// See [`Window::set_title`] for details.
  ///
  /// [`Window::set_title`]: crate::window::Window::set_title
  #[inline]
  pub fn with_title<T: Into<String>>(mut self, title: T) -> Self {
    self.window.title = title.into();
    self
  }

  /// Sets the window fullscreen state.
  ///
  /// See [`Window::set_fullscreen`] for details.
  ///
  /// [`Window::set_fullscreen`]: crate::window::Window::set_fullscreen
  #[inline]
  pub fn with_fullscreen(mut self, fullscreen: Option<Fullscreen>) -> Self {
    self.window.fullscreen = fullscreen;
    self
  }

  /// Requests maximized mode.
  ///
  /// See [`Window::set_maximized`] for details.
  ///
  /// [`Window::set_maximized`]: crate::window::Window::set_maximized
  #[inline]
  pub fn with_maximized(mut self, maximized: bool) -> Self {
    self.window.maximized = maximized;
    self
  }

  /// Sets whether the window will be initially hidden or visible.
  ///
  /// See [`Window::set_visible`] for details.
  ///
  /// [`Window::set_visible`]: crate::window::Window::set_visible
  #[inline]
  pub fn with_visible(mut self, visible: bool) -> Self {
    self.window.visible = visible;
    self
  }

  /// Sets whether the background of the window should be transparent.
  #[inline]
  pub fn with_transparent(mut self, transparent: bool) -> Self {
    self.window.transparent = transparent;
    self
  }

  /// Sets whether the window should have a border, a title bar, etc.
  ///
  /// See [`Window::set_decorations`] for details.
  ///
  /// [`Window::set_decorations`]: crate::window::Window::set_decorations
  #[inline]
  pub fn with_decorations(mut self, decorations: bool) -> Self {
    self.window.decorations = decorations;
    self
  }

  /// Sets whether or not the window will always be on top of other windows.
  ///
  /// See [`Window::set_always_on_top`] for details.
  ///
  /// [`Window::set_always_on_top`]: crate::window::Window::set_always_on_top
  #[inline]
  pub fn with_always_on_top(mut self, always_on_top: bool) -> Self {
    self.window.always_on_top = always_on_top;
    self
  }

  /// Sets the window icon.
  ///
  /// See [`Window::set_window_icon`] for details.
  ///
  /// [`Window::set_window_icon`]: crate::window::Window::set_window_icon
  #[inline]
  pub fn with_window_icon(mut self, window_icon: Option<Icon>) -> Self {
    self.window.window_icon = window_icon;
    self
  }

  /// Builds the window.
  ///
  /// Possible causes of error include denied permission, incompatible system, and lack of memory.
  ///
  /// Platform-specific behavior:
  /// - **Web**: The window is created but not inserted into the web page automatically. Please
  /// see the web platform module for more information.
  #[inline]
  pub fn build<T: 'static>(
    self,
    window_target: &EventLoopWindowTarget<T>,
  ) -> Result<Window, OsError> {
    Window::new(window_target, self.window)
    // TODO request redraw
  }
}

pub struct Window {
  /// Window id.
  pub(crate) window_id: WindowId,
  /// Gtk application window.
  pub(crate) window: gtk::ApplicationWindow,
  /// Window requests sender
  window_requests_tx: Sender<(WindowId, WindowRequest)>,
  scale_factor: f64,
  position: (AtomicI32, AtomicI32),
  size: (AtomicI32, AtomicI32),
  maximized: AtomicBool,
  fullscreen: RefCell<Option<Fullscreen>>,
}

impl Window {
  pub(crate) fn new<T>(
    event_loop_window_target: &EventLoopWindowTarget<T>,
    attributes: WindowAttributes,
  ) -> Result<Self, OsError> {
    let app = &event_loop_window_target.app;
    let window = gtk::ApplicationWindow::new(app);
    let window_id = WindowId(window.get_id());
    event_loop_window_target
      .windows
      .borrow_mut()
      .insert(window_id);

    // Set Width/Height & Resizable
    let scale_factor = window.get_scale_factor();
    let (width, height) = attributes
      .inner_size
      .map(|size| size.to_logical::<f64>(scale_factor as f64).into())
      .unwrap_or((800, 600));
    window.set_resizable(attributes.resizable);
    if attributes.resizable {
      window.set_default_size(width, height);
    } else {
      window.set_size_request(width, height);
    }

    // Set Min/Max Size
    let geom_mask = (if attributes.min_inner_size.is_some() {
      gdk::WindowHints::MIN_SIZE
    } else {
      gdk::WindowHints::empty()
    }) | (if attributes.max_inner_size.is_some() {
      gdk::WindowHints::MAX_SIZE
    } else {
      gdk::WindowHints::empty()
    });
    let (min_width, min_height) = attributes
      .min_inner_size
      .map(|size| size.to_logical::<f64>(scale_factor as f64).into())
      .unwrap_or_default();
    let (max_width, max_height) = attributes
      .max_inner_size
      .map(|size| size.to_logical::<f64>(scale_factor as f64).into())
      .unwrap_or_default();
    window.set_geometry_hints::<ApplicationWindow>(
      None,
      Some(&gdk::Geometry {
        min_width,
        min_height,
        max_width,
        max_height,
        base_width: 0,
        base_height: 0,
        width_inc: 0,
        height_inc: 0,
        min_aspect: 0f64,
        max_aspect: 0f64,
        win_gravity: gdk::Gravity::Center,
      }),
      geom_mask,
    );

    // Set Transparent
    if attributes.transparent {
      if let Some(screen) = window.get_screen() {
        if let Some(visual) = screen.get_rgba_visual() {
          window.set_visual(Some(&visual));
        }
      }

      window.connect_draw(|_, cr| {
        cr.set_source_rgba(0., 0., 0., 0.);
        cr.set_operator(cairo::Operator::Source);
        cr.paint();
        cr.set_operator(cairo::Operator::Over);
        Inhibit(false)
      });
      window.set_app_paintable(true);
    }

    // Rest attributes
    window.set_title(&attributes.title);
    if attributes.fullscreen.is_some() {
      window.fullscreen();
    }
    if attributes.maximized {
      window.maximize();
    }
    window.set_visible(attributes.visible);
    window.set_decorated(attributes.decorations);

    if !attributes.decorations && attributes.resizable {
      window.add_events(EventMask::POINTER_MOTION_MASK | EventMask::BUTTON_MOTION_MASK);

      window.connect_motion_notify_event(|window, event| {
        if let Some(gdk_window) = window.get_window() {
          let (cx, cy) = event.get_root();
          hit_test(&gdk_window, cx, cy);
        }
        Inhibit(false)
      });

      window.connect_button_press_event(|window, event| {
        if event.get_button() == 1 {
          if let Some(gdk_window) = window.get_window() {
            let (cx, cy) = event.get_root();
            let result = hit_test(&gdk_window, cx, cy);

            // this check is necessary, otherwise the window won't recieve the click properly when resize isn't needed
            if result != WindowEdge::__Unknown(8) {
              window.begin_resize_drag(result, 1, cx as i32, cy as i32, event.get_time());
            }
          }
        }
        Inhibit(false)
      });
    }

    window.set_keep_above(attributes.always_on_top);
    if let Some(icon) = &attributes.window_icon {
      window.set_icon(Some(&icon.inner));
    }

    window.show_all();

    let window_requests_tx = event_loop_window_target.window_requests_tx.clone();
    let scale_factor = scale_factor as f64;
    let position = window.get_position();
    let maximized = window.get_property_is_maximized().into();
    Ok(Self {
      window_id,
      window,
      window_requests_tx,
      scale_factor,
      position: (position.0.into(), position.1.into()),
      size: (width.into(), height.into()),
      maximized,
      fullscreen: RefCell::new(attributes.fullscreen),
    })
  }

  pub fn id(&self) -> WindowId {
    self.window_id
  }

  pub fn scale_factor(&self) -> f64 {
    self.scale_factor
  }

  pub fn request_redraw(&self) {
    todo!()
  }

  pub fn inner_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
    let (x, y) = &self.position;
    Ok(PhysicalPosition::new(
      x.load(Ordering::Acquire),
      y.load(Ordering::Acquire),
    ))
  }

  pub fn outer_position(&self) -> Result<PhysicalPosition<i32>, NotSupportedError> {
    let (x, y) = &self.position;
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
    self.position.0.store(x, Ordering::Release);
    self.position.1.store(y, Ordering::Release);

    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::Position((x, y))))
    {
      log::warn!("Fail to send position request: {}", e);
    }
  }

  pub fn inner_size(&self) -> PhysicalSize<u32> {
    let (width, height) = &self.size;
    PhysicalSize::new(
      width.load(Ordering::Acquire) as u32,
      height.load(Ordering::Acquire) as u32,
    )
  }

  pub fn set_inner_size<S: Into<Size>>(&self, size: S) {
    let (width, height) = size.into().to_logical::<i32>(self.scale_factor()).into();
    self.size.0.store(width, Ordering::Release);
    self.size.1.store(height, Ordering::Release);

    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::Size((width, height))))
    {
      log::warn!("Fail to send size request: {}", e);
    }
  }

  pub fn outer_size(&self) -> PhysicalSize<u32> {
    let (width, height) = &self.size;
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
    self.maximized.store(maximized, Ordering::Acquire);
    if let Err(e) = self
      .window_requests_tx
      .send((self.window_id, WindowRequest::Maximized(maximized)))
    {
      log::warn!("Fail to send maximized request: {}", e);
    }
  }

  pub fn is_maximized(&self) -> bool {
    self.maximized.load(Ordering::Release)
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

  pub fn set_cursor_icon(&self, _cursor: CursorIcon) {
    todo!()
  }

  pub fn set_cursor_position<P: Into<Position>>(&self, _position: P) -> Result<(), ExternalError> {
    todo!()
  }

  pub fn set_cursor_visible(&self, _visible: bool) {
    todo!()
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

  //TODO other setters
}

/// Fullscreen modes.
#[derive(Clone, Debug, PartialEq)]
pub enum Fullscreen {
  Exclusive(VideoMode),

  /// Providing `None` to `Borderless` will fullscreen on the current monitor.
  Borderless(Option<MonitorHandle>),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Theme {
  Light,
  Dark,
}

pub(crate) enum WindowRequest {
  Title(String),
  Position((i32, i32)),
  Size((i32, i32)),
  MinSize((i32, i32)),
  MaxSize((i32, i32)),
  Visible(bool),
  Resizable(bool),
  Minimized(bool),
  Maximized(bool),
  DragWindow,
  Fullscreen(Option<Fullscreen>),
  Decorations(bool),
  AlwaysOnTop(bool),
  WindowIcon(Option<Icon>),
  UserAttention(Option<UserAttentionType>),
}

pub(crate) fn hit_test(window: &gdk::Window, cx: f64, cy: f64) -> WindowEdge {
  let (left, top) = window.get_position();
  let (w, h) = (window.get_width(), window.get_height());
  let (right, bottom) = (left + w, top + h);

  let fake_border = 5; // change this to manipulate how far inside the window, the resize can happen

  let display = window.get_display();

  const LEFT: i32 = 00001;
  const RIGHT: i32 = 0b0010;
  const TOP: i32 = 0b0100;
  const BOTTOM: i32 = 0b1000;
  const TOPLEFT: i32 = TOP | LEFT;
  const TOPRIGHT: i32 = TOP | RIGHT;
  const BOTTOMLEFT: i32 = BOTTOM | LEFT;
  const BOTTOMRIGHT: i32 = BOTTOM | RIGHT;

  let result = LEFT
    * (if (cx as i32) < (left + fake_border) {
      1
    } else {
      0
    })
    | RIGHT
      * (if (cx as i32) >= (right - fake_border) {
        1
      } else {
        0
      })
    | TOP
      * (if (cy as i32) < (top + fake_border) {
        1
      } else {
        0
      })
    | BOTTOM
      * (if (cy as i32) >= (bottom - fake_border) {
        1
      } else {
        0
      });

  let edge = match result {
    LEFT => WindowEdge::West,
    TOP => WindowEdge::North,
    RIGHT => WindowEdge::East,
    BOTTOM => WindowEdge::South,
    TOPLEFT => WindowEdge::NorthWest,
    TOPRIGHT => WindowEdge::NorthEast,
    BOTTOMLEFT => WindowEdge::SouthWest,
    BOTTOMRIGHT => WindowEdge::SouthEast,
    // has to be bigger than 7. otherwise it will match the number with a variant of WindowEdge enum and we don't want to do that
    // also if the number ever change, makke sure to change it in the connect_button_press_event for window and webview
    _ => WindowEdge::__Unknown(8),
  };

  window.set_cursor(Some(&Cursor::new_for_display(
    &display,
    match edge {
      WindowEdge::North | WindowEdge::South => CursorType::SbVDoubleArrow,
      WindowEdge::East | WindowEdge::West => CursorType::SbHDoubleArrow,
      WindowEdge::NorthWest => CursorType::TopLeftCorner,
      WindowEdge::NorthEast => CursorType::TopRightCorner,
      WindowEdge::SouthEast => CursorType::BottomRightCorner,
      WindowEdge::SouthWest => CursorType::BottomLeftCorner,
      _ => CursorType::LeftPtr,
    },
  )));

  edge
}
