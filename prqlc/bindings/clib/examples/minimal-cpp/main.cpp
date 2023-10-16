#include <cstring>
#include <iostream>

#include "libprqlc.hpp"

using namespace prqlc;

void print_result(CompileResult res) {
    if (strcmp(res.output, "") == 0) {
        std::cout << "Output: <empty>\n\n";
    } else {
        std::cout << "Output:\n\n" << res.output;
    }
}

int main() {
    const auto prql_query = "from albums | select {album_id, title} | take 3";

    CompileResult res = compile(prql_query, nullptr);
    print_result(res);
    result_destroy(res);

    return 0;
}
