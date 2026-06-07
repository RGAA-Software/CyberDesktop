use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use tracing::{error, info};

#[cfg(windows)]
use windows::Win32::Media::Audio::{
    eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator,
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, MMDeviceEnumerator,
    WAVEFORMATEXTENSIBLE, WAVE_FORMAT_PCM,
};
#[cfg(windows)]
use windows::Win32::Media::Multimedia::WAVE_FORMAT_IEEE_FLOAT as MM_WAVE_FORMAT_IEEE_FLOAT;
#[cfg(windows)]
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_APARTMENTTHREADED,
};

const FFT_SIZE: usize = 2048;
pub const SPECTRUM_BANDS: usize = 128;

pub struct AudioVisualizer {
    spectrum: Arc<Mutex<Vec<f32>>>,
    running: Arc<AtomicBool>,
    _thread: JoinHandle<()>,
}

impl AudioVisualizer {
    pub fn start() -> Option<Self> {
        #[cfg(not(windows))]
        return None;

        #[cfg(windows)]
        {
            let spectrum = Arc::new(Mutex::new(vec![0.0f32; SPECTRUM_BANDS]));
            let running = Arc::new(AtomicBool::new(true));
            let spectrum_clone = spectrum.clone();
            let running_clone = running.clone();

            let thread = thread::spawn(move || {
                if let Err(e) = run_loopback_capture(spectrum_clone, running_clone) {
                    error!("Audio visualizer capture error: {}", e);
                }
            });

            Some(Self {
                spectrum,
                running,
                _thread: thread,
            })
        }
    }

    pub fn spectrum(&self) -> Vec<f32> {
        self.spectrum.lock().unwrap().clone()
    }
}

impl Drop for AudioVisualizer {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

#[cfg(windows)]
fn run_loopback_capture(
    spectrum: Arc<Mutex<Vec<f32>>>,
    running: Arc<AtomicBool>,
) -> anyhow::Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .map_err(|e| anyhow::anyhow!("CoInitializeEx failed: {e}"))?;

        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
            .map_err(|e| anyhow::anyhow!("CoCreateInstance MMDeviceEnumerator failed: {e}"))?;

