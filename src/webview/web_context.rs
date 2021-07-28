use std::path::{Path, PathBuf};

/// A context that is shared between multiple [`WebView`]s.
///
/// A browser would have a context for all the normal tabs and a different context for all the
/// private/incognito tabs.
///
/// [`WebView`]: crate::webview::WebView
#[derive(Debug)]
pub struct WebContext {
  data: WebContextData,
  #[allow(dead_code)] // It's not needed on Windows and macOS.
  os: WebContextImpl,
}

impl WebContext {
  /// Create a new [`WebContext`].
  ///
  /// `data_directory`:
  /// * Whether the WebView window should have a custom user data path. This is useful in Windows
  ///   when a bundled application can't have the webview data inside `Program Files`.
  pub fn new(data_directory: Option<PathBuf>) -> Self {
    let data = WebContextData { data_directory };
    let os = WebContextImpl::new(&data);
    Self { data, os }
  }

  /// A reference to the data directory the context was created with.
  pub fn data_directory(&self) -> Option<&Path> {
    self.data.data_directory()
  }

  /// Set if this context allows automation.
  ///
  /// **Note:** This is currently only enforced on Linux, and has the stipulation that
  /// only 1 context allows automation at a time.
  pub fn set_allows_automation(&mut self, flag: bool) {
    self.os.set_allows_automation(flag);
  }
}

impl Default for WebContext {
  fn default() -> Self {
    let data = WebContextData::default();
    let os = WebContextImpl::new(&data);
    Self { data, os }
  }
}

/// Data that all [`WebContext`] share regardless of platform.
#[derive(Debug, Default)]
struct WebContextData {
  data_directory: Option<PathBuf>,
}

impl WebContextData {
  /// A reference to the data directory the context was created with.
  pub fn data_directory(&self) -> Option<&Path> {
    self.data_directory.as_deref()
  }
}

#[cfg(not(any(
  target_os = "linux",
  target_os = "dragonfly",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd"
)))]
#[derive(Debug)]
struct WebContextImpl;

#[cfg(not(any(
  target_os = "linux",
  target_os = "dragonfly",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd"
)))]
impl WebContextImpl {
  fn new(_data: &WebContextData) -> Self {
    Self
  }

  fn set_allows_automation(&mut self, _flag: bool) {}
}

#[cfg(any(
  target_os = "linux",
  target_os = "dragonfly",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd"
))]
use self::unix::WebContextImpl;

#[cfg(any(
  target_os = "linux",
  target_os = "dragonfly",
  target_os = "freebsd",
  target_os = "netbsd",
  target_os = "openbsd"
))]
pub mod unix {
  //! Unix platform extensions for [`WebContext`](super::WebContext).

  use crate::{application::window::Window, Error};
  use glib::FileError;
  use std::{
    collections::{HashSet, VecDeque},
    rc::Rc,
    sync::{
      atomic::{AtomicBool, Ordering::SeqCst},
      Mutex,
    },
  };
  use url::Url;
  use webkit2gtk::{
    ApplicationInfo, AutomationSessionExt, LoadEvent, SecurityManagerExt, URISchemeRequestExt,
    UserContentManager, WebContext, WebContextBuilder, WebContextExt as WebkitWebContextExt,
    WebView, WebViewExt, WebsiteDataManagerBuilder,
  };

  #[derive(Debug)]
  pub(super) struct WebContextImpl {
    context: WebContext,
    manager: UserContentManager,
    webview_uri_loader: Rc<WebviewUriLoader>,
    registered_protocols: Mutex<HashSet<String>>,
    automation: bool,
  }

  impl WebContextImpl {
    pub fn new(data: &super::WebContextData) -> Self {
      let mut context_builder = WebContextBuilder::new();
      if let Some(data_directory) = data.data_directory() {
        let data_manager = WebsiteDataManagerBuilder::new()
          .local_storage_directory(
            &data_directory
              .join("localstorage")
              .to_string_lossy()
              .into_owned(),
          )
          .indexeddb_directory(
            &data_directory
              .join("databases")
              .join("indexeddb")
              .to_string_lossy()
              .into_owned(),
          )
          .build();
        context_builder = context_builder.website_data_manager(&data_manager);
      }

      let context = context_builder.build();

      let automation = false;
      context.set_automation_allowed(automation);

      // e.g. wry 0.9.4
      let app_info = ApplicationInfo::new();
      app_info.set_name(env!("CARGO_PKG_NAME"));
      app_info.set_version(
        env!("CARGO_PKG_VERSION_MAJOR")
          .parse()
          .expect("invalid wry version major"),
        env!("CARGO_PKG_VERSION_MINOR")
          .parse()
          .expect("invalid wry version minor"),
        env!("CARGO_PKG_VERSION_PATCH")
          .parse()
          .expect("invalid wry version patch"),
      );

      context.connect_automation_started(move |_, auto| {
        auto.set_application_info(&app_info);

        // we do **NOT** support arbitrarily creating new webviews
        // to support this in the future, we would need a way to specify the
        // default WindowBuilder to use to create the window it will use, and
        // possibly "default" webview attributes. difficulty comes in for controlling
        // the owned window that would need to be used.
        auto.connect_create_web_view(move |_| unimplemented!());
      });

      Self {
        context,
        automation,
        manager: UserContentManager::new(),
        registered_protocols: Default::default(),
        webview_uri_loader: Rc::default(),
      }
    }

    pub fn set_allows_automation(&mut self, flag: bool) {
      self.automation = flag;
      self.context.set_automation_allowed(flag);
    }
  }

