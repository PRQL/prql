#include <stdio.h>

#include <libprqlc_c.h>

void print_result(CompileResult res)
{
    printf("---- [ Compiled with %ld errors ]----\n", res.messages_len);
    for (int i = 0; i < res.messages_len; i++)
    {
        Message const *e = &res.messages[i];
        if (e->display != NULL)
        {
            printf("%s", *e->display);
        }
        else if (e->code != NULL)
        {
            printf("[%s] Error: %s\n", *e->code, e->reason);
        }
        else
        {
            printf("Error: %s", e->reason);
        }
    }
    if (*res.output == '\0')
    {
        printf("Output: <empty>\n\n");
    }
    else
    {
        printf("Output:\n%s\n\n", res.output);
    }
}

int main()
{
    char *prql_query;
    prql_query = "from albums | select {album_id, title} | take 3";
    CompileResult res;
    CompileResult res2;

    // default compile option
    res = compile(prql_query, NULL);
    print_result(res);
    if (res.messages_len != 0)
        return 1;
    result_destroy(res);

    // custom compile options
    Options opts;
    opts.format = false;
    opts.signature_comment = false;
    opts.target = "sql.mssql";
    res = compile(prql_query, &opts);
    print_result(res);
    if (res.messages_len != 0)
        return 1;
    result_destroy(res);

    // error handling
    res = compile("from album | select {album_id} | select {title}", NULL);
    print_result(res);
    if (res.messages_len == 0)
        return 1;
    result_destroy(res);

    // error handling
    res = compile("let a = (from album)", NULL);
    print_result(res);
    if (res.messages_len == 0)
        return 1;
    result_destroy(res);

    // intermediate results
    res = prql_to_pl(prql_query);
    print_result(res);

    res2 = pl_to_rq(res.output);
    result_destroy(res);

    print_result(res2);
    result_destroy(res2);

    return 0;
}
