// In-process 7-Zip extraction via official 7z.dll COM-like API (Client7z pattern).
// Uses UTF-16 paths throughout for Unicode path support.

#include "sevenzip_extract.h"

#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#include <windows.h>
#include <objbase.h>
#include <shlwapi.h>

#include <stdint.h>
#include <string.h>

#include <string>

#pragma comment(lib, "ole32.lib")
#pragma comment(lib, "shlwapi.lib")

#ifndef RINOK
#define RINOK(x)                                                                                   \
    {                                                                                              \
        const HRESULT __result_ = (x);                                                             \
        if (__result_ != S_OK)                                                                     \
            return __result_;                                                                      \
    }
#endif

static const GUID IID_IInArchive = {
    0x23170F69, 0x40C1, 0x278A, {0x00, 0x00, 0x00, 0x06, 0x00, 0x60, 0x00, 0x00}};

static const GUID IID_ISetProperties = {
    0x23170F69, 0x40C1, 0x278A, {0x00, 0x00, 0x00, 0x06, 0x00, 0x03, 0x00, 0x00}};

static const GUID IID_IArchiveOpenCallback = {
    0x23170F69, 0x40C1, 0x278A, {0x00, 0x00, 0x00, 0x06, 0x00, 0x10, 0x00, 0x00}};

static const GUID IID_IProgress = {
    0x23170F69, 0x40C1, 0x278A, {0x00, 0x00, 0x00, 0x00, 0x00, 0x05, 0x00, 0x00}};

static const GUID IID_IArchiveExtractCallback = {
    0x23170F69, 0x40C1, 0x278A, {0x00, 0x00, 0x00, 0x06, 0x00, 0x20, 0x00, 0x00}};

static const GUID IID_ICryptoGetTextPassword = {
    0x23170F69, 0x40C1, 0x278A, {0x00, 0x00, 0x00, 0x05, 0x00, 0x10, 0x00, 0x00}};

static const GUID IID_ISequentialInStream = {
    0x23170F69, 0x40C1, 0x278A, {0x00, 0x00, 0x00, 0x03, 0x00, 0x01, 0x00, 0x00}};

static const GUID IID_IInStream = {
    0x23170F69, 0x40C1, 0x278A, {0x00, 0x00, 0x00, 0x03, 0x00, 0x03, 0x00, 0x00}};

static const GUID IID_ISequentialOutStream = {
    0x23170F69, 0x40C1, 0x278A, {0x00, 0x00, 0x00, 0x03, 0x00, 0x02, 0x00, 0x00}};

static const GUID IID_IOutStream = {
    0x23170F69, 0x40C1, 0x278A, {0x00, 0x00, 0x00, 0x03, 0x00, 0x04, 0x00, 0x00}};

static const PROPID kpidPath = 3;
static const PROPID kpidIsDir = 6;
static const PROPID kpidSize = 7;
static const PROPID kpidAttrib = 4;
static const PROPID kpidMTime = 13;

enum ExtractAskMode { kExtract = 0, kTest = 1, kSkip = 2 };
enum ExtractOperationResult { kOpOk = 0 };

typedef HRESULT(STDMETHODCALLTYPE *CreateObjectFunc)(const GUID *clsID, const GUID *iid,
                                                     void **outObject);

struct IProgress : public IUnknown {
    virtual HRESULT STDMETHODCALLTYPE SetTotal(UINT64 total) = 0;
    virtual HRESULT STDMETHODCALLTYPE SetCompleted(const UINT64 *completeValue) = 0;
};

struct ISequentialInStream : public IUnknown {
    virtual HRESULT STDMETHODCALLTYPE Read(void *data, UINT32 size, UINT32 *processedSize) = 0;
};

struct IInStream : public ISequentialInStream {
    virtual HRESULT STDMETHODCALLTYPE Seek(INT64 offset, UINT32 seekOrigin,
                                           UINT64 *newPosition) = 0;
};

struct ISequentialOutStream : public IUnknown {
    virtual HRESULT STDMETHODCALLTYPE Write(const void *data, UINT32 size,
                                            UINT32 *processedSize) = 0;
};

