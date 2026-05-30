use cyber_desktop_core::AppConfig;

use super::data::build_sidebar_sections;
use super::model::SidebarSection;

/// Fingerprint of config fields that affect sidebar section lists.
pub fn sidebar_cache_key(config: &AppConfig) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    fn feed(hash: &mut u64, bytes: &[u8]) {
        for b in bytes {
            *hash ^= *b as u64;
            *hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    feed(&mut hash, config.sidebar_display_mode.as_bytes());
    feed(
        &mut hash,
        &[
            config.show_sidebar_section_pinned as u8,
            config.show_sidebar_section_library as u8,
            config.show_sidebar_section_drives as u8,
            config.show_sidebar_section_cloud as u8,
            config.show_sidebar_section_network as u8,
            config.show_sidebar_section_wsl as u8,
            config.show_sidebar_section_file_tags as u8,
        ],
    );
    for path in &config.pinned_folders {
        feed(&mut hash, path.as_bytes());
        feed(&mut hash, b"\n");
    }
    for tag in &config.file_tags {
        feed(&mut hash, tag.name.as_bytes());
        for path in &tag.paths {
            feed(&mut hash, path.as_bytes());
        }
    }
    hash
}

pub fn build_sidebar_sections_cached(config: &AppConfig) -> Vec<SidebarSection> {
    build_sidebar_sections(config)
}
