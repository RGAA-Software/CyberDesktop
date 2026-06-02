use crate::item::FileItem;

/// Filters directory items by a case-insensitive substring match on display name.
pub fn filter_items_by_query(items: &[FileItem], query: &str) -> Vec<FileItem> {
    let query = query.trim();
    if query.is_empty() {
        return items.to_vec();
    }
    let needle = query.to_ascii_lowercase();
    items
        .iter()
        .filter(|item| item.display_name.to_ascii_lowercase().contains(&needle))
        .cloned()
        .collect()
}