struct IOutStream : public ISequentialOutStream {
    virtual HRESULT STDMETHODCALLTYPE Seek(INT64 offset, UINT32 seekOrigin,
                                           UINT64 *newPosition) = 0;
    virtual HRESULT STDMETHODCALLTYPE SetSize(UINT64 newSize) = 0;
};

struct IArchiveOpenCallback : public IUnknown {
    virtual HRESULT STDMETHODCALLTYPE SetTotal(const UINT64 *files, const UINT64 *bytes) = 0;
    virtual HRESULT STDMETHODCALLTYPE SetCompleted(const UINT64 *files, const UINT64 *bytes) = 0;
};

struct IArchiveExtractCallback : public IProgress {
    virtual HRESULT STDMETHODCALLTYPE GetStream(UINT32 index, ISequentialOutStream **outStream,
                                                INT32 askExtractMode) = 0;
    virtual HRESULT STDMETHODCALLTYPE PrepareOperation(INT32 askExtractMode) = 0;
    virtual HRESULT STDMETHODCALLTYPE SetOperationResult(INT32 operationResult) = 0;
};

struct ICryptoGetTextPassword : public IUnknown {
    virtual HRESULT STDMETHODCALLTYPE CryptoGetTextPassword(BSTR *password) = 0;
};

struct ISetProperties : public IUnknown {
    virtual HRESULT STDMETHODCALLTYPE SetProperties(const wchar_t **names, const PROPVARIANT *values,
                                                    UINT32 numProps) = 0;
};

struct IInArchive : public IUnknown {
    virtual HRESULT STDMETHODCALLTYPE Open(IInStream *stream, const UINT64 *maxCheckStartPosition,
                                           IArchiveOpenCallback *openCallback) = 0;
    virtual HRESULT STDMETHODCALLTYPE Close() = 0;
    virtual HRESULT STDMETHODCALLTYPE GetNumberOfItems(UINT32 *numItems) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetProperty(UINT32 index, PROPID propID,
                                                PROPVARIANT *value) = 0;
    virtual HRESULT STDMETHODCALLTYPE Extract(const UINT32 *indices, UINT32 numItems,
                                              INT32 testMode,
                                              IArchiveExtractCallback *extractCallback) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetArchiveProperty(PROPID propID, PROPVARIANT *value) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetNumberOfProperties(UINT32 *numProps) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetPropertyInfo(UINT32 index, BSTR *name, PROPID *propID,
                                                    VARTYPE *varType) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetNumberOfArchiveProperties(UINT32 *numProps) = 0;
    virtual HRESULT STDMETHODCALLTYPE GetArchivePropertyInfo(UINT32 index, BSTR *name,
                                                             PROPID *propID, VARTYPE *varType) = 0;
};

static void SetError(wchar_t *error_buf, size_t error_buf_len, const wchar_t *message) {
    if (!error_buf || error_buf_len == 0) {
        return;
    }
    wcsncpy_s(error_buf, error_buf_len, message, _TRUNCATE);
}

static GUID FormatGuid(unsigned char formatId) {
    GUID g = {0x23170F69, 0x40C1, 0x278A, {0x10, 0x00, 0x00, 0x01, 0x10, formatId, 0x00, 0x00}};
    return g;
}

static bool PropBool(const PROPVARIANT &prop, bool &result) {
    if (prop.vt == VT_BOOL) {
        result = prop.boolVal != VARIANT_FALSE;
        return true;
    }
    if (prop.vt == VT_EMPTY) {
        result = false;
        return true;
    }
    return false;
}

static bool CreateDirectoryTree(const wchar_t *path) {
    if (!path || !path[0]) {
        return false;
    }
    wchar_t buffer[MAX_PATH * 4];
    wcsncpy_s(buffer, path, _TRUNCATE);
    for (wchar_t *cursor = buffer + 1; *cursor; ++cursor) {
        if (*cursor == L'\\' || *cursor == L'/') {
            const wchar_t saved = *cursor;
            *cursor = L'\0';
            if (!PathFileExistsW(buffer)) {
                if (!CreateDirectoryW(buffer, NULL) && GetLastError() != ERROR_ALREADY_EXISTS) {
                    *cursor = saved;
                    return false;
                }
            }
            *cursor = saved;
        }
    }
    if (!PathFileExistsW(buffer)) {
        if (!CreateDirectoryW(buffer, NULL) && GetLastError() != ERROR_ALREADY_EXISTS) {
            return false;
        }
    }
    return true;
}

