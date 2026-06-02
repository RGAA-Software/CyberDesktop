#pragma once

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/// Archive format hint for handler selection (matches 7-Zip handler IDs).
enum SevenZipFormat {
    SEVENZIP_FORMAT_AUTO = 0,
    SEVENZIP_FORMAT_ZIP = 0x01,
    SEVENZIP_FORMAT_BZIP2 = 0x02,
    SEVENZIP_FORMAT_7Z = 0x07,
    SEVENZIP_FORMAT_XZ = 0x0C,
    SEVENZIP_FORMAT_TAR = 0xEE,
    SEVENZIP_FORMAT_GZIP = 0xEF,
};

typedef void (*SevenZipProgressFn)(void *ctx, uint32_t completed, uint32_t total);
typedef int (*SevenZipCancelFn)(void *ctx);

/// Extract `archive_path` into `dest_dir` using the bundled 7-Zip engine in `dll_path`.
///
/// Returns:
///   0  success
///   1  cancelled (`cancel` returned non-zero)
///  -1  invalid arguments
///  -2  failed to load 7z.dll / CreateObject
///  -3  failed to open archive
///  -4  extraction failed
///  -5  password required
int sevenzip_extract(
    const wchar_t *dll_path,
    const wchar_t *archive_path,
    const wchar_t *dest_dir,
    enum SevenZipFormat format_hint,
    uint32_t thread_count,
    SevenZipProgressFn progress,
    void *progress_ctx,
    SevenZipCancelFn cancel,
    void *cancel_ctx,
    wchar_t *error_buf,
    size_t error_buf_len);

#ifdef __cplusplus
}
#endif
