#include <stdlib.h>
#include <stdio.h>

#include "libjsonnet.h"

typedef struct JsonnetJsonValue* val_t;
typedef struct JsonnetVm* vm_t;

typedef struct native_ctx_t {
    vm_t vm
} native_ctx_t;

val_t native_add(void* nctx, const struct JsonnetJsonValue* const* argv, int* success) {
    native_ctx_t* ctx = nctx;
    double a;
    double b;
    jsonnet_json_extract_number(ctx->vm, argv[0], &a);
    jsonnet_json_extract_number(ctx->vm, argv[1], &b);
    *success = 1;
    return jsonnet_json_make_number(ctx->vm, a + b);
}

int main(int argc, const char **argv)
{
    int error;
    char *output;
    struct JsonnetVm *vm;
    if (argc != 2) {
        fprintf(stderr, "libjsonnet_test_file <file>\n");
        return EXIT_FAILURE;
    }
    vm = jsonnet_make();

    native_ctx_t* native_ctx = malloc(sizeof(native_ctx_t));
    native_ctx->vm = vm;
    const char* params[3] = {"a", "b", NULL};
    jsonnet_native_callback(vm, "nativeAdd", native_add, native_ctx, params);

    output = jsonnet_evaluate_file(vm, argv[1], &error);
    if (error) {
        fprintf(stderr, "%s", output);
        jsonnet_realloc(vm, output, 0);
        jsonnet_destroy(vm);
        return EXIT_FAILURE;
    }
    printf("%s", output);
    jsonnet_realloc(vm, output, 0);
    free(native_ctx);
    jsonnet_destroy(vm);
    return EXIT_SUCCESS;
}
