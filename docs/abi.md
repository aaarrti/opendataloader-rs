# Local C ABI

The `opendataloder_clib` crate exposes a file-writing batch conversion API.
The generated header is `crates/opendataloader_clib/include/opendataloder_clib.h`.

```c
int odl_convert(const char *const *pdf_paths, unsigned int n_paths,
                const char *out_dir, unsigned int mode);
const char *odl_last_error(void);
```

`pdf_paths` and `out_dir` are borrowed, NUL-terminated UTF-8 strings. The
caller owns them and must keep them valid for the duration of the call.
`odl_last_error` returns a borrowed UTF-8 string valid until the next ABI call
on the same thread; it returns `NULL` after a successful conversion.

The mode bits are `ODL_OPTION_JSON` (1), `ODL_OPTION_MARKDOWN` (2),
`ODL_OPTION_IMAGES` (4), and `ODL_OPTION_PARALLEL` (8). At least one format bit
is required. Status `0` means success; `1` is invalid input, `2` is a
conversion/output failure, and `3` means the Rust panic barrier was triggered.
