#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

#define ODL_STATUS_OK 0

#define ODL_STATUS_INVALID_ARGUMENT 1

#define ODL_STATUS_CONVERSION_ERROR 2

#define ODL_STATUS_PANIC 3

#define ODL_OPTION_JSON 1

#define ODL_OPTION_MARKDOWN 2

#define ODL_OPTION_IMAGES 4

#define ODL_OPTION_PARALLEL 8

const char *odl_last_error(void);

/**
 * Convert a batch of local PDFs and write the selected output files.
 */
int odl_convert(const char *const *pdf_paths,
                unsigned int n_paths,
                const char *out_dir,
                unsigned int mode);