  /// [`WebContext`](super::WebContext) items that only matter on unix.
  pub trait WebContextExt {
    /// The GTK [`WebContext`] of all webviews in the context.
    fn context(&self) -> &WebContext;

    /// The GTK [`UserContentManager`] of all webviews in the context.
    fn manager(&self) -> &UserContentManager;

    /// Register a custom protocol to the web context
    fn register_uri_scheme<F>(
      &self,
      name: &str,
      handler: F,
      window: Rc<Window>,
    ) -> crate::Result<()>
    where
      F: Fn(&Window, &str) -> crate::Result<(Vec<u8>, String)> + 'static;

    /// Add a [`WebView`] to the queue waiting to be opened.
    ///
    /// Using the queue prevents data race issues with loading uris for multiple [`WebView`]s in the
    /// same context at the same time. Occasionally, the one of the [`WebView`]s will be clobbered
    /// and it's content will be injected into a different [`WebView`].
    ///
    /// Example of `webview-c` clobbering `webview-b` while `webview-a` is okay:
    /// ```text
    /// webview-a triggers load-change::started
    /// URISchemeRequestCallback triggered with webview-a
    /// webview-a triggers load-change::committed
    /// webview-a triggers load-change::finished
    /// webview-b triggers load-change::started
    /// webview-c triggers load-change::started
    /// URISchemeRequestCallback triggered with webview-c
    /// URISchemeRequestCallback triggered with webview-c
    /// webview-c triggers load-change::committed
    /// webview-c triggers load-change::finished
    /// ```
    ///
    /// In that example, `webview-a` will load fine. `webview-b` will remain empty as the uri was
    /// never loaded. `webview-c` will contain the content of both `webview-b` and `webview-c`
    /// because it was triggered twice even through only started once. The content injected will not
    /// be sequential, and often is interjected in the middle of one of the other contents.
    fn queue_load_uri(&self, webview: Rc<WebView>, url: Url);

    /// Flush all queued [`WebView`]s waiting to load a uri.
    fn flush_queue_loader(&self);

    /// If the context allows automation.
    ///
    /// **Note:** `libwebkit2gtk` only allows 1 automation context at a time.
    fn allows_automation(&self) -> bool;
  }

  impl WebContextExt for super::WebContext {
    fn context(&self) -> &WebContext {
      &self.os.context
    }

    fn manager(&self) -> &UserContentManager {
      &self.os.manager
    }

    fn register_uri_scheme<F>(
      &self,
      name: &str,
      handler: F,
      window: Rc<Window>,
    ) -> crate::Result<()>
    where
      F: Fn(&Window, &str) -> crate::Result<(Vec<u8>, String)> + 'static,
    {
      let mut protocols = self
        .os
        .registered_protocols
        .lock()
        .expect("poisoned protocol hashset");
      let exists = !protocols.insert(name.to_string());
      drop(protocols);

      if exists {
        Err(Error::DuplicateCustomProtocol(name.to_string()))
      } else {
        let context = &self.os.context;
        context
          .get_security_manager()
          .ok_or(Error::MissingManager)?
          .register_uri_scheme_as_secure(name);

        context.register_uri_scheme(name, move |request| {
          if let Some(uri) = request.get_uri() {
            let uri = uri.as_str();

            match handler(&window, uri) {
              Ok((buffer, mime)) => {
                let input = gio::MemoryInputStream::from_bytes(&glib::Bytes::from(&buffer));
                request.finish(&input, buffer.len() as i64, Some(&mime))
              }
              Err(_) => request.finish_error(&mut glib::Error::new(
                FileError::Exist,
                "Could not get requested file.",
              )),
            }
          } else {
            request.finish_error(&mut glib::Error::new(
              FileError::Exist,
              "Could not get uri.",
            ));
          }
        });
        Ok(())
      }
    }

    fn queue_load_uri(&self, webview: Rc<WebView>, url: Url) {
      self.os.webview_uri_loader.push(webview, url)
    }

    fn flush_queue_loader(&self) {
      Rc::clone(&self.os.webview_uri_loader).flush()
    }

    fn allows_automation(&self) -> bool {
      self.os.automation
    }
  }

  #[derive(Debug, Default)]
  struct WebviewUriLoader {
    lock: AtomicBool,
    queue: Mutex<VecDeque<(Rc<WebView>, Url)>>,
  }

  impl WebviewUriLoader {
    /// Check if the lock is in use.
    fn is_locked(&self) -> bool {
      self.lock.swap(true, SeqCst)
    }

    /// Unlock the lock.
    fn unlock(&self) {
      self.lock.store(false, SeqCst)
    }

    /// Add a [`WebView`] to the queue.
    fn push(&self, webview: Rc<WebView>, url: Url) {
      let mut queue = self.queue.lock().expect("poisoned load queue");
      queue.push_back((webview, url))
    }

    /// Remove a [`WebView`] from the queue and return it.
    fn pop(&self) -> Option<(Rc<WebView>, Url)> {
      let mut queue = self.queue.lock().expect("poisoned load queue");
      queue.pop_front()
    }

    /// Load the next uri to load if the lock is not engaged.
    fn flush(self: Rc<Self>) {
      if !self.is_locked() {
        if let Some((webview, url)) = self.pop() {
          // we do not need to listen to failed events because those will finish the change event anyways
          webview.connect_load_changed(move |_, event| {
            if let LoadEvent::Finished = event {
              self.unlock();
              Rc::clone(&self).flush();
            };
          });

          webview.load_uri(url.as_str());
        }
      }
    }
  }
}
