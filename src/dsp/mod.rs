//! The sound engine, free of any plug-in framework so it can be tested and
//! rendered offline on its own.

pub mod env;
pub mod filter;
pub mod lfo;
pub mod modulation;
pub mod osc;
pub mod synth;
pub mod voice;

pub use lfo::{LfoSettings, LfoShape};
pub use modulation::{ModDest, ModSlot, ModSource, MOD_SLOTS};
pub use synth::{Synth, VOICES};
pub use voice::{Patch, VoiceMode};
