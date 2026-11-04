//! Poly Box's sound engine: oscillators, the SEM-style filter, envelopes,
//! LFOs, the modulation matrix and the five voices, plus [`keys`] for playing
//! it from a window. Free of any plug-in framework or GUI toolkit, so the
//! plug-ins, the app and the offline renderer all share it.

pub mod env;
pub mod filter;
pub mod keys;
pub mod lfo;
pub mod modulation;
pub mod osc;
pub mod synth;
pub mod voice;

pub use lfo::{LfoSettings, LfoShape};
pub use modulation::{ModDest, ModSlot, ModSource, MOD_SLOTS};
pub use synth::{Synth, VOICES};
pub use voice::{Patch, VoiceMode};
