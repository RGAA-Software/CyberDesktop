use app_platform_windows::list_recycle_bin_entries;

fn main() {
    match list_recycle_bin_entries() {
        Ok(items) => {
            println!("recycle bin: {} item(s)", items.len());
            for item in items.iter().take(5) {
                println!("  {} -> {}", item.display_name, item.shell_path.display());
            }
        }
        Err(error) => eprintln!("list failed: {error:#}"),
    }
}
