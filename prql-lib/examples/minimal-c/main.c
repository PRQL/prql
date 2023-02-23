#include <stdio.h>

#include <libprql_lib.h>

int main() {
    char *prql_query;
    prql_query = "from albums | select [album_id, title] | take 3";

    int res;
    char res_buffer[256];

    // default compile option
    res = compile(prql_query, NULL, res_buffer);
    printf("%s\n\n", res_buffer);

    // custom compile options
    Options opts;
    opts.format = false;
    opts.signature_comment = false;
    opts.target = "sql.mssql";
    res = compile(prql_query, &opts, res_buffer);
    printf("%s\n\n", res_buffer);

    // error handling
    res = compile("from album | select [album_id] | select [title]", NULL, res_buffer);
    if (res == 0) {
        printf("success\n\n");
    }
    if (res < 0) {
        printf("error with code %d!\n%s\n\n", res, res_buffer);
    }

    // intermediate results
    char* pl_buffer = (char*) malloc(sizeof(char) * 512);
    char* rq_buffer = (char*) malloc(sizeof(char) * 512);

    res = prql_to_pl(prql_query, pl_buffer);
    printf("PL JSON: %s\n\n", pl_buffer);

    res = pl_to_rq(pl_buffer, rq_buffer);
    printf("RQ JSON: %s\n\n", rq_buffer);

    free(pl_buffer);
    free(rq_buffer);

    return 0;
}