static void NormalizeDirPrefix(std::wstring &dir) {
    if (dir.empty()) {
        return;
    }
    wchar_t last = dir.back();
    if (last != L'\\' && last != L'/') {
        dir.push_back(L'\\');
    }
}

class CInFileStream final : public IInStream {
    LONG _refCount = 1;
    HANDLE _handle = INVALID_HANDLE_VALUE;

public:
    CInFileStream() = default;

    ~CInFileStream() {
        if (_handle != INVALID_HANDLE_VALUE) {
            CloseHandle(_handle);
        }
    }

    bool Open(const wchar_t *path) {
        _handle = CreateFileW(path, GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING,
                              FILE_ATTRIBUTE_NORMAL, NULL);
        return _handle != INVALID_HANDLE_VALUE;
    }

    STDMETHODIMP QueryInterface(REFIID iid, void **outObject) override {
        if (!outObject) {
            return E_POINTER;
        }
        *outObject = NULL;
        if (iid == IID_IUnknown || iid == IID_ISequentialInStream || iid == IID_IInStream) {
            *outObject = static_cast<IInStream *>(this);
            AddRef();
            return S_OK;
        }
        return E_NOINTERFACE;
    }

    STDMETHODIMP_(ULONG) AddRef() override { return InterlockedIncrement(&_refCount); }

    STDMETHODIMP_(ULONG) Release() override {
        const LONG count = InterlockedDecrement(&_refCount);
        if (count == 0) {
            delete this;
        }
        return static_cast<ULONG>(count);
    }

    STDMETHODIMP Read(void *data, UINT32 size, UINT32 *processedSize) override {
        if (_handle == INVALID_HANDLE_VALUE) {
            return E_FAIL;
        }
        DWORD read = 0;
        if (!ReadFile(_handle, data, size, &read, NULL)) {
            return HRESULT_FROM_WIN32(GetLastError());
        }
        if (processedSize) {
            *processedSize = read;
        }
        return S_OK;
    }

    STDMETHODIMP Seek(INT64 offset, UINT32 seekOrigin, UINT64 *newPosition) override {
        if (_handle == INVALID_HANDLE_VALUE) {
            return E_FAIL;
        }
        LARGE_INTEGER move{};
        move.QuadPart = offset;
        LARGE_INTEGER newPos{};
        if (!SetFilePointerEx(_handle, move, &newPos,
                              seekOrigin == STREAM_SEEK_SET   ? FILE_BEGIN
                              : seekOrigin == STREAM_SEEK_CUR ? FILE_CURRENT
                                                              : FILE_END)) {
            return HRESULT_FROM_WIN32(GetLastError());
        }
        if (newPosition) {
            *newPosition = static_cast<UINT64>(newPos.QuadPart);
        }
        return S_OK;
    }
};

class COutFileStream final : public IOutStream {
    LONG _refCount = 1;
    HANDLE _handle = INVALID_HANDLE_VALUE;
    FILETIME _mtime{};
    bool _mtimeDefined = false;

public:
    COutFileStream() = default;

    ~COutFileStream() {
        Close();
    }

    bool Open(const wchar_t *path) {
        Close();
        _handle = CreateFileW(path, GENERIC_WRITE, 0, NULL, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL,
                              NULL);
        return _handle != INVALID_HANDLE_VALUE;
    }

    void SetMTime(const FILETIME *mtime) {
        if (mtime) {
            _mtime = *mtime;
            _mtimeDefined = true;
        }
    }

    HRESULT Close() {
        if (_handle == INVALID_HANDLE_VALUE) {
            return S_OK;
        }
        if (_mtimeDefined) {
            SetFileTime(_handle, NULL, NULL, &_mtime);
        }
        const BOOL ok = CloseHandle(_handle);
        _handle = INVALID_HANDLE_VALUE;
        return ok ? S_OK : HRESULT_FROM_WIN32(GetLastError());
    }

