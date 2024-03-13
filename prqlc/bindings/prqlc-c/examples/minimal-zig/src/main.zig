const std = @import("std");
const prql = @cImport({
    @cInclude("../c/prqlc.h");
});

pub fn main() !void {
    var target = "sql.mssql".*;
    // Setup PRQL compiler options
    const options = prql.Options{
        .format = false,
        .signature_comment = false,
        //.target = &target,
        .target = &target
    };

    // Compile the PRQL query
    const prql_query = "from db.albums | select {album_id, title} | take 3";
    const result = prql.compile(prql_query, &options);
    defer prql.result_destroy(result);

    std.debug.print("Compiled with {d} errors\n", .{result.messages_len});
    std.debug.print("Output:\n\n{s}\n", .{result.output});
}

test "simple test" {
    const prql_query = "from db.albums | select {album_id, title} | take 3";
    const result = prql.compile(prql_query, null);
    defer prql.result_destroy(result);
    try std.testing.expect(result.messages_len == 0);
}
