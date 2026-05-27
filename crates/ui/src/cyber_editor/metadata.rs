#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum LineEndingKind {
    Lf,
    CrLf,
    Cr,
}

impl LineEndingKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Lf => "LF",
            Self::CrLf => "CRLF",
            Self::Cr => "CR",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) enum IndentStyle {
    Spaces(u8),
    Tabs,
    Mixed,
    Unknown,
}

impl IndentStyle {
    pub(crate) fn label(&self) -> String {
        match self {
            Self::Spaces(size) => format!("Spaces: {size}"),
            Self::Tabs => "Tabs".to_string(),
            Self::Mixed => "Mixed Indent".to_string(),
            Self::Unknown => "Indent: Auto".to_string(),
        }
    }
}

pub(crate) fn detect_line_ending(text: &str) -> LineEndingKind {
    if text.contains("\r\n") {
        LineEndingKind::CrLf
    } else if text.contains('\n') {
        LineEndingKind::Lf
    } else if text.contains('\r') {
        LineEndingKind::Cr
    } else {
        LineEndingKind::Lf
    }
}

pub(crate) fn detect_indent_style(text: &str) -> IndentStyle {
    let mut tab_lines = 0u32;
    let mut space_lines = 0u32;
    let mut common_space_width = [0u32; 9];

    for line in text.lines().take(200) {
        if line.is_empty() {
            continue;
        }

        let whitespace = line
            .chars()
            .take_while(|ch| *ch == ' ' || *ch == '\t')
            .collect::<String>();

        if whitespace.is_empty() {
            continue;
        }

        if whitespace.starts_with('\t') {
            tab_lines += 1;
            continue;
        }

        if whitespace.chars().all(|ch| ch == ' ') {
            space_lines += 1;
            let width = whitespace.len().min(8);
            if width > 0 {
                common_space_width[width] += 1;
            }
        }
    }

    if tab_lines == 0 && space_lines == 0 {
        return IndentStyle::Unknown;
    }
    if tab_lines > 0 && space_lines > 0 {
        return IndentStyle::Mixed;
    }
    if tab_lines > 0 {
        return IndentStyle::Tabs;
    }

    let mut best_width = 0usize;
    let mut best_count = 0u32;
    for (width, count) in common_space_width.iter().enumerate().skip(1) {
        if *count > best_count {
            best_count = *count;
            best_width = width;
        }
    }

    if best_width == 0 {
        IndentStyle::Spaces(4)
    } else {
        IndentStyle::Spaces(best_width as u8)
    }
}
