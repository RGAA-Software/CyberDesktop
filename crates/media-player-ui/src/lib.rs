#[cfg(feature = "full-app")]
mod audio_player;
#[cfg(feature = "full-app")]
mod media_player_config;
#[cfg(feature = "full-app")]
mod player_page;
#[cfg(feature = "full-app")]
mod playlist;
mod video_surface;

use gpui::App;

pub use app_assets::Assets;
#[cfg(feature = "full-app")]
pub use player_page::{open_main_window, PlayerPage};

#[cfg(feature = "full-app")]
pub fn init(cx: &mut App) {
    let _ = app_assets::Assets.load_fonts(cx);
    gpui_component::init(cx);
}
