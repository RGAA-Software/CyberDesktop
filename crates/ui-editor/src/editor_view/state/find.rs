use cyberfiles_text_engine::{SearchOptions, Searcher};
use gpui::{Entity, Subscription};
use gpui_component::input::InputState;

/// State for the Find / Replace bar. The query/replace fields are real
/// gpui-component text inputs; searching happens only on Enter or button press.
pub(crate) struct FindState {
    pub(crate) query: Entity<InputState>,
    pub(crate) replace: Entity<InputState>,
    pub(crate) replace_mode: bool,
    pub(crate) case_sensitive: bool,
    pub(crate) whole_word: bool,
    pub(crate) regex: bool,
    pub(crate) status: String,
    pub(crate) cached_query: String,
    pub(crate) cached_options: SearchOptions,
    pub(crate) cached_searcher: Option<Searcher>,
    pub(crate) _subs: Vec<Subscription>,
}

impl FindState {
    pub(crate) fn options(&self) -> SearchOptions {
        SearchOptions {
            case_sensitive: self.case_sensitive,
            whole_word: self.whole_word,
            regex: self.regex,
        }
    }
}
