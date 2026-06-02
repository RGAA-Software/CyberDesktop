use std::ops::Deref;

/// Unified Traditional Chinese locale (HK/TW/MO/SG and `zh-Hant`).
pub const LOCALE_ZH_HANT: &str = "zh-HK";

/// Simplified Chinese locale.
pub const LOCALE_ZH_HANS: &str = "zh-CN";

/// Detect system locale and apply `en`, `zh-CN`, or `zh-HK`.
pub fn init_locale() {
    let locale = detect_locale();
    set_locale(&locale);
}

pub fn locale() -> impl Deref<Target = str> {
    rust_i18n::locale()
}

/// Switch app and gpui-component UI strings (calendar, dialogs, etc.).
pub fn set_locale(locale: &str) {
    let locale = normalize_locale(locale);
    rust_i18n::set_locale(locale);
    gpui_component::set_locale(locale);
}

fn detect_locale() -> String {
    sys_locale::get_locale()
        .map(|l| normalize_locale(&l).to_string())
        .unwrap_or_else(|| "en".to_string())
}

/// Map region/script tags to `en`, `zh-CN`, or unified Traditional `zh-HK`.
pub fn normalize_locale(locale: &str) -> &'static str {
    let l = locale.to_ascii_lowercase();

    if l == "en" || l.starts_with("en-") {
        return "en";
    }

    if is_traditional_chinese(&l) {
        return LOCALE_ZH_HANT;
    }

    if l.starts_with("zh") {
        return LOCALE_ZH_HANS;
    }

    "en"
}

/// Taiwan, Hong Kong, Macau, Singapore, and explicit `zh-Hant` → one Traditional locale.
fn is_traditional_chinese(locale: &str) -> bool {
    locale.starts_with("zh-hk")
        || locale.starts_with("zh-tw")
        || locale.starts_with("zh-mo")
        || locale.starts_with("zh-sg")
        || locale.starts_with("zh-hant")
        || locale.contains("-hant")
}