        let device = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|e| anyhow::anyhow!("GetDefaultAudioEndpoint failed: {e}"))?;

        let audio_client: IAudioClient = device
            .Activate(CLSCTX_ALL, None)
            .map_err(|e| anyhow::anyhow!("Activate IAudioClient failed: {e}"))?;

        let mix_format_ptr = audio_client
            .GetMixFormat()
            .map_err(|e| anyhow::anyhow!("GetMixFormat failed: {e}"))?;

        if mix_format_ptr.is_null() {
            anyhow::bail!("GetMixFormat returned null");
        }

        let mix_format = &*mix_format_ptr;
        let n_channels = mix_format.nChannels;
        let sample_rate_val = mix_format.nSamplesPerSec;
        let format_tag_val = mix_format.wFormatTag;
        info!(
            "WASAPI mix format: {n_channels} channels, {sample_rate_val} Hz, format_tag={format_tag_val}"
        );

        let buffer_duration: i64 = 10_000_000i64; // 1 second in 100-nanosecond units

        audio_client
            .Initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK,
                buffer_duration,
                0i64,
                mix_format_ptr,
                None,
            )
            .map_err(|e| anyhow::anyhow!("IAudioClient::Initialize failed: {e}"))?;

        let capture_client: IAudioCaptureClient = audio_client
            .GetService()
            .map_err(|e| anyhow::anyhow!("GetService IAudioCaptureClient failed: {e}"))?;

        audio_client
            .Start()
            .map_err(|e| anyhow::anyhow!("IAudioClient::Start failed: {e}"))?;

        info!("WASAPI loopback capture started");

        let channels = mix_format.nChannels as usize;
        let _sample_rate = mix_format.nSamplesPerSec as usize;
        let format_tag = mix_format.wFormatTag;
        let bits_per_sample = mix_format.wBitsPerSample;
        let block_align = mix_format.nBlockAlign;
        let n_channels_val = mix_format.nChannels;
        const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;
        let subtype_ieee_float = windows::core::GUID::from_values(0x00000003, 0x0000, 0x0010, [0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71]);
        let subtype_pcm = windows::core::GUID::from_values(0x00000001, 0x0000, 0x0010, [0x80, 0x00, 0x00, 0xAA, 0x00, 0x38, 0x9B, 0x71]);
        let (mut is_float, mut is_16bit) = if format_tag == WAVE_FORMAT_PCM as u16 {
            (false, true)
        } else if format_tag == MM_WAVE_FORMAT_IEEE_FLOAT as u16 {
            (true, false)
        } else if format_tag == WAVE_FORMAT_EXTENSIBLE {
            let ext = &*(mix_format_ptr as *const WAVEFORMATEXTENSIBLE);
            let guid = ext.SubFormat;
            if guid == subtype_ieee_float {
                (true, false)
            } else if guid == subtype_pcm {
                (false, true)
            } else {
                (false, false)
            }
        } else {
            (false, false)
        };
        // Fallback heuristic based on bit depth
        if !is_float && !is_16bit {
            let bytes_per_sample = block_align / n_channels_val;
            if bytes_per_sample == 4 || bits_per_sample == 32 {
                is_float = true;
            } else if bytes_per_sample == 2 || bits_per_sample == 16 {
                is_16bit = true;
            }
        }
        info!("WASAPI format parsed: is_float={is_float}, is_16bit={is_16bit}, tag={format_tag}, bits={bits_per_sample}");

        let mut sample_buffer: Vec<f32> = Vec::with_capacity(FFT_SIZE * 2);
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);
        let mut fft_buffer = vec![Complex::new(0.0f32, 0.0f32); FFT_SIZE];
        let mut window = vec![0.0f32; FFT_SIZE];
        for i in 0..FFT_SIZE {
            window[i] = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / FFT_SIZE as f32).cos();
        }

        let mut new_spectrum = vec![0.0f32; SPECTRUM_BANDS];

        while running.load(Ordering::Relaxed) {
            let packet_length_result = capture_client.GetNextPacketSize();
            if packet_length_result.is_err() {
                thread::sleep(Duration::from_millis(5));
                continue;
            }

            let mut packet_length = packet_length_result.unwrap_or(0);
            while packet_length > 0 {
                let mut data_ptr = std::ptr::null_mut();
                let mut frames_available = 0u32;
                let mut flags = 0u32;

                if capture_client
                    .GetBuffer(&mut data_ptr, &mut frames_available, &mut flags, None, None)
                    .is_err()
                {
                    break;
                }

                if !data_ptr.is_null() && frames_available > 0 {
                    let sample_count = frames_available as usize * channels;
                    if is_float {
                        let samples = std::slice::from_raw_parts(data_ptr as *const f32, sample_count);
                        for frame in 0..frames_available as usize {
                            let mut sum = 0.0f32;
                            for ch in 0..channels {
                                sum += samples[frame * channels + ch];
                            }
                            sample_buffer.push(sum / channels as f32);
                        }
                    } else if is_16bit {
                        let samples = std::slice::from_raw_parts(data_ptr as *const i16, sample_count);
                        for frame in 0..frames_available as usize {
                            let mut sum = 0.0f32;
                            for ch in 0..channels {
                                sum += samples[frame * channels + ch] as f32 / 32768.0;
                            }
                            sample_buffer.push(sum / channels as f32);
                        }
                    }
                }

                let _ = capture_client.ReleaseBuffer(frames_available);

                while sample_buffer.len() >= FFT_SIZE {
                    for i in 0..FFT_SIZE {
                        fft_buffer[i] = Complex::new(sample_buffer[i] * window[i], 0.0);
                    }
                    fft.process(&mut fft_buffer);
                    compute_spectrum(&fft_buffer, &mut new_spectrum);

                    {
                        let mut spec = spectrum.lock().unwrap();
                        spec.copy_from_slice(&new_spectrum);
                    }

                    let overlap = FFT_SIZE / 2;
                    sample_buffer.drain(0..overlap);
                }

                match capture_client.GetNextPacketSize() {
                    Ok(size) => packet_length = size,
                    Err(_) => break,
                }
            }

            thread::sleep(Duration::from_millis(5));
        }

        let _ = audio_client.Stop();
        info!("WASAPI loopback capture stopped");
        CoTaskMemFree(Some(mix_format_ptr as *const std::ffi::c_void));
    }

    Ok(())
}

fn compute_spectrum(fft_buffer: &[Complex<f32>], spectrum: &mut [f32]) {
    for i in 0..spectrum.len() {
        spectrum[i] = fft_buffer[i].norm();
    }

    let max_val = spectrum.iter().copied().fold(0.0f32, f32::max).max(0.001);
    for val in spectrum.iter_mut() {
        *val = (*val / max_val).min(1.0);
    }
}
