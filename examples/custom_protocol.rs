// Copyright 2019-2021 Tauri Programme within The Commons Conservancy
// SPDX-License-Identifier: Apache-2.0
// SPDX-License-Identifier: MIT

fn main() -> wry::Result<()> {
  use std::fs::{canonicalize, read};

  use wry::{
    application::{
      event::{Event, StartCause, WindowEvent},
      event_loop::{ControlFlow, EventLoop},
      window::WindowBuilder,
    },
    webview::WebViewBuilder,
  };

  #[allow(dead_code)]
  enum WebviewEvent {
    Focus(bool),
  }
  let event_loop = EventLoop::<WebviewEvent>::with_user_event();
  let window = WindowBuilder::new()
    .with_title("Hello World")
    .build(&event_loop)
    .unwrap();

  let _webview = WebViewBuilder::new(window)
    .unwrap()
    .with_custom_protocol("wry".into(), move |_, requested_asset_path| {
      // Remove url scheme
      let path = requested_asset_path.replace("wry://", "");
      // Read the file content from file path
      let content = read(canonicalize(&path)?)?;

      // Return asset contents and mime types based on file extentions
      // If you don't want to do this manually, there are some crates for you.
      // Such as `infer` and `mime_guess`.
      if path.ends_with(".html") {
        Ok((content, String::from("text/html")))
      } else if path.ends_with(".js") {
        Ok((content, String::from("text/javascript")))
      } else if path.ends_with(".png") {
        Ok((content, String::from("image/png")))
      } else {
        unimplemented!();
      }
    })
    // tell the webview to load the custom protocol
    .with_url("wry://examples/index.html")?
    .build()?;

  #[cfg(target_os = "windows")]
  {
    let proxy = event_loop.create_proxy();
    let proxy_c = proxy.clone();
    _webview
      .add_got_focus(move || {
        let _ = proxy_c.send_event(WebviewEvent::Focus(true));
      })
      .unwrap();
    _webview
      .add_lost_focus(move || {
        let _ = proxy.send_event(WebviewEvent::Focus(false));
      })
      .unwrap();
  }

  event_loop.run(move |event, _, control_flow| {
    *control_flow = ControlFlow::Wait;

    match event {
      Event::NewEvents(StartCause::Init) => {
        println!("Wry application started!");

        // we also need to call `.focus()` at the start so the webview control gains focus
        #[cfg(target_os = "windows")]
        let _ = _webview.focus();
      }
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => *control_flow = ControlFlow::Exit,
      Event::WindowEvent {
        event: WindowEvent::Focused(focus),
        ..
      } => {
        if focus {
          #[cfg(target_os = "windows")]
          let _ = _webview.focus();
        }
      }
      Event::UserEvent(event) => match event {
        WebviewEvent::Focus(focus) => {
          if focus {
            println!("Got Focus")
          } else {
            println!("Lost Focus")
          }
        }
      },
      _ => (),
    }
  });
}
