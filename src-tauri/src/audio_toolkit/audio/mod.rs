mod activity_gate;
// Re-export all audio components
mod device;
mod recorder;
mod resampler;
mod utils;
mod visualizer;

pub use activity_gate::{analyze_activity, should_skip_transcription, AudioActivityStats};
pub use device::{list_input_devices, list_output_devices, CpalDeviceInfo};
pub use recorder::AudioRecorder;
pub use resampler::FrameResampler;
pub use utils::save_wav_file;
pub use visualizer::AudioVisualiser;