    STDMETHODIMP QueryInterface(REFIID iid, void **outObject) override {
        if (!outObject) {
            return E_POINTER;
        }
        *outObject = NULL;
        if (iid == IID_IUnknown || iid == IID_ISequentialOutStream || iid == IID_IOutStream) {
            *outObject = static_cast<IOutStream *>(this);
            AddRef();
            return S_OK;
        }
        return E_NOINTERFACE;
    }

    STDMETHODIMP_(ULONG) AddRef() override { return InterlockedIncrement(&_refCount); }

    STDMETHODIMP_(ULONG) Release() override {
        const LONG count = InterlockedDecrement(&_refCount);
        if (count == 0) {
            delete this;
        }
        return static_cast<ULONG>(count);
    }

    STDMETHODIMP Write(const void *data, UINT32 size, UINT32 *processedSize) override {
        if (_handle == INVALID_HANDLE_VALUE) {
            return E_FAIL;
        }
        DWORD written = 0;
        if (!WriteFile(_handle, data, size, &written, NULL)) {
            return HRESULT_FROM_WIN32(GetLastError());
        }
        if (processedSize) {
            *processedSize = written;
        }
        return S_OK;
    }

    STDMETHODIMP Seek(INT64 offset, UINT32 seekOrigin, UINT64 *newPosition) override {
        if (_handle == INVALID_HANDLE_VALUE) {
            return E_FAIL;
        }
        LARGE_INTEGER move{};
        move.QuadPart = offset;
        LARGE_INTEGER newPos{};
        if (!SetFilePointerEx(_handle, move, &newPos,
                              seekOrigin == STREAM_SEEK_SET   ? FILE_BEGIN
                              : seekOrigin == STREAM_SEEK_CUR ? FILE_CURRENT
                                                              : FILE_END)) {
            return HRESULT_FROM_WIN32(GetLastError());
        }
        if (newPosition) {
            *newPosition = static_cast<UINT64>(newPos.QuadPart);
        }
        return S_OK;
    }

    STDMETHODIMP SetSize(UINT64 newSize) override {
        if (_handle == INVALID_HANDLE_VALUE) {
            return E_FAIL;
        }
        LARGE_INTEGER size{};
        size.QuadPart = static_cast<LONGLONG>(newSize);
        if (!SetFilePointerEx(_handle, size, NULL, FILE_BEGIN)) {
            return HRESULT_FROM_WIN32(GetLastError());
        }
        if (!SetEndOfFile(_handle)) {
            return HRESULT_FROM_WIN32(GetLastError());
        }
        return S_OK;
    }
};

class CArchiveOpenCallback final : public IArchiveOpenCallback {
    LONG _refCount = 1;

public:
    STDMETHODIMP QueryInterface(REFIID iid, void **outObject) override {
        if (!outObject) {
            return E_POINTER;
        }
        *outObject = NULL;
        if (iid == IID_IUnknown || iid == IID_IArchiveOpenCallback) {
            *outObject = static_cast<IArchiveOpenCallback *>(this);
            AddRef();
            return S_OK;
        }
        return E_NOINTERFACE;
    }

    STDMETHODIMP_(ULONG) AddRef() override { return InterlockedIncrement(&_refCount); }

    STDMETHODIMP_(ULONG) Release() override {
        const LONG count = InterlockedDecrement(&_refCount);
        if (count == 0) {
            delete this;
        }
        return static_cast<ULONG>(count);
    }

    STDMETHODIMP SetTotal(const UINT64 *, const UINT64 *) override { return S_OK; }
    STDMETHODIMP SetCompleted(const UINT64 *, const UINT64 *) override { return S_OK; }
};

struct ExtractContext {
    SevenZipProgressFn progress = nullptr;
    void *progress_ctx = nullptr;
    SevenZipCancelFn cancel = nullptr;
    void *cancel_ctx = nullptr;
    uint32_t completed = 0;
    uint32_t total = 0;
    bool password_needed = false;
};

