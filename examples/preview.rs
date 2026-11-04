//! The plug-in's window on its own, without a host or an audio device:
//!
//! ```sh
//! cargo run --release --example preview
//! ```
//!
//! The synth runs silently behind it, so the computer keys and the on-screen
//! keyboard light the voices just as in the real thing. The sliders show the
//! controls but don't change anything here (there's no host to tell). It
//! uses eframe instead of the plug-in's own window code, so it also works
//! under Xvfb in a cloud session, where the plug-in window can't open.

use nih_plug::prelude::{GuiContext, ParamPtr, ParamSetter, PluginApi, PluginState};
use poly_box::dsp::{Patch, Synth};
use poly_box::editor::{draw, KeyBridge, Keyboard, Live};
use poly_box::PolyBoxParams;
use std::sync::Arc;
use std::time::Duration;

/// Stands in for the host.
struct NoHost;

impl GuiContext for NoHost {
    fn plugin_api(&self) -> PluginApi {
        PluginApi::Standalone
    }
    fn request_resize(&self) -> bool {
        false
    }
    unsafe fn raw_begin_set_parameter(&self, _: ParamPtr) {}
    unsafe fn raw_set_parameter_normalized(&self, _: ParamPtr, _: f32) {}
    unsafe fn raw_end_set_parameter(&self, _: ParamPtr) {}
    fn get_state(&self) -> PluginState {
        PluginState {
            version: String::new(),
            params: Default::default(),
            fields: Default::default(),
        }
    }
    fn set_state(&self, _: PluginState) {}
}

struct Preview {
    params: Arc<PolyBoxParams>,
    host: NoHost,
    live: Arc<Live>,
    keyboard: Keyboard,
}

impl eframe::App for Preview {
    fn update(&mut self, ctx: &eframe::egui::Context, _: &mut eframe::Frame) {
        let setter = ParamSetter::new(&self.host);
        draw(
            ctx,
            self.params.clone(),
            &setter,
            &self.live,
            &mut self.keyboard,
        );
    }
}

fn main() -> eframe::Result {
    let live = Arc::new(Live::default());

    // The audio thread, minus the audio.
    let shared = live.clone();
    std::thread::spawn(move || {
        let patch = Patch::default();
        let mut synth = Synth::new(48_000.0);
        synth.set_block(&patch);
        let mut keys = KeyBridge::default();
        loop {
            keys.take_window_keys(&shared, &mut synth);
            for _ in 0..256 {
                synth.next(&patch);
            }
            keys.publish(&shared, &synth);
            std::thread::sleep(Duration::from_millis(5));
        }
    });

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([820.0, 760.0])
            .with_title("Poly Box (preview)"),
        ..Default::default()
    };
    eframe::run_native(
        "Poly Box preview",
        options,
        Box::new(move |_| {
            Ok(Box::new(Preview {
                params: Arc::new(PolyBoxParams::default()),
                host: NoHost,
                live,
                keyboard: Keyboard::default(),
            }))
        }),
    )
}
