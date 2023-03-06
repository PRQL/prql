#include <stdio.h>

#include <libprql_lib.h>

void print_result(CompileResult res) {
    printf("---- [ Compiled with %ld errors ]----\n", res.errors_len);
    for (int i = 0; i < res.errors_len; i++) {
        ErrorMessage const* e = &res.errors[i];
        if (e->display != NULL) {
            printf("%s", *e->display);
        } else if (e->code != NULL) {
            printf("[%s] Error: %s\n", *e->code, e->reason);
        } else {
            printf("Error: %s", e->reason);
        }
    }
    if (*res.output == '\0') {
        printf("Output: <empty>\n\n");
    } else {
        printf("Output:\n%s\n\n", res.output);
    }
}

void free_result(CompileResult res) {
    for (int i = 0; i < res.errors_len; i++) {
        ErrorMessage const* e = &res.errors[i];
        free((void*) *e->code);
        free((void*) e->code);
        free((void*) *e->display);
        free((void*) e->display);
        free((void*) *e->hint);
        free((void*) e->hint);
        free((void*) e->location);
        free((void*) e->reason);
        free((void*) e->span);
    }
    free((void*) res.errors);
    free((void*) res.output);
}


int main() {
    char *prql_query;
    prql_query = "from albums | select [album_id, title] | take 3";
    CompileResult res;

    // default compile option
    res = compile(prql_query, NULL);
    print_result(res);
    free_result(res);

    // custom compile options
    Options opts;
    opts.format = false;
    opts.signature_comment = false;
    opts.target = "sql.mssql";
    res = compile(prql_query, &opts);
    print_result(res);
    free_result(res);

    // error handling
    res = compile("from album | select [album_id] | select [title]", NULL);
    print_result(res);
    free_result(res);

    // error handling
    res = compile("let a = (from album)", NULL);
    print_result(res);
    free_result(res);

    // intermediate results
    res = prql_to_pl(prql_query);
    print_result(res);
    free_result(res);

    res = pl_to_rq(res.output);
    print_result(res);
    free_result(res);
    return 0;
}