class CArchiveExtractCallback final : public IArchiveExtractCallback,
                                        public ICryptoGetTextPassword {
    LONG _refCount = 1;
    IInArchive *_archive = nullptr;
    std::wstring _directoryPath;
    std::wstring _filePath;
    std::wstring _diskFilePath;
    bool _extractMode = false;
    bool _isDir = false;
    bool _attribDefined = false;
    DWORD _attrib = 0;
    bool _mtimeDefined = false;
    FILETIME _mtime{};
    COutFileStream *_outFileStream = nullptr;
    ExtractContext *_ctx = nullptr;

public:
    void Init(IInArchive *archive, const std::wstring &directoryPath, ExtractContext *ctx) {
        _archive = archive;
        _directoryPath = directoryPath;
        NormalizeDirPrefix(_directoryPath);
        _ctx = ctx;
        if (_ctx && _archive) {
            UINT32 numItems = 0;
            if (_archive->GetNumberOfItems(&numItems) == S_OK) {
                _ctx->total = numItems;
            }
        }
    }

    STDMETHODIMP QueryInterface(REFIID iid, void **outObject) override {
        if (!outObject) {
            return E_POINTER;
        }
        *outObject = NULL;
        if (iid == IID_IUnknown || iid == IID_IArchiveExtractCallback || iid == IID_IProgress) {
            *outObject = static_cast<IArchiveExtractCallback *>(this);
            AddRef();
            return S_OK;
        }
        if (iid == IID_ICryptoGetTextPassword) {
            *outObject = static_cast<ICryptoGetTextPassword *>(this);
            AddRef();
            return S_OK;
        }
        return E_NOINTERFACE;
    }

    STDMETHODIMP_(ULONG) AddRef() override { return InterlockedIncrement(&_refCount); }

    STDMETHODIMP_(ULONG) Release() override {
        const LONG count = InterlockedDecrement(&_refCount);
        if (count == 0) {
            delete this;
        }
        return static_cast<ULONG>(count);
    }

    STDMETHODIMP SetTotal(UINT64) override { return S_OK; }
    STDMETHODIMP SetCompleted(const UINT64 *) override { return S_OK; }

    bool IsCancelled() const {
        return _ctx && _ctx->cancel && _ctx->cancel(_ctx->cancel_ctx) != 0;
    }

    STDMETHODIMP GetStream(UINT32 index, ISequentialOutStream **outStream,
                           INT32 askExtractMode) override {
        *outStream = NULL;
        if (_outFileStream) {
            _outFileStream->Release();
            _outFileStream = nullptr;
        }
        if (IsCancelled()) {
            return E_ABORT;
        }
        if (askExtractMode != kExtract) {
            return S_OK;
        }

        PROPVARIANT prop{};
        PropVariantInit(&prop);
        RINOK(_archive->GetProperty(index, kpidPath, &prop));
        if (prop.vt == VT_EMPTY) {
            _filePath = L"[Content]";
        } else if (prop.vt == VT_BSTR) {
            _filePath = prop.bstrVal;
        } else {
            PropVariantClear(&prop);
            return E_FAIL;
        }
        PropVariantClear(&prop);

        PropVariantInit(&prop);
        if (_archive->GetProperty(index, kpidAttrib, &prop) == S_OK && prop.vt == VT_UI4) {
            _attrib = prop.ulVal;
            _attribDefined = true;
        } else {
            _attribDefined = false;
        }
        PropVariantClear(&prop);

        PropVariantInit(&prop);
        _mtimeDefined = false;
        if (_archive->GetProperty(index, kpidMTime, &prop) == S_OK && prop.vt == VT_FILETIME) {
            _mtime = prop.filetime;
            _mtimeDefined = true;
        }
        PropVariantClear(&prop);

        PropVariantInit(&prop);
        _isDir = false;
        if (_archive->GetProperty(index, kpidIsDir, &prop) == S_OK) {
            if (!PropBool(prop, _isDir)) {
                PropVariantClear(&prop);
                return E_FAIL;
            }
        }
        PropVariantClear(&prop);

        const size_t slashPos = _filePath.find_last_of(L"\\/");
        if (slashPos != std::wstring::npos) {
            const std::wstring parent = _directoryPath + _filePath.substr(0, slashPos);
            if (!CreateDirectoryTree(parent.c_str())) {
                return E_ABORT;
            }
        }

        _diskFilePath = _directoryPath + _filePath;
        if (_isDir) {
            if (!CreateDirectoryTree(_diskFilePath.c_str())) {
                return E_ABORT;
            }
            return S_OK;
        }

        if (PathFileExistsW(_diskFilePath.c_str())) {
            if (!DeleteFileW(_diskFilePath.c_str())) {
                return E_ABORT;
            }
        }

        _outFileStream = new COutFileStream();
        if (!_outFileStream->Open(_diskFilePath.c_str())) {
            _outFileStream->Release();
            _outFileStream = nullptr;
            return E_ABORT;
        }
        *outStream = _outFileStream;
        (*outStream)->AddRef();
        return S_OK;
    }

    STDMETHODIMP PrepareOperation(INT32 askExtractMode) override {
        _extractMode = (askExtractMode == kExtract);
        if (IsCancelled()) {
            return E_ABORT;
        }
        return S_OK;
    }

    STDMETHODIMP SetOperationResult(INT32 operationResult) override {
        if (_outFileStream) {
            if (_mtimeDefined) {
                _outFileStream->SetMTime(&_mtime);
            }
            _outFileStream->Close();
            _outFileStream->Release();
            _outFileStream = nullptr;
        }
        if (_extractMode && _attribDefined && !_diskFilePath.empty()) {
            SetFileAttributesW(_diskFilePath.c_str(), _attrib);
        }
        if (_ctx && operationResult == kOpOk) {
            ++_ctx->completed;
            if (_ctx->progress) {
                _ctx->progress(_ctx->progress_ctx, _ctx->completed, _ctx->total);
            }
        }
        if (IsCancelled()) {
            return E_ABORT;
        }
        return S_OK;
    }

    STDMETHODIMP CryptoGetTextPassword(BSTR *password) override {
        if (_ctx) {
            _ctx->password_needed = true;
        }
        if (password) {
            *password = NULL;
        }
        return E_ABORT;
    }
};

static HRESULT SetMultiThread(IInArchive *archive, uint32_t threadCount) {
    if (threadCount == 0) {
        return S_OK;
    }
    ISetProperties *setProps = nullptr;
    if (archive->QueryInterface(IID_ISetProperties, reinterpret_cast<void **>(&setProps)) != S_OK ||
        !setProps) {
        return S_FALSE;
    }
    const wchar_t *names[] = {L"mt"};
    PROPVARIANT values[1]{};
    values[0].vt = VT_UI4;
    values[0].ulVal = threadCount;
    const HRESULT hr = setProps->SetProperties(names, values, 1);
    setProps->Release();
    return hr;
}

static bool TryOpenArchive(CreateObjectFunc createObject, unsigned char formatId,
                           CInFileStream *stream, IInArchive **outArchive) {
    *outArchive = nullptr;
    const GUID clsid = FormatGuid(formatId);
    IInArchive *archive = nullptr;
    if (createObject(&clsid, &IID_IInArchive, reinterpret_cast<void **>(&archive)) != S_OK ||
        !archive) {
        return false;
    }

    const UINT64 seekZero = 0;
    stream->Seek(0, STREAM_SEEK_SET, NULL);

    CArchiveOpenCallback *openCallback = new CArchiveOpenCallback();
    const HRESULT openResult =
        archive->Open(stream, &seekZero, openCallback);
    openCallback->Release();

    if (openResult != S_OK) {
        archive->Release();
        return false;
    }
    *outArchive = archive;
    return true;
}

static const unsigned char kFallbackFormats[] = {
    0x01, 0x07, 0xEF, 0xEE, 0x02, 0x0C,
};

static bool OpenArchiveAuto(CreateObjectFunc createObject, enum SevenZipFormat formatHint,
                            CInFileStream *stream, IInArchive **outArchive) {
    if (formatHint != SEVENZIP_FORMAT_AUTO) {
        if (TryOpenArchive(createObject, static_cast<unsigned char>(formatHint), stream,
                           outArchive)) {
            return true;
        }
    }

    const unsigned char tryOrder[] = {
        static_cast<unsigned char>(formatHint),
        0x01,
        0x07,
        0xEF,
        0xEE,
        0x02,
        0x0C,
    };

    for (unsigned char formatId : tryOrder) {
        if (formatId == 0) {
            continue;
        }
        if (TryOpenArchive(createObject, formatId, stream, outArchive)) {
            return true;
        }
    }
    for (unsigned char formatId : kFallbackFormats) {
        if (TryOpenArchive(createObject, formatId, stream, outArchive)) {
            return true;
        }
    }
    return false;
}

int sevenzip_extract(const wchar_t *dll_path, const wchar_t *archive_path, const wchar_t *dest_dir,
                     enum SevenZipFormat format_hint, uint32_t thread_count,
                     SevenZipProgressFn progress, void *progress_ctx, SevenZipCancelFn cancel,
                     void *cancel_ctx, wchar_t *error_buf, size_t error_buf_len) {
    if (!dll_path || !archive_path || !dest_dir) {
        SetError(error_buf, error_buf_len, L"invalid arguments");
        return -1;
    }

    const HMODULE lib = LoadLibraryW(dll_path);
    if (!lib) {
        SetError(error_buf, error_buf_len, L"failed to load 7z.dll");
        return -2;
    }

    const auto createObject =
        reinterpret_cast<CreateObjectFunc>(GetProcAddress(lib, "CreateObject"));
    if (!createObject) {
        FreeLibrary(lib);
        SetError(error_buf, error_buf_len, L"CreateObject not found in 7z.dll");
        return -2;
    }

    CInFileStream *fileStream = new CInFileStream();
    if (!fileStream->Open(archive_path)) {
        fileStream->Release();
        FreeLibrary(lib);
        SetError(error_buf, error_buf_len, L"cannot open archive file");
        return -3;
    }

    IInArchive *archive = nullptr;
    if (!OpenArchiveAuto(createObject, format_hint, fileStream, &archive)) {
        fileStream->Release();
        FreeLibrary(lib);
        SetError(error_buf, error_buf_len, L"cannot open archive format");
        return -3;
    }

    if (!CreateDirectoryTree(dest_dir)) {
        archive->Close();
        archive->Release();
        fileStream->Release();
        FreeLibrary(lib);
        SetError(error_buf, error_buf_len, L"cannot create destination directory");
        return -3;
    }

    SetMultiThread(archive, thread_count);

    ExtractContext ctx{};
    ctx.progress = progress;
    ctx.progress_ctx = progress_ctx;
    ctx.cancel = cancel;
    ctx.cancel_ctx = cancel_ctx;
    if (progress) {
        progress(progress_ctx, 0, ctx.total);
    }

    std::wstring destPrefix(dest_dir);
    NormalizeDirPrefix(destPrefix);

    CArchiveExtractCallback *extractCallback = new CArchiveExtractCallback();
    extractCallback->Init(archive, destPrefix, &ctx);

    const HRESULT extractResult = archive->Extract(NULL, static_cast<UINT32>(-1), FALSE,
                                                   extractCallback);
    const bool cancelled = extractResult == E_ABORT || extractCallback->IsCancelled();
    const bool passwordNeeded = ctx.password_needed;
    extractCallback->Release();

    archive->Close();
    archive->Release();
    fileStream->Release();
    FreeLibrary(lib);

    if (cancelled) {
        SetError(error_buf, error_buf_len, L"cancelled");
        return 1;
    }
    if (passwordNeeded) {
        SetError(error_buf, error_buf_len, L"password required");
        return -5;
    }
    if (extractResult != S_OK) {
        SetError(error_buf, error_buf_len, L"extract failed");
        return -4;
    }
    if (progress && ctx.total > 0) {
        progress(progress_ctx, ctx.total, ctx.total);
    }
    return 0;
}
